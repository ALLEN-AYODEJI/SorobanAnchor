# Service Management and Rollback

This document describes the service enable/disable toggles and rollback handling for anchor services in SorobanAnchor.

## Overview

The service management system allows anchors to:
- Enable/disable individual services without losing metadata
- Track service configuration history
- Rollback to previous service configurations
- Query current service status

## Service Types

SorobanAnchor supports the following service types:

| Service | Code | Description |
|---------|------|-------------|
| Deposits | 1 | Non-interactive deposits (SEP-6) |
| Withdrawals | 2 | Non-interactive withdrawals (SEP-6) |
| Quotes | 3 | Anchor RFQ and firm quotes (SEP-38) |
| KYC | 4 | Know-Your-Customer compliance |

## API Usage

### Enable a Service

```rust
use anchorkit::service_management::ServiceManager;

let anchor = Address::generate(&env);
let result = ServiceManager::enable_service(&env, &anchor, SERVICE_DEPOSITS);
assert!(result); // true if service was enabled, false if already enabled
```

### Disable a Service

```rust
let result = ServiceManager::disable_service(&env, &anchor, SERVICE_DEPOSITS);
assert!(result); // true if service was disabled, false if already disabled
```

Disabling a service first runs the existing dependent check
(`assert_no_active_dependents`). If the service is a prerequisite for other
services that are still enabled, the call fails and the service remains
enabled, so active dependents are never orphaned. Independent services and
already-disabled services are unaffected and keep their current behavior.

### Check Service Status

```rust
let is_enabled = ServiceManager::is_service_enabled(&env, &anchor, SERVICE_DEPOSITS);
if is_enabled {
    println!("Deposits are enabled");
}
```

### Get Current Service State

```rust
let state = ServiceManager::get_service_state(&env, &anchor);
println!("Enabled services: {}", state.enabled_services.len());
println!("Disabled services: {}", state.disabled_services.len());
println!("Last updated: {}", state.updated_at);
```

### Enable/Disable All Services

```rust
let all_services = vec![SERVICE_DEPOSITS, SERVICE_WITHDRAWALS, SERVICE_QUOTES, SERVICE_KYC];

// Enable all services
ServiceManager::enable_all_services(&env, &anchor, &all_services);

// Disable all services
ServiceManager::disable_all_services(&env, &anchor, &all_services);
```

## Configuration Snapshots

### Create a Snapshot

```rust
let services = vec![SERVICE_DEPOSITS, SERVICE_WITHDRAWALS];
let snapshot_id = ServiceManager::create_snapshot(
    &env,
    &anchor,
    &services,
    "before_maintenance",
);
```

### Retrieve a Snapshot

```rust
if let Some(snapshot) = ServiceManager::get_snapshot(&env, snapshot_id) {
    println!("Snapshot ID: {}", snapshot.snapshot_id);
    println!("Anchor: {}", snapshot.anchor);
    println!("Services: {}", snapshot.services.len());
    println!("Created at: {}", snapshot.created_at);
    println!("Description: {}", snapshot.description);
}
```

### Get Snapshot Count

```rust
let count = ServiceManager::get_snapshot_count(&env);
println!("Total snapshots: {}", count);
```

## Rollback Operations

### Rollback to a Snapshot

```rust
let success = ServiceManager::rollback_to_snapshot(&env, snapshot_id);
if success {
    println!("Rollback successful");
} else {
    println!("Snapshot not found");
}
```

### Rollback Workflow

```rust
// 1. Create snapshot before making changes
let snapshot_id = ServiceManager::create_snapshot(
    &env,
    &anchor,
    &current_services,
    "before_update",
);

// 2. Make service changes
ServiceManager::enable_service(&env, &anchor, SERVICE_QUOTES);
ServiceManager::disable_service(&env, &anchor, SERVICE_WITHDRAWALS);

// 3. If something goes wrong, rollback
if error_occurred {
    ServiceManager::rollback_to_snapshot(&env, snapshot_id);
}
```

