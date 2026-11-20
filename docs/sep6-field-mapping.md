# SEP-6 Field Mapping

This page maps the raw anchor response types in `src/sep6.rs` to the normalized
types the library returns. It covers the three flows: deposit, withdrawal, and
transaction status. It describes only what the code validates and converts.
It does not promise that any anchor supports a given field.

Callers fill the `Raw*` structs from the anchor's JSON. The `initiate_deposit`,
`initiate_withdrawal`, `fetch_transaction_status`, and `list_transactions`
functions return the normalized types.

## Deposit: `RawDepositResponse` → `DepositResponse`

Entry point: `initiate_deposit`.

| Raw field | Normalized field | Requirement and behavior |
|---|---|---|
| `transaction_id` | `transaction_id` | Required. Rejected if empty after trimming. |
| `how` | `how` | Required. Rejected if empty. Not trimmed. |
| `extra_info` | `extra_info` | Optional. Passed through unchanged. |
| `min_amount` | `min_amount` | Optional. Must be `<= max_amount` when both are set. |
| `max_amount` | `max_amount` | Optional. See `min_amount`. |
| `fee_fixed` | `fee_fixed` | Optional. Passed through unchanged. |
| `status` | `status` | Optional. Missing or whitespace-only → `Pending`. Otherwise `TransactionStatus::from_str`. See [status normalization](sep6-status-normalization.md). |
| `clawback_enabled` | `clawback_enabled` | Optional. Passed through unchanged. |
| `stellar_memo` | `stellar_memo` | Optional. Requires `stellar_memo_type` when present. |
| `stellar_memo_type` | `stellar_memo_type` | Must be `text`, `id`, or `hash` when a memo is present. |
| `asset_code` | `asset_code` | Optional. Normalized: trimmed, ASCII-alphanumeric only, at most 12 characters, uppercased. Otherwise rejected. |

Errors are all `Error::invalid_transaction_intent()`, except for invalid asset
codes, which return the asset-code error.

Tests: `test_initiate_deposit_normalizes_response`,
`test_initiate_deposit_missing_fields_returns_error`,
`test_initiate_deposit_blank_transaction_id_rejected`,
`test_initiate_deposit_defaults_status_to_pending`,
`test_deposit_memo_without_memo_type_is_rejected`,
`test_initiate_deposit_min_amount_gt_max_amount_rejected`.

## Withdrawal: `RawWithdrawalResponse` → `WithdrawalResponse`

Entry point: `initiate_withdrawal`.

| Raw field | Normalized field | Requirement and behavior |
|---|---|---|
| `transaction_id` | `transaction_id` | Required. Rejected if empty after trimming. |
| `account_id` | `account_id` | Required. Rejected if empty. |
| `memo` | `memo` | Optional. Requires `memo_type` when present. |
| `memo_type` | `memo_type` | Must be `text`, `id`, or `hash` when a memo is present. |
| `min_amount` | `min_amount` | Optional. Must be `<= max_amount` when both are set. |
| `max_amount` | `max_amount` | Optional. See `min_amount`. |
| `fee_fixed` | `fee_fixed` | Optional. Passed through unchanged. |
| `status` | `status` | Same rule as deposit: missing or blank → `Pending`. |
| `asset_code` | `asset_code` | Same rule as deposit. |

Withdrawal has no `extra_info` or `clawback_enabled` fields.

Tests: `test_initiate_withdrawal_normalizes_response`,
`test_initiate_withdrawal_missing_account_returns_error`,
`test_initiate_withdrawal_blank_transaction_id_rejected`,
`test_initiate_withdrawal_empty_status_defaults_to_pending`,
`test_withdrawal_memo_without_memo_type_is_rejected`,
`test_initiate_withdrawal_min_amount_gt_max_amount_rejected`.

## Transaction status: `RawTransactionResponse` → `TransactionStatusResponse`

Entry point: `fetch_transaction_status`. `list_transactions` applies the same
mapping to a list.

| Raw field | Normalized field | Requirement and behavior |
|---|---|---|
| `transaction_id` | `transaction_id` | Required. `fetch_transaction_status` rejects it if empty after trimming. `list_transactions` silently drops such entries. |
| `kind` | `kind` | Optional. `TransactionKind::from_str`: `withdrawal` or `withdraw` → `Withdrawal`. Any other value, or a missing value, → `Deposit`. |
| `status` | `status` | **Required string.** Empty or whitespace-only → `TransactionStatus::Error` in `fetch_transaction_status`. Otherwise `from_str`. `list_transactions` does not check for empty status. |
| `amount_in` | `amount_in` | Optional. Passed through unchanged. |
| `amount_out` | `amount_out` | Optional. Passed through unchanged. |
| `amount_fee` | `amount_fee` | Optional. Passed through unchanged. |
| `message` | `message` | Optional. Passed through unchanged. |

Note the difference in empty-status handling. On deposit and withdrawal, an
empty status defaults to `Pending`. On transaction status, it becomes `Error`.

Tests: `test_fetch_transaction_status_normalizes_response`,
`test_fetch_transaction_status_missing_id_returns_error`,
`test_fetch_transaction_status_blank_id_returns_error`,
`test_fetch_transaction_status_empty_status_maps_to_error`,
`test_withdrawal_kind_normalization`,
`test_list_transactions_normalizes_all`,
`test_list_transactions_skips_empty_ids`.

## Amount strings: `parse_sep6_amount`

The `Raw*` structs take `u64` amounts. Anchors send amounts as JSON strings, so
callers should convert them first with `parse_sep6_amount`. The normalizers do
not call it.

- Accepted: digits, optionally with a fractional part of only zeros (`"10.0"` → `10`).
- Rejected: a leading sign, a non-zero fraction, non-numeric text, an empty string, or values that overflow `u64`.
