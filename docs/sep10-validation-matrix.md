# SEP-10 JWT Validation Matrix

`verify_sep10_jwt` and `verify_sep10_jwt_with_issuer` are implemented in `src/sep10_jwt.rs`.
Contract-level entry points (`verify_sep10_token`, `verify_sep10_token_matches_attestor`,
`verify_sep10_token_for_subject`) wrap these functions and are in `src/contract.rs`.

Checks are applied in the order listed. The first failure stops processing.

---

## Validation matrix

| # | Dimension | Rule | Outcome on failure | Code / Test |
|---|---|---|---|---|
| 1 | **Token length** | Token must not be empty and must not exceed `MAX_JWT_LEN` (default 2 048, configurable via instance key `JWTMAXLEN`, range 2 048–16 384). | `Err(())` | `src/sep10_jwt.rs` → length guard; `tests/sep10_hardening_tests.rs` → `rejects_empty_token`, `rejects_whitespace_only_token` |
| 2 | **Structure** | Token must contain exactly two `.` separators (three parts). An extra trailing segment or a two-part token is rejected. | `Err(())` | `tests/sep10_hardening_tests.rs` → `rejects_four_part_token`, `accepts_structurally_valid_three_part_token` |
| 3 | **Algorithm** | Header `alg` claim must be exactly `"EdDSA"`. The check is applied to the decoded header JSON; `alg` appearing only in `kid` or as a prefix/suffix does not satisfy it. | `Err(())` | `src/sep10_jwt.rs` → `parse_json_alg` + `alg == b"EdDSA"`; `tests/sep10_hardening_tests.rs` → alg-precision tests |
| 4 | **Signature** | Signature segment must decode to exactly 64 bytes and must pass Ed25519 verification over the `header.payload` signing input using the stored 32-byte `anchor_public_key`. | `Err(())` | `src/sep10_jwt.rs` → `env.crypto().ed25519_verify`; `src/sep10_jwt.rs` inline tests → `mutate_jwt_signature` path |
| 5 | **`exp` — expiry** | `exp` claim must be present (integer) and satisfy `exp + skew > now`. Default clock skew is 60 s, configurable via instance key `JWTSKEW`. A token with `exp == now` is accepted; one with `exp + 60 <= now` is rejected. | `Err(())` | `src/sep10_jwt.rs` → expiry check; `tests/sep10_contract_tests.rs` → `token_accepted_within_skew_window`, `token_rejected_just_outside_skew_window`, `token_accepted_at_exact_expiry_boundary` |
| 6 | **`iat` — issued-at present** | `iat` claim must be present. A token missing `iat` entirely is rejected. | `Err(())` | `src/sep10_jwt.rs` → `parse_json_iat(...).ok_or(())`; `tests/sep10_hardening_tests.rs` → `rejects_token_missing_iat` |
| 7 | **`iat` — non-zero** | `iat` must not be `0`. An explicit zero value is treated as absent/invalid. | `Err(())` | `src/sep10_jwt.rs` → `if iat == 0 { return Err(()) }`; `tests/sep10_hardening_tests.rs` → `rejects_token_with_zero_iat` |
| 8 | **`iat` — not future** | `iat` must not exceed `now + skew`. A token claiming to have been issued more than `skew` seconds in the future is rejected. | `Err(())` | `src/sep10_jwt.rs` → `if iat > now.saturating_add(skew)`; `tests/sep10_hardening_tests.rs` → `rejects_token_with_future_iat`, `accepts_token_with_iat_within_skew_window` |
| 9 | **`iat` — lifetime cap** | `exp - iat` must not exceed `MAX_JWT_LIFETIME` (86 400 s / 24 h). An `iat > exp` is also rejected via `checked_sub`. | `Err(())` | `src/sep10_jwt.rs` → lifetime check; `tests/sep10_contract_tests.rs` → `token_rejected_when_lifetime_exceeds_cap`; `tests/sep10_hardening_tests.rs` → overflow-safety tests |
| 10 | **`nbf` — not-before (optional)** | If present, `nbf` must not exceed `now + skew`. Absent `nbf` is ignored. | `Err(())` | `src/sep10_jwt.rs` → `parse_json_nbf` + skew check; `tests/sep10_hardening_tests.rs` → nbf-skew-boundary tests |
| 11 | **`jti` — replay (optional)** | If present, `jti` must not have been seen before. The SHA-256 of `"jti:" \|\| jti_bytes` is stored in persistent contract storage with TTL derived from the token's remaining lifetime plus 120 ledger buffer. A second invocation with the same `jti` is rejected. `jti` longer than 256 bytes is rejected. The store write is deferred until all other checks pass, so an invalid token cannot poison a `jti` for a later valid token. | `Err(())` | `src/sep10_jwt.rs` → JTI block; `tests/sep10_contract_tests.rs` → `jti_rejected_on_second_invocation`, `different_jtis_each_accepted`, `token_without_jti_accepted`; `tests/sep10_hardening_tests.rs` → JTI-not-poisoned-by-invalid-token test |
| 12 | **`iss` — present and non-empty** | `iss` claim must be present and must not be empty or whitespace-only after trimming. | `Err(())` | `src/sep10_jwt.rs` → `parse_json_iss` + trim check; `tests/sep10_hardening_tests.rs` → `rejects_token_with_whitespace_only_iss` |
| 13 | **`iss` — no control characters** | `iss` must not contain ASCII control characters (0x00–0x1F, 0x7F). A null byte or other control character in the issuer string is rejected. | `Err(())` | `src/sep10_jwt.rs` → control-char scan; `tests/sep10_hardening_tests.rs` → `rejects_token_with_control_char_in_iss` |
| 14 | **`sub` — present and non-empty** | `sub` claim must be present and must not be an empty string. | `Err(())` | `src/sep10_jwt.rs` → `parse_json_sub` + length check; `tests/sep10_hardening_tests.rs` → `rejects_token_with_empty_sub` |
| 15 | **`sub` — optional match** | When `expected_sub` is `Some`, the parsed `sub` must equal it exactly. `verify_sep10_token_matches_attestor` passes the attestor's strkey as `expected_sub`. | `Err(())` | `src/contract.rs` → `verify_sep10_token_matches_attestor`; `tests/sep10_contract_tests.rs` → `contract_register_attestor_with_sep10_roundtrip` |