## Lifecycle Outcomes

The lifecycle operations above return a small set of outcomes. The table below
maps each public outcome to the state it leaves behind and to how an operator
should read it. Only outcomes exercised by the existing tests are listed.

| Operation | Outcome | Resulting state | Operator interpretation |
|-----------|---------|-----------------|-------------------------|
| `enable_service` | `Ok(true)` | Service moved into `enabled_services` | Transition applied |
| `enable_service` | `Ok(false)` | `ServiceToggleState` unchanged | No-op: service already enabled |
| `enable_service` | `Err(ServiceInMaintenance)` | `ServiceToggleState` unchanged | Rejected transition: maintenance window is active |
| `disable_service` | `Ok(true)` | Service moved into `disabled_services` | Transition applied |
| `disable_service` | `Ok(false)` | `ServiceToggleState` unchanged | No-op: service already disabled |
| `disable_service` | `Err(...)` (dependent check) | Service remains enabled | Rejected transition: active dependents would be orphaned |
| `enable_all_services` / `disable_all_services` | per-service outcomes | Each service toggled individually | Mixed results possible; inspect `get_service_state` |
| `create_snapshot` | `ServiceConfigSnapshot` | New snapshot recorded | Snapshot available for rollback |
| `rollback_to_snapshot` | `true` | `ServiceToggleState` restored from snapshot | Rollback applied |
| `rollback_to_snapshot` | `false` | `ServiceToggleState` unchanged | Rejected rollback: snapshot invalid or incomplete |
| `is_service_enabled` | `false` | Service absent from `enabled_services` | Unavailable service (not a rejection) |

### Rejected Transition vs. Unavailable Service

These two conditions are distinct and must not be conflated:

- **Rejected transition** — the requested change was refused and nothing was
  mutated. Examples: `enable_service` returning `Err(ServiceInMaintenance)`
  while a maintenance window is active, `disable_service` failing the
  dependent check, or `rollback_to_snapshot` returning `false` because the
  snapshot is invalid or missing a prerequisite. The prior state is preserved.
- **Unavailable service** — the service is simply not currently enabled, as
  reported by `is_service_enabled` returning `false` or by the service
  appearing in `disabled_services`. No operation was attempted or rejected;
  the service is in its normal disabled state.

### Rollback Outcomes

`rollback_to_snapshot` returns a `bool`. A `true` result means the
`ServiceToggleState` was restored from the snapshot. A `false` result means the
rollback was rejected before any mutation: the snapshot is invalid or
incomplete (for example, a prerequisite service is missing). In both cases the
current `ServiceToggleState` can be inspected with `get_service_state`.

## Data Structures

### ServiceToggleState

Represents the current service state for an anchor:

```rust
pub struct ServiceToggleState {
    pub anchor: Address,              // Anchor address
    pub enabled_services: Vec<u32>,   // Currently enabled services
    pub disabled_services: Vec<u32>,  // Currently disabled services
    pub updated_at: u64,              // Last update timestamp
}
```

### ServiceConfigSnapshot

Represents a saved service configuration:

```rust
pub struct ServiceConfigSnapshot {
    pub snapshot_id: u64,             // Unique identifier
    pub anchor: Address,              // Anchor address
    pub services: Vec<u32>,           // Services in this snapshot
    pub created_at: u64,              // Creation timestamp
    pub description: String,          // Description of snapshot
}
```

## Use Cases

### Maintenance Window

```rust
// Before maintenance
let snapshot_id = ServiceManager::create_snapshot(
    &env,
    &anchor,
    &current_services,
    "pre_maintenance",
);

// Disable services during maintenance
ServiceManager::disable_all_services(&env, &anchor, &all_services);

// After maintenance
ServiceManager::rollback_to_snapshot(&env, snapshot_id);
```

### Service Upgrade

