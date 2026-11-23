# Anchor Health Metrics Reference

This document explains the on-chain health counter fields that
`record_health_event` writes and `get_anchor_health` reads, the meaning of a
zero-event state, and how reset and the last-event timestamp behave.  It
complements the host-side metrics layer described in
[Metrics Guide](./metrics.md).

All fields below belong to the `AnchorHealthMetrics` contract type, defined in
`src/contract.rs` and persisted under the `HLTHCNT` storage prefix.  The
off-chain scoring helpers in `src/anchor_health.rs` are listed separately at
the end because operators may encounter their output alongside these
counters.

## Field reference

| Field | Type | Unit | Meaning |
|---|---|---|---|
| `anchor` | `Address` | n/a | The anchor these counters belong to |
| `success_count` | `u64` | calls | Total successful endpoint calls recorded via `record_health_event` with `success = true` |
| `failure_count` | `u64` | calls | Total failed endpoint calls recorded via `record_health_event` with `success = false` |
| `total_calls` | `u64` | calls | `success_count + failure_count`, recomputed on every event |
| `uptime_bps` | `u32` | basis points (0–10000) | Uptime derived on write: `success_count * 10000 / total_calls`. `10000` equals 100% |
| `last_event_at` | `u64` | ledger seconds | Ledger timestamp of the most recent `record_health_event` call. `0` when no event has ever been recorded |

## How `uptime_bps` is derived

`uptime_bps` is recomputed inside `record_health_event` after each call:

```text
total_calls   = success_count + failure_count
uptime_bps    = success_count * 10000 / total_calls
```

The multiplication uses `saturating_mul`, so counters near `u64::MAX` saturate
instead of wrapping.  `10000` basis points equals `100.00%`; `5000` equals
`50.00%`.

## Zero-denominator behavior (no events yet)

This state is explicit in both the writer and the reader, and it is covered by
tests.

- When no event has ever been recorded, `get_anchor_health` returns a zeroed
  `AnchorHealthMetrics`: `success_count = 0`, `failure_count = 0`,
  `total_calls = 0`, `uptime_bps = 0`, `last_event_at = 0`.
- A zero `total_calls` makes the uptime denominator zero.  In that case
  `uptime_bps` is set to `0`, not to `10000`.  An anchor with no recorded
  events therefore reports **0% uptime**, not 100%.

This matches the off-chain helper `uptime_percent(success, failure)` in
`src/anchor_health.rs`, which also returns `0.0` when both counters are zero.

## Reset behavior (`reset_anchor_health`)

`reset_anchor_health` is admin-only.  It zeroes the counters but does **not**
clear the timestamp:

- `success_count`, `failure_count`, `total_calls`, and `uptime_bps` are all
  set to `0`.
- `last_event_at` is set to the **current ledger timestamp** (`now`), not to
  `0`.

Consequence for operators: after a reset the anchor reports `0%` uptime until
the next event is recorded, and `last_event_at` records _when the reset
happened_.  Distinguish the two states with the counters, not the timestamp:

- `last_event_at = 0` → no event has ever been recorded (fresh anchor).
- `last_event_at > 0` with `total_calls = 0` → counters were reset at that
  ledger time.

## `last_event_at` semantics

`last_event_at` is the ledger timestamp of the last _recorded health event_,
not the last time the anchor was queried.  It only advances when
`record_health_event` runs.  `get_anchor_health` reads it without updating it.

## What `record_health_event` requires

- Admin authorization (the caller must hold the admin role or a capability that
  grants this operation).
- The anchor must authenticate the call (`anchor.require_auth()`).
- The anchor must be a registered attestor (`check_attestor`).

On success it increments the relevant counter, recomputes `total_calls` and
`uptime_bps`, sets `last_event_at = now`, persists the entry, extends its TTL,
and publishes a `("health", "event", anchor)` event carrying
`(success, uptime_bps)`.

## Off-chain scoring helpers in `src/anchor_health.rs`

These live in the host-side scoring module, not on-chain.  They are listed here
because their output appears next to the on-chain counters in dashboards.

| Helper | Returns | Zero-input behavior |
|---|---|---|
| `uptime_percent(success, failure)` | `f64` percentage 0.0–100.0 | `0.0` when both counts are zero |
| `bps_to_percent_str(bps)` | `String` like `"99.50%"` | `"0.00%"` for `0` bps; pure formatter, does not compute uptime |
| `HealthWindow` fields `started_at` / `ended_at` | Unix seconds | Window boundaries, not a last-event timestamp |
| `HealthWindow::success_rate()` | `f64` in `[0.0, 1.0]` | `0.0` when `total_calls == 0` |
| `HealthWindow::routing_success_rate()` | `f64` in `[0.0, 1.0]` | `1.0` when no routing was attempted (no attempts = assumed perfect routing) |

The off-chain `HealthWindow` does not carry a last-event timestamp; it carries
window boundary timestamps.  The composite 0–100 score produced by
`score_window` is a separate signal from the on-chain `uptime_bps` counter and
is documented in the inline tests of `src/anchor_health.rs`.

## Related documentation

- [Metrics Guide](./metrics.md) — host-side counters, gauges, and latency
  summaries.
- [Health Checks](./health-checks.md) — what each health and readiness query
  measures, and what it does not.