### Additional check in `verify_sep10_jwt_with_issuer`

| # | Dimension | Rule | Outcome on failure | Code / Test |
|---|---|---|---|---|
| 16 | **`iss` — exact match** | The `iss` claim (trimmed) must equal the caller-supplied `expected_issuer` (trimmed). An empty `expected_issuer` is rejected before any token parsing. | `Err(())` | `src/sep10_jwt.rs` → `verify_sep10_jwt_with_issuer`; `tests/sep10_hardening_tests.rs` → `with_issuer_accepts_matching_iss`, `with_issuer_rejects_mismatched_iss`, `with_issuer_rejects_empty_expected_issuer`, `with_issuer_trims_whitespace_before_comparison` |

---

## Configurable parameters

| Parameter | Storage key | Default | Setter |
|---|---|---|---|
| Maximum token length | `JWTMAXLEN` (instance) | 2 048 chars | `set_jwt_max_len` (range 2 048–16 384) |
| Clock skew tolerance | `JWTSKEW` (instance) | 60 s | `set_jwt_skew` |

Both parameters are read inside `verify_sep10_jwt` on every invocation.

---

## Limitations and scope

- These checks cover the token structure and claim semantics as implemented and tested. They do not constitute a conformance claim beyond what the tests demonstrate.
- The implementation parses JWT fields with a hand-written scanner rather than a general JSON parser. Escape sequences inside string values (e.g. `\uXXXX`) are not decoded; the raw bytes are compared directly.
- Signature verification uses the Soroban host's `ed25519_verify` builtin. No other algorithm (`HS256`, `RS256`, etc.) is accepted.
- The JTI replay cache persists across ledger boundaries within the TTL window but is scoped to a single contract instance. A token replayed against a different contract instance is not detected.
- There is no revocation mechanism for individual tokens beyond JTI replay protection and natural expiry.
