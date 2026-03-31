# Session Message Model Spec

This file is the canonical implementation-oriented model sketch for the rebuilt `session + message` system.

It is still a draft, but it is meant to be concrete enough to guide the first clean implementation and to align older model docs.

## Public Ledger

### `sessions`

- `id`
- `parent_session_id` (`nullable`)
- `fork_point` (`nullable`)
- `created_at`
- `updated_at`

Current stance:

- no `soul_id`
- no participant list
- `session` is a shared ledger container, not a single-soul thread
- first-pass fork lineage lives here as `parent_session_id + fork_point`

### `session_effects`

- `id`
- `session_id`
- `effect_type`
- `idempotency_key`
- `status`
- `source_hook_id`
- `source_turn_id`
- `result_ref` (`nullable`)
- `error_text` (`nullable`)
- `created_at`
- `updated_at`

Current stance:

- first use is `hook_fork_handoff`
- effect rows are the first-pass source of truth for hook idempotency and recursion control
- `status` is intentionally lightweight; detailed per-stage traces still live in logs

### `accounts`

- `id`
- `name`
- `created_at`
- `updated_at`

Current stance:

- this is the stable object behind `actor_type=account`

### `souls`

- `id`
- `memory`
- `created_at`
- `updated_at`

Current stance:

- this is both a long-lived subject object and the stable object behind `actor_type=soul`

### `messages`

- `id`
- `actor_type` (`account | soul | system`)
- `actor_id`
- `content` (`jsonb`)
- `state` (`pending | fixed`)
- `version`
- `deleted_at` (`nullable`)
- `created_at`
- `updated_at`

Current stance:

- no public `type`
- no `role`
- no `tool_call`
- no `tool_result`
- no `compact`

`content.parts[]` first-pass types:

- `{"type":"text","text":"..."}`
- `{"type":"image","mime_type":"image/png","data_base64":"..."}`

Current repo note:

- the core model supports structured `parts[]`
- the current API/CLI send surface is still text-first and does not yet expose image input

### `r_session_messages`

- `session_id`
- `message_id`
- `session_seq`
- `created_at`

Current stance:

- this is the only public ordering source
- `messages` itself does not carry public sequence
- public messages are session-local speech acts even though storage is normalized through relations

Suggested constraints:

- `(session_id, session_seq)` unique
- `(session_id, message_id)` unique

### `message_events`

- `id`
- `message_id`
- `action` (`patch | insert | remove | fix | delete`)
- `actor_type`
- `actor_id`
- `base_version`
- `payload` (`jsonb`)
- `created_at`

First-pass payloads:

- `patch`
  - `{"patches":[{"index":0,"merge":{...}}]}`
- `insert`
  - `{"items":[{"index":1,"part":{...}}]}`
- `remove`
  - `{"indexes":[1,3]}`
- `fix`
  - `{}`
- `delete`
  - `{}` or a very small optional reason payload

## Soul Runtime

### `soul_sessions`

- `id`
- `soul_id`
- `session_id`
- `session_memory`
- `provider_state` (`jsonb nullable`)
- `next_seq`
- `last_seen_session_seq`
- `parent_soul_session_id` (`nullable`)
- `fork_point` (`nullable`)
- `created_at`
- `updated_at`

Current stance:

- this is the runtime container for one `soul × session`
- session-scoped runtime memory belongs here, not in public `sessions`
- `memory.session(...)` writes this `session_memory` layer for the current `soul × session`
- this layer should be treated as a replace-whole core index, not as a multi-note store
- provider continuity belongs here, but it is not canonical transcript truth

Suggested constraints:

- `(soul_id, session_id)` unique

Fork current stance:

- explicit fork input is `(parent_session_id, fork_point, request_id)`
- `fork_point` is a position inside the parent `r_soul_session_messages` view
- fork copies only the reference prefix `<= fork_point`; it does not clone message rows or other runtime artifacts
- child `session_memory` is copied by value from the latest parent `soul_session` state while the fork lock is held
- child `provider_state` starts empty
- child `next_seq` restarts from its own local space after the copied prefix
- parent and child runtime views diverge immediately after fork; later compacts and message-view rewrites are independent

`provider_state` current stance:

- this is optional opaque provider continuation state stored on `soul_sessions`
- it is tied to the provider and `basis_soul_session_seq` that produced it
- it may be reused by the runtime, but it is never transcript truth

Suggested minimal envelope shape:

- `provider`
- `basis_soul_session_seq`
- `opaque`
- `schema_version` (`optional`)

Seq and frontier invariants:

- `next_seq` is the next allocatable `r_soul_session_messages.soul_session_seq`
- `last_seen_session_seq` is the highest public `r_session_messages.session_seq` already incorporated by this `soul_session`
- `turn.input_through_session_seq` is the public-session frontier consumed by that turn
- `turn.base_soul_session_seq` is the assembly frontier before execution
- `turn.end_soul_session_seq` is the assembly frontier when the turn stopped
- `provider_state.basis_soul_session_seq` is the assembly frontier that produced that provider state

### `turns`

