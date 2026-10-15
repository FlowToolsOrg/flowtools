//! Explicit T0 recovery after restores, authenticated by two independent quorums.
//! No reset, pin replacement, rollback grant, executable identity or client API.
use super::*;

pub const RECOVERY_PAYLOAD_TYPE: &str = "application/vnd.flowtools.trust-recovery.v1+json";
const REQUEST: &str = "package-trust-recovery-request-v1";
const RECEIPT: &str = "package-trust-recovery-receipt-v1";
const MAX_CHECKPOINT: usize = 8_388_608;
const REQUEST_LIFETIME: u64 = 86_400;
const APPROVAL_LIFETIME: u64 = 300;

pub(super) struct RecoveryAuthority {
    pins: BootstrapPins,
    digest: String,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scope {
    marker: Option<String>,
    generation: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    format_version: u16,
    id: String,
    profile_digest: String,
    scope: Scope,
    state_digest: Option<String>,
    prior_receipt_digest: Option<String>,
    issued_at: u64,
}
/// T0 metadata for an out-of-band signer; never a grant or caller-selected nonce.
pub struct TrustRecoveryChallenge {
    id: String,
    digest: String,
    issued_at: u64,
    authority_digest: String,
}
impl TrustRecoveryChallenge {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn request_digest(&self) -> &str {
        &self.digest
    }
    pub fn issued_at(&self) -> u64 {
        self.issued_at
    }
    pub fn authority_digest(&self) -> &str {
        &self.authority_digest
    }
}
/// Signing contract, not authorization by itself. Verify the real DSSE quorum.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustRecoveryApproval {
    pub format_version: u16,
    pub request_digest: String,
    pub checkpoint_sha256: String,
    pub root_sha256: String,
    pub authority_sha256: String,
    pub issued_at: u64,
    pub expires_at: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    format_version: u16,
    request: Request,
    checkpoint: String,
    approval: String,
    completed_at: u64,
}

fn metadata(connection: &rusqlite::Connection, key: &str) -> Result<Option<String>, PackageError> {
    let size: Option<i64> = connection
        .query_row(
            "SELECT length(CAST(value AS BLOB)) FROM core_metadata WHERE key=?1",
            [key],
            |r| r.get(0),
        )
        .optional()
        .map_err(storage)?;
    match size {
        None => Ok(None),
        Some(n) if n < 0 || n > MAX_STATE_BYTES as i64 => Err(PackageError::TrustStateInvalid),
        Some(_) => connection
            .query_row("SELECT value FROM core_metadata WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .map(Some)
            .map_err(storage),
    }
}
fn read<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, PackageError> {
    serde_json::from_value(
        strict_json(text.as_bytes()).map_err(|_| PackageError::TrustStateInvalid)?,
    )
    .map_err(|_| PackageError::TrustStateInvalid)
}
fn encode<T: Serialize>(value: &T) -> Result<String, PackageError> {
    let text = serde_json::to_string(value).map_err(|_| PackageError::TrustStateInvalid)?;
    if text.len() > MAX_STATE_BYTES {
        return Err(PackageError::TrustBudgetExceeded);
    }
    Ok(text)
}
fn put(connection: &rusqlite::Connection, key: &str, text: &str) -> Result<(), PackageError> {
    connection.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,text]).map_err(storage)?;
    Ok(())
}
fn scope(connection: &rusqlite::Connection) -> Result<Scope, PackageError> {
    let value = Scope {
        marker: metadata(connection, RECOVERY_KEY)?,
        generation: metadata(connection, "recovery-generation")?,
    };
    if [&value.marker, &value.generation]
        .into_iter()
        .flatten()
        .any(|v| v.is_empty() || v.len() > 128)
    {
        return Err(PackageError::TrustStateInvalid);
    }
    Ok(value)
}
fn digest(value: &Request) -> Result<String, PackageError> {
    Ok(hash(encode(value)?.as_bytes()))
}
fn profile_digest(connection: &rusqlite::Connection) -> Result<String, PackageError> {
    let filename = connection
        .path()
        .filter(|path| !path.is_empty() && *path != ":memory:")
        .ok_or(PackageError::TrustStateInvalid)?;
    let path = std::fs::canonicalize(filename).map_err(|_| PackageError::TrustStorageFailed)?;
    // Bind the actual T0-opened database, without persisting its raw path.
    // Copied SQLite requests/receipts cannot authorize another profile.
    Ok(hash(path.as_os_str().as_encoded_bytes()))
}
fn validate_request(request: &Request) -> Result<(), PackageError> {
    if request.format_version != 1
        || !uuid::Uuid::parse_str(&request.id).is_ok_and(|id| id.to_string() == request.id)
        || !valid_digest(&request.profile_digest)
        || (request.scope.marker.is_none() && request.scope.generation.is_none())
        || [&request.scope.marker, &request.scope.generation]
            .into_iter()
            .flatten()
            .any(|v| v.is_empty() || v.len() > 128)
        || [&request.state_digest, &request.prior_receipt_digest]
            .into_iter()
            .flatten()
            .any(|d| !valid_digest(d))
        || request.issued_at > MAX_SAFE_INTEGER
    {
        return Err(PackageError::TrustStateInvalid);
    }
    Ok(())
}
fn preserves(candidate: &State, baseline: &State) -> Result<(), PackageError> {
    if candidate.pins_digest != baseline.pins_digest
        || candidate.observed_time < baseline.observed_time
        || !candidate.roots.starts_with(&baseline.roots)
        || baseline
            .known_versions
            .iter()
            .any(|v| !candidate.known_versions.contains(v))
    {
        return Err(PackageError::RootRollback);
    }
    Ok(())
}
fn checkpoint(
    trust: &PackageTrustStore,
    bytes: &[u8],
) -> Result<(State, VerifiedRoot), PackageError> {
    if bytes.len() > MAX_CHECKPOINT {
        return Err(PackageError::TrustBudgetExceeded);
    }
    let (state, root) = trust
        .parse_state(std::str::from_utf8(bytes).map_err(|_| PackageError::TrustStateInvalid)?)?;
    trust
        .recovery
        .as_ref()
        .ok_or(PackageError::TrustRecoveryAuthorityRequired)?;
    Ok((state, root))
}

