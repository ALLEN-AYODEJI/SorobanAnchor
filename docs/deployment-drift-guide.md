# Deployment Drift Report Guide

This guide explains how to read a deployment drift report produced by the drift
detection module (`deployment_drift.rs`) and how to triage the differences it
reports. It describes only the fields and comparison scope that are actually
implemented. It does **not** claim to detect configuration differences outside
those fields.

## What drift detection compares

Drift detection compares an **expected** deployment configuration against an
**observed** deployment configuration. The comparison scope is limited to:

- **Contract version** — the expected contract version versus the observed one.
- **Network** — the expected network identifier versus the observed one.
- **Admin address** — the expected admin address versus the observed one.
- **Expected config entries** — each expected configuration entry is compared
  against the corresponding observed entry.

Differences outside these fields are **not** detected by this module. For
example, drift detection does not inspect on-chain state, storage layout,
balances, event history, or any configuration key that is not part of the
expected config entries. If you need to verify something outside this scope,
use a separate check; do not assume a clean report means the deployment is
identical in every respect.

## Report fields

A drift report (`DriftReport`) contains:

| Field | Meaning |
| --- | --- |
| `is_clean` | `true` when no drift items were found; `false` when at least one difference was detected. |
| `environment` | The environment the report was produced for (for example, the deployment environment name). |
| `snapshot_captured_at` | Timestamp indicating when the observed snapshot was captured. |
| `items` | The list of detected differences (`DriftItem`). Empty when `is_clean` is `true`. |

Each drift item (`DriftItem`) contains:

| Field | Meaning |
| --- | --- |
| `field` | The configuration field that differs (for example, contract version, network, admin address, or a config entry key). |
| `expected` | The value from the expected configuration. |
| `observed` | The value found in the observed configuration. |
| `severity` | The severity assigned to this difference (`DriftSeverity`). |

### Severity levels

`DriftSeverity` classifies how important a difference is. Reports can be
filtered with `items_at_or_above`, which returns only the items whose severity
is at or above a given level. Use this to focus on the differences that matter
most for your situation before working through the rest.

## The `deployment.drift_detected` event

When drift is detected through the logged detection path
(`detect_drift_logged`), a structured event named `deployment.drift_detected`
is emitted. This event records that drift was found so it can be observed by
logging and monitoring. A clean report does not emit this event.

## Triage sequence

When a drift report shows differences, work through these steps in order. The
first steps are read-only; remediation is planned only after the difference is
understood.

1. **Read the report.** Check `is_clean`, `environment`, and
   `snapshot_captured_at`. Confirm the report is for the environment and time
   window you expect. If `is_clean` is `true`, there are no differences within
   the implemented comparison scope.
2. **Filter by severity.** Use `items_at_or_above` to narrow the `items` list to
   the severities that require attention. Start with the highest severity.
3. **Compare expected vs observed per item.** For each remaining item, read
   `field`, `expected`, and `observed` together. Understand exactly what
   changed before deciding what to do.
4. **Confirm whether the difference is intended.** Check whether the observed
   value reflects a deliberate change (for example, a planned upgrade or a
   configuration update). Consult the release and upgrade procedures linked
   below. If the difference is intended, record that; if it is not, treat it as
   unexpected drift.
5. **Plan a reviewable remediation.** Only after the difference is understood,
   plan the fix. Remediation should be non-destructive and reviewable: prepare
   the change, have it reviewed, and apply it through the normal change
   procedure rather than making ad-hoc or irreversible edits.

## Related procedures

- [Release checklist](release-checklist.md) — steps to follow when preparing and
  verifying a release.
- [Upgrade playbook](upgrade-playbook.md) — the procedure for performing an
  upgrade, including how intended configuration changes are made and reviewed.

Use these procedures when a triaged difference turns out to be an intended
change, or when you need to plan a reviewable remediation for unexpected drift.