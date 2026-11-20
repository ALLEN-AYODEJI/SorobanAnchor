# Invalid config sample (`invalid-config-sample.toml`)

`invalid-config-sample.toml` is **intentionally invalid**. It exists to show
how the strict config validator reports a schema error. **Do not use it as a
deployment config.**

## Intentionally invalid field

| Field | Value in sample | Why it fails |
|---|---|---|
| `attestors.registry[0].address` | `XXXX…` (54 characters) | Stellar account addresses must start with `G`, followed by uppercase letters or digits. The schema's `pattern` rule (`^G[A-Z0-9]{53,55}$`) rejects it. |

The length is within the schema's 54–56 range, so only the `pattern` rule
fails. The value is a placeholder, not a real address or endpoint.

## Expected diagnostic

The validator reports one error, in this form:

```
schema validation failed:
attestors.registry[0].address: [pattern] ...
```

The `[pattern]` label is the schema keyword that was violated. The validator
does not echo the value back.

## Running it

From the repository root:

```bash
cargo run -- offline validate --config examples/invalid-config-sample.toml
```

A failure is the expected result. The command reads local files only and makes
no network calls. See [`docs/ONBOARDING.md`](../docs/ONBOARDING.md) for context.
