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