pub(super) fn check_roles(
    trust: &PackageTrustStore,
    root: &VerifiedRoot,
) -> Result<(), PackageError> {
    if let Some(authority) = &trust.recovery {
        if root
            .payload()
            .root_keys
            .iter()
            .chain(root.payload().publishers.iter().flat_map(|p| &p.keys))
            .any(|key| {
                authority
                    .pins
                    .keys
                    .iter()
                    .any(|recovery| recovery.keyid == key.keyid)
            })
        {
            return Err(PackageError::TrustInvalid);
        }
    }
    Ok(())
}
fn verify_approval(
    authority: &RecoveryAuthority,
    root: &VerifiedRoot,
    request: &Request,
    checkpoint: &[u8],
    envelope: &[u8],
    now: u64,
    floor: u64,
) -> Result<TrustRecoveryApproval, PackageError> {
    validate_request(request)?;
    validate_clock(VerificationClock { now, floor })?;
    if now < request.issued_at || now - request.issued_at > REQUEST_LIFETIME {
        return Err(PackageError::TrustExpired);
    }
    let decoded = decode_envelope(envelope, RECOVERY_PAYLOAD_TYPE)?;
    let approval: TrustRecoveryApproval = parse_payload(&decoded.payload)?;
    if approval.format_version != 1
        || approval.request_digest != digest(request)?
        || approval.checkpoint_sha256 != hash(checkpoint)
        || approval.root_sha256 != root.payload_digest()
        || approval.authority_sha256 != authority.digest
        || approval.issued_at < request.issued_at
    {
        return Err(PackageError::TrustInvalid);
    }
    let clock = VerificationClock { now, floor };
    validate_time(
        approval.issued_at,
        approval.expires_at,
        clock,
        APPROVAL_LIFETIME,
    )?;
    validate_time(
        root.payload().issued_at,
        root.payload().expires_at,
        clock,
        MAX_ROOT_LIFETIME,
    )?;
    verify_threshold(
        &decoded,
        &root.payload().root_keys,
        root.payload().root_threshold,
        &root.payload().revoked_key_ids,
    )?;
    verify_threshold(
        &decoded,
        &authority.pins.keys,
        authority.pins.threshold,
        &[],
    )?;
    Ok(approval)
}
fn receipt_baseline(trust: &PackageTrustStore, receipt: &Receipt) -> Result<State, PackageError> {
    if receipt.format_version != 1
        || receipt.checkpoint.len() > MAX_CHECKPOINT.div_ceil(3) * 4
        || receipt.approval.len() > MAX_ENVELOPE_BYTES.div_ceil(3) * 4
    {
        return Err(PackageError::TrustStateInvalid);
    }
    let bytes = decode_base64(&receipt.checkpoint).map_err(|_| PackageError::TrustStateInvalid)?;
    let (mut state, root) = checkpoint(trust, &bytes)?;
    let envelope = decode_base64(&receipt.approval).map_err(|_| PackageError::TrustStateInvalid)?;
    // Approval expiry applies to the actual commit, not future normal operation.
    // Reauthenticate the historical receipt and retain its floor on every load.
    verify_approval(
        trust
            .recovery
            .as_ref()
            .ok_or(PackageError::TrustRecoveryAuthorityRequired)?,
        &root,
        &receipt.request,
        &bytes,
        &envelope,
        receipt.completed_at,
        state.observed_time,
    )
    .map_err(|_| PackageError::TrustStateInvalid)?;
    state.observed_time = receipt.completed_at;
    Ok(state)
}

