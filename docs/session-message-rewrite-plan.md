# Session Message Rewrite Plan

This file maps the code paths that must be rewritten for the clean-slate session/message model.

Assumptions:

- no backward compatibility
- no database foreign keys
- `docs/session-message-model-spec.md` is canonical
- `docs/session-message-schema-draft.md` and `crates/santi-db/src/db/migrations/0001_init.sql` define the new storage baseline

## What Must Change

The current code still assumes the old model:

- `sessions` owns `soul_id`
- `sessions` stores session memory and next message seq
- `messages` stores `type`, `role`, and raw text content
- tool artifacts are persisted as normal session messages
- one `TurnStore` appends directly to `messages + r_session_messages`

The new model requires:

- public ledger and soul runtime split
- public `messages` with `actor_type`, `actor_id`, `content`, `state`, `version`
- `soul_sessions` as the `(soul_id, session_id)` runtime container
- `turns` as runtime execution boundaries
- `tool_calls`, `tool_results`, and `compacts` as runtime tables
- `r_soul_session_messages` as the only provider-assembly ordering source
- all cross-table ownership validation in business logic, not database FKs

## Highest-Impact Files

### `santi-core`

- `crates/santi-core/src/model/message.rs`
  - fully incompatible with the new model
  - current `Message` is still `{type, role, content: String}`
  - must be replaced by public-ledger message types plus explicit runtime artifact types

- `crates/santi-core/src/model/session.rs`
  - fully incompatible
  - current `Session` still carries `soul_id` and `memory`
  - should become a public session object only
  - `soul_session` needs its own model

- `crates/santi-core/src/port/turn_store.rs`
  - incompatible abstraction
  - today it assumes one store can load context, list session messages, and append typed chat/artifact messages
  - should be replaced by narrower ports around:
    - public session query
    - soul session runtime state
    - turn creation/finalization
    - public message write
    - runtime artifact write
    - provider assembly query

- `crates/santi-core/src/port/memory_store.rs`
  - partially incompatible
  - `write_session_memory(session_id, ...)` should move to `(soul_id, session_id)` or `soul_session_id`

- `crates/santi-core/src/port/session_query.rs`
  - partially incompatible
  - currently shaped around old `Session` and old `Message`
  - should expose public-session reads separately from soul-runtime reads

- `crates/santi-core/src/service/session/kernel/transcript.rs`
  - incompatible with the new provider-assembly model
  - today it derives provider input from message `type/role`
  - should instead consume assembled runtime blocks from `r_soul_session_messages`

- `crates/santi-core/src/service/session/kernel/tool_artifact.rs`
  - should be deleted or rewritten
  - current code serializes tool artifacts into fake `Message` rows
  - in the new model, tool artifacts are first-class runtime tables

- `crates/santi-core/src/service/session/kernel/runtime_prompt.rs`
  - compatible directionally, but needs new inputs
  - `session_memory` should come from `soul_sessions.session_memory`

### `santi-db`

- `crates/santi-db/src/repo/session_repo.rs`
  - fully incompatible
  - depends on removed columns: `sessions.soul_id`, `sessions.memory`, `sessions.next_session_seq`
  - should be split into:
    - `session_repo` for public sessions only
    - `soul_session_repo` for runtime container and seq frontier

- `crates/santi-db/src/repo/message_repo.rs`
  - fully incompatible
  - inserts and queries old `messages(type, role, content text)`
  - must support new public message shape with `JSONB content`, `state`, `version`, `deleted_at`

- `crates/santi-db/src/repo/relation_repo.rs`
  - too small for the new model
  - should likely become:
    - `session_message_relation_repo`
    - `soul_session_message_relation_repo`

- `crates/santi-db/src/repo/soul_repo.rs`
  - mostly still usable
  - still owns `souls.memory`

- `crates/santi-db/src/adapter/turn_store.rs`
  - should be deleted, not evolved
  - it bakes in the old one-store abstraction and old message persistence model

