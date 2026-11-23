# Health Checks and Readiness

AnchorKit exposes several read-only on-chain queries that report different
aspects of contract and anchor state.  A process health signal is **not** proof
that every configured dependency is reachable: every query listed here reads
only on-chain persistent or temporary storage and performs no external network
call.  Use this document to pick the query that answers the question you
actually have, and see [the runbook](./RUNBOOK.md) for where each one fits.

## At a glance

| Question | Query | Returns |
|---|---|---|
| Is the contract itself initialized and configured? | `get_health_status` | `HealthStatus` enum |
| Is a specific anchor registered and configured for a service? | `get_anchor_readiness` | `AnchorReadinessReport` |
| Is a specific attestor being throttled by the rate limiter? | `get_rate_limiter_health` | `RateLimiterHealth` |
| Is the cached metadata for an anchor still usable? | `get_metadata_freshness` | `MetadataFreshnessReport` |
| What are the cumulative health counters for an anchor? | `get_anchor_health` | `AnchorHealthMetrics` |
| Has the contract ever been initialized? | `is_initialized` | `bool` |

## `get_health_status` — overall contract health

Returns the `HealthStatus` enum (`src/contract.rs`):

| Variant | Value | Condition |
|---|---|---|
| `Healthy` | `0` | Contract is initialized **and** an explicit rate-limit config (`RL_CONFIG`) is present |
| `Degraded` | `1` | Contract is initialized but no `RL_CONFIG` is stored (fallback defaults in use) |
| `Unavailable` | `2` | Contract has not been initialized (`initialized_key` absent) |

What it checks — exactly two on-chain persistent keys: the initialization flag
and the rate-limit config key.

What it does **not** check:

- External or HTTP reachability of any anchor endpoint.
- Whether any anchor is registered, or has any service configured.
- Per-anchor health counters, metadata cache, or quote validity.
- Any external dependency (database, indexer, upstream API).

A `Healthy` status means "the contract's own subsystem state is configured",
not "every configured dependency is reachable".

## `get_anchor_readiness` — per-anchor service readiness

Returns `AnchorReadinessReport` for a given anchor (`src/contract.rs`):

| Field | Type | Meaning |
|---|---|---|
| `is_registered` | `bool` | The anchor is a registered attestor |
| `deposit_ready` | `bool` | `SERVICE_DEPOSITS` is in the anchor's configured services |
| `withdrawal_ready` | `bool` | `SERVICE_WITHDRAWALS` is in the anchor's configured services |
| `quote_ready` | `bool` | The anchor advertises `SERVICE_QUOTES` **and** holds a latest quote whose `valid_until > now` |
| `kyc_ready` | `bool` | `SERVICE_KYC` is in the anchor's configured services |
| `checked_at` | `u64` | Ledger timestamp when the report was generated |

`quote_ready` is the only field with an extra condition: the service must be
configured and a non-expired quote must exist on-chain.  The other service
flags only check membership in the configured services set.

What it does **not** check:

- Whether the anchor's HTTP endpoint is up or accepting traffic.  A
  `deposit_ready = true` report means "the contract records that this anchor is
  configured for deposits", not "the anchor is currently accepting deposits
  over the network".
- External connectivity, DNS, TLS, or the anchor's `stellar.toml`.

The boolean helpers `is_deposit_ready` and `is_quote_ready` expose the same
per-service checks individually.

## `get_rate_limiter_health` — rate limiter diagnostics

Returns `RateLimiterHealth` for a given attestor (`src/contract.rs`):

| Field | Type | Meaning |
|---|---|---|
| `submission_count` | `u32` | Effective submissions in the current window. `0` when the window has expired |
| `max_submissions` | `u32` | Configured per-window limit |
| `window_length` | `u32` | Sliding window length, in ledgers |
| `window_start_ledger` | `u32` | Ledger sequence at which the current window started |
| `is_throttled` | `bool` | `true` when the window has not expired and `submission_count >= max_submissions` |

Window expiry is `window_start_ledger + window_length <= current_ledger`.  An
expired window reports `submission_count = 0` and `is_throttled = false`.

What it does **not** check: any external dependency.  This is purely the
on-chain rate limiter's accounting for one attestor.

## `get_metadata_freshness` — metadata cache freshness

Returns `MetadataFreshnessReport` for a given anchor (`src/contract.rs`):

| Field | Type | Meaning |
|---|---|---|
| `state` | `MetadataCacheState` | `Missing`, `Fresh`, `Stale`, or `Expired` based on the entry's age versus its TTL and stale-while-revalidate window |
| `age_seconds` | `u64` | Age of the cached entry in seconds. `0` when the entry is missing |
| `needs_refresh` | `bool` | `true` when the entry is stale or the stored `needs_refresh` flag is set |
| `freshness_score` | `u32` | Confidence score in `[0, 100]` (linear age decay, halved when stale, minus 10 when `needs_refresh` is set) |

What it does **not** check: it inspects only the on-chain temporary metadata
cache.  It does not re-fetch metadata from the anchor and does not verify the
anchor is reachable.  `Missing`, `Stale`, or `Expired` describe cache state,
not anchor availability.

## On-chain health counters

`get_anchor_health` returns the cumulative `AnchorHealthMetrics` counters for
an anchor.  Those fields, the zero-event state, and reset behavior are
documented in [Health Metrics Reference](./health-metrics.md).

## External reachability is not implied

No query on this page performs an external call.  Treat every status as a
statement about on-chain contract state:

- `Healthy` from `get_health_status` does not mean any anchor endpoint is up.
- `deposit_ready = true` does not mean the anchor is accepting deposits now.
- `is_throttled = false` does not mean the anchor is responsive.
- `Fresh` metadata does not mean the anchor is reachable; it means the local
  cache entry is within its TTL.

To verify an anchor's HTTP endpoint is actually reachable, use the off-chain
monitoring loop and the proof-of-possession helpers in `src/anchor_health.rs`,
not any on-chain function.

## Diagnostic routing

Pick the query that matches the question:

- "Is the contract set up?" → `get_health_status` (or `is_initialized` for the
  initialization flag alone).
- "Is this anchor configured for a service?" → `get_anchor_readiness` (or
  `is_deposit_ready` / `is_quote_ready` for a single service).
- "Is this attestor being throttled?" → `get_rate_limiter_health`.
- "Is the cached metadata still usable?" → `get_metadata_freshness`.
- "Is the anchor's endpoint actually reachable?" → off-chain monitor in
  `src/anchor_health.rs`; no on-chain function answers this.

See [the runbook troubleshooting section](./RUNBOOK.md#troubleshooting-common-issues)
for where these fit during incident response.
