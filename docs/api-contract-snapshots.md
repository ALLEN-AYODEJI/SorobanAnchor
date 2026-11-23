# API Contract Snapshots

API contract snapshots capture the public surface of AnchorKit at a point in
time so that regressions — removed functions, changed signatures, altered error
codes — are caught before a release ships.

---

## Table of Contents

1. [What a snapshot contains](#1-what-a-snapshot-contains)
2. [Generating a snapshot](#2-generating-a-snapshot)
3. [Comparing snapshots](#3-comparing-snapshots)
4. [Snapshot storage convention](#4-snapshot-storage-convention)
5. [CI integration](#5-ci-integration)
6. [Reviewing a snapshot change](#6-reviewing-a-snapshot-change)
7. [What counts as a breaking change](#7-what-counts-as-a-breaking-change)

---

## 1  What a snapshot contains

A snapshot is a plain-text file produced by `cargo rustdoc --output-format json`
(rustdoc JSON) trimmed to the public API surface. It records:

- All `pub` functions, their signatures, and doc comments
- All `pub` types (structs, enums, type aliases) and their fields/variants
- All `pub` trait definitions and their required methods
- All stable error codes from `src/errors.rs`
- The `supported_seps` list from `src/contract.rs`

It intentionally excludes private items, test modules, and build metadata.

---

## 2  Generating a snapshot

### Prerequisites

```bash
# rustdoc JSON output requires nightly for the --output-format json flag
rustup install nightly
rustup component add rust-docs --toolchain nightly
```

### Generate

```bash
# From the repo root
cargo +nightly rustdoc --lib -- \
    -Z unstable-options \
    --output-format json \
    --document-private-items=false

# The JSON lands at:
#   target/doc/anchorkit.json
```

Then extract just the stable public surface into the snapshot format:

```bash
./scripts/snapshot_api.sh
# Writes: api_snapshots/anchorkit-<VERSION>.json
```

`scripts/snapshot_api.sh` calls `rustdoc` as above, then uses `jq` to strip
internal implementation details and write a deterministic, sorted JSON file
under `api_snapshots/`.

### Snapshot script (snapshot_api.sh)

```bash
#!/usr/bin/env bash
set -euo pipefail

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*= *"\(.*\)"/\1/')
OUT="api_snapshots/anchorkit-${VERSION}.json"

mkdir -p api_snapshots

cargo +nightly rustdoc --lib -- \
    -Z unstable-options \
    --output-format json \
    --document-private-items=false 2>/dev/null

# Extract public surface: functions, types, traits — sorted for stable diffs
jq '
  .index
  | to_entries
  | map(select(.value.visibility == "public"))
  | map({
      id:   .key,
      name: .value.name,
      kind: .value.kind,
      docs: .value.docs
    })
  | sort_by(.name)
' target/doc/anchorkit.json > "$OUT"

echo "Snapshot written to $OUT"
```

---

## 3  Comparing snapshots

To diff two snapshots side by side:

```bash
# Compare current version against the previous release
./scripts/diff_api_snapshot.sh api_snapshots/anchorkit-0.1.0.json \
                                api_snapshots/anchorkit-0.2.0.json
```

The comparison is read-only. Review the listed removed and added names in the
output; exit status `1` means a breaking removal was found, while status `0`
means no removed names were found (additions are non-breaking). To inspect the
full snapshot contents, use `git diff --no-index` with the two JSON paths.

Or compare the working tree against a tagged release:

```bash
# Generate a snapshot for the current working tree
./scripts/snapshot_api.sh

# Diff against the last tagged snapshot
PREV=$(ls api_snapshots/ | sort -V | tail -2 | head -1)
CURR=$(ls api_snapshots/ | sort -V | tail -1)
./scripts/diff_api_snapshot.sh "api_snapshots/$PREV" "api_snapshots/$CURR"
```

The diff script highlights:

| Symbol | Meaning |
|--------|---------|
| `+` | New public item (additive — not breaking) |
| `-` | Removed public item (breaking) |
| `~` | Changed signature or doc (potentially breaking) |

### Diff script (diff_api_snapshot.sh)

```bash
#!/usr/bin/env bash
set -euo pipefail

OLD=$1
NEW=$2

echo "=== Removed or changed items (potentially breaking) ==="
diff <(jq -r '.[].name' "$OLD" | sort) \
     <(jq -r '.[].name' "$NEW" | sort) \
  | grep '^<' | sed 's/^< /  - /'

echo ""
echo "=== Added items ==="
diff <(jq -r '.[].name' "$OLD" | sort) \
     <(jq -r '.[].name' "$NEW" | sort) \
  | grep '^>' | sed 's/^> /  + /'
```

---

## 4  Snapshot storage convention

```
api_snapshots/
  anchorkit-0.1.0.json    # snapshot for each tagged release
  anchorkit-0.2.0.json
  ...
```

- One file per release tag, named `anchorkit-<VERSION>.json`.
- Committed to the repository so historical comparisons work without fetching
  old tags.
- The snapshot for the current development HEAD is not committed; it is
  generated on demand or in CI.

---

## 5  CI integration

Add a step to the CI workflow to catch API regressions on every PR against
`main`:

```yaml
# .github/workflows/ci.yml  — api-snapshot job
- name: Generate API snapshot
  run: bash scripts/snapshot_api.sh

- name: Compare against baseline
  run: |
    BASELINE=$(ls api_snapshots/ | sort -V | tail -1)
    CURRENT=api_snapshots/anchorkit-current.json
    bash scripts/snapshot_api.sh
    mv api_snapshots/anchorkit-$(grep '^version' Cargo.toml | \
        head -1 | sed 's/.*= *"\(.*\)"/\1/').json "$CURRENT"
    bash scripts/diff_api_snapshot.sh "api_snapshots/$BASELINE" "$CURRENT"
```

The job passes as long as no public items are removed or have their signatures
changed. Additive changes always pass.

---

## 6  Reviewing a snapshot change

Regenerating a snapshot rewrites the file in place, so a committed snapshot
diff can hide an accidental public API change unless you inspect it. Always
follow the generate-then-diff order below and review the result before
committing.

### Step 1 — Generate the snapshot for the current tree

```bash
./scripts/snapshot_api.sh
# Writes: api_snapshots/anchorkit-<VERSION>.json
```

The version is read from `Cargo.toml`, so the output path is deterministic.

### Step 2 — Diff against the baseline snapshot

```bash
./scripts/diff_api_snapshot.sh api_snapshots/anchorkit-<OLD>.json \
                              api_snapshots/anchorkit-<NEW>.json
```

The script takes the old snapshot first and the new snapshot second. Its exit
status is the review signal:

| Exit status | Meaning |
|-------------|---------|
| `0` | No removed names found — additions only, non-breaking |
| `1` | Removed names found — potentially breaking, review required |

### Step 3 — Interpret the output

The diff script prints two sections:

- **Removed or changed items** (`-` lines): names present in the old snapshot
  but missing from the new one. Each one is a candidate breaking change — a
  removed function, type, trait, or error code.
- **Added items** (`+` lines): names present only in the new snapshot. These
  are additive and non-breaking.

For anything listed under removed items, inspect the full snapshot with
`git diff --no-index` (or `git diff` on the committed file) to see whether a
signature, field, or error code changed rather than a whole item disappearing.

### Step 4 — Classify the change

- **Accidental change:** the removed or changed item was not intended. Do not
  commit the regenerated snapshot. Fix the code so the public surface matches
  the baseline, then regenerate and re-diff.
- **Intentional change:** the public API change is deliberate. Confirm it
  against section 7, note it in the changelog under `### Breaking Changes` if
  it is breaking, and get the required maintainer approvals before committing
  the snapshot.

> **Do not commit a regenerated snapshot without reviewing the diff.** A
> snapshot that is simply regenerated and committed will pass CI while silently
> accepting whatever public API change produced it. The snapshot is only
> trustworthy if the diff was read and the public API impact was understood.

---

## 7  What counts as a breaking change

| Change | Breaking |
|--------|---------|
| Removing a `pub` function or type | Yes |
| Changing a function signature (parameters, return type) | Yes |
| Renumbering an error code in `errors.rs` | Yes |
| Adding a required field to a `pub` struct | Yes |
| Adding a new `pub` function or type | No |
| Adding an optional/defaulted field | No |
| Changing doc comments only | No |
| Adding a new error code at the end | No |

Breaking changes require two maintainer approvals and must be noted in the
changelog under a `### Breaking Changes` heading.

---

## References

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [governance-and-security.md](governance-and-security.md)
- [error-codes.md](error-codes.md)
- [changelog-generation.md](changelog-generation.md)
- [ONBOARDING.md](ONBOARDING.md)
