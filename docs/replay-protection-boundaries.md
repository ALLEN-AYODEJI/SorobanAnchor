# Replay Protection: Timestamp Boundaries

Attestation submissions carry a timestamp that the contract validates against
the current ledger time before accepting the payload. This document explains
the three validity cases, the exact boundary values, and what each test covers.

**Sources:** `src/replay_detection.rs` — `is_timestamp_valid`,
`is_default_timestamp_valid`, `calculate_timestamp_age`, and the
`#[cfg(test)]` module starting at line 557.

---

## Default bounds

Two constants define the default window:

| Constant | Value | Meaning |
|---|---|---|
| `DEFAULT_MAX_AGE_SECS` | `300` | Maximum age of a past timestamp (5 minutes) |
| `DEFAULT_CLOCK_SKEW_SECS` | `60` | Maximum forward drift of a future timestamp (1 minute) |

`is_default_timestamp_valid(timestamp, now)` applies both constants.
`is_timestamp_valid(timestamp, now, max_age, max_future_skew)` takes explicit
values and is used in tests to verify boundary behaviour independently of the
defaults.

Neither function changes the validation window. Do not adjust the constants to
make a test pass — fix the timestamp instead.

---

## The three cases

### 1. Valid — timestamp is within the window

A timestamp is accepted when it falls inside the closed interval
`[now - max_age_secs, now + max_future_skew_secs]`.

```
         ←── max_age_secs ──┤      ├── max_future_skew_secs ──→
                            ▼      ▼
  ... ──────────────────────[──────]──────────────────────── ...
                          now-300  now  now+60
                                   ↑
                                 valid zone (inclusive on both ends)
```

**Code path** (`is_timestamp_valid`):

- When `timestamp <= now`: age = `now.saturating_sub(timestamp)`. Accepted if
  `age <= max_age_secs`.
- When `timestamp > now`: drift = `timestamp.saturating_sub(now)`. Accepted if
  `drift <= max_future_skew_secs`.

**Test:** `test_timestamp_within_bounds_accepted`
(`src/replay_detection.rs`, line 557)

```
now = 1_000_000
is_timestamp_valid(1_000_000,       now, 300, 60)  → true   (age = 0)
is_timestamp_valid(1_000_000 - 100, now, 300, 60)  → true   (age = 100 ≤ 300)
is_timestamp_valid(1_000_000 + 30,  now, 300, 60)  → true   (drift = 30 ≤ 60)
```

---

### 2. Expired — timestamp is too far in the past

A timestamp is rejected when its age exceeds `max_age_secs`. The boundary is
**inclusive**: a timestamp exactly `max_age_secs` seconds old is accepted; one
second older is rejected.

**Test:** `test_timestamp_at_exact_boundaries_accepted` and
`test_stale_and_future_beyond_boundaries_rejected`
(`src/replay_detection.rs`, lines 576 and 593)

```
now = 1_000_000, max_age = 300

timestamp = now - 300  (age = 300 = max_age)  → accepted   ← exact boundary
timestamp = now - 301  (age = 301 > max_age)  → rejected
```

The error raised is `ErrorCode::InvalidTimestamp` (code 5). It indicates the
submission arrived outside the allowed window, not that the timestamp value
is malformed. The fix is to submit with a fresh timestamp.

---

### 3. Future — timestamp is too far ahead of ledger time

A timestamp is rejected when its forward drift exceeds `max_future_skew_secs`.
The boundary is likewise **inclusive**: exactly `max_future_skew_secs` seconds
ahead is accepted; one second further is rejected.

A small forward allowance exists to absorb clock skew between the submitter
and the ledger. It is not intended as a scheduling mechanism for future-dated
submissions.

**Test:** `test_timestamp_at_exact_boundaries_accepted` and
`test_stale_and_future_beyond_boundaries_rejected`
(`src/replay_detection.rs`, lines 576 and 593)

```
now = 1_000_000, max_future_skew = 60

timestamp = now + 60  (drift = 60 = max_future_skew)  → accepted   ← exact boundary
timestamp = now + 61  (drift = 61 > max_future_skew)  → rejected
```

