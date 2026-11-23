# Request ID Deduplication

AnchorKit provides two complementary deduplication layers: a host-side guard
(`HostReplayGuard`) that runs before any business logic, and an in-process
store (`DeduplicationStore`) that caches operation outcomes across retries.
This document describes the scope of each, their expiry rules, how collisions
are handled, and what callers are responsible for supplying.

**Sources:** `src/request_deduplication.rs`, `src/host_replay_prevention.rs`

---

## Scope

Neither layer provides global uniqueness guarantees. Deduplication is
**process-local and time-bounded**.

| Layer | Where state lives | Scope boundary | Shared across processes? |
|---|---|---|---|
| `HostReplayGuard` | In-memory `Vec` | Single process instance | No |
| `DeduplicationStore` | In-memory `BTreeMap` | Single process instance | No |

A request ID that was accepted and expired on one host is not known to a
second host running the same anchor. If your deployment runs multiple
instances, each instance makes independent accept/replay decisions for the
same ID. If cross-instance deduplication is required, that must be provided
by external shared state (a database, Redis, etc.) outside this library.

---

## HostReplayGuard

`HostReplayGuard` sits at the host boundary. It is intended to reject
duplicate transaction submissions **before** business logic runs.

### How it works

A guard is created with a `window_secs` value. Each call to `check(request_id,
actor, now_secs)` does the following:

1. Prunes all entries whose `first_seen_at` is older than `now_secs -
   window_secs` (entries where `first_seen_at < cutoff` are dropped).
2. Searches the remaining entries for `request_id`.
3. If found: increments `replay_count` and returns `CheckResult::Replay {
   first_seen_at }` with the timestamp from step 1.
4. If not found: records a new entry with `first_seen_at = now_secs`,
   increments `accepted_count`, and returns `CheckResult::Accepted`.

The dedup key is `request_id` alone. The `actor` parameter is recorded in the
entry for diagnostics but does not affect duplicate detection — two calls with
the same `request_id` and different actors are still treated as a replay.

### Window expiry

An entry is considered expired when `first_seen_at < now_secs - window_secs`.
The boundary is **exclusive on the past side**: an entry recorded at `t=1000`
with `window_secs=60` is still live at `now=1060` and is pruned at `now=1061`.

```
recorded at t=1000, window=60s:
  t=1059  → live  (1059 - 1000 = 59 < 60)
  t=1060  → live  (1060 - 1000 = 60, cutoff = 1060 - 60 = 1000; retain(e >= 1000) keeps it)
  t=1061  → pruned (1061 - 60 = 1001 > 1000)
```

Passing `window_secs = 0` creates an **infinite window**: nothing is ever
pruned and every ID seen since guard creation is remembered.

Test coverage: `test_expired_id_is_accepted_again` (`src/host_replay_prevention.rs`),
`test_prune_expired_removes_old_entries`, `test_infinite_window_never_expires`.

### Empty request IDs

The guard does not validate the format of `request_id`. An empty string `""`
is accepted as a valid key on the first call and treated as a replay on all
subsequent calls within the window, regardless of actor. If your protocol
treats an empty ID as "no idempotency key", reject it **before** calling
`check`. See `test_empty_request_id_is_accepted_then_replayed`.

### Thread safety

`HostReplayGuard` is not `Sync`. Wrap it in a `Mutex` for use across threads.

---

## DeduplicationStore

`DeduplicationStore` caches operation outcomes so that retried calls receive
the same result without re-running the operation.

### Key construction

A `DeduplicationKey` is composed of an `operation` name (e.g. `"deposit"`)
and a `request_id` (e.g. `"txn-001"`). The store uses both fields together as
the lookup key — the same `request_id` under a different `operation` is an
independent entry.

```rust
let key = DeduplicationKey::new("deposit", "txn-001");
// Different operation → different entry, independent TTL.
let key2 = DeduplicationKey::new("withdrawal", "txn-001");
```

The internal map key uses **length-delimited encoding**
(`<len>:<value>|<len>:<value>`) so that `operation="a"`, `request_id="b:c"`
and `operation="a:b"`, `request_id="c"` always produce distinct keys. Callers
do not need to avoid colons in either field.

Test coverage: `colon_in_components_does_not_cause_key_collision`
(`src/request_deduplication.rs`).

### Case sensitivity

