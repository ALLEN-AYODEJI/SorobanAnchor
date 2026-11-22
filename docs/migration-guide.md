# Contract Migration and Upgrade Guide

This document describes the contract upgrade and migration process for SorobanAnchor, including data preservation strategies and testing procedures.

## Overview

Contract upgrades are critical operations that must preserve all stored data while advancing the contract logic. SorobanAnchor implements a two-phase upgrade process:

1. **Upgrade**: `upgrade(new_wasm_hash)` replaces WASM, increments the code patch version, and records upgrade event/audit data. It does not advance the stored schema or rewrite quotes.
2. **Migrate**: `migrate(new_schema_version, batch_size)` rewrites affected stored records and commits schema/history when traversal finishes. It does not replace WASM.

Both require initialization and primary-admin authorization. Follow the
[Upgrade Playbook §5](./upgrade-playbook.md#5-step-by-step-upgrade-procedure):
upload the release WASM, upgrade, then migrate only when a supported schema
advance is required. These are separate transactions, potentially including
several migration batches; there is no atomic upgrade-plus-migration guarantee.
A later failed call does not undo an earlier upgrade or completed batch.
See [ADR-0003](./adr/ADR-0003-contract-lifecycle-and-schema-versioning.md),
[entry points](../src/contract.rs), and
[authorization tests](../tests/contract_init_upgrade_tests.rs).

## Upgrade Process

### Prerequisites

- Admin authorization required
- Valid WASM hash (non-zero)
- Contract must be initialized

### Upgrade Flow

```rust
// 1. Admin calls upgrade with new WASM hash
client.upgrade(&new_wasm_hash);

// 2. Contract validates:
//    - Caller is admin
//    - WASM hash is non-zero
//    - Contract is initialized

// 3. New WASM is deployed
// 4. Verify representative stored data against the pre-upgrade baseline
```

### Upgrade Authorization

Only the admin address set during initialization can authorize upgrades:

```rust
let admin = Address::generate(&env);
client.initialize(&admin);

// Later, only admin can upgrade
client.upgrade(&new_wasm_hash);  // ✓ Succeeds
```

## Migration Process

### Schema Versioning

Each contract version has a schema version number:

- **Version 0**: Uninitialized; `initialize()` writes schema version 1.
- **Version 1+**: Schema versions recorded after initialization and migrations.

### Inspect the schema version (read-only)

Before planning an upgrade, query the deployed contract's
[`get_schema_version`](./CONTRACT_FUNCTIONS.md#get_schema_versionenv-env---u32).
Using the Stellar CLI, supply your contract ID, RPC URL, network passphrase,
and a public source account address in the environment variables below:

```bash
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  --source-account "$ADMIN_PUBLIC_KEY" \
  --send=no \
  -- get_schema_version
# Example output: 1
```

`--send=no` keeps the invocation simulation-only. The query requires no admin
authorization and only reads storage; no signing key is needed. Its result is
a single `u32`, not an object: `0` means uninitialized, `1` is the initial
schema written by `initialize`, and `2` means the V1-to-V2 migration completed.
This is the stored data schema version, distinct from the software version
returned by `get_version`. The snippet performs no upgrade or migration.

### Migration Flow

```rust
// 1. Admin calls migrate with new version
client.migrate(&new_version, &batch_size);

// 2. Contract validates:
//    - Caller is admin
//    - New version > current version
//    - New version != 0

// 3. Schema version is updated
// 4. Data transformations are applied (if any)
```

### Schema-Version Outcomes

`get_schema_version()` only reads the stored version; it does not migrate
contract state. `initialize()` sets the initial version to 1. Migration outcomes
for the currently registered resolver are:

| Current version | Request or operation | Outcome |
|-----------------|----------------------|---------|
| Any | `get_schema_version()` | Returns the stored version without changing state (0 before initialization). |
| V1 | `migrate(2, batch_size)` | Supported V1 → V2 migration; quote records are processed in batches and the version advances when complete. |
| V2 | Migrate to V2 or V1 | Rejected; migrations must advance the version. |
| V0 | Migrate to V2 | Rejected; no V0 → V2 migration step is registered. |
| V1 | Migrate to V3 | Rejected; V3 is newer than the version supported by this code. |

This table describes the resolver implemented today, not a policy guarantee
that future schema versions will support any particular transition.

## Migration Preflight

Keep steps 1–4 read-only using the public-key source and `--send=no` options
in [Upgrade Preflight](#upgrade-preflight-read-only-dry-run), including for
queries that refresh TTL. Do not submit initialization, upload, upgrade, or
migration transactions as preflight checks.

1. Record `is_initialized()`, `get_version()`, and `get_schema_version()`.
   Stop if uninitialized: schema 0 is the default before initialization, not
   a supported migration starting point. Capture `get_migration_count()` and
   `get_migration_record(idx)` for existing indices below that count.
2. Query `get_admin()` and compare it with the intended signing authority's
   public address. Delegated roles do not authorize migration. Reading an
   address does not prove signing authority. See `non_admin_cannot_migrate`
   in [migration tests](../tests/migration_tests.rs) and the uninitialized-call
   cases in [initialization tests](../tests/contract_init_upgrade_tests.rs).
3. Confirm the release target against `LATEST_SCHEMA_VERSION` and `ALL_STEPS`
   in [the resolver](../src/migration.rs). Currently only V1 → V2 is registered.
   Target 0 returns `ValidationError`; same-version requests and downgrades
   return `IllegalTransition`; targets above V2 return
   `UnsupportedCapabilityVersion`; missing steps return `ValidationError`.
   Skips are not generally supported. Resolver tests include
   `test_validate_migration_zero_target_fails`,
   `test_validate_migration_version_too_new_fails`, and
   `test_validate_migration_skipping_step_fails`.
4. Capture representative records using the
   [state-preservation procedure](#state-preservation-procedure). If new code
   is required, perform playbook §5.1–5.2 as explicit state-changing steps
   first, then simulate migration against the installed code with `--send=no`.
   Simulation neither persists progress nor proves later batches will succeed.
5. **Explicit migration step:** submit an admin-authorized
   `migrate(target, batch_size)` following
   [playbook §5.3](./upgrade-playbook.md#53-call-migrate-if-schema-version-advances).
   Repeat while the stored schema is below the target. At completion verify
   target schema, one new V1 → V2 history record labeled `quotes_v1_to_v2`,
   and the comparisons below. A request after reaching the target is rejected.

## Batch-Size Semantics

For V1 → V2, `batch_size` accepts every positive `u32`: 1 through
4,294,967,295 inclusive. Zero returns `ValidationError`, even with no quotes.
Negative and out-of-range values cannot be represented by the API.
Initialization, admin authorization, and target validation run first and may
return their errors before batch validation.

The limit counts **quotes rewritten**, not index entries scanned, storage
operations, or ledger budget. Entries at or below the saved quote-ID cursor
are skipped; missing anchor references, missing records, and already-V2 records
do not consume the rewrite allowance. The implementation reads the quote index
and may scan additional entries. No positive value guarantees a transaction
fits network resource limits; there is no code-defined recommended batch size.

If further index entries remain after reaching the allowance, the call saves a
cursor and returns without advancing schema/history. The next call resumes
after that cursor and can use a different positive size. Finishing traversal
removes the cursor and commits schema/history, even with no quotes to rewrite.
The call returns unit, not a progress count: query `get_schema_version()` for
completion. Completed batches persist across calls; batches that rewrite quotes
also append admin audit entries.

See `migrate_quotes_to_v2` in [contract.rs](../src/contract.rs) and these
[migration tests](../tests/migration_tests.rs):
`quote_migration_advances_cursor_in_batches` (size 1 and resumption),
`quote_migration_zero_batch_rejected_without_progress` (zero),
`quote_migration_max_batch_completes_with_or_without_quotes` (upper boundary
and empty data), and `quote_migration_logs_admin_audit_per_batch` (audit data).
These fixtures are not resource-budget benchmarks.

## Upgrade Preflight (Read-Only Dry Run)

There is no dedicated upgrade dry-run mode: `anchorkit deploy --dry-run` only
skips a fresh deployment and has no upgrade or migrate path. The read-only
preflight below uses the Stellar CLI (flags checked against `stellar` 26.0.0
`--help`) to report the current and target versions without changing state.

Always pass `--send=no`. With the default (`--send=default`), the CLI submits
whenever simulation reports ledger writes, and `get_migration_count` /
`get_migration_record` refresh their storage TTL, which counts as a write.
Use the admin's **public key** as the source so nothing can be signed.

```bash
NET="--rpc-url $RPC_URL --network-passphrase $PASSPHRASE"
READ="--id $CONTRACT_ID --source-account $ADMIN_PUBLIC_KEY --send=no $NET"
WASM_FILE="target/wasm32-unknown-unknown/release/anchorkit.wasm"

# ── Read-only: current state ──────────────────────────────────────────────
stellar contract invoke $READ -- get_version          # {major, minor, patch, upgraded_at}
stellar contract invoke $READ -- get_schema_version   # stored schema version
stellar contract invoke $READ -- get_migration_count  # history length (N)
stellar contract invoke $READ -- get_migration_record --idx 0   # 0..N-1; errors if N == 0

# ── Read-only: installed vs. target code ──────────────────────────────────
stellar contract fetch --id "$CONTRACT_ID" $NET --out-file current.wasm
sha256sum current.wasm "$WASM_FILE"                    # identical hash → nothing to upgrade
stellar contract info interface --wasm "$WASM_FILE"    # target exports migrate / get_schema_version
```

Neither the contract nor the WASM exposes the target schema version. Take it
from the release notes (`LATEST_SCHEMA_VERSION` in `src/migration.rs`). After
a successful `upgrade`, `get_version` reports the current patch + 1.

`migrate` can only be simulated against the code that is **currently
installed**, so run this check after `upgrade` and before the real `migrate`
call:

```bash
# ── Read-only: simulate migrate on the upgraded code ──────────────────────
stellar contract invoke $READ -- migrate --new_schema_version 2 --batch_size "$BATCH_SIZE"
```

A simulation error shows the call would be rejected: `#24` (`IllegalTransition`)
means the contract is already at or past that version, and `#27`
(`UnsupportedCapabilityVersion`) means the target is newer than the code
supports. A successful simulation covers only the first batch. It does not
show whether further `migrate` calls will be needed.

**State-changing steps.** These sign and submit transactions, and require
`--source-account` to be the admin's secret key or identity:
`stellar contract upload`, `-- upgrade`, and `-- migrate` without `--send=no`.
See the [Upgrade Playbook](./upgrade-playbook.md) §5 for that procedure.

## State-Preservation Procedure

Use the same known IDs and read-only options before upgrading and after
migration completes. These representative checks do not guarantee untested
state. The upgrade fixtures use minimal WASM and describe replacement as a
no-op in the test environment; they do not prove compatibility of arbitrary
release WASM.

1. Read known attestations with `get_attestation(id)` and compare issuer,
   subject, and timestamp. Evidence: `attestations_preserved_after_upgrade`
   in [migration_tests.rs](../tests/migration_tests.rs).
2. Read quotes with `get_quote(anchor, quote_id)` and compare anchor, rate,
   and fee percentage. For migrated V1 quotes also verify schema V2 and
   `routing_reason = None`. Evidence: `quotes_preserved_after_upgrade` and
   `v1_quote_readable_after_migration` in the same test file.
3. Read sessions with `get_session(session_id)` and compare initiator and
   closed status, as asserted by `sessions_preserved_after_upgrade`.
   `multiple_data_types_preserved_after_upgrade` checks attestation, quote,
   and session accessibility together.
4. Compare `get_schema_version()`, `get_migration_count()`, and
   `get_migration_record(idx)` with the baseline and intended transition.
   Partial batches leave the stored schema unchanged. Evidence:
   `quote_migration_advances_cursor_in_batches` and the resolver test
   `test_get_migration_record_returns_correct_data` in
   [migration.rs](../src/migration.rs). New per-batch admin audit entries are
   expected (`quote_migration_logs_admin_audit_per_batch`); that test does
   not establish preservation of all historical audit logs.

## Schema Changes

### Handling Breaking Changes

When a schema change is required:

1. **Plan the migration**
   - Identify affected data types
   - Design transformation logic
   - Plan rollback strategy

2. **Implement migration logic**
   - Add migration code in `migrate()` function
   - Transform existing data
   - Validate data integrity

3. **Test thoroughly**
   - Test data preservation
   - Test transformation logic
   - Test rollback scenarios

### Example: Adding a Field

If adding a new field to `Attestation`:

```rust
// Old schema (v1)
pub struct Attestation {
    pub id: u64,
    pub issuer: Address,
    pub subject: Address,
    pub timestamp: u64,
    pub payload_hash: Bytes,
    pub signature: Bytes,
    pub schema_version: u32,
}

// New schema (v2)
pub struct Attestation {
    pub id: u64,
    pub issuer: Address,
    pub subject: Address,
    pub timestamp: u64,
    pub payload_hash: Bytes,
    pub signature: Bytes,
    pub schema_version: u32,
    pub verified_at: u64,  // New field
}

// Migration logic
fn migrate_attestations_v1_to_v2(env: &Env) {
    // For each existing attestation:
    // - Load old data
    // - Set verified_at to current timestamp
    // - Save updated record
}
```

## Testing Upgrades and Migrations

### Test Coverage

The test suite includes the representative cases below. Some older integration
fixtures (`migration_can_skip_versions`, `migration_to_higher_version_succeeds`,
and `quote_migration_is_idempotent`) still request unsupported V3+ targets.
Those expectations conflict with the current resolver and are not evidence of
supported transitions; use the resolver and its boundary tests for that policy.

1. **Data Preservation Tests**
   - Attestations preserved after upgrade
   - Quotes preserved after upgrade
   - Sessions preserved after upgrade
   - Multiple data types preserved together

2. **Migration Path Tests**
   - Migration to higher version succeeds
   - Unregistered skipped-version paths are rejected by the resolver
   - Migration to same version fails
   - Migration to lower version fails
   - Migration to zero version fails

3. **Data Compatibility Tests**
   - Data remains consistent across multiple upgrades
   - Schema version tracked correctly
   - Records include schema version

4. **Authorization Tests**
   - Only admin can upgrade
   - Only admin can migrate

### Running Migration Tests

```bash
# Run all migration tests
cargo test migration_tests

# Run specific test
cargo test migration_tests::attestations_preserved_after_upgrade

# Run with output
cargo test migration_tests -- --nocapture
```

## Rollback Strategy

Migration cannot downgrade the schema. Replacing WASM does not reverse stored
transformations or reset the code patch counter. Verify that a previous artifact
can read the current layout before restoring it. Recovery may require a reviewed
forward repair and the pre-upgrade export; there is no automatic schema rollback.
See [Upgrade Playbook §7](./upgrade-playbook.md#7-rollback-procedure).

## Best Practices

### Before Upgrading

1. **Backup data**
   - Export all attestations, quotes, sessions
   - Store backup in secure location

2. **Test thoroughly**
   - Run full test suite
   - Test on testnet first
   - Verify data preservation

3. **Plan rollback**
   - Document previous WASM hash
   - Document previous schema version
   - Have rollback procedure ready

### During Upgrade

1. **Coordinate with users**
   - Announce maintenance window
   - Provide status updates
   - Minimize downtime

2. **Monitor closely**
   - Watch for errors
   - Verify data accessibility
   - Check audit logs

### After Upgrade

1. **Verify integrity**
   - Spot-check data samples
   - Run health checks
   - Monitor for anomalies

2. **Document changes**
   - Record upgrade timestamp
   - Document schema changes
   - Update runbooks

## Troubleshooting

### Common Issues

**Issue**: Upgrade fails with "InvalidPayload"
- **Cause**: WASM hash is all zeros
- **Solution**: Provide valid non-zero WASM hash

**Issue**: Migration fails with "InvalidPayload"
- **Cause**: Version is zero or not advancing
- **Solution**: Use version > current version

**Issue**: Data not accessible after upgrade
- **Cause**: Data not properly preserved
- **Solution**: Check migration logic, restore from backup

**Issue**: Unauthorized upgrade attempt
- **Cause**: Caller is not admin
- **Solution**: Use admin address for upgrade

## References

- [Soroban Contract Upgrades](https://developers.stellar.org/docs/learn/storing-data)
- [Admin Audit Log](./admin-audit-log.md)
- [Governance and Security](./governance-and-security.md)
- [Upgrade Playbook](./upgrade-playbook.md) — step-by-step production upgrade and rollback procedure

## Configuration File Compatibility

This section covers **host configuration files** (`configs/*.json`, `configs/*.toml`,
validated against `config_schema.json`). It is separate from **contract schema
versioning** described above. Contract data migrations (`migrate`) change
on-chain stored data. Config schema changes affect only the files that the
host reads. The two do not share a version number, and a config change does
not require a contract migration, or the reverse.

What the current code guarantees:

- **No config schema version field.** Config files do not declare a schema
  version. `contract.version` is the version of the anchor deployment, not of
  the config schema.
- **Unknown keys are rejected.** Every object in `config_schema.json` sets
  `additionalProperties: false`, and the typed structs in `src/config.rs` use
  `deny_unknown_fields`. A config that uses a removed or renamed key fails
  validation. It is not silently ignored. See the "Strict unknown-field
  rejection" section of the [README](../README.md#configuration).
- **Schema reference.** `config_schema.json` at the repository root is the
  authoritative description of accepted fields and values.

How schema changes are announced:

- Changes to config fields are recorded in the changelog generated from commit
  history (see [changelog-generation.md](changelog-generation.md)). A change that
  breaks existing configs must include a `BREAKING CHANGE:` footer in the commit
  message.

There is no dedicated host-config migration guide yet. Until one exists, check
the changelog and `config_schema.json` diff for the release you are upgrading to.
This section does not promise any backward compatibility beyond what is listed
above.
