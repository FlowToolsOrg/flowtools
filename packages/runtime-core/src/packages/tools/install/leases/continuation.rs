//! One live logical call, not an accepted-job replay or a rollback selector.
use super::*;
use crate::{dependencies::PackagePin, packages::tools::media::MediaActionV1};
pub(crate) struct LiveCallBinding<'a> {
    pub id: &'a str,
    pub run: &'a str,
    pub consumer: &'a PackagePin,
    pub action: MediaActionV1,
}
pub(crate) struct LiveToolCall {
    stored: StoredToolLease,
    id: String,
    run: String,
    consumer: PackagePin,
    action: MediaActionV1,
    proofs: Vec<Arc<crate::packages::trust::HeldDescriptor>>,
}
impl LiveToolCall {
    fn validate(
        &self,
        store: &DataStore,
        job: &JobSnapshot,
        lock: &DependencyLock,
        stored: &StoredToolLease,
        binding: &LiveCallBinding<'_>,
    ) -> std::result::Result<(), ErrorCode> {
        let (id, run, consumer, action) =
            (binding.id, binding.run, binding.consumer, binding.action);
        self.stored.validate(job, lock)?;
        stored.validate(job, lock)?;
        if !self.stored.matches_scope(store)? || !stored.matches_scope(store)? {
            return Err(ErrorCode::RecoveryPending);
        }
        if self.stored.digest != stored.digest
            || self.id != id
            || self.run != run
            || self.consumer != *consumer
            || self.action != action
            || !stored.retained
            || job.state != JobState::Running
            || self.proofs.len() != stored.images.len()
            || stored.images.iter().any(|image| {
                self.proofs
                    .iter()
                    .filter(|proof| proof.original().package_digest() == image.pin().archive_digest)
                    .count()
                    != 1
            })
        {
            return Err(ErrorCode::DependencyArtifactMismatch);
        }
        Ok(())
    }
    fn check_trust(
        &self,
        trust: &PackageTrustStore,
        store: &mut DataStore,
        now: u64,
    ) -> std::result::Result<(), ErrorCode> {
        for proof in &self.proofs {
            trust
                .check_held_descriptor(store, proof, now)
                .map_err(|e| failed(ToolInstallError::Admission(e.into())))?;
        }
        images::require_no_pending(&store.connection).map_err(|e| failed(e.into()))
    }
}
impl ToolAuthority {
    pub(crate) fn begin_live_call(
        &self,
        store: &mut DataStore,
        job: &JobSnapshot,
        lock: &DependencyLock,
        lease: &ToolLease,
        binding: &LiveCallBinding<'_>,
    ) -> std::result::Result<LiveToolCall, ErrorCode> {
        let (id, run, consumer, action) =
            (binding.id, binding.run, binding.consumer, binding.action);
        // Strict ordinary floors at the logical-call boundary; a later effect
        // cannot manufacture continuity from an old durable row or cached pin.
        self.check_active(store, job, lock, lease)?;
        lease.media_tool(lock, consumer, action)?;
        if !uuid(id) || !uuid(run) {
            return Err(ErrorCode::InputInvalid);
        }
        let live = LiveToolCall {
            stored: lease.stored.clone(),
            id: id.into(),
            run: run.into(),
            consumer: consumer.clone(),
            action,
            proofs: lease.held_descriptors.clone(),
        };
        live.validate(store, job, lock, &lease.stored, binding)?;
        let mut clock = self.clock()?;
        live.check_trust(&self.trust, store, clock())?;
        Ok(live)
    }
    pub(crate) fn check_live_call(
        &self,
        store: &mut DataStore,
        job: &JobSnapshot,
        lock: &DependencyLock,
        lease: &ToolLease,
        live: &LiveToolCall,
        binding: &LiveCallBinding<'_>,
    ) -> std::result::Result<(), ErrorCode> {
        live.validate(store, job, lock, &lease.stored, binding)?;
        let receipts = self.active_receipts(store, job, lock, lease)?;
        if live.proofs.iter().any(|proof| {
            receipts
                .iter()
                .filter(|receipt| {
                    receipt.package_digest() == proof.original().package_digest()
                        && receipt.descriptor_digest() == proof.original().descriptor_digest()
                })
                .count()
                != 1
        }) {
            return Err(ErrorCode::DependencyArtifactMismatch);
        }
        let mut clock = self.clock()?;
        live.check_trust(&self.trust, store, clock())
    }
    pub(crate) fn restore_live_call(
        &mut self,
        store: &mut DataStore,
        job: &JobSnapshot,
        lock: &DependencyLock,
        stored: &StoredToolLease,
        live: &LiveToolCall,
        binding: &LiveCallBinding<'_>,
    ) -> std::result::Result<ToolLease, ErrorCode> {
        live.validate(store, job, lock, stored, binding)?;
        let mut clock = self.clock()?;
        live.check_trust(&self.trust, store, clock())?;
        let pins = stored
            .images
            .iter()
            .map(|image| image.pin().clone())
            .collect::<Vec<_>>();
        #[cfg(test)]
        let sources = if self.fixture_clock.is_some() {
            consumer_sources::read_for_call_fixture(
                &self.trust,
                store,
                &pins,
                &live.proofs,
                &mut *clock,
            )
        } else {
            consumer_sources::read_for_call(&self.trust, store, &pins, &live.proofs, &mut *clock)
        }
        .map_err(failed)?;
        #[cfg(not(test))]
        let sources =
            consumer_sources::read_for_call(&self.trust, store, &pins, &live.proofs, &mut *clock)
                .map_err(failed)?;
        let mut held = Vec::new();
        for image in &stored.images {
            let archive = sources
                .archives
                .iter()
                .find(|archive| {
                    archive.archive().descriptor().package_digest() == image.pin().archive_digest
                })
                .ok_or(ErrorCode::DependencyArtifactMismatch)?;
            let current =
                lease_source::hold_existing_for_call(&self.trust, store, archive, &mut *clock)
                    .map_err(failed)?;
            if current.reference() != image {
                return Err(ErrorCode::DependencyArtifactMismatch);
            }
            let key = value_digest(current.reference()).map_err(failed)?;
            let current = match self.held.get(&key).and_then(Weak::upgrade) {
                Some(existing) => existing,
                None => {
                    let current = Arc::new(current);
                    self.held.insert(key, Arc::downgrade(&current));
                    current
                }
            };
            held.push(current);
        }
        live.check_trust(&self.trust, store, clock())?;
        live.validate(store, job, lock, stored, binding)?;
        Ok(ToolLease {
            stored: stored.clone(),
            _held: held,
            checked_receipts: sources
                .archives
                .iter()
                .map(|archive| archive.archive().descriptor().clone())
                .collect(),
            held_descriptors: live.proofs.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::types::Value as SqlValue;
    #[test]
    fn malformed_generation_is_bounded_inside_sql_and_never_reset_or_deleted() {
        let empty = DataStore::memory().unwrap();
        assert_eq!(generation(&empty).unwrap(), None);
        for value in [
            SqlValue::Text(String::new()),
            SqlValue::Text("x".repeat(129)),
            SqlValue::Text("界".repeat(64)),
            SqlValue::Text("x".repeat(1_048_576)),
            SqlValue::Blob(vec![0; 1_048_576]),
        ] {
            let bytes = match &value {
                SqlValue::Text(v) => v.len(),
                SqlValue::Blob(v) => v.len(),
                _ => unreachable!(),
            };
            let store = DataStore::memory().unwrap();
            store
                .connection
                .execute(
                    "INSERT INTO core_metadata(key,value) VALUES('recovery-generation',?1)",
                    [value],
                )
                .unwrap();
            assert_eq!(generation(&store), Err(ErrorCode::StoreCorrupt));
            let actual:i64=store.connection.query_row("SELECT length(CAST(value AS BLOB)) FROM core_metadata WHERE key='recovery-generation'",[],|r|r.get(0)).unwrap();
            assert_eq!(actual as usize, bytes);
        }
        for value in ["x".repeat(128), "界".repeat(42)] {
            let store = DataStore::memory().unwrap();
            store
                .connection
                .execute(
                    "INSERT INTO core_metadata(key,value) VALUES('recovery-generation',?1)",
                    [&value],
                )
                .unwrap();
            assert_eq!(generation(&store).unwrap(), Some(value));
        }
    }
}
