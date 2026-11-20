# Architecture overview

This project intentionally separates the on-chain Soroban contract from the native host/runtime tooling that loads configs, validates responses, and exercises parser logic during tests. The canonical module map still lives in [README.md](../README.md) and the source tree; this page is a short map for contributors rather than a duplicate of the detailed protocol guides.

## High-level structure

- `src/contract.rs` — the Soroban contract entry points, state handling, and contract-facing logic.
- `src/lib.rs` — the public Rust API surface that re-exports the native and WASM library modules.
- `src/sep6.rs`, `src/sep10_jwt.rs`, `src/sep24.rs`, and `src/sep38.rs` — protocol parsers and normalization layers for Stellar ecosystem formats.
- `src/domain_validator.rs`, `src/response_validator.rs`, and `src/rate_limiter.rs` — validation, policy checks, and runtime guard rails.
- `src/retry.rs`, `src/multi_asset_routing.rs`, and related modules — host-side behavior for retries, routing, and transaction tracking.
- `tests/` and `test_snapshots/` — integration and regression coverage, including parser and snapshot fixtures.

## Native vs. WASM split

The project is built in two modes with intentionally different feature sets:

- Native builds include the CLI, runtime helpers, config loading, and HTTP-facing components needed for local tooling and validation.
- WASM builds compile the `no_std` contract logic for Soroban with the host-only pieces removed.

For the current supported matrix and commands, see [README.md](../README.md) and [docs/build-matrix.md](build-matrix.md). The design rationale is captured in [docs/adr/ADR-0002-dual-build-strategy.md](adr/ADR-0002-dual-build-strategy.md).

### Responsibility matrix

This matrix shows how representative modules fall into three groups. It is a
guide, not a complete module inventory. The authoritative gates are the
`#[cfg(...)]` attributes in [`src/lib.rs`](../src/lib.rs) and the feature
definitions in [`Cargo.toml`](../Cargo.toml).

| Group | Compiled when | Representative modules | Gate in `src/lib.rs` |
|-------|---------------|------------------------|----------------------|
| Native-only (host) | `wasm` feature is off | `http_client`, `webhook`, `sep6`, `sep24`, `stellar_toml`, `metrics` | `#[cfg(not(feature = "wasm"))]` |
| Std-only (host) | `std` feature is on (default) | `config`, `env_fingerprint` | `#[cfg(feature = "std")]` |
| Shared (both builds) | Always | `contract`, `errors`, `deterministic_hash`, `domain_validator`, `rate_limiter`, `migration` | No gate |

How to read the matrix:

- **Native-only** and **std-only** modules are removed from the WASM build.
  They handle network calls, filesystem access, and local configuration. They
  never become part of the Soroban contract binary.
- **Shared** modules compile in both builds. Being shared does not mean every
  function in the module runs on-chain. Check the module's own documentation
  for which entry points the contract calls.
- The contract entry point lives in `src/contract.rs`. It is compiled in both
  modes.

For the rationale behind this split, see
[ADR-0002](adr/ADR-0002-dual-build-strategy.md). For the exact build commands
and feature combinations, see [`docs/build-matrix.md`](build-matrix.md).

## Typical data flow

1. A runtime component reads configuration or a remote response payload.
2. A parser/validator module normalizes the raw data into the library's canonical request/response types.
3. Contract logic enforces schema rules, pending-state checks, and attestation/routing decisions.
4. Integration tests and snapshots verify the parsed values and edge cases without assuming a live deployment topology.

This keeps protocol details in the parser files while the higher-level design remains in the docs and ADRs.

## Related documentation

- [README.md](../README.md) — current project overview and build commands
- [docs/ONBOARDING.md](ONBOARDING.md) — local setup and contributor workflow
- [docs/CONTRIBUTING.md](CONTRIBUTING.md) — PR and validation guidance
- [docs/error-codes.md](error-codes.md) — stable contract error definitions
- [docs/CONTRACT_FUNCTIONS.md](CONTRACT_FUNCTIONS.md) — contract-level function reference

The architecture should stay high-level: it explains the separation of responsibilities and points readers to the protocol-specific docs and code, rather than duplicating every implementation detail.
