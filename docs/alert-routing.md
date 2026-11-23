# Alert Route Selection and Failure Visibility

This document describes the routing behavior implemented by
[`AlertRouter`](../src/alert_routing.rs). It covers destination selection only;
AnchorKit does not send webhooks, email, or other external notifications.

## Configured route selection

`AlertRouterConfig::from_monitoring_config()` builds routes from
`monitoring.alerts`:

| Alert field | Router field | Matching behavior |
|---|---|---|
| `severity` | `AlertRule.severity` | Parsed exactly as `info`, `warning`, `error`, or `critical` |
| `condition` | `AlertRule.scope` | Exact match when non-empty; an empty condition matches every scope |
| `recipients` | `AlertRule.routes` | Each non-empty recipient becomes a webhook route |

The rules are evaluated in configuration order. Every matching rule contributes
its routes, and the combined list is deduplicated by `(channel, destination)`
while preserving first-match order. A scope-aware rule does not match an alert
with no scope, while a scope-free rule matches both scoped and unscoped alerts.

This mapping is covered by the `from_monitoring_config_accepts_known_severities`,
`from_monitoring_config_rejects_unknown_severity`,
`from_monitoring_config_rejects_blank_severity`, and
`from_monitoring_config_fills_routes_from_recipients` tests.

## Fallback behavior

If no rule matches, `AlertRouter::route()` returns the configured
`default_routes`. This is a fallback for routing selection, not a retry or
delivery mechanism.

- With at least one default route, an unmatched alert is routed to those
  destinations.
- With no default routes, the router returns an empty `Vec` and
  `has_route()` returns `false`.
- The router never invents an implicit destination.

These outcomes are covered by `unknown_scope_falls_back_to_default`,
`missing_category_without_default_returns_empty_and_is_visible`, and
`missing_category_falls_back_to_default_routes`. The first test covers a
configured catch-all default, while the second covers an absent fallback and
the caller-visible `has_route()` signal.

Monitoring configuration currently constructs rules from `monitoring.alerts`
and provides an empty `default_routes` list. Operators can supply default routes
only through direct `AlertRouterConfig` construction. Therefore, an unmatched
severity/condition pair in monitoring configuration has no route unless the
application constructs a router with an explicit default.

## Routing failure versus delivery outcome

Keep these states separate:

| State | Meaning | Visibility |
|---|---|---|
| Route selected | At least one configured destination was selected | Non-empty `route()` result; `has_route()` is `true` |
| Route unavailable | No rule or configured default selected a destination | Empty `route()` result; `has_route()` is `false` |
| Delivery successful | A host process successfully completed delivery to a channel | Not reported by `AlertRouter` |
| Delivery failed | A host process could not deliver to a destination | Not reported by `AlertRouter` |

The router does not attempt delivery and cannot distinguish an unreachable
webhook, rejected email, or unavailable PagerDuty integration from an otherwise
valid route. Host code is responsible for implementing channel delivery,
retry policy, and delivery-result reporting. A selected route is therefore not
a delivery guarantee.

Routing misses are not automatically logged or sent by this module. Callers
should check `has_route()` after routing and log or escalate an empty result.
This visibility contract is tested by
`missing_category_without_default_returns_empty_and_is_visible` and
`no_match_and_no_default_returns_empty`.
