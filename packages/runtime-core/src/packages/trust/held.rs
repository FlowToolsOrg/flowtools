//! Ephemeral exact-descriptor continuity. Not a lease, grant or run selector.
//! Only current ordinary admission may mint this proof. Restore/reopen cannot.
use super::*;

pub(in crate::packages) struct HeldDescriptor {
    original: VerifiedPackageDescriptor,
    writer: uuid::Uuid,
    database: String,
    generation: Option<String>,
    pins: String,
}
fn generation(store: &DataStore) -> Result<Option<String>, PackageError> {
    let row: Option<Option<String>> = store.connection.query_row(
        "SELECT CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB)) BETWEEN 1 AND 128 THEN value ELSE NULL END FROM core_metadata WHERE key='recovery-generation'",
        [], |r| r.get(0),
    ).optional().map_err(storage)?;
    match row {
        None => Ok(None),
        Some(Some(value)) => Ok(Some(value)),
        Some(None) => Err(PackageError::TrustInvalid),
    }
}
fn database(store: &DataStore) -> Result<String, PackageError> {
    let path = store.database_path().ok_or(PackageError::TrustInvalid)?;
    let path = std::fs::canonicalize(path).map_err(|_| PackageError::TrustInvalid)?;
    let path = path.to_str().ok_or(PackageError::TrustInvalid)?;
    Ok(hash(path.as_bytes()))
}
impl HeldDescriptor {
    fn check_scope(
        &self,
        trust: &PackageTrustStore,
        store: &DataStore,
    ) -> Result<(), PackageError> {
        if self.writer != store.writer_instance()
            || self.database != database(store)?
            || self.generation != generation(store)?
            || self.pins != trust.pins_digest
        {
            return Err(PackageError::TrustInvalid);
        }
        Ok(())
    }
    pub(in crate::packages) fn original(&self) -> &VerifiedPackageDescriptor {
        &self.original
    }
}
impl PackageTrustStore {
    pub(in crate::packages) fn hold_current_descriptor(
        &self,
        store: &mut DataStore,
        original: &VerifiedPackageDescriptor,
        now: u64,
    ) -> Result<HeldDescriptor, PackageError> {
        let database = database(store)?;
        let generation = generation(store)?;
        self.check_receipts(store, &[original], &original.payload().target, now)?;
        let held = HeldDescriptor {
            original: original.clone(),
            writer: store.writer_instance(),
            database,
            generation,
            pins: self.pins_digest.clone(),
        };
        held.check_scope(self, store)?;
        Ok(held)
    }
    pub(in crate::packages) fn check_held_descriptor(
        &self,
        store: &mut DataStore,
        held: &HeldDescriptor,
        now: u64,
    ) -> Result<(), PackageError> {
        held.check_scope(self, store)?;
        self.check_receipts_with(
            store,
            &[&held.original],
            &held.original.payload().target,
            now,
            |receipt, policy| {
                validate_held_floors(
                    receipt.payload(),
                    receipt.package_digest(),
                    policy,
                    &held.original,
                )
            },
        )?;
        held.check_scope(self, store)
    }
    pub(in crate::packages) fn verify_held_descriptor(
        &self,
        store: &mut DataStore,
        held: &HeldDescriptor,
        input: DescriptorVerification<'_>,
    ) -> Result<VerifiedPackageDescriptor, PackageError> {
        held.check_scope(self, store)?;
        self.observe(store, input.now)?;
        let tx = store
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (state, root) = self.load(&tx)?;
        let policy = AdmissionPolicy {
            target: input.target,
            clock: VerificationClock {
                now: input.now,
                floor: state.observed_time,
            },
            known_versions: state.known_versions,
            highest_release_sequence: 0,
            approved_rollback_digest: None,
        };
        let result = verify_package_inner(
            &root,
            input.envelope,
            input.manifest,
            input.archive,
            &policy,
            Some(&held.original),
        )?;
        // Existing identity only: never append/replace known versions or change
        // release floors through continuation. observe() already committed time.
        drop(tx);
        held.check_scope(self, store)?;
        Ok(result)
    }
}
