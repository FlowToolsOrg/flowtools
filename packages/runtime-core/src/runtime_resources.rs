//! Native resource lifecycle. Every IO result returns through current authority.
use super::*;
use crate::resources::{native::*, *};
use std::sync::Arc;

fn entry(
    slot: String,
    source: Arc<crate::file_workflows::resource_source::HeldResource>,
    rights: Vec<ResourceRight>,
) -> Entry {
    Entry {
        slot,
        info: ResourceInfo {
            reference: Uuid::new_v4().to_string(),
            kind: source.kind(),
            name: source.name().into(),
            size: source.size(),
            revision: Uuid::new_v4().to_string(),
            rights,
        },
        source: Some(source),
    }
}

impl RuntimeCore {
    fn resource_policy(
        &self,
        identity: &CommandIdentity,
        epoch: u32,
    ) -> Result<ResourceGrant, ErrorCode> {
        let (manifest, command) = self
            .catalog
            .command(&identity.plugin_id, &identity.command_id)?;
        if CommandIdentity::from_manifest(&identity.caller, manifest, command) != *identity
            || !readonly_command(command)
        {
            return Err(ErrorCode::GrantRevoked);
        }
        let policy = self
            .policy
            .as_ref()
            .ok_or(ErrorCode::CapabilityUnavailable)?;
        policy.verify_resource_record(&self.data, identity)?;
        let record = policy
            .resource_records
            .iter()
            .find(|r| r.identity == *identity)
            .ok_or(ErrorCode::ApprovalRequired)?;
        if record.epoch != epoch {
            return Err(ErrorCode::GrantRevoked);
        }
        let grant = record.grant.as_ref().ok_or(ErrorCode::GrantRevoked)?;
        if grant.expires_at <= now() {
            return Err(ErrorCode::ResourceExpired);
        }
        Ok(grant.clone())
    }
    pub(super) fn resource_owner(
        &mut self,
        key: PermissionKey,
    ) -> Result<ResourceOwner, ErrorCode> {
        if self.management_only || self.cold_started {
            return Err(ErrorCode::InteractionRequired);
        }
        let (manifest, command) = self.catalog.command(&key.plugin_id, &key.command_id)?;
        if !readonly_command(command) {
            return Err(ErrorCode::CapabilityUnavailable);
        }
        let identity = CommandIdentity::from_manifest(key.target.caller(), manifest, command);
        let epoch = self.broker.authorize_command(&identity, command, now())?;
        let grant = self.resource_policy(&identity, epoch)?;
        self.resources
            .owners
            .retain(|_, o| o.public.expires_at > now());
        if self.resources.owners.len() >= 32 {
            return Err(ErrorCode::BudgetExceeded);
        }
        let owner = ResourceOwner {
            id: Uuid::new_v4().to_string(),
            identity,
            expires_at: grant.expires_at.min(now() + 300_000.0),
            scopes: grant.scopes,
        };
        self.resources.owners.insert(
            owner.id.clone(),
            Owner {
                public: owner.clone(),
                epoch,
                entries: Vec::new(),
            },
        );
        Ok(owner)
    }
    fn check_owner(&self, id: &str) -> Result<&Owner, ErrorCode> {
        let owner = self
            .resources
            .owners
            .get(id)
            .ok_or(ErrorCode::ScopeDenied)?;
        if owner.public.expires_at <= now() {
            return Err(ErrorCode::ResourceExpired);
        }
        self.resource_policy(&owner.public.identity, owner.epoch)?;
        Ok(owner)
    }
    pub fn prepare_resource_selection(
        &mut self,
        connection: &str,
        request: &Request,
    ) -> Result<ResourceIo, ErrorCode> {
        if request.version != PROTOCOL_MAJOR {
            return Err(ErrorCode::ProtocolMismatch);
        }
        if !valid_key(&request.request_id) {
            return Err(ErrorCode::InvalidRequest);
        }
        let proof = request.session.as_ref().ok_or(ErrorCode::SessionInvalid)?;
        if proof.instance_id != self.instance_id {
            return Err(ErrorCode::InstanceMismatch);
        }
        let session = self
            .sessions
            .get(&proof.session_id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if session.connection != connection
            || session.caller != "local-manager"
            || self.stopping
            || self.management_only
        {
            return Err(ErrorCode::SessionInvalid);
        }
        let Call::ResourceSelect(selection) = &request.call else {
            return Err(ErrorCode::InvalidRequest);
        };
        if selection.path.len() > 32768 {
            return Err(ErrorCode::InputInvalid);
        }
        let owner = self.check_owner(&selection.owner)?;
        let required = if selection.kind == ResourceKind::File {
            ResourceRight::Read
        } else {
            ResourceRight::List
        };
        if !owner
            .public
            .scopes
            .iter()
            .any(|s| s.slot == selection.slot && s.operations.contains(&required))
        {
            return Err(ErrorCode::ScopeDenied);
        }
        if !owner.public.scopes.iter().any(|s| s.slot == selection.slot)
            || owner.entries.iter().any(|e| e.slot == selection.slot)
        {
            return Err(ErrorCode::ScopeDenied);
        }
        let profile = self
            .data
            .profile()
            .ok_or(ErrorCode::StorageFailed)?
            .to_owned();
        // Directory::open retains the drive and every normal ancestor. File
        // selection replaces the last directory with a held file. Also reserve
        // the protected-profile walk, scratch metadata and cancellation thread
        // before opening anything; invalid path shapes still use the OS guard.
        let reserved = std::path::Path::new(&selection.path)
            .components()
            .count()
            .saturating_sub(1)
            + 1
            + profile.components().count().saturating_sub(1)
            + 2;
        if entry_slots(owner.entries.iter()) + reserved > MAX_RUN_REFS
            || self.resources.slots() + reserved > MAX_PROFILE_REFS
            || self.resources.active.len() >= 4
            || self
                .resources
                .active
                .values()
                .any(|a| a.owner.as_deref() == Some(&selection.owner))
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let id = Uuid::new_v4().to_string();
        let cancellation = Arc::new(Cancellation::default());
        self.resources.active.insert(
            id.clone(),
            Active {
                reserved,
                owner: Some(selection.owner.clone()),
                run: None,
                slot: selection.slot.clone(),
                right: ResourceRight::Stat,
                cancellation: cancellation.clone(),
                operation: None,
            },
        );
        Ok(ResourceIo {
            id,
            cancellation,
            action: IoAction::Select {
                path: selection.path.clone().into(),
                kind: selection.kind,
                profile,
            },
        })
    }
    pub fn finish_resource_selection(
        &mut self,
        id: &str,
        result: ResourceIoResult,
    ) -> Result<ResourceInfo, ErrorCode> {
        let active = self
            .resources
            .active
            .remove(id)
            .ok_or(ErrorCode::SessionInvalid)?;
        active.cancellation.check()?;
        let owner_id = active.owner.ok_or(ErrorCode::SessionInvalid)?;
        let owner = self.check_owner(&owner_id)?;
        let rights = owner
            .public
            .scopes
            .iter()
            .find(|s| s.slot == active.slot)
            .ok_or(ErrorCode::ScopeDenied)?
            .operations
            .clone();
        let IoValue::Selected(source) = result.0? else {
            return Err(ErrorCode::ExecutionFailed);
        };
        let (_, shared, own) = source.handles();
        let slots = shared + own + 1;
        if slots + 2 > active.reserved
            || entry_slots(owner.entries.iter()) + slots + 2 > MAX_RUN_REFS
            || self.resources.slots() + slots + 2 > MAX_PROFILE_REFS
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        if self
            .resource_observed_bytes()?
            .checked_add(source.observed_bytes())
            .is_none_or(|v| v > MAX_OBSERVED)
        {
            return Err(ErrorCode::BudgetExceeded);
        }
        let selected = entry(active.slot, source, rights);
        let info = selected.info.clone();
        self.resources
            .owners
            .get_mut(&owner_id)
            .unwrap()
            .entries
            .push(selected);
        Ok(info)
    }
    pub(super) fn drop_resource_owner(&mut self, id: &str) -> Result<(), ErrorCode> {
        if self
            .resources
            .active
            .values()
            .any(|a| a.owner.as_deref() == Some(id))
        {
            return Err(ErrorCode::RuntimeBusy);
        }
        self.resources
            .owners
            .remove(id)
            .ok_or(ErrorCode::ScopeDenied)?;
        Ok(())
    }
    pub(super) fn submit_resources(
        &mut self,
        connection: &str,
        caller: &str,
        submission: ResourceSubmission,
    ) -> Result<JobReceipt, ErrorCode> {
        let owner = self.check_owner(&submission.owner)?;
        if owner.public.identity.caller != caller
            || owner.public.identity.plugin_id != submission.submit.plugin_id
            || owner.public.identity.command_id != submission.submit.command_id
            || submission.submit.background
            || owner.entries.is_empty()
            || self
                .resources
                .active
                .values()
                .any(|a| a.owner.as_deref() == Some(&submission.owner))
        {
            return Err(ErrorCode::ScopeDenied);
        }
        let marker = digest(
            &json!({"owner":owner.public.id,"epoch":owner.epoch,"refs":owner.entries.iter().map(|e|&e.info.reference).collect::<Vec<_>>()}),
        );
        let receipt = self.submit_bound(connection, caller, submission.submit, Some(marker))?;
        let owner = self
            .resources
            .owners
            .remove(&submission.owner)
            .ok_or(ErrorCode::ScopeDenied)?;
        let entries = owner
            .entries
            .into_iter()
            .map(|mut e| {
                e.info.reference = Uuid::new_v4().to_string();
                e
            })
            .collect();
        self.resources.runs.insert(
            receipt.run_id.clone(),
            Run {
                entries,
                cursors: HashMap::new(),
                retired: HashMap::new(),
            },
        );
        Ok(receipt)
    }
    fn check_resource_call(
        &self,
        run: &str,
        slot: &str,
        right: ResourceRight,
    ) -> Result<(), ErrorCode> {
        self.check_run(run)?;
        let job = self.jobs.get(run).ok_or(ErrorCode::JobNotFound)?;
        let session = job
            .broker_session
            .as_ref()
            .ok_or(ErrorCode::SessionInvalid)?;
        let identity = self.broker.recheck_resource(session, slot, right, now())?;
        self.resource_policy(&identity, job.snapshot.grant_epoch)?;
        Ok(())
    }
    pub fn prepare_resource_call(
        &mut self,
        run: &str,
        operation: ResourceOperation,
    ) -> Result<ResourceIo, ErrorCode> {
        if let Some(service) = self.service_invocations.get_mut(run) {
            service.failure = Some(ErrorCode::ScopeDenied);
            return Err(ErrorCode::ScopeDenied);
        }
        if self.resources.active.len() >= 4
            || self
                .resources
                .active
                .values()
                .any(|a| a.run.as_deref() == Some(run))
        {
            return Err(ErrorCode::RuntimeBusy);
        }
        let binding = self.resources.runs.get(run).ok_or(ErrorCode::ScopeDenied)?;
        if binding.slots() + 2 > MAX_RUN_REFS || self.resources.slots() + 2 > MAX_PROFILE_REFS {
            return Err(ErrorCode::BudgetExceeded);
        }
        if let ResourceOperation::Release { reference } = &operation {
            if let Some((slot, right)) = binding.retired.get(reference) {
                let (slot, right) = (slot.clone(), *right);
                self.check_resource_call(run, &slot, right)?;
                self.broker.invoke_resource(
                    self.jobs[run].broker_session.as_ref().unwrap(),
                    &slot,
                    right,
                    now(),
                )?;
                let id = Uuid::new_v4().to_string();
                let cancellation = Arc::new(Cancellation::default());
                self.resources.active.insert(
                    id.clone(),
                    Active {
                        reserved: 2,
                        owner: None,
                        run: Some(run.into()),
                        slot,
                        right,
                        cancellation: cancellation.clone(),
                        operation: Some(operation),
                    },
                );
                return Ok(ResourceIo {
                    id,
                    cancellation,
                    action: IoAction::Released,
                });
            }
        }
        let (selected, right) = match &operation {
            ResourceOperation::Resolve { slot } => {
                let e = binding
                    .entries
                    .iter()
                    .find(|e| e.slot == *slot && e.source.is_some())
                    .ok_or(ErrorCode::ScopeDenied)?;
                (e, *e.info.rights.first().ok_or(ErrorCode::ScopeDenied)?)
            }
            ResourceOperation::Release { reference } => {
                let e = binding
                    .entries
                    .iter()
                    .find(|e| e.info.reference == *reference)
                    .ok_or(ErrorCode::ScopeDenied)?;
                (e, *e.info.rights.first().ok_or(ErrorCode::ScopeDenied)?)
            }
            ResourceOperation::Stat { reference } => (
                binding
                    .entries
                    .iter()
                    .find(|e| e.info.reference == *reference)
                    .ok_or(ErrorCode::ScopeDenied)?,
                ResourceRight::Stat,
            ),
            ResourceOperation::Read { reference, .. } => (
                binding
                    .entries
                    .iter()
                    .find(|e| e.info.reference == *reference)
                    .ok_or(ErrorCode::ScopeDenied)?,
                ResourceRight::Read,
            ),
            ResourceOperation::List { reference, .. } => (
                binding
                    .entries
                    .iter()
                    .find(|e| e.info.reference == *reference)
                    .ok_or(ErrorCode::ScopeDenied)?,
                ResourceRight::List,
            ),
        };
        self.check_resource_call(run, &selected.slot, right)?;
        if !selected.info.rights.contains(&right) {
            return Err(ErrorCode::ScopeDenied);
        }
        let source = selected
            .source
            .as_ref()
            .ok_or(ErrorCode::ScopeDenied)?
            .clone();
        let action = match &operation {
            ResourceOperation::Release { .. } => IoAction::Released,
            ResourceOperation::Read {
                revision,
                offset,
                max_bytes,
                ..
            } => {
                if *revision != selected.info.revision {
                    return Err(ErrorCode::RevisionConflict);
                }
                if *max_bytes == 0 || *max_bytes > MAX_CHUNK {
                    return Err(ErrorCode::BudgetExceeded);
                }
                IoAction::Read(source, *offset, *max_bytes)
            }
            ResourceOperation::List {
                reference,
                revision,
                cursor,
                limit,
            } => {
                if *revision != selected.info.revision {
                    return Err(ErrorCode::RevisionConflict);
                }
                if *limit == 0 || *limit > 64 {
                    return Err(ErrorCode::BudgetExceeded);
                }
                if binding.slots() + *limit as usize * 2 + 3 > MAX_RUN_REFS
                    || self.resources.slots() + *limit as usize * 2 + 3 > MAX_PROFILE_REFS
                {
                    return Err(ErrorCode::BudgetExceeded);
                }
                let index = match cursor {
                    Some(c) => {
                        let (r, index) = binding.cursors.get(c).ok_or(ErrorCode::ScopeDenied)?;
                        if r != reference {
                            return Err(ErrorCode::ScopeDenied);
                        }
                        *index
                    }
                    None => 0,
                };
                IoAction::List(source, index, *limit as usize)
            }
            _ => IoAction::Validate(source),
        };
        let slot = selected.slot.clone();
        self.broker.invoke_resource(
            self.jobs[run].broker_session.as_ref().unwrap(),
            &slot,
            right,
            now(),
        )?;
        let id = Uuid::new_v4().to_string();
        let cancellation = Arc::new(Cancellation::default());
        let reserved = if let ResourceOperation::List { limit, .. } = &operation {
            *limit as usize * 2 + 3
        } else {
            2
        };
        self.resources.active.insert(
            id.clone(),
            Active {
                reserved,
                owner: None,
                run: Some(run.into()),
                slot,
                right,
                cancellation: cancellation.clone(),
                operation: Some(operation),
            },
        );
        Ok(ResourceIo {
            id,
            cancellation,
            action,
        })
    }
    pub fn finish_resource_call(
        &mut self,
        id: &str,
        result: ResourceIoResult,
    ) -> Result<ResourceResponse, ErrorCode> {
        let active = self
            .resources
            .active
            .remove(id)
            .ok_or(ErrorCode::SessionInvalid)?;
        let run = active.run.ok_or(ErrorCode::SessionInvalid)?;
        let result = (|| {
            active.cancellation.check()?;
            self.check_resource_call(&run, &active.slot, active.right)?;
            let value = result.0?;
            let operation = active.operation.ok_or(ErrorCode::ExecutionFailed)?;
            let binding = self
                .resources
                .runs
                .get_mut(&run)
                .ok_or(ErrorCode::ScopeDenied)?;
            match (operation, value) {
                (ResourceOperation::Resolve { slot }, IoValue::Checked) => binding
                    .entries
                    .iter()
                    .find(|e| e.slot == slot && e.source.is_some())
                    .map(|e| ResourceResponse::Info(e.info.clone()))
                    .ok_or(ErrorCode::ScopeDenied),
                (ResourceOperation::Stat { reference }, IoValue::Checked) => binding
                    .entries
                    .iter()
                    .find(|e| e.info.reference == reference)
                    .map(|e| ResourceResponse::Info(e.info.clone()))
                    .ok_or(ErrorCode::ScopeDenied),
                (ResourceOperation::Release { reference }, IoValue::Checked) => {
                    if binding.retired.contains_key(&reference) {
                        return Ok(ResourceResponse::Released);
                    }
                    if binding.retired.len() >= 544 {
                        return Err(ErrorCode::BudgetExceeded);
                    }
                    binding.entries.retain(|e| e.info.reference != reference);
                    binding
                        .retired
                        .insert(reference.clone(), (active.slot.clone(), active.right));
                    binding.cursors.retain(|_, (r, _)| r != &reference);
                    Ok(ResourceResponse::Released)
                }
                (
                    ResourceOperation::Read {
                        reference, offset, ..
                    },
                    IoValue::Bytes(bytes),
                ) => {
                    use base64::Engine;
                    let size = binding
                        .entries
                        .iter()
                        .find(|e| e.info.reference == reference)
                        .and_then(|e| e.info.size)
                        .ok_or(ErrorCode::ScopeDenied)?;
                    let next_offset = offset
                        .checked_add(bytes.len() as u32)
                        .ok_or(ErrorCode::BudgetExceeded)?;
                    Ok(ResourceResponse::Chunk(ResourceChunk {
                        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
                        next_offset,
                        eof: next_offset == size,
                    }))
                }
                (
                    ResourceOperation::List {
                        reference, cursor, ..
                    },
                    IoValue::Children(children, next),
                ) => {
                    if let Some(cursor) = cursor {
                        binding.cursors.remove(&cursor);
                    }
                    let mut entries = Vec::new();
                    for child in children {
                        let child = entry(
                            active.slot.clone(),
                            Arc::new(child),
                            vec![ResourceRight::Stat],
                        );
                        entries.push(child.info.clone());
                        binding.entries.push(child);
                    }
                    let next_cursor = next.map(|index| {
                        let id = Uuid::new_v4().to_string();
                        binding.cursors.insert(id.clone(), (reference, index));
                        id
                    });
                    Ok(ResourceResponse::Page(ResourcePage {
                        entries,
                        next_cursor,
                    }))
                }
                _ => Err(ErrorCode::ExecutionFailed),
            }
        })();
        if let Some((pending, recovered)) = self.resources.deferred.remove(&run) {
            self.finish_internal(&run, pending, recovered);
        }
        result
    }
    pub fn active_resource_calls(&self) -> usize {
        self.resources.active.len()
    }
    pub fn tick_resources(&mut self) -> Result<(), ErrorCode> {
        let mut expired = Vec::new();
        for id in self.resources.owners.keys() {
            match self.check_owner(id) {
                Ok(_) => {}
                Err(ErrorCode::ResourceExpired | ErrorCode::GrantRevoked) => {
                    expired.push(id.clone())
                }
                Err(code) => return Err(code),
            }
        }
        for id in expired {
            for active in self
                .resources
                .active
                .values()
                .filter(|a| a.owner.as_deref() == Some(&id))
            {
                active.cancellation.cancel()?;
            }
            if !self
                .resources
                .active
                .values()
                .any(|a| a.owner.as_deref() == Some(&id))
            {
                self.resources.owners.remove(&id);
            }
        }
        Ok(())
    }
    pub fn resource_cancellation(&self, id: &str) -> Result<Arc<Cancellation>, ErrorCode> {
        self.resources
            .active
            .get(id)
            .map(|a| a.cancellation.clone())
            .ok_or(ErrorCode::SessionInvalid)
    }
    pub fn check_resource_io(&self, id: &str) -> Result<(), ErrorCode> {
        let active = self
            .resources
            .active
            .get(id)
            .ok_or(ErrorCode::SessionInvalid)?;
        if self.stopping {
            return Err(ErrorCode::ExecutionInterrupted);
        }
        if let Some(run) = &active.run {
            self.check_resource_call(run, &active.slot, active.right)
        } else {
            self.check_owner(active.owner.as_deref().ok_or(ErrorCode::SessionInvalid)?)
                .map(|_| ())
        }
    }
    pub fn resources_available(&self, run: &str) -> bool {
        self.resources.runs.contains_key(run)
    }
    pub(super) fn resource_observed_bytes(&self) -> Result<u64, ErrorCode> {
        let plans = crate::file_workflows::store::FilePlanStore::list(&self.data)?;
        let existing = plans.iter().map(|p| u64::from(p.total_bytes)).sum::<u64>();
        let media = crate::file_workflows::media_io::retained_input_ids(&self.data)?.len() as u64
            * crate::file_workflows::media_io::MAX_INPUT_BYTES
            * 2;
        existing
            .checked_add(media)
            .and_then(|n| n.checked_add(self.resources.observed()))
            .ok_or(ErrorCode::BudgetExceeded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        root: std::path::PathBuf,
        core: RuntimeCore,
        manager: SessionProof,
        cli: SessionProof,
        desktop: SessionProof,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("flowtools-resource-core-{}", Uuid::new_v4()));
            std::fs::create_dir(&root).unwrap();
            std::fs::create_dir(root.join("files")).unwrap();
            std::fs::create_dir(root.join("profile")).unwrap();
            let store = DataStore::open(&root.join("profile/runtime.sqlite")).unwrap();
            let resource = BuiltinCatalog::embedded_with_media()
                .command("plugin-resource-reader", "run")
                .unwrap()
                .0
                .clone();
            let mut core = RuntimeCore::managed_catalog(
                HashMap::from([
                    ("manager".into(), "local-manager".into()),
                    ("cli".into(), "local-cli".into()),
                    ("desktop".into(), "local-desktop".into()),
                ]),
                store,
                false,
                false,
                BuiltinCatalog::from_host_manifests(vec![resource]).unwrap(),
            )
            .unwrap();
            fn open(core: &mut RuntimeCore, who: &str) -> SessionProof {
                match core
                    .handle(
                        who,
                        Request {
                            version: PROTOCOL_MAJOR,
                            request_id: Uuid::new_v4().to_string(),
                            session: None,
                            call: Call::Open(OpenSession {
                                token: who.into(),
                                client_version: CLIENT_VERSION.into(),
                                expected_instance_id: None,
                            }),
                        },
                    )
                    .outcome
                {
                    Outcome::Session(proof) => proof,
                    other => panic!("{other:?}"),
                }
            }
            let manager = open(&mut core, "manager");
            let cli = open(&mut core, "cli");
            let desktop = open(&mut core, "desktop");
            Self {
                root,
                core,
                manager,
                cli,
                desktop,
            }
        }
        fn request(&mut self, who: &str, call: Call) -> Outcome {
            let proof = match who {
                "manager" => self.manager.clone(),
                "cli" => self.cli.clone(),
                _ => self.desktop.clone(),
            };
            self.core
                .handle(
                    who,
                    Request {
                        version: PROTOCOL_MAJOR,
                        request_id: Uuid::new_v4().to_string(),
                        session: Some(proof),
                        call,
                    },
                )
                .outcome
        }
        fn grant(&mut self) {
            let (manifest, _) = self
                .core
                .catalog
                .command("plugin-resource-reader", "run")
                .unwrap();
            let grant = ResourceGrant {
                format_version: 2,
                capability_profile: crate::native_contract::NATIVE_CAPABILITY_PROFILE.into(),
                plugin_id: "plugin-resource-reader".into(),
                command_id: "run".into(),
                target: GrantTarget::Cli,
                package_digest: digest(manifest),
                scopes: vec![ResourceScope {
                    slot: "source".into(),
                    operations: vec![
                        ResourceRight::Read,
                        ResourceRight::Stat,
                        ResourceRight::List,
                    ],
                }],
                expires_at: now() + 60000.0,
                max_calls: 4096,
            };
            assert!(matches!(
                self.request("manager", Call::ResourceGrant(grant)),
                Outcome::ResourcePermissions(_)
            ));
        }
        fn owner(&mut self) -> ResourceOwner {
            match self.request(
                "manager",
                Call::ResourceOwner(ResourceOwnerRequest {
                    key: PermissionKey {
                        plugin_id: "plugin-resource-reader".into(),
                        command_id: "run".into(),
                        target: GrantTarget::Cli,
                    },
                }),
            ) {
                Outcome::ResourceOwner(o) => o,
                v => panic!("{v:?}"),
            }
        }
        fn select(
            &mut self,
            owner: &ResourceOwner,
            path: &std::path::Path,
            kind: ResourceKind,
        ) -> ResourceInfo {
            let request = Request {
                version: PROTOCOL_MAJOR,
                request_id: Uuid::new_v4().to_string(),
                session: Some(self.manager.clone()),
                call: Call::ResourceSelect(ResourceSelection {
                    owner: owner.id.clone(),
                    slot: "source".into(),
                    path: path.to_str().unwrap().into(),
                    kind,
                }),
            };
            let work = self
                .core
                .prepare_resource_selection("manager", &request)
                .unwrap();
            let id = work.id.clone();
            self.core
                .finish_resource_selection(&id, work.perform())
                .unwrap()
        }
        fn submit(&mut self, owner: &ResourceOwner) -> String {
            let submit = SubmitJob {
                plugin_id: "plugin-resource-reader".into(),
                command_id: "run".into(),
                input: json!({}),
                background: false,
                deadline: now() + 30000.0,
                idempotency_key: Uuid::new_v4().to_string(),
            };
            match self.request(
                "cli",
                Call::ResourceSubmit(ResourceSubmission {
                    owner: owner.id.clone(),
                    submit,
                }),
            ) {
                Outcome::Receipt(r) => {
                    let spec = self.core.take_run(&r.run_id).unwrap();
                    self.core.validate_runner_spec(&spec).unwrap();
                    r.run_id
                }
                v => panic!("{v:?}"),
            }
        }
        fn call(
            &mut self,
            run: &str,
            op: ResourceOperation,
        ) -> Result<ResourceResponse, ErrorCode> {
            let work = self.core.prepare_resource_call(run, op)?;
            let id = work.id.clone();
            self.core.finish_resource_call(&id, work.perform())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.core.resources = State::default();
            self.core.private_jobs = None;
            let store = std::mem::replace(&mut self.core.data, DataStore::memory().unwrap());
            drop(store);
            assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(self
                .root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("flowtools-resource-core-"));
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
    fn resolved(f: &mut Fixture, run: &str) -> ResourceInfo {
        match f
            .call(
                run,
                ResourceOperation::Resolve {
                    slot: "source".into(),
                },
            )
            .unwrap()
        {
            ResourceResponse::Info(i) => i,
            _ => panic!(),
        }
    }
    #[test]
    fn resource_ancestor_handles_and_pending_selection_are_bounded_before_native_open() {
        let mut f = Fixture::new();
        f.grant();
        let path = f.root.join("files/handles.txt");
        std::fs::write(&path, b"held").unwrap();
        let make_request =
            |owner: &ResourceOwner, path: &std::path::Path, session: &SessionProof| Request {
                version: PROTOCOL_MAJOR,
                request_id: Uuid::new_v4().to_string(),
                session: Some(session.clone()),
                call: Call::ResourceSelect(ResourceSelection {
                    owner: owner.id.clone(),
                    slot: "source".into(),
                    path: path.to_str().unwrap().into(),
                    kind: ResourceKind::File,
                }),
            };
        let owner = f.owner();
        // These ancestors do not exist: refusal must precede filesystem IO.
        let deep = (0..MAX_RUN_REFS).fold(f.root.clone(), |p, _| p.join("deep"));
        assert!(matches!(
            f.core
                .prepare_resource_selection("manager", &make_request(&owner, &deep, &f.manager)),
            Err(ErrorCode::BudgetExceeded)
        ));
        assert_eq!(f.core.resources.slots(), 0);
        let work = f
            .core
            .prepare_resource_selection("manager", &make_request(&owner, &path, &f.manager))
            .unwrap();
        let reservation = f.core.resources.active[&work.id].reserved;
        assert_eq!(f.core.resources.slots(), reservation);
        let id = work.id.clone();
        f.core
            .finish_resource_selection(&id, work.perform())
            .unwrap();
        let held = path.components().count(); // Ref + drive/ancestors/file
        assert_eq!(f.core.resources.slots(), held);
        let mut count = 1;
        loop {
            let owner = f.owner();
            match f
                .core
                .prepare_resource_selection("manager", &make_request(&owner, &path, &f.manager))
            {
                Ok(work) => {
                    assert_eq!(f.core.resources.slots(), count * held + reservation);
                    assert!(f.core.resources.slots() <= MAX_PROFILE_REFS);
                    let id = work.id.clone();
                    f.core
                        .finish_resource_selection(&id, work.perform())
                        .unwrap();
                    count += 1;
                }
                Err(ErrorCode::BudgetExceeded) => break,
                other => panic!("unexpected selection: {:?}", other.err()),
            }
        }
        assert!(count > 1 && count < MAX_PROFILE_REFS / 2);
        assert_eq!(f.core.resources.slots(), count * held);
        assert!(count * held + reservation > MAX_PROFILE_REFS);
        f.core.resources.owners.clear();
        assert_eq!(f.core.resources.slots(), 0);
        std::fs::write(path, b"all held handles released").unwrap();
    }
    #[test]
    fn resource_jobs_share_the_original_metadata_fence_before_read_cancel_and_terminal_release() {
        for terminal_trigger in [false, true] {
            let mut f = Fixture::new();
            f.grant();
            let path = f.root.join("files/job-fence.txt");
            std::fs::write(&path, b"private").unwrap();
            let owner = f.owner();
            f.select(&owner, &path, ResourceKind::File);
            let run = f.submit(&owner);
            let source = resolved(&mut f, &run);
            let original: String = f
                .core
                .data
                .connection
                .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                    r.get(0)
                })
                .unwrap();
            if terminal_trigger {
                f.core.data.connection.execute_batch("CREATE TRIGGER substitute_resource_terminal AFTER UPDATE ON jobs BEGIN UPDATE jobs SET metadata='{}' WHERE run_id=NEW.run_id; END;").unwrap();
                f.core.finish(&run,Ok(json!({"type":"json","value":{"result":{"kind":"file","bytes":7,"entries":0}}})));
                assert_eq!(f.core.state(&run), Some(JobState::Interrupted));
                let actual: String = f
                    .core
                    .data
                    .connection
                    .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                        r.get(0)
                    })
                    .unwrap();
                assert_eq!(actual, original);
            } else {
                f.core
                    .data
                    .connection
                    .execute(
                        "UPDATE jobs SET metadata=metadata||' ' WHERE run_id=?1",
                        [&run],
                    )
                    .unwrap();
                assert!(matches!(
                    f.core.prepare_resource_call(
                        &run,
                        ResourceOperation::Read {
                            reference: source.reference,
                            revision: source.revision,
                            offset: 0,
                            max_bytes: 1
                        }
                    ),
                    Err(ErrorCode::StoreCorrupt)
                ));
                assert_eq!(f.core.active_resource_calls(), 0);
                f.core.cancel(&run);
                let actual: String = f
                    .core
                    .data
                    .connection
                    .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                        r.get(0)
                    })
                    .unwrap();
                assert_eq!(actual, original + " ");
            }
            assert!(f.core.stopping());
            assert!(f.core.resources_available(&run));
            assert!(std::fs::write(&path, b"must retain the held input").is_err());
        }
    }
    #[test]
    fn stat_only_grants_never_prepare_content_reads_and_expired_owners_release_the_actual_file() {
        let mut f = Fixture::new();
        f.grant();
        let mut grant = f.core.policy.as_ref().unwrap().resource_records[0]
            .grant
            .clone()
            .unwrap();
        grant.scopes[0].operations = vec![ResourceRight::Stat];
        assert!(matches!(
            f.request("manager", Call::ResourceGrant(grant)),
            Outcome::ResourcePermissions(_)
        ));
        let owner = f.owner();
        let path = f.root.join("files/selection.txt");
        std::fs::write(&path, b"private").unwrap();
        let request = Request {
            version: PROTOCOL_MAJOR,
            request_id: Uuid::new_v4().to_string(),
            session: Some(f.manager.clone()),
            call: Call::ResourceSelect(ResourceSelection {
                owner: owner.id,
                slot: "source".into(),
                kind: ResourceKind::File,
                path: path.to_str().unwrap().into(),
            }),
        };
        assert!(matches!(
            f.core.prepare_resource_selection("manager", &request),
            Err(ErrorCode::ScopeDenied)
        ));
        assert_eq!(f.core.active_resource_calls(), 0);
        f.grant();
        let owner = f.owner();
        f.select(&owner, &path, ResourceKind::File);
        assert!(std::fs::write(&path, b"denied").is_err());
        f.core
            .resources
            .owners
            .get_mut(&owner.id)
            .unwrap()
            .public
            .expires_at = now() - 1.0;
        f.core.tick_resources().unwrap();
        assert!(f.core.check_owner(&owner.id).is_err());
        std::fs::write(&path, b"released after expiry").unwrap();
        let owner = f.owner();
        let mut changed = f
            .core
            .catalog
            .command("plugin-resource-reader", "run")
            .unwrap()
            .0
            .clone();
        changed["version"] = json!("0.0.2");
        f.core.catalog = BuiltinCatalog::from_host_manifests(vec![changed]).unwrap();
        assert!(matches!(
            f.core.check_owner(&owner.id),
            Err(ErrorCode::GrantRevoked)
        ));
    }
    #[test]
    fn resource_bindings_refuse_cross_owner_caller_run_epoch_and_restart_and_keep_history_private()
    {
        let mut f = Fixture::new();
        f.grant();
        let path = f.root.join("files/PRIVATE-NAME.txt");
        std::fs::write(&path, b"PRIVATE-CONTENT").unwrap();
        let owner = f.owner();
        let selected = f.select(&owner, &path, ResourceKind::File);
        let wrong = ResourceSubmission {
            owner: owner.id.clone(),
            submit: SubmitJob {
                plugin_id: "plugin-resource-reader".into(),
                command_id: "run".into(),
                input: json!({}),
                background: false,
                deadline: now() + 30000.0,
                idempotency_key: Uuid::new_v4().to_string(),
            },
        };
        assert!(matches!(
            f.request("desktop", Call::ResourceSubmit(wrong)),
            Outcome::Error(RuntimeError {
                code: ErrorCode::ScopeDenied
            })
        ));
        let run = f.submit(&owner);
        let info = resolved(&mut f, &run);
        assert_ne!(selected.reference, info.reference);
        assert!(f
            .call(
                &run,
                ResourceOperation::Stat {
                    reference: selected.reference
                }
            )
            .is_err());
        let chunk = f
            .call(
                &run,
                ResourceOperation::Read {
                    reference: info.reference.clone(),
                    revision: info.revision.clone(),
                    offset: 0,
                    max_bytes: 65536,
                },
            )
            .unwrap();
        assert!(matches!(
            chunk,
            ResourceResponse::Chunk(ResourceChunk {
                eof: true,
                next_offset: 15,
                ..
            })
        ));
        let owner2 = f.owner();
        f.select(&owner2, &path, ResourceKind::File);
        let run2 = f.submit(&owner2);
        assert!(f
            .call(
                &run2,
                ResourceOperation::Stat {
                    reference: info.reference.clone()
                }
            )
            .is_err());
        let stored: String = f
            .core
            .data
            .connection
            .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                r.get(0)
            })
            .unwrap();
        for secret in [
            path.to_str().unwrap(),
            "PRIVATE-NAME",
            "PRIVATE-CONTENT",
            info.reference.as_str(),
            owner.id.as_str(),
        ] {
            assert!(!stored.contains(secret));
        }
        f.grant();
        assert_eq!(
            f.call(
                &run,
                ResourceOperation::Stat {
                    reference: info.reference
                }
            ),
            Err(ErrorCode::GrantRevoked)
        );
        f.core.resources = State::default();
        assert!(f.core.check_owner(&owner2.id).is_err());
        assert!(!f.core.resources_available(&run2));
    }
    #[test]
    fn resource_directory_children_are_stat_only_and_page_cursors_are_single_use_and_bound() {
        let mut f = Fixture::new();
        f.grant();
        let directory = f.root.join("files");
        for n in 0..12 {
            std::fs::write(directory.join(format!("{n:02}.txt")), b"sample").unwrap();
        }
        let owner = f.owner();
        f.select(&owner, &directory, ResourceKind::Directory);
        let run = f.submit(&owner);
        let source = resolved(&mut f, &run);
        let ResourceResponse::Page(page) = f
            .call(
                &run,
                ResourceOperation::List {
                    reference: source.reference.clone(),
                    revision: source.revision.clone(),
                    cursor: None,
                    limit: 8,
                },
            )
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(page.entries.len(), 8);
        let child = &page.entries[0];
        assert_eq!(child.rights, vec![ResourceRight::Stat]);
        assert!(f
            .call(
                &run,
                ResourceOperation::Read {
                    reference: child.reference.clone(),
                    revision: child.revision.clone(),
                    offset: 0,
                    max_bytes: 1
                }
            )
            .is_err());
        let cursor = page.next_cursor.unwrap();
        let operation = ResourceOperation::List {
            reference: source.reference.clone(),
            revision: source.revision.clone(),
            cursor: Some(cursor),
            limit: 8,
        };
        // Retained child handles and their Ref each consume a slot. Releasing
        // children makes room for the next page; asking for eight is reserved
        // before IO even when the eventual page contains fewer entries.
        assert!(matches!(
            f.core.prepare_resource_call(&run, operation.clone()),
            Err(ErrorCode::BudgetExceeded)
        ));
        for child in &page.entries[1..] {
            f.call(
                &run,
                ResourceOperation::Release {
                    reference: child.reference.clone(),
                },
            )
            .unwrap();
        }
        f.call(&run, operation.clone()).unwrap();
        assert!(f.call(&run, operation).is_err());
        assert!(matches!(
            f.core.prepare_resource_call(
                &run,
                ResourceOperation::List {
                    reference: source.reference.clone(),
                    revision: source.revision.clone(),
                    cursor: None,
                    limit: 32
                }
            ),
            Err(ErrorCode::BudgetExceeded)
        ));
        let bytes_before = f.core.resources.observed();
        let slots_before = f.core.resources.slots();
        f.call(
            &run,
            ResourceOperation::Release {
                reference: source.reference.clone(),
            },
        )
        .unwrap();
        f.call(
            &run,
            ResourceOperation::Release {
                reference: source.reference.clone(),
            },
        )
        .unwrap();
        assert_eq!(f.core.resources.observed(), bytes_before);
        assert_eq!(f.core.resources.slots(), slots_before - 1);
        std::fs::write(directory.join("NEW.txt"), b"changed").unwrap();
        assert_eq!(
            f.call(
                &run,
                ResourceOperation::Stat {
                    reference: child.reference.clone()
                }
            ),
            Err(ErrorCode::RevisionConflict)
        );
    }
    #[test]
    fn resource_revocation_after_io_and_cancellation_before_drain_never_deliver_bytes_or_finish_early(
    ) {
        let mut f = Fixture::new();
        f.grant();
        let path = f.root.join("files/source.txt");
        std::fs::write(&path, b"secret").unwrap();
        let owner = f.owner();
        f.select(&owner, &path, ResourceKind::File);
        let run = f.submit(&owner);
        let source = resolved(&mut f, &run);
        let work = f
            .core
            .prepare_resource_call(
                &run,
                ResourceOperation::Read {
                    reference: source.reference,
                    revision: source.revision,
                    offset: 0,
                    max_bytes: 6,
                },
            )
            .unwrap();
        let id = work.id.clone();
        let result = work.perform();
        f.grant();
        assert_eq!(
            f.core.finish_resource_call(&id, result),
            Err(ErrorCode::GrantRevoked)
        );
        let owner = f.owner();
        f.select(&owner, &path, ResourceKind::File);
        let run = f.submit(&owner);
        let source = resolved(&mut f, &run);
        let work = f
            .core
            .prepare_resource_call(
                &run,
                ResourceOperation::Stat {
                    reference: source.reference,
                },
            )
            .unwrap();
        let id = work.id.clone();
        f.core.cancel(&run);
        f.core.finish(&run, Err(ErrorCode::Aborted));
        assert_eq!(f.core.state(&run), Some(JobState::Cancelling));
        assert_eq!(f.core.active_resource_calls(), 1);
        assert!(f.core.finish_resource_call(&id, work.perform()).is_err());
        assert_eq!(f.core.state(&run), Some(JobState::Cancelled));
        assert_eq!(f.core.active_resource_calls(), 0);
        assert!(!f.core.resources_available(&run));
    }
}