pub(super) fn guard(
    trust: &PackageTrustStore,
    connection: &rusqlite::Connection,
    current: Option<&str>,
) -> Result<(), PackageError> {
    let current_scope = scope(connection)?;
    let raw = metadata(connection, RECEIPT)?;
    if current_scope.marker.is_none() && current_scope.generation.is_none() {
        return if raw.is_none() {
            Ok(())
        } else {
            Err(PackageError::TrustStateInvalid)
        };
    }
    let receipt: Receipt = read(&raw.ok_or(PackageError::TrustRecoveryRequired)?)?;
    if receipt.request.scope != current_scope
        || receipt.request.profile_digest != profile_digest(connection)?
    {
        return Err(PackageError::TrustRecoveryRequired);
    }
    let baseline = receipt_baseline(trust, &receipt)?;
    let (current, _) = trust.parse_state(current.ok_or(PackageError::TrustStateInvalid)?)?;
    preserves(&current, &baseline)
}

impl PackageTrustStore {
    /// Immutable release-owned recovery pins, separate from root/publisher roles.
    /// Ordinary new(pins) denies recovery; never accept these pins from clients.
    pub fn with_recovery_authority(
        pins: BootstrapPins,
        mut recovery: BootstrapPins,
    ) -> Result<Self, PackageError> {
        let mut trust = Self::new(pins)?;
        validate_key_set(&recovery.keys, recovery.threshold, 2)?;
        recovery.keys.sort_by(|a, b| a.keyid.cmp(&b.keyid));
        if recovery
            .keys
            .iter()
            .any(|key| trust.pins.keys.iter().any(|p| p.keyid == key.keyid))
        {
            return Err(PackageError::TrustInvalid);
        }
        let digest = hash(
            &serde_json::to_vec(&(recovery.threshold, &recovery.keys))
                .map_err(|_| PackageError::TrustInvalid)?,
        );
        trust.recovery = Some(RecoveryAuthority {
            pins: recovery,
            digest,
        });
        Ok(trust)
    }
    /// Export an authoritative T0 observation before restore. Export is unsigned.
    /// The external root quorum must attest completeness and sign AFTER challenge.
    pub fn export_recovery_checkpoint(
        &self,
        store: &mut DataStore,
        now: u64,
    ) -> Result<Vec<u8>, PackageError> {
        self.recovery
            .as_ref()
            .ok_or(PackageError::TrustRecoveryAuthorityRequired)?;
        self.observe(store, now)?;
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (_, root) = self.load(&tx)?;
        validate_time(
            root.payload().issued_at,
            root.payload().expires_at,
            VerificationClock { now, floor: now },
            MAX_ROOT_LIFETIME,
        )?;
        let text = raw_state(&tx)?.ok_or(PackageError::TrustStateRequired)?;
        if text.len() > MAX_CHECKPOINT {
            return Err(PackageError::TrustBudgetExceeded);
        }
        checkpoint(self, text.as_bytes())?;
        Ok(text.into_bytes())
    }

    /// Explicit T0 request; does not clear either restore marker or restore grants.
    pub fn begin_trust_recovery(
        &self,
        store: &mut DataStore,
        now: u64,
    ) -> Result<TrustRecoveryChallenge, PackageError> {
        let authority = self
            .recovery
            .as_ref()
            .ok_or(PackageError::TrustRecoveryAuthorityRequired)?;
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let state = raw_state(&tx)?;
        match guard(self, &tx, state.as_deref()) {
            Err(PackageError::TrustRecoveryRequired) => (),
            Ok(()) => return Err(PackageError::TrustInvalid),
            Err(e) => return Err(e),
        }
        if let Some(raw) = &state {
            let (baseline, _) = self.parse_state(raw)?;
            validate_clock(VerificationClock {
                now,
                floor: baseline.observed_time,
            })?;
        } else {
            validate_clock(VerificationClock { now, floor: 0 })?;
        }
        let current_scope = scope(&tx)?;
        let prior = metadata(&tx, RECEIPT)?;
        let mut request = Request {
            format_version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            profile_digest: profile_digest(&tx)?,
            scope: current_scope,
            state_digest: state.as_deref().map(|s| hash(s.as_bytes())),
            prior_receipt_digest: prior.as_deref().map(|s| hash(s.as_bytes())),
            issued_at: now,
        };
        if let Some(raw) = metadata(&tx, REQUEST)? {
            let previous: Request = read(&raw)?;
            validate_request(&previous)?;
            if previous.scope == request.scope
                && previous.profile_digest == request.profile_digest
                && previous.state_digest == request.state_digest
                && previous.prior_receipt_digest == request.prior_receipt_digest
            {
                if now < previous.issued_at {
                    return Err(PackageError::ClockRollback);
                }
                if now - previous.issued_at <= REQUEST_LIFETIME {
                    request = previous;
                }
            }
        }
        let challenge = TrustRecoveryChallenge {
            id: request.id.clone(),
            digest: digest(&request)?,
            issued_at: request.issued_at,
            authority_digest: authority.digest.clone(),
        };
        put(&tx, REQUEST, &encode(&request)?)?;
        tx.commit().map_err(storage)?;
        Ok(challenge)
    }

