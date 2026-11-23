# Alert Deduplication and Suppression

This document explains when two alerts are considered duplicates and when a
later alert can be emitted again, using the types in `src/alert_dedup.rs`.
All examples are synthetic and contain no incident details.

Two mechanisms work together:

- `AlertDeduplicator` — automatic, time-windowed deduplication keyed by a
  caller-provided fingerprint.
- `AlertSuppressor` — explicit, operator-managed suppression for a fixed
  duration (for example, planned maintenance).

`AlertFilter` applies both in one `should_deliver` call.

## Configuration

`DedupConfig` controls the deduplicator:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `window_seconds` | `u64` | `300` | How long a fingerprint is suppressed after it fires. `0` disables deduplication — every event fires |
| `max_keys` | `usize` | `10000` | Maximum tracked fingerprints. When full, the entry with the oldest last-fired timestamp is evicted. `0` means unlimited |

## The deduplication key

The fingerprint is an opaque, caller-provided string used as an exact-match key
in a `BTreeMap`.  It identifies the _condition_, not each individual event.
Recommended forms:

```text
anchor:example.com:health:critical
contract:attestor_replay:GADDR...
```

Two alerts are duplicates **iff** they share the same fingerprint **and** the
second arrives within `window_seconds` of the first.

## When an alert fires or is suppressed

`AlertDeduplicator::should_fire(fingerprint, now)` returns `bool`:

| Situation | Result | Side effect |
|---|---|---|
| `window_seconds == 0` | `true` (dedup disabled) | none |
| First occurrence of a fingerprint | `true` (fires) | records `(last_fired_at = now, suppress_count = 0)` |
| Same fingerprint, `now - last_fired < window_seconds` | `false` (suppressed) | `suppress_count` incremented (saturating); a suppression diagnostic is appended to the log |
| Same fingerprint, `now - last_fired >= window_seconds` | `true` (fires again) | entry overwritten with `(now, 0)` |

### Expiry boundary

The boundary is inclusive: at exactly `now - last_fired == window_seconds` the
alert fires again.  This is covered by the `alert_fires_exactly_at_ttl_boundary`
test.  Just before the boundary (`window_seconds - 1`) the alert is still
suppressed.

### `suppress_count` resets on re-fire

The suppression count only accumulates _within_ a single window.  When the
window expires and the alert fires again, the entry is overwritten and
`suppress_count` resets to `0`.

## Bounded memory and eviction

When the map is full (`map.len() >= max_keys`, with `max_keys > 0`), inserting a
new fingerprint evicts the entry with the **minimum `last_fired_at` timestamp**
— the oldest-fired key, not the lexicographically smallest.  This is covered by
the `eviction_uses_oldest_timestamp_not_lexical_order` regression test.

## Suppression diagnostics are not dropped silently

Every suppressed call appends a JSON object to an internal log:

```json
{"alert_key":"anchor:example.com:health:critical"}
```

The fingerprint is serialized as a JSON string literal, so quotes, backslashes,
and control characters are escaped and cannot produce an invalid entry.  The
log is bounded by `max_keys`; when full, the oldest entries are dropped to keep
the newest.  `drain_suppression_log()` returns and clears the buffered entries,
so suppressed alerts are available for operator correlation rather than dropped
silently.

Inspector methods: `last_fired_at(fingerprint)`,
`suppress_count_for(fingerprint)`, `tracked_count()`, and `config()`.  Manual
reset: `reset(fingerprint)` and `reset_all()`.

## Explicit operator suppression (`AlertSuppressor`)

`AlertSuppressor` holds explicit entries keyed by fingerprint:

| Method | Behavior |
|---|---|
| `suppress(fingerprint, now, expires_at, reason)` | Stores an entry. Rejects `expires_at <= now` (zero-duration or past) — such an entry could never suppress a real alert. Overwrites any existing entry, so a window can be extended or shortened |
| `is_suppressed(fingerprint, now)` | `true` when an entry exists with `expires_at > now`. Expired entries are removed lazily on access |
| `unsuppress(fingerprint)` | Immediate removal |
| `purge_expired(now)` | Removes all entries with `expires_at <= now` |
| `active_count(now)` | Count of entries with `expires_at > now` |

An entry is active only while `expires_at > now`.

## `AlertFilter` — combining both

`AlertFilter::should_deliver(fingerprint, now)` applies the suppressor first,
then the deduplicator:

```text
if suppressor.is_suppressed(fingerprint, now):       do NOT deliver
else if dedup.should_fire(fingerprint, now):         deliver
else:                                                 do NOT deliver
```

An alert is delivered only when it is not explicitly suppressed **and** not
within the dedup window.  After both the suppression window and the dedup window
have passed, the alert is delivered again.

## Synthetic examples

### Deduplication within a window

```text
window_seconds = 300

t=1000  should_fire("anchor:example.com:health:critical", 1000)  -> true  (fires)
t=1100  should_fire("anchor:example.com:health:critical", 1100)  -> false (suppressed, suppress_count=1)
t=1300  should_fire("anchor:example.com:health:critical", 1300)  -> true  (>= 1000+300, fires again, suppress_count reset to 0)
```

### Manual suppression overrides dedup

```text
window_seconds = 300

t=1000  suppressor.suppress("anchor:example.com:health:critical", now=1000, expires_at=2000, "planned maintenance")
t=1500  filter.should_deliver("anchor:example.com:health:critical", 1500)  -> false (suppressor blocks; dedup never consulted)
t=2500  filter.should_deliver("anchor:example.com:health:critical", 2500)  -> true  (suppression expired at 2000; dedup window also passed)
```

### Eviction by oldest timestamp

```text
max_keys = 2, window_seconds = 9999

t=1000  should_fire("z-alert", 1000)  -> true   (recorded)
t=2000  should_fire("a-alert", 2000)  -> true   (recorded)
t=3000  should_fire("m-alert", 3000)  -> true   (map full; "z-alert" has the oldest timestamp and is evicted, not "a-alert")
t=3001  should_fire("z-alert", 3001) -> true   (evicted, so it fires again)
t=3001  should_fire("a-alert", 3001) -> false  (still tracked, still within its window)
```

## Related documentation

- [Metrics Guide](./metrics.md) — counters and latency summaries for
  host-side operations.
- [Health Metrics Reference](./health-metrics.md) — on-chain health counters
  that feed alert fingerprints.
- [Health Checks](./health-checks.md) — what each health and readiness query
  measures.
