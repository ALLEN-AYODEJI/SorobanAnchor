# Config Change Impact Guide

Changing a config field touches more than one file. This guide lists what to revisit when you add, rename, or tighten a field in an anchor config, and which validation commands confirm the change.

A change to `config_schema.json` alone does **not** update the rest of the repository. The schema is hand-written (no generator reads the Rust types), and several consumers keep their own copies or rules. Work through the table, not just the first row.

## Where the config contract lives

| Layer | File | Role |
|---|---|---|
| Schema | `config_schema.json` | Hand-written JSON Schema. Objects with `properties` set `additionalProperties: false`, so unknown fields are rejected |
| Rust types | `src/config.rs` | `RuntimeConfig` and nested structs, all with `#[serde(deny_unknown_fields)]` |
| Runtime validator | `src/config.rs` (`validate_against_schema`, `validate_runtime_config`) | Checks the embedded schema, then deserialises, then cross-field checks |
| Build-time check | `build.rs` | Validates every file in `configs/` against the schema, and runs a substring check on a few schema limits |
| Samples | `configs/*.json` and `configs/*.toml` | Three anchors, each in both formats, that must describe the same contract |
| Bundled copy | `scripts/package_release.sh` | Copies `config_schema.json` into `schemas/` in release bundles |

## Decision table

**Required** rows must change in the same pull request. **Conditional** rows apply only when the situation in the first column is true.

| Change | Update | Required or conditional | Why |
|---|---|---|---|
| Add a new field | `config_schema.json` (add the property) | Required | The schema rejects unknown fields, so the sample configs fail until it is added |
| Add a new field | `src/config.rs` (add it to the matching struct) | Required | `deny_unknown_fields` rejects the field at runtime if the struct does not know it |
| Add a new field | `configs/*.json` and `configs/*.toml` | Conditional: only if the sample should show the field, or it is required | Both formats must stay in step (see the pair test below) |
| Add a required field | `config_schema.json` `required` list, plus every sample in both formats | Required | Missing fields fail `build.rs` and the validators |
| Rename a field | `config_schema.json`, `src/config.rs`, and every sample in both formats | Required | An old name is rejected as unknown |
| Rename a field | `README.md`, `docs/RUNBOOK.md`, `docs/attestor-roles.md`, and any `examples/*.sh` that show the old name | Conditional: only if the docs or examples mention it | Documentation is not checked by the validators |
| Tighten a limit (`maxLength`, `maxItems`, `maximum`, `minLength`) | `config_schema.json` | Required | The schema is the only enforcement of this limit for configs |
| Tighten a limit | Existing samples | Required if any sample exceeds the new limit | Otherwise `build.rs` fails the build |
| Tighten a limit | `build.rs` substring check | Conditional: only if the limit is one of the six values `build.rs` checks | See the note on the consistency check below |
| Change a cross-field rule (for example, references between attestors and services) | `validate_runtime_config` in `src/config.rs` | Conditional | The schema cannot express cross-field rules |
| Any change to a release | `scripts/package_release.sh` | Not needed | Copies the schema into the bundle automatically |
| Any change to a release | `scripts/validate_bundle.sh` | Conditional: only if you add a new file that must be in the bundle | The script checks for named files |
| Any change | Generated API docs and `api_snapshots/` | Not affected by config schema changes | The API snapshots cover Rust public items, not config files |

### Note on the build-time consistency check

`build.rs` runs `validate_schema_consistency`. It checks that `config_schema.json` contains six exact strings:

- `"maxLength": 64`, `"maxLength": 16`, `"maxLength": 256`
- `"maxItems": 100`
- `"maximum": 86400`, `"maximum": 10000`

This is a substring match against the file text. It does not read any Rust constant. The messages name `MAX_NAME_LEN`, `MAX_VERSION_LEN`, and similar constants, but those are not defined in `src/`. Because the check only looks for text, changing one field's limit can leave the check passing while another field still uses the old value. Review the schema diff directly.

## Commands to run after a config change

Run these from the repository root. Each command is taken from the repository's scripts or tests.

```bash
# Validate one config against the schema (JSON or TOML)
python3 scripts/validate_config_strict.py configs/remittance-anchor.toml config_schema.json

# Validate every config in configs/ (uses ajv-cli if installed, otherwise the Python validator)
bash scripts/validate_all.sh

# Runtime and schema tests for the example configs
cargo test --test config_validation_tests

# Build-script failure messages for invalid configs
cargo test --test build_schema_failure_tests

# Offline check through the CLI (no network required)
cargo run --bin anchorkit -- offline validate
```

`cargo build` also runs `build.rs`, which validates `configs/` and fails the build on an invalid file. Use `cargo build` to confirm that a rebuild picks up the change.

`config_validation_tests` includes `json_toml_config_pairs_agree_on_required_fields`, which fails when the JSON and TOML versions of a sample disagree on `contract.name`, `contract.version`, or `contract.network`. Run it after editing either format.

## Checklist

- [ ] `config_schema.json` updated, and `additionalProperties` is still `false` on the object you changed
- [ ] Matching struct in `src/config.rs` updated, with `deny_unknown_fields` still present
- [ ] Both `configs/*.json` and `configs/*.toml` updated, or the field left optional
- [ ] `scripts/validate_all.sh` passes
- [ ] `cargo test --test config_validation_tests` and `cargo test --test build_schema_failure_tests` pass
- [ ] Docs and examples that show the old field name are updated, if any

## Related

- [CLI exit-status reference](cli-exit-status.md) — exit codes for `offline validate`
- `docs/RUNBOOK.md` — pre-deployment config checks
