# SEP-6 Status Normalization

Anchors may send status strings that differ in case, spacing, or wording from
the library's canonical SEP-6 statuses. This page explains how those strings
are mapped. The source of truth is the code in `src/sep6.rs`, and the tests
listed below check it.

## Rules that apply to every status string

1. The input is **trimmed and lowercased** (`normalize_status_token`). `"  COMPLETED  "` and `"completed"` are the same status.
2. Matching is then exact. Unlisted spellings are **not** recognized. The library does not fuzzy-match, strip punctuation, or translate vendor synonyms.

Two functions read status strings, and they handle unknown values differently:

| Function | Returns | Unrecognized input |
|---|---|---|
| `TransactionStatus::from_str` | a `TransactionStatus` variant | `TransactionStatus::Error` |
| `classify_status_str` | a `StatusCategory` | `StatusCategory::Unknown` |

Unknown values are never reported as `Completed`. Note that `"error"` itself is a
known status. It maps to `TransactionStatus::Error` and `StatusCategory::Failed`.
An unrecognized string also maps to `TransactionStatus::Error`, but it is
classified as `StatusCategory::Unknown`. The two are different.

## Examples

| Raw string (after trim/lowercase) | `TransactionStatus` | `StatusCategory` |
|---|---|---|
| `pending` | `Pending` | `Active` |
| `incomplete` | `Incomplete` | `Active` |
| `pending_external` | `PendingExternal` | `Active` |
| `pending_anchor` | `PendingAnchor` | `Active` |
| `pending_trust` | `PendingTrust` | `Active` |
| `pending_user` | `PendingUser` | `Active` |
| `pending_user_transfer_start` | `PendingUser` | `Active` |
| `pending_user_transfer_complete` | `PendingUser` | `Active` |
| `pending_stellar` | `PendingStellar` | `Active` |
| `waiting_customer_action` | `WaitingCustomerAction` | `Active` |
| `pending_customer_info_update` | `WaitingCustomerAction` | `Active` |
| `completed` | `Completed` | `Completed` |
| `refunded` | `Refunded` | `Refunded` |
| `expired` | `Expired` | `Expired` |
| `no_market` | `NoMarket` | `Failed` |
| `too_small` | `TooSmall` | `Failed` |
| `too_large` | `TooLarge` | `Failed` |
| `error` | `Error` | `Failed` |
| `  COMPLETED  ` | `Completed` | `Completed` |
| `ach_processing` (not registered) | `Error` | `Unknown` |
| `""` or whitespace (in `fetch_transaction_status`) | `Error` | n/a |

For deposit and withdrawal responses, an empty or whitespace-only status
defaults to `Pending`, not `Error`. See [SEP-6 Field Mapping](sep6-field-mapping.md).

The table is a sample, not a full list. The complete set of accepted strings is
in the `match` arms of `classify_status_str` and `TransactionStatus::from_str`.

## Vendor-specific aliases

A vendor-specific spelling should be registered with `VendorStatusMap` instead
of being added to the core parser:

```rust
use anchorkit::sep6::{VendorStatusMap, TransactionStatus};

let mut map = VendorStatusMap::new();
map.register("ach_processing", TransactionStatus::PendingExternal);

let r = map.resolve("ach_processing");
assert_eq!(r.canonical, TransactionStatus::PendingExternal);
assert_eq!(r.vendor_status, "ach_processing"); // raw value kept

let unknown = map.resolve("some_other_status");
assert_eq!(unknown.canonical, TransactionStatus::Error); // falls back to from_str
```

`resolve` checks the vendor map first, using the trimmed and lowercased key. It
falls back to `TransactionStatus::from_str` only when no alias is registered.
The raw string is always kept in `vendor_status`.

Registering an alias does not change `classify_status_str`. If a vendor alias
should also be categorized, check that the category logic produces the expected
result.

## Maintainers: when adding an alias

When you add a new alias or canonical status:

1. Add the spelling to the matching arm in `TransactionStatus::from_str`, or register it in the vendor map, as appropriate.
2. Add the matching arm in `classify_status_str` if its category matters to callers.
3. Update the mapping tests in `src/sep6.rs`. They are the reference for accepted spellings:
   - `test_classify_status_str_all_categories`
   - `test_classify_status_str_normalizes_input`
   - `test_transaction_status_classify_active`, `_completed`, `_refunded`, `_expired`, `_failed`
   - `test_fetch_transaction_status_unknown_status_maps_to_error`
4. Do not assume every vendor spelling is recognized. Only the strings listed in the code and tests are supported.
