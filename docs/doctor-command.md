# `anchorkit doctor` — Command Scope

`anchorkit doctor` prints a checklist of the environment the SorobanAnchor tooling runs in. This page lists each diagnostic it performs, what it needs to run, and what a successful run does **not** prove.

Source of truth: `doctor()` and the `check_*` functions in `src/main.rs`.

> **A successful `doctor` run is not a deployment verification.** It does not show that a contract is deployed at the expected address, that its code matches a release, that an anchor is serving requests, or that a transaction would succeed. Use it as a local readiness check only.

## Usage

```bash
anchorkit doctor                 # uses the default network
anchorkit doctor --network testnet
anchorkit doctor --fix           # may install the WASM target (see below)
```

Exit status: `0` when every required check passes, `1` when any check fails. Warnings do not change the exit status. See [CLI exit-status reference](cli-exit-status.md).

## Diagnostics

Each row is one line of output. "Local" means the check reads only the machine or repository. "Remote" means the check sends a request to a network service.

| Diagnostic | What it observes | Scope | Effect on exit status | Evidence |
|---|---|---|---|---|
| Stellar CLI | Runs `stellar --version`; passes if it starts and the version can be parsed; warns if the version cannot be parsed; fails if it is not on `PATH` | Local | Fail | `check_stellar_cli` |
| Required Dependencies | Checks that `cargo` and `rustc` run | Local | Fail | `check_required_dependencies`; unit test `test_check_required_dependencies_passes_in_dev_environment` |
| WASM Target | Runs `rustup target list --installed` and looks for `wasm32-unknown-unknown` | Local | Fail | `check_wasm_target` |
| Build Artifacts | Looks for `target/wasm32-unknown-unknown/` and any build output under `release` or `debug` | Local | Warning only | `check_build_artifacts_at`; unit tests `test_check_build_artifacts_*` |
| Contract ID | Checks that `ANCHOR_CONTRACT_ID` is set and non-empty; prints only the first 16 characters | Local | Warning only | `check_contract_id_env` |
| Admin Secret | Checks that `ANCHOR_ADMIN_SECRET` is set and starts with `S`; the value is never printed | Local | Warning if unset or empty; fail if set but not `S...`-shaped | `check_admin_secret_env` |
| Network | Sends an HTTP GET to the RPC URL for the selected network (5-second timeout). HTTP 2xx or 404 passes; other HTTP status warns; connection errors fail | **Remote** | Fail on connection error | `check_network_connectivity`, `check_network_connectivity_url` |
| Contract Deployment | Only when `ANCHOR_CONTRACT_ID` is set. Runs `stellar contract invoke ... -- is_initialized` against the RPC URL | **Remote** | Fail | `check_contract_deployment` |
| Endpoint Proof of Possession (PoP) | Only when a contract ID is set. Reads the attestor count and checks each attestor's proof. Printed for information | **Remote** | Never changes exit status | `check_endpoint_proofs` |
| Config Files | Validates each `.json` and `.toml` file in `configs/` against the full runtime config schema | Local | Fail on invalid file; warning if `configs/` is missing | `check_config_files_in`; unit tests `test_check_config_files_*` |
| wasm-opt (optional) | Runs `wasm-opt --version` | Local | Warning only; never fails the run | `check_optional_tool`; unit tests `test_check_optional_tool_*` |
| Environment Fingerprint | Prints a summary of the local toolchain and platform | Local | None (informational) | `anchorkit::EnvironmentFingerprint::collect` |

### Network and contract calls

`doctor` is not purely offline. Two diagnostics contact a network service:

- The **Network** check makes one HTTP GET to the RPC URL for the chosen network. The URL comes from a saved network profile if one exists, otherwise from the built-in endpoint for that network.
- If `ANCHOR_CONTRACT_ID` is set, the **Contract Deployment** and **Endpoint PoP** checks run `stellar contract invoke` against that contract.

Do not read a passing Network check as proof that the anchor service or its contracts are healthy. Only the RPC endpoint was reached.

### What `--fix` does

`--fix` affects one check: when the `wasm32-unknown-unknown` target is missing, it runs `rustup target add` for that target. No other check changes the environment.

## Prerequisites

- `cargo`, `rustc`, and `rustup` on `PATH`, for the dependency and WASM target checks.
- The Stellar CLI on `PATH`, for the Stellar CLI check and for the contract checks.
- Network access to the RPC endpoint, for the Network check. Without it the check fails, but the rest of the report still runs.
- `ANCHOR_CONTRACT_ID` and `ANCHOR_ADMIN_SECRET` are optional. Without them, the contract checks are skipped or warn. The Stellar CLI falls back to a key named `default` when `ANCHOR_ADMIN_SECRET` is unset.
- Run from the repository root. Build Artifacts and Config Files resolve `target/` and `configs/` relative to the current directory.

## Limitations

- **Remote health is not checked.** A passing Network check shows only that the RPC URL responded to a GET request. It does not check the anchor's own services.
- **Contract checks are shallow.** Contract Deployment confirms that `is_initialized` can be invoked. It does not check the contract's code hash, version, or configuration.
- **Results depend on the local environment.** A machine without optional tools still passes. Build Artifacts checks only that a build output directory exists, not that it matches the current source.
- **Secrets are not printed.** Only presence and format are checked. Do not treat the secret check as proof that the key is authorised on the contract.
- **Warnings do not fail the run.** Missing config directories, missing build artifacts, unset contract ID, and missing optional tools all exit `0`.

## Related

- [CLI exit-status reference](cli-exit-status.md)
- `scripts/test_doctor.sh` runs `doctor` under several environment states.
