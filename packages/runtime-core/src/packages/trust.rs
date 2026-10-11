//! T0 durable descriptor trust, borrowing Runtime's existing SQLite writer.
//! No production pins, installer, rollback approval, grants or execution API.
use super::*;
use crate::data::DataStore;
use rusqlite::{params, OptionalExtension, TransactionBehavior};

const KEY: &str = "package-trust-state-v1";
pub(crate) const RECOVERY_KEY: &str = "package-trust-recovery-required";
const MAX_ROOTS: usize = 32;
const MAX_KNOWN: usize = 1024;
const MAX_STATE_BYTES: usize = 16_777_216;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RootRecord {
    envelope: String,
    observed_at: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    format_version: u16,
    pins_digest: String,
    observed_time: u64,
    roots: Vec<RootRecord>,
    known_versions: Vec<KnownVersionIdentity>,
}
/// T0 diagnostics only. Returning metadata cannot authorize an installation.
pub struct TrustSnapshot {
    pub root_version: u64,
    pub root_digest: String,
    pub observed_time: u64,
    pub known_versions: Vec<KnownVersionIdentity>,
}
/// Borrowed verifier input assembled by T0, not a serializable client request.
pub struct DescriptorVerification<'a> {
    pub envelope: &'a [u8],
    pub manifest: &'a [u8],
    pub archive: &'a [u8],
    pub target: PackageTarget,
    pub now: u64,
}
/// Pins are supplied by the T0 release/configuration, never from package/IPC data.
/// No cached mutable floor or second connection is retained by this adapter.
pub struct PackageTrustStore {
    pins: BootstrapPins,
    pins_digest: String,
}
impl PackageTrustStore {
    pub fn new(mut pins: BootstrapPins) -> Result<Self, PackageError> {
        validate_key_set(&pins.keys, pins.threshold, 2)?;
        pins.keys
            .sort_by(|left, right| left.keyid.cmp(&right.keyid));
        let digest = hash(
            &serde_json::to_vec(&(pins.threshold, &pins.keys))
                .map_err(|_| PackageError::TrustInvalid)?,
        );
        Ok(Self {
            pins,
            pins_digest: digest,
        })
    }

    pub fn bootstrap(
        &self,
        store: &mut DataStore,
        envelope: &[u8],
        now: u64,
    ) -> Result<VerifiedRoot, PackageError> {
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        recovery_guard(&tx)?;
        if raw_state(&tx)?.is_some() {
            return Err(PackageError::RootRollback);
        }
        let root = bootstrap_root(envelope, &self.pins, VerificationClock { now, floor: 0 })?;
        let state = State {
            format_version: 1,
            pins_digest: self.pins_digest.clone(),
            observed_time: now,
            roots: vec![RootRecord {
                envelope: general_purpose::STANDARD.encode(envelope),
                observed_at: now,
            }],
            known_versions: vec![],
        };
        save(&tx, &state)?;
        tx.commit().map_err(storage)?;
        Ok(root)
    }

    pub fn snapshot(&self, store: &mut DataStore) -> Result<TrustSnapshot, PackageError> {
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (state, root) = self.load(&tx)?;
        Ok(TrustSnapshot {
            root_version: root.payload().version,
            root_digest: root.payload_digest().into(),
            observed_time: state.observed_time,
            known_versions: state.known_versions,
        })
    }

    /// Observe T0 time even if subsequent root/package verification refuses.
    /// A failed DB write refuses before a verifier can return a new receipt.
    fn observe(&self, store: &mut DataStore, now: u64) -> Result<(), PackageError> {
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (mut state, _) = self.load(&tx)?;
        validate_clock(VerificationClock {
            now,
            floor: state.observed_time,
        })?;
        if now != state.observed_time {
            state.observed_time = now;
            save(&tx, &state)?;
        }
        tx.commit().map_err(storage)
    }

    pub fn rotate(
        &self,
        store: &mut DataStore,
        envelope: &[u8],
        now: u64,
    ) -> Result<VerifiedRoot, PackageError> {
        self.observe(store, now)?;
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (mut state, previous) = self.load(&tx)?;
        if state.roots.len() >= MAX_ROOTS {
            return Err(PackageError::TrustBudgetExceeded);
        }
        let root = rotate_root(
            &previous,
            envelope,
            VerificationClock {
                now,
                floor: state.observed_time,
            },
        )?;
        state.roots.push(RootRecord {
            envelope: general_purpose::STANDARD.encode(envelope),
            observed_at: now,
        });
        save(&tx, &state)?;
        tx.commit().map_err(storage)?;
        Ok(root)
    }

    pub fn verify_descriptor(
        &self,
        store: &mut DataStore,
        input: DescriptorVerification<'_>,
    ) -> Result<VerifiedPackageDescriptor, PackageError> {
        self.verify_with_hook(store, input, || Ok(()))
    }

