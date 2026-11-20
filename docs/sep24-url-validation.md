# SEP-24 Interactive URL Validation

`validate_interactive_url` and related helpers are implemented in `src/sep24.rs`.
Origin-pinning integration tests are in `tests/sep24_origin_pinning_tests.rs`.

---

## What `validate_interactive_url` checks

The function signature is:

```rust
pub fn validate_interactive_url(url: &str, allowed_origin: Option<&str>) -> Result<(), AnchorKitError>
```

Validation proceeds in four stages applied in order:

### Stage 1 — Normalization

The raw URL string is passed through `normalize_url`, which lowercases the scheme and
host, strips the default HTTPS port (443), and removes a bare trailing slash. This means
`HTTPS://Anchor.Example.COM:443/` and `https://anchor.example.com/` are treated as the
same URL before any structural check runs.

### Stage 2 — Structural validation

`validate_anchor_domain` enforces:

| Rule | Example rejected |
|---|---|
| Scheme must be `https` | `http://anchor.example.com/deposit` |
| No userinfo component | `https://user:pass@anchor.example.com/deposit` |
| No IP address literals | `https://[::1]/deposit`, `https://[2001:db8::1]/deposit` |
| Must have a valid registered domain | relative paths, `data:` URIs, empty string |

A URL that passes normalization but reduces to exactly `https://` (no host) is also
rejected after this stage.

### Stage 3 — Origin pinning (optional)

When `allowed_origin` is `Some`, the normalized origin of the URL (scheme + host +
non-default port) is compared against the normalized origin of `allowed_origin`. The
comparison is case-insensitive on the host and scheme. A different host, a different
scheme, or a non-default port on one side but not the other all constitute a cross-origin
mismatch and return `ErrorCode::InvalidEndpointFormat`.

When `allowed_origin` is `None`, this check is skipped entirely and any structurally
valid HTTPS URL is accepted.

The `allowed_origin` value is itself normalized before comparison. An `allowed_origin`
that fails normalization (e.g. `"not a valid origin"`) causes the function to return an
error immediately.

### Stage 4 — Open-redirect detection

The query string is scanned for parameters whose names match a known set of redirect
keywords **and** whose decoded values contain `://` (indicating an absolute URL). A
relative path value or an empty value is not flagged. Percent-encoding in both key and
value is decoded before the check, so `%52%45%54%55%52%4E_%55%52%4C=https%3A%2F%2F...`
is treated the same as `RETURN_URL=https://...`.

Redirect parameter names checked:

```
redirect  redirect_url  redirect_uri  callback  next
return_url  return_uri  continue  dest  destination  goto  target  to
```

---

## Accepted URL examples

| URL | `allowed_origin` | Result |
|---|---|---|
| `https://anchor.example.com/sep24/deposit?asset=USDC` | `None` | Accepted |
| `https://anchor.example.com/sep24/deposit` | `Some("https://anchor.example.com")` | Accepted |
| `HTTPS://ANCHOR.EXAMPLE.COM/DEPOSIT` | `None` | Accepted (normalized to lowercase) |
| `https://anchor.example.com:443/deposit` | `None` | Accepted (default port stripped) |
| `https://anchor.example.com/auth?redirect=/local/path` | `None` | Accepted (relative redirect value) |
| `https://anchor.example.com/auth?redirect=` | `None` | Accepted (empty redirect value) |
| `https://anchor.example.com/deposit?asset=USDC&amount=100` | `None` | Accepted (safe query params) |

---

## Rejected URL examples

| URL | `allowed_origin` | Reason |
|---|---|---|
| `http://anchor.example.com/deposit` | `None` | Non-HTTPS scheme |
| `/deposit/interactive` | `None` | Relative path, no scheme |
| `data:text/html,<h1>phish</h1>` | `None` | Non-HTTPS scheme |
| `https://user:pass@anchor.example.com/deposit` | `None` | Userinfo present |
| `https://[::1]/deposit` | `None` | IP literal |
| `https://evil.example.com/deposit` | `Some("https://anchor.example.com")` | Origin mismatch |
| `https://anchor.example.com:8443/deposit` | `Some("https://anchor.example.com")` | Port mismatch |
| `https://anchor.example.com/auth?redirect=https://evil.com` | `None` | Open redirect (absolute value) |
| `https://anchor.example.com/return?callback=https://phish.com` | `None` | Open redirect |
| `https://anchor.example.com/sep24/deposit?%52%45%54%55%52%4E_%55%52%4C=https%3A%2F%2Fevil.example.com` | `None` | Percent-encoded redirect param |
| `https://Anchor.Example.COM:8443/deposit` | `Some("https://anchor.example.com")` | Port mismatch (case-insensitive host, port still differs) |

---

## Origin pinning: when to use it and what it provides

Origin pinning is an optional layer on top of structural validation. When you pass the
anchor's expected origin as `allowed_origin`, the validator confirms that the redirect
URL resolves to the same HTTP origin — the same scheme, host, and effective port.

This prevents an anchor endpoint from returning a redirect URL pointing to a different
host. It does not prove that the page at the URL is trustworthy, that the TLS certificate
is valid at fetch time, or that the content served by the anchor is safe. Origin pinning
narrows the redirect target to a known domain; it does not audit what that domain serves.

