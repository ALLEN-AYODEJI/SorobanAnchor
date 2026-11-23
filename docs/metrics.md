# Metrics Guide

This document lists the metrics exposed by AnchorKit's host-side operations.

## Overview

AnchorKit provides a lightweight metrics layer for operational visibility. The `metrics` module exports counters, gauges, and latency summaries that track HTTP requests, webhook deliveries, retries, and contract calls.

## Core Metrics

### HTTP Metrics

| Metric Name | Type | Unit | Description |
|---|---|---|---|
| `http.requests` | Counter | count | Total outbound HTTP POST requests started |
| `http.successes` | Counter | count | HTTP POST requests completed with status < 400 |
| `http.error_responses` | Counter | count | HTTP POST requests completed with status ≥ 400 |
| `http.transport_errors` | Counter | count | HTTP POST requests that failed at transport layer |

### Webhook Metrics

| Metric Name | Type | Unit | Description |
|---|---|---|---|
| `webhook.deliveries` | Counter | count | Webhook delivery operations started |
| `webhook.attempts` | Counter | count | Individual webhook POST attempts (includes retries) |
| `webhook.successes` | Counter | count | Webhook deliveries that eventually succeeded |
| `webhook.failures` | Counter | count | Webhook deliveries that exhausted all attempts |
| `webhook.dlq_entries` | Counter | count | Entries appended to dead-letter queue |
| `webhook.dlq_depth` | Gauge | count | Total entries currently held in DLQ |

### Per-Operation Metrics

These metrics are generated dynamically per operation name. Replace `<operation>` with the actual operation identifier (e.g., `contract_call`, `sep6_deposit`).

| Metric Pattern | Type | Unit | Description |
|---|---|---|---|
| `<operation>.calls` | Counter | count | Total invocations of the operation |
| `<operation>.successes` | Counter | count | Operations that succeeded |
| `<operation>.failures` | Counter | count | Operations that failed |
| `<operation>.retry.attempts` | Counter | count | Closure executions inside retry loop |
| `<operation>.retry.backoffs` | Counter | count | Sleeps between attempts (actual retries) |
| `<operation>.retry.successes` | Counter | count | Retry loops that returned Ok |
| `<operation>.retry.failures` | Counter | count | Retry loops that gave up with Err |

**Standard operation names:**

- `contract_call` — Soroban contract invocations

### Latency Metrics

Latency observations are recorded per operation and aggregated as summaries:

| Field | Type | Unit | Description |
|---|---|---|---|
| `count` | u64 | count | Number of observations recorded |
| `total_ms` | u64 | milliseconds | Sum of all observed latencies |
| `max_ms` | u64 | milliseconds | Largest single observed latency |
| `avg_ms()` | u64 | milliseconds | Mean latency (computed from total_ms / count) |

## Behavior: Cumulative, Snapshot, and Reset

Every metric in the registry is one of three kinds, and each kind has a
distinct update and read semantic.  All updates use saturating arithmetic
(`saturating_add`), so a counter never wraps past `u64::MAX` and never panics
under `overflow-checks = true`.

| Kind | How it updates | Read semantic | Reset |
|---|---|---|---|
| Counter (`incr`, `incr_by`, `record_call`) | Monotonically increasing; values are summed and never decremented | Cumulative total since the registry was created | Never reset automatically; only by dropping and recreating the registry |
| Gauge (`set_gauge`) | Last-write-wins; the previous value is replaced, not summed | Point-in-time snapshot of the last written value | Replaced on the next `set_gauge`; no automatic reset |
| Latency summary (`observe_latency_ms`) | `count` and `total_ms` are summed (saturating); `max_ms` tracks the largest single observation | Aggregated summary; `avg_ms()` is `total_ms / count` | Never reset automatically |

`avg_ms()` returns `0` when `count == 0` (zero-denominator → `0`), so an
operation that has never been timed reports a mean latency of `0`, not a
division error.

### `snapshot()` does not reset

`MetricsRegistry::snapshot()` copies the current counters, gauges, and latency
summaries into an owned `MetricsSnapshot`.  It is a point-in-time copy for
export to a dashboard backend; it does not clear or decrement the registry.
The registry keeps its values after a snapshot, so two snapshots taken back to
back differ only by events that arrived between them.

### Opt-in and threading

`MetricsRegistry::from_monitoring_config` returns `Some` only when
`monitoring.enable_metrics` is explicitly `true`.  When it is `false` or
absent, the registry is not initialized and host-side operations proceed
without instrumentation.  The registry is intentionally not `Sync`; host-side
anchor operations in this crate are single-threaded blocking calls, mirroring
the interior-mutability pattern used by webhook delivery.

No monitoring backend or export format is defined by the registry.  The
embedding application decides how and when to read a `MetricsSnapshot` and
ship it elsewhere.

## Label Cardinality

All metrics use **bounded label dimensions**:

- Operation names are statically defined in code (not user-provided strings)
- No unbounded cardinality sources (e.g., transaction IDs, user addresses)
- Webhook and HTTP metrics have no labels beyond the base metric name

## Namespacing

Metrics can be namespaced using `MetricsRegistry::with_namespace()`. When a namespace is configured (e.g., `"anchorkit.fiat_ramp"`), all metric names are prefixed: `<namespace>.<metric_name>`.

Runtime configs declare the namespace via `monitoring.metrics_namespace`.

## API Stability

Metric names and semantics documented here are part of the public crate API. However:

- Internal metric implementation details (storage format, collection mechanism) are not guaranteed stable
- New metrics may be added in minor versions
- Metric removal or renaming follows semantic versioning (major version bump)

## Example Usage

```rust
use anchorkit::metrics::{names, MetricsRegistry};

let metrics = MetricsRegistry::with_namespace("my_anchor");
metrics.record_call(names::CONTRACT_CALL, true);

assert_eq!(metrics.counter("my_anchor.contract_call.calls"), 1);
assert_eq!(metrics.counter("my_anchor.contract_call.successes"), 1);
```

## Configuration

Enable metrics in your runtime config:

```json
{
  "monitoring": {
    "enable_metrics": true,
    "metrics_namespace": "anchorkit.production"
  }
}
```

When `enable_metrics` is `false` or omitted, the metrics registry is not initialized and operations proceed without instrumentation.
