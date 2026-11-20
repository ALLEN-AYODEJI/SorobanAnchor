# SEP-10 Identity Relationships

Three distinct identities appear in the SEP-10 flow. They are related but not interchangeable,
and confusing them is the most common source of registration failures.

Relevant source: `src/contract.rs` (`set_sep10_jwt_verifying_key`, `register_attestor`,
`verify_sep10_token_matches_attestor`) and `src/sep10_jwt.rs` (`verify_sep10_jwt`).

---

## The three identities

### 1. Token issuer (`sep10_issuer`)

The issuer is a Soroban `Address` stored on-chain as the key under which the anchor's
Ed25519 public key is registered. It identifies *which anchor* signed the JWT, not the
user or the attestor.

```
set_sep10_jwt_verifying_key(issuer: Address, public_key: Bytes)
```

The public key is stored under a hash of the issuer address (`"SEP10KEY" || issuer_xdr`).
When a token is later verified, the issuer address is used to look up that key.

The issuer address typically corresponds to the anchor's operational identity as registered
in the deployment — for example, an address derived from the anchor's stellar.toml `SIGNING_KEY`.
The contract does not enforce a specific naming convention; the operator chooses the address
when calling `set_sep10_jwt_verifying_key`.

The JWT's `iss` *string claim* is a separate field inside the token payload. When
`verify_sep10_jwt_with_issuer` is used, the `iss` string is compared against an
expected value. When `verify_sep10_token` (the standard contract entry point) is used,
the `iss` claim is validated for presence and format but is **not** compared against the
issuer `Address` — the issuer address only selects the public key.

### 2. Registered attestor (`attestor`)

The attestor is the on-chain identity that is authorised to submit attestations and call
attestor-gated contract functions. It is a Soroban `Address` stored in the attestor
registry after a successful `register_attestor` call.

```
register_attestor(attestor: Address, sep10_token: String, sep10_issuer: Address, public_key: BytesN<32>)
```

During registration the contract calls `verify_sep10_token_matches_attestor`, which
verifies the token using the key stored for `sep10_issuer` **and** requires the token's
`sub` claim to equal `attestor.to_string()` (the attestor's strkey). This check binds
the JWT to the specific address being registered: a token whose `sub` names a different
address cannot register a different attestor.

After registration, `is_attestor(attestor)` returns `true` and the address may be used
in attestation submissions.

### 3. Address used in contract calls

When an attestor submits an attestation or calls an attestor-gated function, the contract
checks `is_attestor_internal` against the calling address. This is the same `Address`
value that was passed to `register_attestor`. Authorization is by address equality — the
contract does not re-verify a JWT at call time.

---

## How they fit together

```
                   ┌─────────────────────────────────────────────────────────────┐
                   │  Admin setup (set_sep10_jwt_verifying_key)                   │
                   │                                                              │
                   │  issuer Address  ──────► stored Ed25519 public key          │
                   └──────────────────────────────────────────────────────────────┘
                                                    │
                                                    │ used to verify JWT signature
                                                    ▼
                   ┌─────────────────────────────────────────────────────────────┐
                   │  Attestor registration (register_attestor)                   │
                   │                                                              │
                   │  sep10_token.sub  ──must equal──►  attestor Address         │
                   │  sep10_token signed by issuer key                           │
                   │                                                              │
                   │  On success: attestor Address stored in registry            │
                   └──────────────────────────────────────────────────────────────┘
                                                    │
                                                    │ stored registry entry
                                                    ▼
                   ┌─────────────────────────────────────────────────────────────┐
                   │  Contract calls (submit_attestation, etc.)                   │
                   │                                                              │
                   │  calling Address  ──checked against──►  registry entry      │
                   └──────────────────────────────────────────────────────────────┘
```

In numbered steps:

1. An admin calls `set_sep10_jwt_verifying_key(ISSUER_ADDR, ed25519_pubkey)`. This
   associates the anchor's signing key with `ISSUER_ADDR` on-chain.

2. A participant obtains a SEP-10 JWT from the anchor. The JWT's `sub` claim contains
   the participant's own address strkey (e.g. `GATTESTOR...`), and the JWT is signed
   by the private key corresponding to `ed25519_pubkey`.

3. The participant calls `register_attestor(GATTESTOR_ADDR, jwt, ISSUER_ADDR, attestor_pubkey)`.
   The contract:
   - Looks up the Ed25519 key for `ISSUER_ADDR`.
   - Verifies the JWT signature, `exp`, `iat`, `iss`, and `sub`.
   - Confirms `sub == GATTESTOR_ADDR.to_string()`.
   - Stores `GATTESTOR_ADDR` in the attestor registry.

4. Future contract calls that require an attestor pass `GATTESTOR_ADDR` as the caller.
   The contract checks the registry; no JWT is re-presented.

---

## Common confusion points

**The issuer `Address` is not the `iss` JWT string.**
`set_sep10_jwt_verifying_key` takes a Soroban `Address` to key the public key lookup.
The `iss` field inside the JWT payload is a free-form string (typically an HTTPS URL).
They serve different purposes and are not compared to each other by the standard
`verify_sep10_token` path.

**The attestor address must appear as `sub`, not `iss`.**
The contract binds the registration to `attestor.to_string()` by passing it as
`expected_sub` to `verify_sep10_jwt`. Putting the attestor address in `iss` instead has
no effect on the `sub` check and the registration would be rejected if `sub` does not
match.

**The issuer and the attestor can be the same address in test setups.**
`register_attestor_with_sep10` in the test utilities passes the same address as both
`attestor` and `sep10_issuer`. This is legal but only works because in those tests the
same signing key is registered for that address. In a real deployment the issuer (anchor
signing identity) and each attestor (participant identity) are separate parties.

---

## Source references

| Concept | Location |
|---|---|
| Storing the issuer key | `src/contract.rs` → `set_sep10_jwt_verifying_key` (line ~3023) |
| Verifying token + sub binding | `src/contract.rs` → `verify_sep10_token_matches_attestor` (line ~3163) |
| Registration flow | `src/contract.rs` → `register_attestor` (line ~3244) |
| Registry check at call time | `src/contract.rs` → `is_attestor_internal` (line ~9223) |
| `sub` extraction and comparison | `src/sep10_jwt.rs` → `parse_json_sub`, `verify_sep10_jwt` |
| Integration tests | `tests/sep10_contract_tests.rs` → `contract_register_attestor_with_sep10_roundtrip` |
| Test utility (same-address shortcut) | `tests/sep10_test_util.rs` → `register_attestor_with_sep10` |
