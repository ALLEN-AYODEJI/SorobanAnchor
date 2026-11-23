# Transport versus HTTP Retry Classification

Outbound HTTP operations can fail before an HTTP response exists, or complete
with an HTTP status response. `TransportErrorKind` and `HttpStatusClass`
classify these cases separately.

## Transport failure representatives

`classify_transport_error()` inspects a lowercase human-readable transport
error string. The following representatives are backed by tests in
`src/http_client.rs`:

| Error string representative | Classified kind | `is_transport_error_retryable()` |
|---|---|---|
| `"connection timed out"` / `"request timeout"` / `"DEADLINE exceeded"` | `Timeout` | `true` |
| `"DNS lookup failed"` / `"failed to resolve hostname"` / `"no such host"` | `DnsFailure` | `true` |
| `"connection refused"` | `ConnectionRefused` | `false` |
| `"TLS handshake failed"` / `"SSL certificate verification error"` / `"certificate expired"` | `TlsError` | `false` |
| `"unexpected end of stream"` / `"broken pipe"` | `Other` | `false` |

Classification is heuristic string matching, not a structured transport
exception. A TLS or DNS error from an application-specific wrapper may still
need to preserve its original transport context.

## HTTP status representatives

`classify_http_status_code()` classifies a completed HTTP response by status
code. The following representatives are backed by tests in `src/http_client.rs`:

| Status | Classified class | `is_http_status_retryable()` |
|---|---|---|
| `200`, `204`, `301`, `302`, `399` | `Success` | `false` |
| `408`, `429`, `500`, `502`, `503`, `504` | `Retryable` | `true` |
| `400`, `401`, `403`, `404`, `405`, `409`, `410`, `422`, `451`, `501`, `505` | `Permanent` | `false` |

The status mapping currently treats `200..=399` as `Success`, `408`, `429`,
`500`, `502`, `503`, and `504` as `Retryable`, and every other status as
`Permanent`.

## Classification does not configure retries

The classifier and predicate functions are decision helpers. They do not run
network requests and do not themselves retry or repeat a request.

The caller's retry policy still decides whether to retry, how many attempts to
make, and which backoff or jitter policy to apply. For example:

- A timeout or DNS kind can be marked retryable by the transport predicate,
  but the operation must still have a retry budget and caller policy that
  enables another attempt.
- A `429` or `503` status can be marked retryable, but a caller can choose not
  to retry it based on method, idempotency, timeout budget, or backoff limits.
- A `Permanent` HTTP response or `TlsError` transport failure can remain
  terminal under caller policy instead of triggering another request.

These safety limits are especially important for write-like operations.
Retryable does not mean safe-to-repeat. Callers must use idempotency keys,
deduplication, or application-specific commit/rollback rules before repeating
a mutating operation.

## Existing webhook behavior

The webhook delivery implementation uses `RetryConfig` and retries every
`http_post` outcome until `max_attempts` is exhausted. The HTTP and transport
classifiers described here are not consulted by that webhook loop. Thus:

- The classifiers describe reusable classification policy.
- Webhook retry behavior is independently controlled by
  `RetryConfig.max_attempts`, backoff, and the caller-supplied transport closure.
- HTTP `503` exhaustion and transport-error exhaustion are both observable in
  the DLQ; transport errors have status `0`, while HTTP failures retain the
  last status code.

The DLQ behavior is covered by `deliver_stores_dlq_entry_after_exhaustion` and
`deliver_stores_dlq_entry_on_transport_error` tests in `src/webhook.rs`.