---

### 4. Zero — always rejected

A timestamp of `0` is unconditionally rejected regardless of `now` or the
configured bounds. Zero is used as a sentinel for an unset field and is never
a valid submission timestamp.

**Test:** `test_stale_and_future_beyond_boundaries_rejected` (line 593)

```
is_timestamp_valid(0, any_now, any_max_age, any_skew)  → false
```

---

## Boundary summary table

All examples use the defaults (`max_age = 300`, `max_future_skew = 60`) with
`now = 1_000_000`.

| Timestamp | Offset from now | Result | Reason |
|---|---|---|---|
| `0` | — | **rejected** | zero is always invalid |
| `999_699` | −301 s | **rejected** | age 301 > max_age 300 |
| `999_700` | −300 s | **accepted** | age 300 = max_age (inclusive boundary) |
| `999_701` | −299 s | **accepted** | age 299 < max_age |
| `1_000_000` | 0 s | **accepted** | current ledger time |
| `1_000_059` | +59 s | **accepted** | drift 59 < max_future_skew |
| `1_000_060` | +60 s | **accepted** | drift 60 = max_future_skew (inclusive boundary) |
| `1_000_061` | +61 s | **rejected** | drift 61 > max_future_skew |
| `u64::MAX` | far future | **rejected** | drift overflows; `saturating_sub` caps it |

**Test covering the default-bounds helper:** `test_direction_safe_comparison_no_underflow`
(`src/replay_detection.rs`, line 614)

```rust
// From the test — using is_default_timestamp_valid:
assert!(is_default_timestamp_valid(1_000_000 - 300, 1_000_000));   // exact past boundary
assert!(!is_default_timestamp_valid(1_000_000 - 301, 1_000_000));  // one second past
assert!(is_default_timestamp_valid(1_000_000 + 60,  1_000_000));   // exact future boundary
assert!(!is_default_timestamp_valid(1_000_000 + 61,  1_000_000));  // one second past
```

---

## Underflow safety

The comparison is direction-aware to prevent unsigned integer underflow. When
`now` is small (e.g. a freshly started testnet with low ledger timestamps),
the past-side check still works correctly because age is computed as
`now.saturating_sub(timestamp)` rather than `timestamp - now`.

**Test:** `test_direction_safe_comparison_no_underflow` (line 614)

```
now = 50, max_age = 300, max_future_skew = 60

is_timestamp_valid(20,  50, 300, 60)  → true   (age = 30)
is_timestamp_valid(100, 50, 300, 60)  → true   (drift = 50 ≤ 60)
is_timestamp_valid(120, 50, 300, 60)  → false  (drift = 70 > 60)
```

---

## `calculate_timestamp_age`

`calculate_timestamp_age(timestamp, now, max_age, max_future_skew)` returns
structured output for callers that need to know the magnitude as well as the
direction of the offset:

| Return value | Meaning |
|---|---|
| `Some(Ok(age))` | Timestamp is in the past, age in seconds, within `max_age` |
| `Some(Err(drift))` | Timestamp is in the future, drift in seconds, within `max_future_skew` |
| `None` | Timestamp is `0`, or age > `max_age`, or drift > `max_future_skew` |

`None` from this function corresponds exactly to `false` from `is_timestamp_valid`
with the same arguments.

---

## Errors raised

| Situation | Error code | Code number |
|---|---|---|
| Timestamp outside the valid window | `InvalidTimestamp` | 5 |
| Duplicate request ID (passes timestamp check but ID already seen) | `ReplayAttack` | 6 |

A submission that fails with `InvalidTimestamp` has a timing problem, not an
ID collision. A submission that fails with `ReplayAttack` has a duplicate ID
problem. The two codes are distinct so callers can handle them differently.

See [`docs/error-codes.md`](error-codes.md) for the full table.

---

## Related references

- Full timestamp validation implementation: `src/replay_detection.rs`
- Host-side deduplication window (separate, not ledger-based): [`docs/request-id-deduplication.md`](request-id-deduplication.md)
- Error code definitions: [`docs/error-codes.md`](error-codes.md)