- `crates/santi-db/src/adapter/memory_store.rs`
  - partially incompatible
  - `write_session_memory` currently writes `sessions.memory`
  - must write `soul_sessions.session_memory`

- `crates/santi-db/src/adapter/session_query.rs`
  - incompatible at the data-shape level
  - returns old `Session` and old `Message`

- `crates/santi-db/src/db/seed.rs`
  - likely needs adjustment for any assumptions about initial session shape

### `santi-runtime`

- `crates/santi-runtime/src/session/send.rs`
  - highest rewrite priority
  - currently persists:
    - user message as public message
    - tool_call/tool_result as fake session messages
    - assistant message as public message
  - currently builds provider input by listing session messages directly
  - new flow should instead:
    - resolve or create the relevant `soul_session`
    - create a `turn`
    - write public user message + `r_session_messages`
    - append provider-assembly rows via `r_soul_session_messages`
    - persist tool/runtime artifacts in runtime tables
    - finalize the `turn`

- `crates/santi-runtime/src/session/memory.rs`
  - must shift session memory writes from `session_id` to soul-session-scoped writes

- `crates/santi-runtime/src/session/query.rs`
  - should separate public ledger reads from runtime assembly reads

- `crates/santi-runtime/src/runtime/tools.rs`
  - compatible overall
  - but tool dispatch results should feed `tool_calls` / `tool_results`, not fake message rows

### `santi-api`

- `crates/santi-api/src/schema/session.rs`
  - incompatible response shape
  - `SessionMessageResponse` still exposes `{type, role, content: String}`
  - must be redesigned around public message shape and/or explicit API projection rules

- `crates/santi-api/src/handler/session.rs`
  - depends on old session/message response types
  - create/get/send/memory endpoints need new service contracts

- `crates/santi-api/src/state.rs`
  - wiring depends on `TurnStore`
  - composition root will need new ports/adapters

## Port Rewrite Direction

Replace `TurnStore` with smaller ports.

Suggested first-pass split:

- `SessionLedgerPort`
  - create/get session
  - append public message
  - list public session messages
  - append message event

- `SoulSessionPort`
  - get/create `soul_session`
  - read/write `session_memory`
  - allocate `next_seq`
  - update `last_seen_session_seq`
  - read/write `provider_state`

- `TurnPort`
  - create turn
  - mark completed
  - mark failed
  - fetch turn context

- `RuntimeArtifactPort`
  - insert `tool_call`
  - insert `tool_result`
  - insert `compact`
  - append `r_soul_session_messages`

- `SoulQueryPort`
  - get soul
  - write soul memory

## Business-Layer Validation That Must Exist

Because there are no DB FKs, the rewritten repos/services must explicitly validate:

- `session_id` exists before writing `r_session_messages`
- `message_id` exists before attaching to `r_session_messages`
- `soul_id` and `session_id` exist before creating `soul_session`
- `soul_session_id` exists before creating `turn`
- `turn_id` exists before creating `tool_call` or `compact`
- `tool_call_id` exists before creating `tool_result`
- `r_soul_session_messages` targets resolve into the same `soul_session`
- `message` targets in `r_soul_session_messages` already belong to the matching public `session`

## Recommended Rewrite Order

1. Rewrite `santi-core` models so old `type/role` assumptions disappear
2. Replace `TurnStore` with narrower ports in `santi-core`
3. Rebuild `santi-db` repos and adapters to match the clean-slate schema
4. Rewrite `santi-runtime/src/session/send.rs` around `turns` and runtime artifacts
5. Update `santi-api` schemas and handlers to the new public message projection
6. Reconnect query/memory paths and only then restore e2e coverage

## Files Safe To Delete During Rewrite

- `crates/santi-db/src/adapter/turn_store.rs`
- `crates/santi-core/src/service/session/kernel/tool_artifact.rs`

They encode the old fake-artifact-as-message path and are likely more misleading than reusable.