    pub(super) fn verify_with_hook(
        &self,
        store: &mut DataStore,
        input: DescriptorVerification<'_>,
        before_commit: impl FnOnce() -> Result<(), PackageError>,
    ) -> Result<VerifiedPackageDescriptor, PackageError> {
        self.observe(store, input.now)?;
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (mut state, root) = self.load(&tx)?;
        let policy = AdmissionPolicy {
            target: input.target,
            clock: VerificationClock {
                now: input.now,
                floor: state.observed_time,
            },
            known_versions: state.known_versions.clone(),
            highest_release_sequence: 0,
            approved_rollback_digest: None,
        };
        let receipt = verify_package(
            &root,
            input.envelope,
            input.manifest,
            input.archive,
            &policy,
        )?;
        let payload = receipt.payload();
        let known = KnownVersionIdentity {
            kind: payload.kind.clone(),
            publisher: payload.publisher.clone(),
            id: payload.id.clone(),
            target: payload.target.clone(),
            version: payload.version.clone(),
            package_digest: receipt.package_digest().into(),
            release_sequence: payload.release_sequence,
        };
        if !state
            .known_versions
            .iter()
            .any(|item| version_key(item) == version_key(&known))
        {
            if state.known_versions.len() >= MAX_KNOWN {
                return Err(PackageError::TrustBudgetExceeded);
            }
            state.known_versions.push(known);
        }
        save(&tx, &state)?;
        before_commit()?;
        tx.commit().map_err(storage)?;
        // Only return after the exact identity/floor has committed durably.
        Ok(receipt)
    }

    fn load(
        &self,
        connection: &rusqlite::Connection,
    ) -> Result<(State, VerifiedRoot), PackageError> {
        recovery_guard(connection)?;
        let text = raw_state(connection)?.ok_or(PackageError::TrustStateRequired)?;
        let state: State = serde_json::from_value(
            strict_json(text.as_bytes()).map_err(|_| PackageError::TrustStateInvalid)?,
        )
        .map_err(|_| PackageError::TrustStateInvalid)?;
        if state.format_version != 1
            || state.pins_digest != self.pins_digest
            || state.observed_time > MAX_SAFE_INTEGER
            || state.roots.is_empty()
            || state.roots.len() > MAX_ROOTS
            || state.known_versions.len() > MAX_KNOWN
        {
            return Err(PackageError::TrustStateInvalid);
        }
        let mut root = None;
        let mut floor = 0;
        for record in &state.roots {
            if record.observed_at > state.observed_time
                || record.observed_at < floor
                || record.envelope.len() > MAX_ENVELOPE_BYTES.div_ceil(3) * 4
            {
                return Err(PackageError::TrustStateInvalid);
            }
            let bytes =
                decode_base64(&record.envelope).map_err(|_| PackageError::TrustStateInvalid)?;
            let clock = VerificationClock {
                now: record.observed_at,
                floor,
            };
            let verified = if let Some(previous) = &root {
                rotate_root(previous, &bytes, clock)
            } else {
                bootstrap_root(&bytes, &self.pins, clock)
            }
            .map_err(|_| PackageError::TrustStateInvalid)?;
            floor = record.observed_at;
            root = Some(verified);
        }
        let mut versions = HashSet::new();
        let mut releases = HashSet::new();
        for known in &state.known_versions {
            if !valid_id(&known.publisher)
                || !valid_id(&known.id)
                || !valid_version(&known.version)
                || !valid_digest(&known.package_digest)
                || known.release_sequence == 0
                || known.release_sequence > MAX_SAFE_INTEGER
                || !versions.insert(version_key(known))
                || !releases.insert(release_key(known))
            {
                return Err(PackageError::TrustStateInvalid);
            }
        }
        Ok((state, root.ok_or(PackageError::TrustStateInvalid)?))
    }
}

fn storage(_: rusqlite::Error) -> PackageError {
    PackageError::TrustStorageFailed
}
fn recovery_guard(connection: &rusqlite::Connection) -> Result<(), PackageError> {
    let count: u32 = connection
        .query_row(
            "SELECT count(*) FROM core_metadata WHERE key IN (?1,'recovery-generation')",
            [RECOVERY_KEY],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if count != 0 {
        return Err(PackageError::TrustRecoveryRequired);
    }
    Ok(())
}
fn raw_state(connection: &rusqlite::Connection) -> Result<Option<String>, PackageError> {
    let size: Option<i64> = connection
        .query_row(
            "SELECT length(CAST(value AS BLOB)) FROM core_metadata WHERE key=?1",
            [KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(storage)?;
    match size {
        None => Ok(None),
        Some(size) if size < 0 || size > MAX_STATE_BYTES as i64 => {
            Err(PackageError::TrustStateInvalid)
        }
        Some(_) => connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key=?1",
                [KEY],
                |row| row.get(0),
            )
            .map(Some)
            .map_err(storage),
    }
}
fn save(connection: &rusqlite::Connection, state: &State) -> Result<(), PackageError> {
    let text = serde_json::to_string(state).map_err(|_| PackageError::TrustStateInvalid)?;
    if text.len() > MAX_STATE_BYTES {
        return Err(PackageError::TrustBudgetExceeded);
    }
    connection.execute("INSERT INTO core_metadata(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![KEY,text]).map_err(storage)?;
    Ok(())
}
fn version_key(known: &KnownVersionIdentity) -> String {
    serde_json::to_string(&(
        &known.kind,
        &known.publisher,
        &known.id,
        &known.target,
        &known.version,
    ))
    .expect("bounded identity")
}
fn release_key(known: &KnownVersionIdentity) -> String {
    serde_json::to_string(&(
        &known.kind,
        &known.publisher,
        &known.id,
        &known.target,
        known.release_sequence,
    ))
    .expect("bounded identity")
}
