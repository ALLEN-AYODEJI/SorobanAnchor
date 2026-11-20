# Configuration Reference

This page helps you find the right section of an AnchorKit configuration, the
example that shows it in use, and the steps to take when validation fails.

**`config_schema.json` is the source of truth.** It defines the valid shape of
every configuration, and it rejects unknown keys. This page does not repeat
field-level validation rules. For the exact constraints on a field, read the
schema.

- Schema: [`config_schema.json`](../config_schema.json)
- Validation overview and strict unknown-key behavior: [README, Configuration](../README.md#configuration)

---

## Section index

Each top-level section below exists in `config_schema.json`. The example
column links to a tracked file in `configs/`. Use the `.toml` or `.json`
version, whichever format you prefer.

| Section | What it configures | Example |
|---------|--------------------|---------|
| `contract` | Anchor identity: name, version, network, and optional admin address | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `attestors` | The attestor registry (`registry`), listing each attestor's address, role, and enabled state | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `sessions` | Session tracking, timeouts, and audit log retention | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `operations` | Operation templates (`templates`), each tied to an attestor and an operation type | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `remittance` | Remittance corridors, exchange-rate handling, and fee structure | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `stablecoin` | Stablecoin token details, reserve composition, supply caps, and collateral types | [`stablecoin-issuer.toml`](../configs/stablecoin-issuer.toml) |
| `compliance` | Compliance thresholds and limits | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `storage` | Storage TTLs, session caching, and audit log settings | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `security` | Signature and nonce requirements, endpoint pins, and rate limits | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `monitoring` | Metrics, operation logging, and alert definitions | [`remittance-anchor.toml`](../configs/remittance-anchor.toml) |
| `proxy` | Outbound proxy settings | No tracked example yet. Use the schema directly. |

Not every example uses every section. `remittance` appears only in the
remittance sample, and `stablecoin` appears only in the stablecoin sample.
Every tracked sample includes `contract`, `attestors`, `sessions`,
`operations`, `compliance`, `storage`, `security`, and `monitoring`. No tracked
sample includes `proxy`.

If a section you need is not listed here, check `config_schema.json` first.
The schema is authoritative, and this index does not list every field.

---

## Troubleshooting validation failures

Validate one file with the validator, from the repository root:

```bash
python3 scripts/validate_config_strict.py configs/remittance-anchor.toml config_schema.json
```

To validate every tracked config, run `./scripts/validate_all.sh`.

Each failure prints one line per problem, in the form `ERROR: <path>: <message>`.
The validator exits with one of these codes:

| Exit code | Meaning |
|-----------|---------|
| `0` | The config is valid. |
| `1` | The config failed validation, a file was missing, or the file could not be parsed. |
| `2` | A required Python package is missing, or the command was used incorrectly. |

Use the error class below to decide what to change. Change the config or the
environment. Do not change the schema to make a failure go away, unless the
schema itself is wrong and that is reviewed separately.

### Unknown keys

Example message:

```
ERROR: root: Additional properties are not allowed ('monitorring' was unexpected)
```

Unknown keys are rejected. They are never ignored. Compare the key with the
section names in the [section index](#section-index). Fix the spelling, or move
the key to the right section. If the key is not in the schema at all, remove it
from the config. Do not add it to the schema unless the code reads it.

### Malformed values

Example message, from [`examples/invalid-config-sample.toml`](../examples/invalid-config-sample.toml):

```
ERROR: attestors -> registry -> 0 -> address: 'XXXXXXXX...' does not match '^G[A-Z0-9]{53,55}$'
```

The path tells you which value to fix. Look up that path in
`config_schema.json` and correct the value to match its type, enum, length,
or pattern. A value of `XXXX...` is a placeholder. Replace it with a real
value from your own records, not a made-up one.

A TOML syntax problem appears as `ERROR: Failed to parse config: ...`. Fix the
syntax at the line the parser names. The config is not partly loaded.

### Missing files

Example messages:

```
ERROR: Config file not found: <path>
ERROR: Schema file not found: <path>
```

Check the path you passed. Run the command from the repository root, or pass
the full path. The schema argument must point to `config_schema.json`.

### Missing validator dependency

Example message:

```
ERROR: jsonschema not installed. Run: pip install jsonschema
```

Install the package with `pip install jsonschema`, then run the command again.
This is exit code `2`, not a problem with your config.

### Environment-specific secrets

Keys, passwords, and tokens do not belong in a config file. The strict schema
exists to catch mistakes, not to hold secrets. If a validation error points at
a value that is a secret, remove the value from the config. Supply it as an
environment variable or from your CI secret store instead, as described in the
[README configuration section](../README.md#configuration).

Do not paste real credentials into an issue, a pull request, or a validation
log while you troubleshoot. Redact them first.

### Runtime reloads

When a long-running process reloads a config file with `RuntimeConfigManager`,
a file that fails to parse or validate is rejected. The previously loaded
configuration stays active. Fixing the file and saving it again applies the
change on the next reload.
