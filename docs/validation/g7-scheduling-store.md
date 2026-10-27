# G7 P3.1b2 durable schedule positions and claims

Date: 2026-10-27. Prototype; continuation of the full G7 implementation.

`runtime-core/src/scheduling/store.rs` borrows the existing DataStore writer.
A bounded formatVersion-1 record in `core_metadata` holds at most 128 finite
schedules and 256 KiB of metadata. It stores immutable command identity, grant
epoch and input digest, never raw input. A cursor position and random claim
token commit together before returning a dispatch attempt. Completion requires
that exact token and is idempotent only for the same result.

Cancellation uses optimistic revision checks and cannot rearm a schedule.
Unknown committed attempts recover as interrupted rather than replaying an
effect. Overlap filtering leaves ineligible positions unconsumed. Clock rollback,
malformed state, write failure and overflow refuse without resetting positions.
Canonical database identity is hashed into the record; copying the database to
another path or changing the supported-restore generation refuses automatic
claiming with `RECOVERY_PENDING`. No recovery approval or grant is manufactured.

Eight real SQLite tests passed, including reopen, copied database/restore scope,
query-only write failure, exact-token completion, cancellation/revisions,
corruption preservation, overlap and clock rollback. Ignored evidence:
`execution-validation/g7-shell/scheduling-store-20261027.log` / `.exit`.

This step persists claims; the following step connects authenticated management,
private inputs, Host dispatch and current policy checks. It is not a new database,
timer, job grant or an exactly-once external-effect guarantee. Existing manual
acceptance waiver applies; full G7 remains under implementation.
