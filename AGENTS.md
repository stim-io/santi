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
- `docs/system-model.md`: top-level runtime model overview and design principles
- `docs/runtime-primitives.md`: current core object model and primitive definitions
- `docs/layer-responsibility-analogy.md`: durable analogy for core/runtime/middleware responsibility boundaries
- `docs/lifecycle.md`: soul/session lifecycle and fork hook model
- `docs/hook-reload-boundary.md`: runtime boundary for hook source inputs and whole-set reload
- `docs/dev-environment.md`: local development baseline and smoke entrypoints
- `docs/cold-start-verification.md`: cold-start operational verification flow for common runtime smoke checks
- `docs/dev-faq.md`: local troubleshooting notes for common development and smoke/integration issues
- `docs/redis-lock-plan.md`: minimal Redis-based concurrency lock plan for `session/send`
- `docs/crate-architecture.md`: stable crate layering and refactor guidance
- `docs/stim-santi-boundary.md`: high-level product boundary between public session ledger and soul runtime
- `docs/composition-root.md`: composition root rules for the single `santi` HTTP host, mode assembly, and crate refactor constraints
- `docs/local-mode.md`: local mode assembly, storage, and single-process rules for the `santi` internal local runtime
- `docs/runtime-ports-db-adapters-boundary.md`: boundary between runtime-facing ports and db adapter ownership
- `docs/service-config-and-bootstrap.md`: startup config precedence, mode requirements, and fail-fast bootstrap boundary for `santi`
- `docs/http-api-contract.md`: minimal stable `/api/v1` HTTP contract for the current resource set
- `docs/meta-and-error-schema.md`: shared success meta and error schema for HTTP, CLI, and local mode
- `docs/architecture-adr.md`: decision record for the `santi` service boundary, CLI split, and compatibility rules
- `../AGENTS.md`: repo-root product and deployment boundary across `santi/`, `santi-link/`, and `santi-cli/`

## Update Rules

- Put ongoing design reasoning into `docs/`.
- Keep `AGENTS.md` short and durable.
- Only add indexes here for files that are likely to remain central.
