# Live Network Smoke Tests

This document describes the live network smoke test workflow, the required
environment setup, the available test cases, and the safety controls that
prevent accidental execution during regular CI.

## Overview

SorobanAnchor ships a dedicated live-test suite (`tests/live_smoke_tests.rs`)
that exercises a deployed contract on the Stellar testnet.  These tests verify
real network reachability and on-chain behavior that the local simulation
environment cannot replicate.

All tests in this suite are **opt-in**.  They will be silently skipped unless
the required environment variables are set.  This design ensures they can never
block or slow standard CI pipelines.

## Required Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `SOROBAN_ANCHOR_INTEGRATION` | **Yes** — must be `testnet` | Master gate; enables the live test suite |
| `ANCHOR_CONTRACT_ID` | **Yes** | Contract ID of the deployed AnchorKit contract on the target network |
| `ANCHOR_ADMIN_SECRET` | **Yes** | Stellar secret key of the contract admin (used for read-only invocations and the smoke attestation) |
| `STELLAR_RPC_URL` | No (default: `https://soroban-testnet.stellar.org`) | Override for the Stellar RPC endpoint |
| `STELLAR_NETWORK` | No (default: `testnet`) | Network name for logging and the Stellar CLI |
| `STELLAR_NETWORK_PASSPHRASE` | No (default: Stellar testnet passphrase) | Override for the network passphrase |

## Running Locally

```bash
# Export required variables
export SOROBAN_ANCHOR_INTEGRATION=testnet
export ANCHOR_CONTRACT_ID=<deployed-contract-id>
export ANCHOR_ADMIN_SECRET=<admin-secret-key>

# Run the dedicated live smoke suite
cargo test --test live_smoke_tests -- --nocapture

# Or use Make
make smoke-test-live

# Run the live step inside the CLI integration harness
make integration-test-live
```

## Test Cases

| Test | What is verified | Expected outcome |
|------|-----------------|-----------------|
| `live_smoke_environment_report` | Prints detected environment config | Always passes (diagnostic only) |
| `live_smoke_get_admin` | Contract is reachable; admin address is a valid Stellar address | Non-empty address starting with `G`, length 56 |
| `live_smoke_is_initialized` | `is_initialized` returns `true` | `"true"` in output |
| `live_smoke_get_schema_version` | Schema version is positive | Integer > 0 |
| `live_smoke_supported_seps` | All five SEPs (6, 10, 24, 31, 38) are declared | Output contains `"6"`, `"10"`, `"24"`, `"31"`, `"38"` |
| `live_smoke_get_attestor_count` | Attestor counter is readable | Any non-negative integer |
| `live_smoke_get_version` | Contract version struct is present | Non-empty output containing version fields |
| `live_smoke_attestation_submission_idempotent` | Smoke attestation succeeds on first run and is idempotent (replay-rejected) on subsequent runs | New ID on first run; `ReplayAttack` on re-runs |

## Safety Controls

- **Master gate**: All tests check `SOROBAN_ANCHOR_INTEGRATION == "testnet"` at the top and skip immediately if not set.  This single guard prevents accidental execution in any environment that has not been explicitly prepared.
- **No-funds guarantee**: No test in this suite transfers funds or modifies financial state.  The only write operation is a single smoke attestation with a deterministic hash.
- **Idempotent writes**: The smoke attestation uses a fixed, well-known payload hash (`SMOKE_TEST_PAYMENT_HASH...`).  Re-running the suite on a contract that has already seen this hash will receive a `ReplayAttack` error, which is treated as a passing outcome.
- **Soft failures**: Unexpected but recoverable conditions (e.g., `AttestorNotRegistered` on a fresh contract) are treated as expected outcomes rather than failures, with a descriptive log message.

## CI Integration

The `live-smoke` job in `.github/workflows/ci.yml`:

