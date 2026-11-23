# Key Rotation Scope

"Key rotation" covers two distinct operations in AnchorKit that must not be
conflated. One is a local secret update that changes what credentials your
process uses to sign or authenticate outbound requests. The other is an
on-chain contract operation that changes the public key the contract accepts
attestations from. Each has its own entry points, its own authorization
requirements, and its own effect. Doing one does not do the other.

**Sources:** `src/contract.rs` — `rotate_sep10_key`, `revoke_attestor`,
`reactivate_attestor`, `register_attestor`

---

## The two rotation scopes

### 1. Local secret rotation

Local rotation changes a secret key stored outside the contract — in your
keystore, environment, secrets manager, or config file. Examples:

- Rotating the Ed25519 signing key your attestor service uses to sign outbound
  attestation payloads.
- Rotating API credentials or bearer tokens used by the host application.
- Rotating the SEP-10 JWT signing key stored on the host before updating the
  contract with the new public key via `rotate_sep10_key`.

**Effect:** Only the local process is affected. The contract continues to
verify signatures against whatever public key it has stored. If the new local
key does not match the contract's stored public key, every subsequent
attestation will fail with `ErrorCode::UnauthorizedAttestor` (code 4) or
`ErrorCode::SignatureVerificationFailed` (code 61).

**No on-chain transaction is required** to rotate a local secret, but unless
the contract is also updated, the new secret is unusable for attestation.

---

### 2. On-chain identity rotation

On-chain rotation changes what the contract accepts. There are two entry
points depending on what you are rotating.

#### `rotate_sep10_key` — SEP-10 JWT verification key

Replaces the Ed25519 public key the contract uses to verify SEP-10 JWTs for a
given issuer address.

```
rotate_sep10_key(env, issuer: Address, new_public_key: Bytes<32>)
```

- Requires **admin authorization** (`require_admin`).
- `new_public_key` must be exactly 32 bytes; any other length is rejected with
  `ErrorCode::ValidationError`.
- The old key is preserved under a secondary storage key (`SEP10OLD`) for one
  TTL period so that in-flight tokens signed with the old key can drain
  gracefully. After the TTL expires, only the new key is accepted.
- Emits a `sep10key.rotated` event carrying the issuer address.

#### `revoke_attestor` + `register_attestor` (or `reactivate_attestor`) — attestor identity

Replacing an attestor's registered public key requires a revoke-then-re-register
(or revoke-then-reactivate) sequence. There is no single "update key" entry
point for active attestors.

| Step | Entry point | Authorization | Effect |
|---|---|---|---|
| 1 | `revoke_attestor(attestor)` | Primary admin | Removes active registration and stored public key; writes `AttestorRevocationRecord` preserving the old key and metadata |
| 2a | `register_attestor(...)` with new key | Primary admin | Creates a fresh registration with the new public key |
| 2b | `reactivate_attestor(attestor)` | Primary admin | Restores the **stored** public key from the revocation record |

**Step 2b is not a key rotation.** `reactivate_attestor` restores the same
key that was present at revocation time — the key recorded in
`AttestorRevocationRecord.public_key`. If the original key is compromised,
reactivation restores the compromised key. Use step 2a (re-register with a
new key) instead when the reason for rotation is a compromise.

---

## What each operation does and does not affect

| | Local secret rotation | `rotate_sep10_key` | Revoke + re-register |
|---|---|---|---|
| Changes what the contract verifies | **No** | **Yes** | **Yes** |
| Requires on-chain transaction | No | Yes | Yes (two transactions) |
| Requires admin authorization | No | Yes | Yes |
| Old key gracefully drained | n/a | Yes (one TTL period) | No — revocation is immediate |
| Preserves existing attestations | Yes | Yes | Yes |
| Preserves existing KYC records | Yes | Yes | Yes |
| Revocation record written | No | No | Yes |
| Audit log entry written | No | No | Yes |

---

## Required sequence for each scenario

### Rotating a SEP-10 signing key

1. Generate the new Ed25519 keypair off-chain.
2. Update your host's keystore or secrets manager with the new private key.
3. Call `rotate_sep10_key(issuer, new_public_key)` from the admin account.
4. Verify the event `sep10key.rotated` was emitted.
5. Allow in-flight tokens signed by the old key to expire naturally (the old
   key remains accepted for one TTL period after rotation).

Do not start step 2 after step 3 — if you update the contract before your
host has the matching private key, JWT verification fails for all requests
until the host key is updated.

### Rotating an attestor's signing key (non-compromise)

1. Generate the new keypair off-chain.
2. Call `revoke_attestor(attestor)` from the admin account. The attestor can
   no longer submit attestations from this point.
3. Call `register_attestor(attestor, new_public_key, ...)` with the new public
   key.
4. Update the attestor service's local keystore with the new private key.
5. Verify `is_attestor(attestor)` returns `true` and that the new public key
   is stored.

### Rotating a compromised attestor key

The sequence is the same as above. Do **not** use `reactivate_attestor` —
it would restore the compromised key. Use `register_attestor` with the new
key at step 3.

Review `get_attestor_revocation_info(attestor)` after revocation to confirm
the revocation record and the `revoked_at` timestamp before proceeding.

---

## Authority requirements

Both `rotate_sep10_key` and `revoke_attestor` require the **primary admin**
account (the address stored at contract initialization). There is no delegated
capability that permits key rotation without primary-admin authorization.
Confirm the primary admin address with `get_admin()` before constructing the
rotation transaction.

The session-based entry point `revoke_attestor_with_session` requires either
the primary admin or an address holding the `AdminRole::AttestorAdmin` role,
but it does not write an `AttestorRevocationRecord` and cannot be followed by
`reactivate_attestor`. Use `revoke_attestor` (non-session) when you intend to
reactivate later.

Verify the authority you hold before starting a rotation sequence. A
mid-sequence failure (revocation succeeded, re-registration failed due to
wrong authorization) leaves the attestor inactive.

---

## What rotation does not do

- **Rotation does not invalidate existing attestations.** Records written
  before revocation remain in storage for their normal lifetime.
- **Rotation does not automatically coordinate both layers.** No contract
  operation updates your local keystore, and no local keystore change updates
  the contract. Each layer must be updated explicitly in the right order.
- **`reactivate_attestor` is not a key rotation.** It restores the key from
  the revocation record. Use it only when the goal is to restore a
  temporarily revoked attestor whose key is still valid.
- **Config `enabled: false` is not a revocation.** Setting `enabled: false`
  in the config file has no effect on the contract. The on-chain registration
  remains active until `revoke_attestor` is called.

---

## Related references

- Revocation workflow and verification steps: [`docs/attestor-roles.md` — Operational note: revoke a compromised or retired attestor](attestor-roles.md#operational-note-revoke-a-compromised-or-retired-attestor)
- Admin key management and multi-sig requirements: [`docs/governance-and-security.md` — Admin Key Management](governance-and-security.md#admin-key-management)
- Contract entry point reference: [`docs/CONTRACT_FUNCTIONS.md`](CONTRACT_FUNCTIONS.md)
- Error codes for authorization failures: [`docs/error-codes.md`](error-codes.md) — codes 3 (`AttestorNotRegistered`), 4 (`UnauthorizedAttestor`), 28 (`Unauthorized`), 59 (`AttestorRevoked`), 61 (`SignatureVerificationFailed`)
- Source: `src/contract.rs` — `rotate_sep10_key` (line 3043), `revoke_attestor` (line 3348)
