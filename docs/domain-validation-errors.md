# Domain Validation Errors

`validate_anchor_domain` and `validate_anchor_domain_with_policy` are pure
string parsers — they perform no network I/O. Every rejection they produce
describes why a string does not conform to the required format or policy.
None of them say anything about whether a later HTTP connection will succeed.

This document groups every rejection by its kind, explains what each one means
and what the appropriate diagnostic step is, then clarifies which failure
classes are explicitly outside the validator's scope.

**Source:** `src/domain_validator.rs`
**Error code reference:** [`docs/error-codes.md`](error-codes.md) — code 12
(`InvalidEndpointFormat`).

---

## All validations return one error code

The validator returns `AnchorKitError` with `ErrorCode::InvalidEndpointFormat`
(code 12) for every rejection. There is no separate error variant per rule;
the code alone does not tell you which rule failed. Use the diagnostic steps
below to narrow down the cause.

---

## Syntax errors

These rejections fire before any host-level logic runs. They indicate the
input string is structurally unusable.

| Condition | Example input | Diagnostic step |
|---|---|---|
| Empty or whitespace-only | `""` or `"   "` | Check that the config field is populated; `offline validate` catches blank required fields. |
| Leading or trailing whitespace on a non-blank value | `" https://anchor.example.com"` | Trim the value before passing it to the validator. The validator intentionally does not trim for you — a space is a signal of a misconfigured value. |
| Shorter than 12 bytes | `"https://a"` | A valid HTTPS URL requires at least scheme + host + TLD, e.g. `https://a.bc`. |
| Longer than 2048 bytes | _(any URL > 2048 chars)_ | Shorten the URL; anchor origins must not carry query strings or long paths. |
| Scheme is not exactly `https://` | `"http://anchor.example.com"` | Change the scheme to `https://`. HTTP is never accepted — `ftp://`, `ws://`, `wss://`, and others are also rejected. |
| Forbidden characters in URL | URL containing `<`, `>`, `{`, `}`, `\|`, `\`, `^`, `` ` ``, `[`, `]`, control characters, or spaces | Remove or percent-encode the offending character. `[` and `]` bracket notation for IPv6 is also caught here and rejected. |
| Userinfo (credentials) embedded in authority | `"https://user:pass@anchor.example.com"` | Remove the `user:pass@` prefix. Credentials in URLs are never accepted. |
| Multi-colon authority without brackets | `"https://host:8080:extra"` | A valid authority has at most one colon (separating host and port). Fix the malformed URL. |
| Path is not empty or `/` | `"https://anchor.example.com/api/v1"` | Anchor domains are origins — strip everything after the host (and optional port). |

---

## Host / hostname errors

These fire inside `validate_host`, after the authority is successfully
extracted from the URL.

| Condition | Example input | Diagnostic step |
|---|---|---|
| Empty host | `"https://"` | Supply a hostname after `https://`. |
| Space inside host | `"https://anchor .example.com"` | Remove the space; hostnames are ASCII only. |
| Port present but empty | `"https://anchor.example.com:"` | Either omit the colon or supply a numeric port. |
| Port contains non-digit characters | `"https://anchor.example.com:80a"` | Use a plain integer port, e.g. `:443`. |
| Port out of range | `"https://anchor.example.com:0"` or `"https://anchor.example.com:65536"` | Use a port in the range 1–65535. |
| Hostname exceeds 253 bytes | _(hostname portion > 253 bytes)_ | Shorten the hostname. RFC 1035 §2.3.4 caps total hostname length at 253 bytes. |
| No dot in hostname | `"https://localhost"` | Bare hostnames without a dot are rejected. Use a fully-qualified domain name. |
| Consecutive dots | `"https://anchor..example.com"` | Fix the double-dot; each label separator must be a single `.`. |
| Leading or trailing dot | `"https://.anchor.example.com"` | Remove the leading/trailing dot. |
| All-numeric labels (dotted-decimal IPv4) | `"https://192.168.1.1"` | IP addresses in dotted-decimal form are rejected. Use a hostname. |
| Label longer than 63 bytes | _(any single DNS label > 63 bytes)_ | Shorten the label. RFC 1035 §2.3.4 caps each DNS label at 63 bytes. |
| Label starts or ends with a non-alphanumeric character | `"https://-anchor.example.com"` or `"https://anchor-.example.com"` | Labels must start and end with ASCII `[a-z0-9A-Z]`. Hyphens are allowed only in the interior. |
| Label contains a non-ASCII or non-alphanumeric/hyphen character | `"https://ançhor.example.com"` (IDN / Unicode) | Unicode internationalized domain names (Punycode or raw Unicode) are not accepted. Use the ASCII-compatible encoding (ACE) form `xn--...` if that is a hard requirement, then verify it passes. |