- `id`
- `soul_session_id`
- `trigger_type` (`session_send | system`)
- `trigger_ref` (`nullable`)
- `input_through_session_seq`
- `base_soul_session_seq`
- `end_soul_session_seq` (`nullable`)
- `status` (`running | completed | failed`)
- `error_text` (`text nullable`)
- `created_at`
- `updated_at`
- `finished_at` (`nullable`)

Current stance:

- `turn` is one execution attempt for one `soul_session`
- it records the public frontier it consumed, the runtime frontier it started from, the runtime frontier it stopped at, and whether it completed or failed
- runtime artifacts created during execution belong to that `turn`
- failure belongs on the `turn`, not on public messages
- first pass does not model cancellation, retry state, or a separate persisted `run` object

Minimal field semantics:

- `end_soul_session_seq` is null only while the turn is still `running`
- `error_text` is null unless `status=failed`
- `finished_at` is null only while the turn is still `running`

### `tool_calls`

- `id`
- `turn_id`
- `tool_name`
- `arguments` (`jsonb`)
- `created_at`

Current stance:

- `tool_call` is the immutable request record for one tool invocation
- it belongs to a `turn`; `soul_session_id` is derived through that `turn`
- do not add a separate lifecycle status machine on `tool_call` in the first pass

### `tool_results`

- `id`
- `tool_call_id`
- `output` (`jsonb nullable`)
- `error_text` (`text nullable`)
- `created_at`

Current stance:

- `tool_result` is the terminal record for one `tool_call`
- success sets `output` and leaves `error_text` null
- tool-level failure leaves `output` null and sets `error_text`
- if execution stops before any `tool_result` is written, that failure belongs on `turn`

Suggested constraints:

- `(tool_call_id)` unique
- exactly one of `output` or `error_text` must be non-null

### `compacts`

- `id`
- `turn_id`
- `summary`
- `start_session_seq`
- `end_session_seq`
- `created_at`

Current stance:

- `compact` is an immutable runtime summary block over the inclusive public-session interval `[start_session_seq, end_session_seq]`
- it belongs to the `turn` that created it; `soul_session_id` is derived through that `turn`
- keep the persisted object plain in the first pass
- schema shape, storage path, and explicit `session compact` entrypoints are present now
- compacts are consumed during provider snapshot assembly when referenced by `r_soul_session_messages`
- automatic compact generation is still not wired into the main `session/send` runtime path

Constraints:

- `start_session_seq <= end_session_seq`

### `r_soul_session_messages`

- `soul_session_id`
- `target_type` (`message | compact | tool_call | tool_result`)
- `target_id`
- `soul_session_seq`
- `created_at`

Current stance:

- this is the unique assembly truth for provider snapshot construction
- it is a weak-reference relation layer
- it owns runtime ordering, not content

`target_type` meanings:

- `message` -> a public ledger `messages.id`
- `compact` -> a runtime `compacts.id`
- `tool_call` -> a runtime `tool_calls.id`
- `tool_result` -> a runtime `tool_results.id`

Additional constraints:

- `(soul_session_id, soul_session_seq)` unique
- `(soul_session_id, target_type, target_id)` unique
- `message` targets must already belong to the matching public `session` through `r_session_messages`
- `compact` targets must trace through `turn_id` to the same `soul_session_id`
- `tool_call` targets must trace through `turn_id` to the same `soul_session_id`
- `tool_result` targets must trace through `tool_call_id` to a `tool_call` in the same `soul_session_id`

## Core Constraints

- every public message must specify exactly one `actor_type + actor_id`
- public messages are session-local facts ordered through `r_session_messages`
- public messages support only `pending` and `fixed`
- `delete` is soft delete only
- `fixed` messages do not allow further mutation
- only matching `actor_type + actor_id` may mutate a message
- `patch`, `insert`, and `remove` operate only on `content.parts[]`
- all public mutation must target the latest `version`
- indexes must be strictly valid; no implicit gap creation or auto-repair
- `role` is runtime projection only
- all `tool_call`, `tool_result`, and `compact` facts stay out of public `messages`
- `provider_state` is optional opaque provider continuation state only
- `turn` is the runtime execution boundary; failure state belongs there, not in public message lifecycle
- all relation tables use the `r_` prefix

## Snapshot Construction Rule

Provider request snapshots are constructed from:

- public `messages` as content truth
- `r_session_messages` as public session ordering truth
- runtime objects referenced through `r_soul_session_messages`
- soul/session runtime memory
- runtime-generated meta blocks

Assembly rules:

- public messages enter provider snapshot only when referenced by `r_soul_session_messages` with `target_type=message`
- `r_session_messages` alone does not define provider assembly order
- `r_soul_session_messages` is the only ordering source for provider snapshot construction
- `compact`, `tool_call`, and `tool_result` enter provider snapshot only through `r_soul_session_messages`
- relation rows are assembly material, not snapshot rows themselves

The snapshot is a projection.

It is not the source of truth.

## Still Deferred

- exact `system.actor_id` persistence via a future `system_triggers` object
- exact size limits and storage policy for `image.data_base64`
- the final audit/reference link between public messages and runtime traces
