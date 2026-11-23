# Connection-Policy and Webhook Timeout Semantics

This page explains which operation each outbound timeout covers and which
timeouts are currently enforced. It does not infer timeout behavior from a
provider or from configuration examples.

## `ConnectionPolicy`

`ConnectionPolicy` is expressed in seconds. The verified defaults are:

| Field | Verified application | Default |
|---|---|---|
| `connect_timeout_secs` | TCP connection and TLS handshake setup when `build_client_with_policy()` is used | `10` seconds |
| `read_timeout_secs` | Stored policy value only; not passed to the reqwest 0.12 blocking client builder | `30` seconds |
| `total_timeout_secs` | Whole-request wall-clock budget for connect, send, and receive when `build_client_with_policy()` is used | `60` seconds |

`0` disables the corresponding `connect_timeout_secs` or
`total_timeout_secs` setting in `build_client_with_policy()`. The value stored
in `read_timeout_secs` is informational only and does not disable or enable
any reqwest operation in the current blocking builder.

`build_client_with_policy()` applies `connect_timeout_secs` and
`total_timeout_secs`. It does not apply `read_timeout_secs` separately.
`build_client()` accepts a plain seconds-based `timeout_secs` value and applies
it as the whole-request timeout; `0` disables that timeout.

The preset policies define verified values:

| Preset | Connect | Read field | Whole-request | Redirect behavior |
|---|---:|---:|---:|---|
| `default` | 10 s | 30 s | 60 s | Follow, maximum 3 |
| `aggressive` | 5 s | 15 s | 20 s | Do not follow |
| `conservative` | 20 s | 60 s | 120 s | Follow, maximum 5 |
| `strict` | 8 s | 20 s | 30 s | Do not follow |

Only connect and whole-request values are enforced by
`build_client_with_policy()` at present. Redirect behavior is a separate
policy and is covered by redirect tests, not by timeout tests.

## Webhook `timeout_ms`

`WebhookDeliveryConfig::timeout_ms` is a per-attempt timeout in milliseconds.
The proxy-aware helpers `deliver_webhook_with_proxy()` and
`deliver_webhook_with_proxy_traced()` convert it to a reqwest total
request-duration budget.

- A positive value is converted exactly to milliseconds.
- `0` means unset and selects the verified 30-second webhook default.
- Sub-second values remain sub-second; they are not rounded to whole seconds.
- Backoff delay and retry count are separate. `timeout_ms` is applied to each
  HTTP attempt, not to the complete retry sequence.
- Retry behavior is governed by `retry_config.max_attempts` and the backoff
  fields in `RetryConfig`.

The transport-neutral `deliver_webhook()` and `deliver_webhook_traced()`
functions accept an injected HTTP closure. They do not create or enforce a
timeout themselves. Timeout behavior is enforced only if the caller's transport
implements it. The proxy-aware reqwest wrappers implement the timeout by
building the client with `timeout_ms`.

Tests verify millisecond preservation and the `0` default:
`webhook_timeout_preserves_millisecond_unit`. The webhook delivery tests verify
first-attempt success, retry exhaustion, DLQ population, and transport errors;
they do not prove a live endpoint reached by a real network call.

## Timeout versus anchor rejection

An HTTP timeout is a local transport outcome. It means this process stopped
waiting or the transport did not complete within the configured budget. It does
not prove that an anchor received or processed the request.

An HTTP status code is a response from a reachable endpoint and should be
handled separately from a timeout or transport error. A timeout may therefore
occur even when an endpoint would later accept or reject a request; AnchorKit
cannot infer that outcome from the timeout.

Likewise, a successful status only means the transport received that status.
It does not establish a business-level result unless the application separately
validates the response and payload. Delivery failures after retry exhaustion are
recorded in the webhook DLQ with the last status code (or `0` for a transport
failure) and attempt count.

For how transport failures and completed HTTP status codes are classified,
see [Transport versus HTTP Retry Classification](./transport-http-retry-classification.md).