The value to pass is typically the anchor's `transfer_server_sep0024` field from
`stellar.toml`, e.g. `"https://anchor.example.com"`. Do not include a path; only the
scheme and host (and non-default port if applicable) are compared.

---

## Transaction ID validation

`validate_transaction_id` enforces structural rules on SEP-24 transaction IDs:

| Rule | Constraint |
|---|---|
| Non-empty | At least 1 character |
| Maximum length | At most 64 characters |
| Allowed characters | ASCII alphanumeric, `-`, `_` only |
| Start/end | Must not start or end with `-` or `_` |
| Consecutive specials | No `--` or `__` sequences |
| Alphanumeric content | At least one ASCII alphanumeric character |

Transaction IDs are anchor-assigned opaque tokens. The normalizer in
`initiate_interactive_deposit_with_origin` and `initiate_interactive_withdrawal_with_origin`
only requires that the ID is non-blank (`require_transaction_id`); callers that want
stricter structural enforcement can call `validate_transaction_id` directly.

```rust
// Accepted
validate_transaction_id("tx-123")       // ok
validate_transaction_id("tx_abc_456")   // ok
validate_transaction_id("ABC123")       // ok

// Rejected
validate_transaction_id("")             // empty
validate_transaction_id("-tx-123")      // leading hyphen
validate_transaction_id("tx--123")      // consecutive hyphens
validate_transaction_id("tx-123-")      // trailing hyphen
```

---

## Service function flow

`initiate_interactive_deposit_with_origin` and `initiate_interactive_withdrawal_with_origin`
apply validation in this order:

1. `require_redirect_url` — rejects blank or whitespace-only URL immediately.
2. `normalize_url` — normalizes the URL.
3. `validate_interactive_url` — runs all four stages above.
4. `require_transaction_id` — rejects blank or whitespace-only transaction ID.

The functions without `_with_origin` in the name are thin wrappers that pass `None` as
`allowed_origin`.

---

## Tests

| Test | File | What it covers |
|---|---|---|
| `test_validate_interactive_url_accepts_https` | `src/sep24.rs` | Basic HTTPS acceptance |
| `test_validate_interactive_url_rejects_http` | `src/sep24.rs` | HTTP scheme rejection |
| `test_validate_interactive_url_rejects_relative` | `src/sep24.rs` | Relative path rejection |
| `test_validate_interactive_url_rejects_data_uri` | `src/sep24.rs` | `data:` URI rejection |
| `test_validate_interactive_url_rejects_userinfo` | `src/sep24.rs` | Userinfo rejection |
| `test_validate_interactive_url_rejects_ip_literal` | `src/sep24.rs` | IPv6 literal rejection |
| `test_validate_interactive_url_normalizes_before_checking` | `src/sep24.rs` | Uppercase scheme/host accepted after normalization |
| `test_validate_interactive_url_strips_default_port` | `src/sep24.rs` | `:443` stripped before validation |
| `test_validate_interactive_url_rejects_cross_origin_redirect` | `src/sep24.rs` | Origin mismatch → `InvalidEndpointFormat` |
| `test_validate_interactive_url_rejects_open_redirect_redirect` | `src/sep24.rs` | `redirect=https://...` rejected |
| `test_validate_interactive_url_rejects_open_redirect_callback` | `src/sep24.rs` | `callback=https://...` rejected |
| `test_validate_interactive_url_rejects_open_redirect_next` | `src/sep24.rs` | `next=https://...` rejected |
| `test_validate_interactive_url_accepts_redirect_path_only` | `src/sep24.rs` | Relative redirect value accepted |
| `test_validate_interactive_url_rejects_open_redirect_return_url` | `src/sep24.rs` | `return_url=https://...` rejected |
| `url_matching_origin_accepted` | `tests/sep24_origin_pinning_tests.rs` | Matching origin accepted |
| `url_on_different_domain_rejected` | `tests/sep24_origin_pinning_tests.rs` | Different domain rejected |
| `url_same_domain_different_port_rejected` | `tests/sep24_origin_pinning_tests.rs` | Same domain, different port rejected |
| `allowed_origin_none_skips_check_accepts_valid_https` | `tests/sep24_origin_pinning_tests.rs` | `None` origin skips pinning check |
| `origin_comparison_is_case_insensitive` | `tests/sep24_origin_pinning_tests.rs` | Case-insensitive host comparison |
| `encoded_mixed_case_redirect_parameter_is_rejected` | `tests/sep24_origin_pinning_tests.rs` | Percent-encoded redirect param rejected |
| `invalid_allowed_origin_is_rejected` | `tests/sep24_origin_pinning_tests.rs` | Malformed `allowed_origin` rejected |
| `validate_interactive_url_rejects_http_scheme` | `tests/sep24_origin_pinning_tests.rs` | HTTP rejected even without pinning |
| `deposit_with_http_redirect_url_rejected` | `tests/sep24_origin_pinning_tests.rs` | HTTP deposit URL rejected end-to-end |
| `withdrawal_with_http_redirect_url_rejected` | `tests/sep24_origin_pinning_tests.rs` | HTTP withdrawal URL rejected end-to-end |