Request IDs are matched exactly as supplied. `"txn-ABC"` and `"txn-abc"` are
different keys. Normalize casing before constructing a key if your protocol
treats them as equivalent.

Test coverage: `mixed_case_request_ids_remain_distinct`.

### TTL and expiry

Every entry carries an `expires_at` timestamp set to `now_secs +
default_ttl_secs` at record time. An entry is live while `expires_at >
now_secs` (strict inequality). At the exact boundary second (`now_secs ==
expires_at`) the entry is treated as expired.

```
recorded at now=1000, TTL=10 → expires_at=1010:
  now=1009  → live     (1010 > 1009)
  now=1010  → expired  (1010 > 1010 is false)
  now=1011  → expired
```

A TTL of `0` is rejected at construction time with a panic — a zero TTL would
expire every entry immediately, making deduplication ineffective.

Test coverage: `expired_key_is_accepted_as_new_after_ttl`, `zero_ttl_is_rejected_by_new`.

### Capacity cap

`DeduplicationStore::with_capacity(ttl, max_entries)` sets a hard cap on the
number of live entries. When an insertion would exceed `max_entries`, the
store first evicts expired entries. If the store is still full after eviction,
the new entry is **silently dropped** — the operation proceeds as if
unrecorded, meaning a subsequent call for the same key is not treated as a
duplicate. Use `purge_expired` on a background interval to keep the store
below capacity in high-traffic deployments.

`DeduplicationStore::new(ttl)` creates a store with no capacity cap (`max_entries = 0`).

### Result caching

The first call that records a key stores either `DeduplicationResult::Success(summary)`
or `DeduplicationResult::Failure(error)`. Both success and failure outcomes
are cached for the full TTL. A retried operation that previously failed does
not re-run; it receives the cached failure.

```rust
store.record_failure(&key, "anchor_unavailable", 0);
// Subsequent calls within TTL:
assert_eq!(
    store.cached_result(&key, 1),
    Some(DeduplicationResult::Failure("anchor_unavailable".into())),
);
```

If the original operation produced a transient failure that you do not want
cached (e.g. a network timeout), do not call `record_failure` for it. Only
record outcomes you want returned on retry.

Test coverage: `cached_result_returns_failure`, `cached_result_none_for_expired`.

---

## Caller responsibilities

### Supplying the request ID

The store does not generate or validate request IDs. Callers must supply a
stable identifier that:

- Is the same across all retries of the same logical operation.
- Is different for distinct logical operations (even if the operation type is
  the same).

A content hash, client-generated idempotency key, or transaction reference
are all suitable. A timestamp or sequence counter is not suitable unless it is
guaranteed stable across retries.

### Choosing TTL and window size

Set the TTL (or window) to cover the longest realistic retry interval for the
operation, plus a safety margin. A deposit with a 5-minute retry window needs
a TTL greater than 300 seconds. Setting it too short means a late retry is
accepted as a new operation; setting it too long holds memory longer than
necessary.

### Claiming uniqueness

Do not describe request IDs as "globally unique" in your documentation or
error messages unless your ID generation actually enforces that property (e.g.
a UUID v4 generator with a collision-resistant probability). Both layers here
perform only local dedup within a single process and time window.

### Purging expired entries

Call `DeduplicationStore::purge_expired(now_secs)` periodically to reclaim
memory. Expired entries are not removed automatically on lookup — they remain
in the map until explicitly purged or the store is dropped.

---

## Limitations

- **No shared state.** Each process instance has its own store and guard.
  Restart-resilient deduplication requires a durable external store.
- **No ordering guarantee.** The store uses a `BTreeMap` keyed by an internal
  string; insertion order is not preserved.
- **Capacity drop is silent.** When a capped store is full after eviction, the
  insertion is dropped without returning an error. Monitor store size if
  capacity matters.
- **IDs are not logged.** Neither layer logs request IDs to avoid exposing
  payload hashes or issuer identifiers in structured output. The
  `HostReplayGuard` counters (`replay_count`, `accepted_count`) are available
  for metrics.

---

## Related references

- Replay protection for timestamps (on-chain): [`docs/replay-protection-boundaries.md`](replay-protection-boundaries.md)
- Error codes for replay and timestamp rejection: [`docs/error-codes.md`](error-codes.md) — codes 5 (`InvalidTimestamp`) and 6 (`ReplayAttack`)
- Source: `src/request_deduplication.rs`, `src/host_replay_prevention.rs`
