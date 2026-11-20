# CLI Exit-Status Reference

This page documents the process exit status of selected `anchorkit` commands, so scripts can tell a validation failure from a successful run without parsing the human-readable output.

Source of truth: `src/main.rs`. Exit codes are set by `std::process::exit(...)` calls and by normal return from `main`.

## Stability

- Only the statuses listed below are documented. Commands not listed here have **no guaranteed exit-status contract**; do not rely on their codes.
- Exit status is the only machine-facing signal described here. The text printed to stdout and stderr (`✅`, `❌`, `[offline]` lines, and similar) is for humans and may change between releases. It is **not** an API contract.
- The codes below use `0` for success and `1` for failure. A non-zero code other than `1` is not documented and should be treated as an unexpected error.

## Status summary

| Command | `0` | `1` | Notes |
|---|---|---|---|
| `anchorkit doctor` (and `--fix`) | All checks passed | One or more checks failed | Local environment checks only; see [doctor command scope](doctor-command.md) |
| `anchorkit offline validate` | All config files parsed and validated | One or more files could not be read, or failed validation | No network access |
| `anchorkit offline simulate` | Simulation ran to completion | Config validation failed before the simulation started | Simulation is local only |
| `anchorkit deploy --dry-run` | Pre-deployment validation passed; deployment was skipped | Pre-deployment validation failed | Nothing is deployed |
| Usage errors (unknown subcommand, missing required argument) | — | Non-zero (the argument parser rejects the invocation) | Tested only as "non-zero"; the specific code is not documented |

## Per-command details

### `doctor`

- Exit `0` when every check passes and the report ends with "Your environment is ready."
- Exit `1` when any check fails. The report ends with "Some checks failed" and, unless `--fix` was given, suggests re-running with `--fix`.
- Optional tools that are missing do not turn the run red, so a clean contributor machine can still exit `0`.

```bash
anchorkit doctor
echo "exit: $?"
```

A `0` from `doctor` means the local environment is ready. It does **not** prove that a deployed contract or a remote anchor is healthy. See [the doctor command scope reference](doctor-command.md).

### `offline validate`

- Exit `0` when every config file in the target set parses and validates.
- Exit `1` when any file cannot be read or fails validation. Each failing path is printed to stderr.
- With `--config <path>`, only that file is checked. Without it, every `.json` and `.toml` file in `configs/` is checked.
- If `configs/` does not exist, the command prints a warning and exits `0`. If `configs/` contains no config files, it prints `(no config files found)` and also exits `0`. Scripts that need a hard failure in these cases should pass `--config` explicitly.

```bash
anchorkit offline validate --config configs/remittance-anchor.json
echo "exit: $?"
```

### `offline simulate`

- Runs config validation first. If validation fails, the command prints an abort message and exits `1` before any simulated step runs.
- Otherwise the simulated steps run and the command exits `0`. The steps are local; a `0` here does **not** show that a live anchor, network, or contract would accept the same operations.

### `deploy --dry-run`

- Runs pre-deployment validation for the selected network. If it fails, the command exits `1` and prints "Aborting."
- If validation passes, the command prints that deployment is skipped and exits `0`.
- `--dry-run` does not build WASM or contact the network to deploy.

```bash
anchorkit deploy --network testnet --dry-run
echo "exit: $?"
```

### Other failures

Some commands that call the Stellar CLI exit `1` with an error message when that tool cannot be started (for example, "is the Stellar CLI installed?"). These are reported as `1`, but they are not part of the documented contract above, so do not use them as a stable signal.

## Related

- [Doctor command scope](doctor-command.md) — what `doctor` checks and what it cannot prove.
- `scripts/test_doctor.sh` exercises `doctor` in several environment states.
