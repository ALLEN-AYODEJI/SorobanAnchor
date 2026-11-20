# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for the AnchorKit project.

An ADR captures a significant architectural decision made by the project team, including the context, the decision, and the consequences.

## ADR Index

| ID | Title | Status |
|----|-------|--------|
| [ADR-0001](ADR-0001-record-architecture-decisions.md) | Record Architecture Decisions | Accepted |
| [ADR-0002](ADR-0002-dual-build-strategy.md) | Dual-Build Strategy | Accepted |
| [ADR-0003](ADR-0003-contract-lifecycle-and-schema-versioning.md) | Contract Lifecycle and Schema Versioning | Accepted |

## What is an ADR?

An Architecture Decision Record is a short document that captures:

- **Context** — the forces and circumstances that led to the decision
- **Decision** — what was decided and why alternatives were rejected
- **Consequences** — the trade-offs, risks, and follow-up work that result

## When to write an ADR

Write an ADR when a change alters how the system is designed or how its parts depend on each other. Examples include changing the build separation, the contract lifecycle, or the schema versioning approach. The format is described in [ADR-0001](ADR-0001-record-architecture-decisions.md).

Do not write an ADR for routine work, such as:

- adding or fixing tests,
- editing documentation,
- routine dependency or CI updates,
- bug fixes that keep the existing design.

If you are unsure, open an issue first and ask whether the change needs an ADR.

When an ADR is needed, link it from the implementation issue and the pull request. Use the issue to discuss the proposal, and use the ADR to record the decision once it is agreed. Each ADR should cover the **context** (what forces led to the decision), the **decision** (what was chosen), and the **consequences** (the trade-offs and follow-up work).

## How to write a new ADR

1. Copy `ADR-0000-template.md` to `ADR-XXXX-title.md`
2. Fill in the sections
3. Update this index
4. Open a PR for review

## Status meanings

| Status | Meaning |
|--------|---------|
| Proposed | Under discussion |
| Accepted | Agreed and implemented |
| Deprecated | Superseded by a later ADR |
| Superseded | Replaced by a newer ADR |
