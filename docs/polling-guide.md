# Transaction Status Polling Guide

`PollConfig` and `poll_transaction_status` are defined in `src/sep6.rs`.

---

## PollConfig fields

```rust
pub struct PollConfig {
    pub interval_ms: u64,
    pub max_duration_ms: u64,
    pub terminal_states: Vec<TransactionStatus>,
}
```

| Field | Type | Default | Meaning |
|---|---|---|---|
| `interval_ms` | `u64` | `2_000` | Milliseconds between fetch attempts. `sleep_fn` is called with this value after each non-terminal response. |
| `max_duration_ms` | `u64` | `60_000` | Maximum wall-clock budget for the entire poll loop. The loop stops with `TimedOut` once `elapsed_ms + interval_ms >= max_duration_ms`. |
| `terminal_states` | `Vec<TransactionStatus>` | see below | Status values that end polling immediately. Any response whose `status` is in this list returns `PollResult::Completed`. |

`PollConfig::default()` sets `interval_ms = 2_000` and `max_duration_ms = 60_000`, meaning at most **29 fetch attempts** are issued before the loop times out (the timeout check fires before the sleep on the last iteration).

The documented interval and duration are caller-supplied and are not a recommended production value. Choose values appropriate for the expected settlement latency of your anchor.

---

## Default terminal states

The default `terminal_states` list covers every status that cannot change further:

| Status | Meaning |
|---|---|
| `Completed` | Transaction settled successfully. |
| `Refunded` | Funds returned to the sender. |
| `Expired` | Transaction window closed without settlement. |
| `Error` | Anchor reported an unrecoverable error. |
| `NoMarket` | No liquidity available for the requested route. |
| `TooSmall` | Amount is below the anchor's minimum. |
| `TooLarge` | Amount exceeds the anchor's maximum. |

A custom `terminal_states` list can expand or restrict this set. The loop only inspects the list you supply; it has no built-in knowledge of which statuses are truly final.

---

## How the loop works

```
fetch (with up to 3 transient retries)
│
├─ Err (non-retryable)  ──► PollResult::Failed(e)
│
├─ Ok, status is terminal ─► PollResult::Completed(resp)
│
└─ Ok, status is pending
    │
    ├─ elapsed_ms + interval_ms >= max_duration_ms ─► PollResult::TimedOut
    │
    └─ sleep(interval_ms) ; elapsed_ms += interval_ms ; loop
```

Each outer loop iteration wraps the fetch in `retry_with_backoff` with a fixed inner config of **3 attempts, 100 ms initial delay, 1 000 ms ceiling, multiplier 2**. The inner retry fires only for errors classified as transient by `is_retryable` (see below). The outer sleep and elapsed counter are only advanced when the fetch succeeds but returns a non-terminal status.

---

## PollResult variants

```rust
pub enum PollResult {
    Completed(TransactionStatusResponse),
    TimedOut,
    Failed(crate::errors::Error),
}
```

| Variant | When it occurs |
|---|---|
| `Completed(resp)` | A fetch returned a status present in `terminal_states`. `resp` holds the full normalized response. |
| `TimedOut` | `max_duration_ms` elapsed without a terminal status. The transaction may still be in progress on the anchor's side; `TimedOut` is a polling budget exhaustion, not evidence of transaction failure. |
| `Failed(e)` | A fetch returned a non-retryable error. The inner `retry_with_backoff` exhausted its attempts or the error code was not classified as transient. |

### Distinguishing a pending transaction from a polling error

`TimedOut` and `Failed` have different implications:

- **`TimedOut`** means the anchor consistently returned a non-terminal status throughout the budget. The transaction is still alive. You can call `poll_transaction_status` again with a new `PollConfig`, or query the anchor directly at a later time.
- **`Failed(e)`** means the fetch itself broke — network, auth, or an unrecoverable anchor error. Inspect `e.code` to decide whether to surface the error to the user or retry at the application level.

A status of `TransactionStatus::Error` is a **terminal state** (the anchor considers it final) and produces `PollResult::Completed`, not `PollResult::Failed`. An anchor-side error and a polling infrastructure error are therefore distinct in the return type.

---

## Transient error codes

`is_retryable` in `src/retry.rs` returns `true` for these error codes:

| ErrorCode | Description |
|---|---|
| `AttestationNotFound` | Attestation record not yet visible. |
| `StaleQuote` | Quote has just expired. |
| `NoQuotesAvailable` | No quotes returned; may resolve shortly. |
| `CacheExpired` | Cached response stale. |
| `CacheNotFound` | Cache entry missing. |

All other codes are non-retryable and cause an immediate `PollResult::Failed`.

---

## Injecting sleep

`poll_transaction_status` accepts a `sleep_fn: impl FnMut(u64)` so callers control the sleep mechanism. Pass `|_| {}` in tests to run synchronously without real delays. The `elapsed_ms` counter is incremented by `interval_ms` regardless of what `sleep_fn` does, so the timeout math is consistent whether sleep is real or mocked.

---

## Tests

| Test | File | What it covers |
|---|---|---|
| `test_poll_completes_before_timeout` | `src/sep6.rs` | Terminal status on first fetch returns `Completed` immediately. |
| `test_poll_times_out` | `src/sep6.rs` | Persistent pending status exhausts `max_duration_ms` and returns `TimedOut`. |
| `test_poll_retries_transient_error_then_succeeds` | `src/sep6.rs` | Transient errors on the first two attempts are retried; third attempt succeeds. |
| `test_poll_terminal_state_detection_all_variants` | `src/sep6.rs` | Each of the seven default terminal statuses individually triggers `Completed`. |
