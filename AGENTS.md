# AGENTS

## Purpose

This file manages two things only:

- core constraints that should stay stable while the system evolves
- key file indexes for the most important design documents

Detailed system thinking belongs in `docs/`, not here.

## Core Constraints

- We are building a customized personal agent service, not recreating `opencode` or `openclaw`.
- Reference projects are used to extract reusable principles, not implementation parity.
- Explore external projects only when needed by the current task.
- Sync back only durable knowledge that affects product, architecture, safety, or operating decisions.
- Prefer a small number of stable primitives over early abstraction growth.
- `session` is currently a work unit, not a security boundary.
- HTTP capabilities are currently open; `scope` / `tenant` comes later.
- `soul_dir` and `session_dir` are normal directories used as unified agent resource spaces.
- Testing follows an e2e-first strategy: use black-box end-to-end cases to validate the main path, add focused `crates/santi-api/tests` only where e2e reveals weak spots, and keep tracing strong enough to diagnose known classes of failure.
- `scripts/dev/send.mjs` is the canonical local send helper: create sessions explicitly when needed and always issue session turns strictly sequentially, never concurrently against the same session.
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
- `docs/lifecycle.md`: soul/session lifecycle and fork hook model
- `docs/dev-environment.md`: local development and e2e environment baseline
- `docs/dev-faq.md`: local troubleshooting notes for common development and e2e issues
- `docs/redis-lock-plan.md`: minimal Redis-based concurrency lock plan for `session/send`
- `docs/crate-architecture.md`: stable crate layering and refactor guidance

## Update Rules

- Put ongoing design reasoning into `docs/`.
- Keep `AGENTS.md` short and durable.
- Only add indexes here for files that are likely to remain central.