```rust
// Create snapshot before upgrade
let snapshot_id = ServiceManager::create_snapshot(
    &env,
    &anchor,
    &current_services,
    "pre_upgrade",
);

// Disable service for upgrade
ServiceManager::disable_service(&env, &anchor, SERVICE_DEPOSITS);

// Perform upgrade...

// Re-enable service
ServiceManager::enable_service(&env, &anchor, SERVICE_DEPOSITS);

// If upgrade fails, rollback
if upgrade_failed {
    ServiceManager::rollback_to_snapshot(&env, snapshot_id);
}
```

### Gradual Rollout

```rust
// Start with limited services
let initial_services = vec![SERVICE_DEPOSITS];
ServiceManager::enable_all_services(&env, &anchor, &initial_services);

// Create snapshot
let snapshot_id = ServiceManager::create_snapshot(
    &env,
    &anchor,
    &initial_services,
    "phase_1",
);

// Gradually enable more services
ServiceManager::enable_service(&env, &anchor, SERVICE_WITHDRAWALS);
ServiceManager::enable_service(&env, &anchor, SERVICE_QUOTES);

// If issues arise, rollback to phase 1
if issues_detected {
    ServiceManager::rollback_to_snapshot(&env, snapshot_id);
}
```

### Emergency Disable

```rust
// If a service has issues, disable it immediately
ServiceManager::disable_service(&env, &anchor, SERVICE_DEPOSITS);

// Create snapshot for investigation
let snapshot_id = ServiceManager::create_snapshot(
    &env,
    &anchor,
    &current_services,
    "emergency_disable",
);

// Later, when issue is resolved, re-enable
ServiceManager::enable_service(&env, &anchor, SERVICE_DEPOSITS);
```

## Best Practices

### 1. Always Create Snapshots Before Major Changes

```rust
// Good
let snapshot_id = ServiceManager::create_snapshot(&env, &anchor, &services, "before_change");
make_service_changes(&env, &anchor);

// Bad - no snapshot
make_service_changes(&env, &anchor);
```

### 2. Use Descriptive Snapshot Descriptions

```rust
// Good
ServiceManager::create_snapshot(&env, &anchor, &services, "before_maintenance_2024-06-01");

// Bad
ServiceManager::create_snapshot(&env, &anchor, &services, "snap1");
```

### 3. Test Rollback Procedures

```rust
// Test that rollback works
let snapshot_id = ServiceManager::create_snapshot(&env, &anchor, &services, "test");
ServiceManager::disable_service(&env, &anchor, SERVICE_DEPOSITS);
assert!(ServiceManager::rollback_to_snapshot(&env, snapshot_id));
assert!(ServiceManager::is_service_enabled(&env, &anchor, SERVICE_DEPOSITS));
```

### 4. Monitor Service State Changes

```rust
// Log service state changes
let old_state = ServiceManager::get_service_state(&env, &anchor);
ServiceManager::enable_service(&env, &anchor, SERVICE_QUOTES);
let new_state = ServiceManager::get_service_state(&env, &anchor);

log_state_change(&old_state, &new_state);
```

### 5. Coordinate with Admin Audit Log

```rust
use anchorkit::admin_audit_log::AdminAuditLog;

// Log service changes in audit log
AdminAuditLog::log_change(
    &env,
    &admin,
    "service_toggle",
    &anchor.to_string(),
    "deposits_enabled",
    "deposits_disabled",
);
```

## Troubleshooting

### Service State Not Updating

**Problem**: Service state changes are not persisting.

**Solutions**:
1. Verify `enable_service()` or `disable_service()` returns `true`
2. Check that the anchor address is correct
3. Verify storage is not full

### Rollback Not Working

**Problem**: Rollback does not restore the previous state.

**Solutions**:
1. Verify the snapshot ID is valid
2. Check that the snapshot was created before the changes
3. Ensure the snapshot contains the expected services

### Blank Descriptions Rejected

**Problem**: `schedule_window` rejects a maintenance description.

**Solutions**:
1. Descriptions must not be empty after trimming whitespace
2. Provide a nonblank description (e.g. `"scheduled maintenance"`)
3. Nonblank descriptions are preserved exactly as provided