- **NEVER** runs automatically on push or pull_request events.
- **ONLY** runs when triggered via `workflow_dispatch` with `run_live_tests` set to `true`.
- Requires the `testnet-smoke` GitHub Actions environment, which can be configured with its own protection rules (e.g., required reviewers).
- Reads `ANCHOR_CONTRACT_ID`, `ANCHOR_ADMIN_SECRET`, and `STELLAR_RPC_URL` from repository secrets.

### How to trigger manually

1. Go to the repository → **Actions** → **CI**.
2. Click **Run workflow**.
3. Set **Run live testnet smoke tests** to `true`.
4. Click **Run workflow**.

### Configuring secrets

In repository **Settings → Secrets and variables → Actions**:

| Secret | Value |
|--------|-------|
| `ANCHOR_CONTRACT_ID` | The deployed contract ID on testnet |
| `ANCHOR_ADMIN_SECRET` | The admin secret key for the testnet contract |
| `STELLAR_RPC_URL` | (optional) Custom RPC endpoint |

> **Security note**: Never commit secret keys to the repository.  Use GitHub
> Actions secrets or a secrets manager.  The `ANCHOR_ADMIN_SECRET` value used
> in CI should be a dedicated testnet key with no mainnet funds.

## Synthetic Probe Limitations

The synthetic probe runner (`synthetic_probe.rs`) is a lightweight reachability
check, **not** a comprehensive service monitor.  A passing probe means a single
narrow path responded within its timeout; it makes **no claim about production
availability**, correctness of business logic, or end-to-end anchor operation.

### What current probes exercise

| Probe | What it checks |
|-------|----------------|
| `Ping` | Basic connectivity to the configured endpoint and round-trip latency |
| `StellarToml` | The `stellar.toml` document can be fetched and parsed |
| `Sep6Info` | The SEP-6 `/info` response has the expected shape |
| `Custom` | An operator-supplied label, used for ad-hoc checks |

Each probe returns a `ProbeOutcome`:

- `Success` — responded within `latency_threshold_ms`.
- `SlowSuccess` — responded, but slower than `latency_threshold_ms`.
- `Failure` — errored or returned an unexpected result.
- `SlowFailure` — errored or returned an unexpected result after exceeding the
  threshold.

Every probe is bounded by `MAX_PROBE_TIMEOUT_MS`; a probe that does not complete
within that bound is treated as a failure rather than hanging the runner.

### Required environment assumptions

- **Network access** to each configured anchor endpoint is required for live
  probes.  Without it, probes report `Failure`/`SlowFailure`.
- **Injected `probe_fn`** — local tests substitute a probe function, so the
  runner's scheduling, thresholding, and outcome classification are testable
  without any live network.
- **Timeout thresholds** — `latency_threshold_ms` and `MAX_PROBE_TIMEOUT_MS`
  shape the outcome classification; results are only meaningful relative to
  these configured values.

### Where local simulation ends

Local tests exercise the runner logic (probe dispatch, outcome classification,
threshold handling) via the injected `probe_fn`.  They do **not** exercise real
network behavior.  Verifying actual reachability requires the opt-in live
environment described above; secrets are supplied only through the safe
environment configuration (`SOROBAN_ANCHOR_INTEGRATION`, `ANCHOR_CONTRACT_ID`,
`ANCHOR_ADMIN_SECRET`, `STELLAR_RPC_URL`) and are never printed.

### Known blind spots

- Probes cover **only the configured endpoints**, not the full set of services
  an anchor depends on.
- A passing probe does **not** verify business logic, transaction correctness,
  or data integrity.
- Probe results do **not** establish overall service availability or uptime.

## Prerequisites

The `stellar` CLI must be available in `PATH` when running locally.  Install it
via Cargo:

```bash
cargo install --locked stellar-cli --features opt
stellar --version
```

The CI job installs the CLI automatically.  If the CLI is unavailable, all live
tests that invoke it will print a skip notice and exit cleanly rather than
failing the run.