---

## Policy errors

`validate_anchor_domain_with_policy` runs the same syntax checks first. After
those pass it evaluates the `DomainPolicy` rules against the extracted
hostname. Policy rejections carry the same `InvalidEndpointFormat` code, but
the `context` field on the error will contain the blocked host.

Policy rules use **suffix matching** on the bare hostname (no scheme, no port,
no path). The first matching rule wins. When no rule matches, the default
action is allow unless the policy was constructed with `DomainPolicy::deny_all`.

| Condition | Diagnostic step |
|---|---|
| Host matches a `Deny` rule | Check whether the deny rule is correct; suffix matching means `"example.com"` blocks `"sub.example.com"` as well. Use a more specific allow rule placed before the deny rule to carve out exceptions. |
| Host does not match any rule and `default_deny` is `true` | Add an explicit allow rule for the host, or switch to a less restrictive policy. |
| Pattern is too broad | A pattern like `"com"` matches every `.com` host. Use the full registrable domain. |

Policy matching is case-insensitive and operates only on the hostname string.
It does not look up DNS or compare IP addresses; a deny rule on `"bad.example.com"` does
not block a different hostname that resolves to the same IP as `bad.example.com`.

---

## What the validator does not cover

The following failure modes are outside the validator's scope. A URL that
passes `validate_anchor_domain` can still fail for any of these reasons at
connection time.

| Failure mode | Why it is not in the validator | Where to investigate |
|---|---|---|
| DNS resolution fails | The validator never calls a resolver. | Use `dig` or `nslookup` on the hostname. |
| Hostname resolves to a private, loopback, or link-local address | Rejection is by string form only. `127.0.0.1.nip.io` passes validation even though it resolves to `127.0.0.1`. | Inspect the resolver output; add an egress proxy or network policy if SSRF risk is a concern. |
| TLS handshake fails | No connection is opened during validation. | Check the certificate chain and that the port is correct. |
| HTTP 4xx / 5xx response | The validator only parses the URL string. | Check the anchor's `stellar.toml` endpoint and service availability. |
| Redirect target is a disallowed URL | Redirect destinations are not re-validated. | Use `ConnectionPolicy::strict()` to disable redirects, or add allow rules covering the redirect targets. |

**Do not disable validation to make a test pass.** A bypassed validator hides
the string problem and may allow a malformed URL into persistent storage. Fix
the input string instead, or use a valid test domain like
`https://test.example.com`.

---

## Quick reference

```
ErrorCode::InvalidEndpointFormat  (code 12)
  │
  ├── Syntax error — string is structurally invalid
  │     • blank / whitespace / too short / too long
  │     • wrong scheme, forbidden chars, userinfo, bad path
  │     ↳ Fix the input string; run `anchorkit offline validate`
  │
  ├── Host error — hostname portion fails RFC 1035 rules
  │     • IP address, bare hostname, bad port
  │     • label length, IDN / Unicode, hyphen at label edge
  │     ↳ Check DNS name rules; use an FQDN with ASCII labels
  │
  └── Policy error — host is syntactically valid but rule-blocked
        • deny rule matched, or default_deny and no allow matched
        ↳ Review DomainPolicy rules; check context field for blocked host
```

---

## Related references

- Full error code table: [`docs/error-codes.md`](error-codes.md)
- What the validator checks (and does not check): [`docs/governance-and-security.md` — Anchor domain validation scope](governance-and-security.md#anchor-domain-validation-scope)
- Source implementation: `src/domain_validator.rs`
- Tests: `src/domain_validator.rs` — `#[cfg(test)]` module starting at line 524
