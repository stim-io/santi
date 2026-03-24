# Rewrite Plan

This file records the current rewrite baseline so implementation can proceed without re-deriving the model.

## Baseline

- keep the existing top-level directory convention
- rewrite the backend internals around PostgreSQL + `sqlx`
- use `santi/docker-compose.yml` for local development
- treat the current docs as the source of truth, not the old SQLite implementation

## Core Model

- `soul` is the long-lived subject
- `session` is the runtime work unit
- `message` is the full-fidelity fact record
- `session_seq` is immutable and session-local
- `compact` is an immutable segment with `start_session_seq` and `end_session_seq`
- session view may legally interleave compact segments and raw message gaps
- `memory` stays in the top-level system message and is not absorbed by compact

## Local Runtime

- PostgreSQL is the persistence baseline
- local development runs through `santi/docker-compose.yml`
- streaming responses continue to use SSE

## Backend Layout

```text
crates/api/src
├── main.rs
├── lib.rs
├── app.rs
├── openapi.rs
├── telemetry.rs
├── config.rs
├── state.rs
├── db/
├── repo/
├── handler/
├── schema/
├── model/
├── service/
└── runtime/
```

Module intent:

- `handler`: HTTP and SSE surface only
- `schema`: request/response contracts only
- `model`: domain objects and invariants
- `repo`: PostgreSQL + `sqlx` persistence access only
- `service`: usecase orchestration, organized by domain first and then by usecase
- `runtime`: system message, session view, provider input, tool runtime
- `db`: `sqlx` pool, migrations, seed, and DB infrastructure only

## Phase 1 Scope

- `souls`
- `sessions`
- `messages`
- `r_session_messages`
- `POST /api/v1/sessions/{id}/send`
- `write_session_memory`
- `write_soul_memory`
- `bash`

Phase 1 intentionally excludes:

- compact persistence tables
- fork persistence
- eventbus persistence
- automatic compact
- automatic fork

## PostgreSQL 0001 Direction

Phase 1 migration should define:

- `souls`
- `sessions`
- `messages`
- `r_session_messages`

Important constraints:

- `session_seq` lives in `r_session_messages`, not in `messages`
- `sessions` should maintain `next_session_seq` for safe allocation
- `messages.type` and `messages.role` should use `CHECK` constraints first

## session/send Shape

- persist the user message in a short transaction
- build the top-level system message from identity, `soul_memory`, `session_memory`, and `santi_meta`
- assemble the session view from adopted compact segments and raw gaps
- render provider input as raw or summary blocks with `<santi-meta>`
- stream output through SSE
- persist tool-call facts and final assistant facts in separate short transactions
- stage the rewrite through `service/session/send.rs` rather than extending the old flat `responses` path

## Layering Notes

- prefer `repo/` over `*_store` naming for persistence access
- keep `service/` free of direct SQL details
- keep `db/` focused on pool, migrate, seed, and transaction helpers
- short-term dual wiring is acceptable, but new persistence work should land in `repo/` first
- prefer `service/session/send.rs` over flat names like `response_turn.rs` when the usecase clearly belongs to a domain
- avoid process-internal HTTP callbacks; tool-side memory writes should call service/repo paths directly
- give `session/send` its own SSE/event encoding path instead of reusing `handler/responses.rs`
- the legacy `/api/v1/responses` path has been removed from the main runtime surface; continue extracting reusable turn kernel pieces into neutral modules
- move provider request/response loop types toward `service/turn` so provider adapters no longer depend on legacy `responses` naming

## Notes

- old SQLite-specific assumptions should not be carried forward
- old code is reference material only unless explicitly reused
- temporary dual-wiring during the rewrite is acceptable, but PostgreSQL + `sqlx` is the target baseline