    pub fn recover_trust(
        &self,
        store: &mut DataStore,
        checkpoint: &[u8],
        approval: &[u8],
        clock: impl FnMut() -> u64,
    ) -> Result<TrustSnapshot, PackageError> {
        self.recover_with_hook(store, checkpoint, approval, clock, || Ok(()))
    }

    pub(in super::super) fn recover_with_hook(
        &self,
        store: &mut DataStore,
        checkpoint_bytes: &[u8],
        approval: &[u8],
        mut clock: impl FnMut() -> u64,
        before_commit: impl FnOnce() -> Result<(), PackageError>,
    ) -> Result<TrustSnapshot, PackageError> {
        let authority = self
            .recovery
            .as_ref()
            .ok_or(PackageError::TrustRecoveryAuthorityRequired)?;
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = raw_state(&tx)?;
        match guard(self, &tx, current.as_deref()) {
            Err(PackageError::TrustRecoveryRequired) => (),
            Ok(()) => return Err(PackageError::TrustInvalid),
            Err(e) => return Err(e),
        }
        let request: Request =
            read(&metadata(&tx, REQUEST)?.ok_or(PackageError::TrustRecoveryRequired)?)?;
        let prior = metadata(&tx, RECEIPT)?;
        if request.scope != scope(&tx)?
            || request.profile_digest != profile_digest(&tx)?
            || request.state_digest != current.as_deref().map(|s| hash(s.as_bytes()))
            || request.prior_receipt_digest != prior.as_deref().map(|s| hash(s.as_bytes()))
        {
            return Err(PackageError::TrustInvalid);
        }
        let (mut candidate, root) = checkpoint(self, checkpoint_bytes)?;
        if let Some(raw) = current {
            let (baseline, _) = self.parse_state(&raw)?;
            preserves(&candidate, &baseline)?;
        }
        if let Some(raw) = prior {
            let baseline = receipt_baseline(self, &read(&raw)?)?;
            preserves(&candidate, &baseline)?;
        }
        let now = clock();
        let verified_approval = verify_approval(
            authority,
            &root,
            &request,
            checkpoint_bytes,
            approval,
            now,
            candidate.observed_time,
        )?;
        let mut receipt = Receipt {
            format_version: 1,
            request,
            checkpoint: general_purpose::STANDARD.encode(checkpoint_bytes),
            approval: general_purpose::STANDARD.encode(approval),
            completed_at: now,
        };
        candidate.observed_time = now;
        save(&tx, &candidate)?;
        put(&tx, RECEIPT, &encode(&receipt)?)?;
        tx.execute("DELETE FROM core_metadata WHERE key=?1", [REQUEST])
            .map_err(storage)?;
        before_commit()?;
        let commit_now = clock();
        validate_clock(VerificationClock {
            now: commit_now,
            floor: now,
        })?;
        if commit_now - receipt.request.issued_at > REQUEST_LIFETIME {
            return Err(PackageError::TrustExpired);
        }
        let commit_clock = VerificationClock {
            now: commit_now,
            floor: now,
        };
        validate_time(
            verified_approval.issued_at,
            verified_approval.expires_at,
            commit_clock,
            APPROVAL_LIFETIME,
        )?;
        validate_time(
            root.payload().issued_at,
            root.payload().expires_at,
            commit_clock,
            MAX_ROOT_LIFETIME,
        )?;
        if commit_now != now {
            receipt.completed_at = commit_now;
            candidate.observed_time = commit_now;
            save(&tx, &candidate)?;
            put(&tx, RECEIPT, &encode(&receipt)?)?;
        }
        tx.commit().map_err(storage)?;
        // The two markers are retained as audit provenance, not erased to bypass.
        Ok(TrustSnapshot {
            root_version: root.payload().version,
            root_digest: root.payload_digest().into(),
            observed_time: candidate.observed_time,
            known_versions: candidate.known_versions,
        })
    }
}
