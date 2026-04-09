# AGENTS

## Purpose

This file manages two things only:

- the stable role of `santi/` as the core runtime layer beneath the repo-root product and deployment boundary
- core constraints that should stay stable while the system evolves
- key file indexes for the most important design documents

Detailed system thinking belongs in `docs/`, not here.

## Core Constraints

- We are building a customized personal agent service, not recreating `opencode` or `openclaw`.
- Reference projects are used to extract reusable principles, not implementation parity.
- Explore external projects only when needed by the current task.
- Sync back only durable knowledge that affects product, architecture, safety, or operating decisions.
- Prefer a small number of stable primitives over early abstraction growth.
- Keep core traits narrow: define only the atomic boundary capabilities runtime truly depends on.
- If an interface can be recomposed from lower-level atomic operations with near-zero quality loss, keep it out of the core trait and implement that composition in runtime instead.
- When the local optimum is already known, prefer the fastest direct refactor over compatibility-preserving transition layers.
- Drop obsolete structure quickly: do not preserve backward compatibility inside `santi/` unless the user explicitly asks for it.
- Do not carry shims, aliases, or temporary adapter layers longer than needed for one focused refactor step.
- Prefer end-state-first refactors: move directly toward the target boundary instead of stretching work across many compatibility phases.
- Use full-path smoke validation to correct course quickly after invasive refactors; prefer fast end-to-end verification over elaborate backward-compat scaffolding.
- `core` defines the atomic boundary traits required by runtime.
- `middleware` implements those core traits through adapters.
- `runtime` composes those traits and owns orchestration, concurrency, and business composition.
- `santi` is always the single HTTP service host; 中文 `单机` vs `分布式` is an assembly/dependency topology distinction, not a service-boundary distinction.
- Until the user explicitly says otherwise, all active runtime iteration should target standalone only; do not spend implementation effort on distributed-mode iteration while the standalone architecture is still being tightened.
- `session` is the public shared ledger container and a work unit, not a security boundary.
- HTTP capabilities are currently open; `scope` / `tenant` comes later.
- `soul_dir` and `session_dir` are normal directories used as unified agent resource spaces.
- Testing should start from executable smoke and integration checks on the main path, add focused `crates/santi-api/tests` only where those checks reveal weak spots, and keep tracing strong enough to diagnose known classes of failure.
- Tests must target non-real model API call scenarios only; any verification that depends on real API calls belongs in docs or runbooks, not automated test logic or scripts.
- concurrent `session/send` on the same session is a fail-fast conflict: return `409`, do not queue, silently serialize, or retry implicitly.

## Reference Project Index

### `opencode`

- Role: reference for coding-agent behavior, tool orchestration, and workspace-oriented UX
- Repo path: `/Users/zqxy123/Projects/giants.ai/opencode`

### `openclaw`

- Role: reference for persistent personal-agent framing, gateway thinking, and extensibility
- Repo path: `/Users/zqxy123/Projects/giants.ai/openclaw`

## Key File Index

- `AGENTS.md`: stable constraints and file index
- `docs/README.md`: docs structure map and core bucket guidance
- `docs/operations/documentation.md`: must-read docs update guide, canonical-source rule, and anti-duplication process
- `docs/architecture/overview.md`: top-level runtime model overview and design principles
- `docs/architecture/runtime/glossary.md`: current core object model and primitive definitions
- `docs/architecture/layers/principles.md`: durable layering and ownership rules
- `docs/architecture/runtime/lifecycle-and-hooks.md`: soul/session lifecycle, hook points, and reload boundary
- `docs/operations/local-dev/setup.md`: local development baseline and smoke entrypoints
- `scripts/verify.sh`: workspace verify entrypoint; runs no-skips, fmt check, and locked workspace tests
- `scripts/package.sh`: release packaging entrypoint for a target triple; writes archives to `dist/`
- `scripts/verify/no-skips.sh`: fast guard that fails on skipped tests
- `docs/operations/local-dev/verification.md`: cold-start operational verification flow for common runtime smoke checks
- `docs/operations/testing.md`: test-construction standard for choosing smoke, integration, and focused regression coverage
- `docs/operations/local-dev/troubleshooting.md`: local troubleshooting notes for common development and smoke/integration issues
- `docs/contracts/runtime/session-locking.md`: concurrency lock contract for `session/send`, `fork`, and `compact`
- `docs/architecture/layers/crate-map.md`: stable crate ownership and refactor guidance
- `docs/architecture/product/stim-boundary.md`: product-facing boundary between `stim` and `santi`
- `docs/architecture/topology/service-and-assemblies.md`: service boundary, topology assembly, bootstrap, and standalone rules
- `docs/contracts/runtime/adapter-boundaries.md`: boundary between runtime-facing ports and db adapter ownership
- `docs/contracts/http/api-surface.md`: minimal stable `/api/v1` HTTP contract for the current resource set
- `docs/contracts/http/envelopes-and-errors.md`: shared success meta and error schema for HTTP, CLI, and 单机 / 分布式 assembly
- `docs/architecture/decisions/0001-service-boundary.md`: decision record for the `santi` service boundary, CLI split, and compatibility rules
- `../AGENTS.md`: repo-root product and deployment boundary across `santi/`, `santi-link/`, and `santi-cli/`

## Release Policy

- This workspace follows a long-lived beta-only `0.1.0-beta.N` release line.
- Keep packaging and verification entrypoints aligned with that beta-only workflow.
- Skipped tests are not allowed in committed sources; `scripts/verify/no-skips.sh` is part of the required verification gate.

## Update Rules

- Put ongoing design reasoning into `docs/`.
- Keep `AGENTS.md` short and durable.
- Only add indexes here for files that are likely to remain central.
- Before changing doc structure or adding new docs, read `docs/operations/documentation.md` and follow its canonical-source, split/merge, and no-history-baggage rules.
