# Session Message Model

Canonical implementation-oriented model for the `session + message` system.

## Model split

- **public ledger**: shared session facts visible across actors
- **soul runtime**: per-`soul × session` execution state, provider continuity, and runtime artifacts
- provider snapshots are projections assembled from both layers; they are not source-of-truth objects

## Public ledger

### `sessions`

Fields:

- `id`
- `parent_session_id` (`nullable`)
- `fork_point` (`nullable`)
- `created_at`
- `updated_at`

Rules:

- `session` is a shared ledger container, not a single-soul thread
- `sessions` does not carry `soul_id` or a participant list
- fork lineage lives here as `parent_session_id + fork_point`

### `accounts`

Fields: `id`, `name`, `created_at`, `updated_at`

Rule: this is the stable object behind `actor_type=account`.

### `souls`

Fields: `id`, `memory`, `created_at`, `updated_at`

Rule: this is both the long-lived agent subject and the stable object behind `actor_type=soul`.

### `messages`

Fields:

- `id`
- `actor_type` (`account | soul | system`)
- `actor_id`
- `content` (`jsonb`)
- `state` (`pending | fixed`)
- `version`
- `deleted_at` (`nullable`)
- `created_at`
- `updated_at`

Rules:

- public messages are actor-authored ledger facts
- public messages do not store `role`, `tool_call`, `tool_result`, or `compact`
- `content.parts[]` is the canonical content shape
- supported parts are:
  - `{"type":"text","text":"..."}`
  - `{"type":"image","mime_type":"image/png","data_base64":"..."}`
- the current API/CLI send surface may stay text-first even though the model supports structured parts

### `r_session_messages`

Fields: `session_id`, `message_id`, `session_seq`, `created_at`

Rules:

- this is the only public ordering source
- `messages` does not carry public sequence

Constraints:

- `(session_id, session_seq)` unique
- `(session_id, message_id)` unique

### `message_events`

Fields:

- `id`
- `message_id`
- `action` (`patch | insert | remove | fix | delete`)
- `actor_type`
- `actor_id`
- `base_version`
- `payload` (`jsonb`)
- `created_at`

Payload shape:

- `patch` -> `{"patches":[{"index":0,"merge":{...}}]}`
- `insert` -> `{"items":[{"index":1,"part":{...}}]}`
- `remove` -> `{"indexes":[1,3]}`
- `fix` -> `{}`
- `delete` -> `{}` or a very small optional reason payload

### `session_effects`

Fields:

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

Rules:

- effect rows are the source of truth for hook idempotency and recursion control
- `status` stays lightweight; detailed stage traces belong in logs

## Soul runtime

Boundary rules:

- core defines atomic traits only
- middleware implements those traits through adapters
- runtime owns orchestration, concurrency, and business composition
- `acquire` is the sanctioned get-or-create atom; it is not lock semantics

### `soul_sessions`

Fields:

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

Rules:

- this is the runtime container for one `soul × session`
- session-scoped runtime memory belongs here, not in public `sessions`
- `memory.session(...)` writes `session_memory`
- `session_memory` is a replace-whole index, not a note log
- `provider_state` belongs here, but is never transcript truth

Constraints:

- `(soul_id, session_id)` unique

Fork rules:

- explicit input is `(parent_session_id, fork_point, request_id)`
- `fork_point` is a position in the parent `r_soul_session_messages` view
- fork copies only the reference prefix `<= fork_point`
- child `session_memory` is copied by value while the fork lock is held
- child `provider_state` starts empty
- child `next_seq` restarts in its own local space
- parent and child runtime views diverge immediately after fork

`provider_state` envelope:

- `provider`
- `basis_soul_session_seq`
- `opaque`
- `schema_version` (`optional`)

Invariants:

- `next_seq` is the next allocatable `r_soul_session_messages.soul_session_seq`
- `last_seen_session_seq` is the highest public `r_session_messages.session_seq` already incorporated
- `provider_state.basis_soul_session_seq` is the assembly frontier that produced that provider state

### `turns`

Fields:

- `id`
- `soul_session_id`
- `trigger_type` (`session_send | system`)
- `trigger_ref` (`nullable`)
- `input_through_session_seq`
- `base_soul_session_seq`
- `end_soul_session_seq` (`nullable`)
- `status` (`running | completed | failed`)
- `error_text` (`nullable`)
- `created_at`
- `updated_at`
- `finished_at` (`nullable`)

Rules:

- `turn` is one execution attempt for one `soul_session`
- failure belongs on `turn`, not on public messages
- runtime artifacts created during execution belong to that `turn`
- keep lifecycle split as `start_turn` / `complete_turn` / `fail_turn`
- do not introduce a unified `mark_turn` abstraction
- `load_turn_context` is not a stable contract surface; runtime should assemble it from smaller reads
- `list_assembly_items` is not a stable contract surface; runtime should assemble it from atomic entry reads
- do not introduce `resolve_assembly_target`

Field rules:

- `end_soul_session_seq` is null only while `status=running`
- `error_text` is null unless `status=failed`
- `finished_at` is null only while `status=running`

### `tool_calls`

Fields: `id`, `turn_id`, `tool_name`, `arguments`, `created_at`

Rules:

- `tool_call` is the immutable request record for one tool invocation
- it belongs to a `turn`
- do not add a separate `tool_call` lifecycle state machine

### `tool_results`

Fields: `id`, `tool_call_id`, `output` (`nullable`), `error_text` (`nullable`), `created_at`

Rules:

- `tool_result` is the terminal record for one `tool_call`
- success sets `output`
- tool-level failure sets `error_text`
- if execution stops before any `tool_result` exists, the failure belongs on `turn`

Constraints:

- `(tool_call_id)` unique
- exactly one of `output` or `error_text` must be non-null

### `compacts`

Fields: `id`, `turn_id`, `summary`, `start_session_seq`, `end_session_seq`, `created_at`

Rules:

- `compact` is an immutable runtime summary over inclusive public-session interval `[start_session_seq, end_session_seq]`
- it belongs to the `turn` that created it
- compacts are included in provider assembly only when referenced by `r_soul_session_messages`

Constraint:

- `start_session_seq <= end_session_seq`

### `r_soul_session_messages`

Fields:

- `soul_session_id`
- `target_type` (`message | compact | tool_call | tool_result`)
- `target_id`
- `soul_session_seq`
- `created_at`

Rules:

- this is the unique assembly truth for provider snapshot construction
- it owns runtime ordering, not content
- it is a weak-reference relation layer

Target meanings:

- `message` -> `messages.id`
- `compact` -> `compacts.id`
- `tool_call` -> `tool_calls.id`
- `tool_result` -> `tool_results.id`

Constraints:

- `(soul_session_id, soul_session_seq)` unique
- `(soul_session_id, target_type, target_id)` unique
- `message` targets must belong to the matching public `session` through `r_session_messages`
- `compact` targets must trace through `turn_id` to the same `soul_session_id`
- `tool_call` targets must trace through `turn_id` to the same `soul_session_id`
- `tool_result` targets must trace through `tool_call_id` to a `tool_call` in the same `soul_session_id`

## Cross-model invariants

- every public message specifies exactly one `actor_type + actor_id`
- public messages are session-local facts ordered through `r_session_messages`
- public messages support only `pending` and `fixed`
- `delete` is soft delete only
- `fixed` messages cannot mutate further
- only matching `actor_type + actor_id` may mutate a message
- `patch`, `insert`, and `remove` operate only on `content.parts[]`
- all public mutation targets the latest `version`
- indexes must be strictly valid; no implicit gap creation or auto-repair
- `role` is runtime projection only
- `tool_call`, `tool_result`, and `compact` stay out of public `messages`
- `provider_state` is optional opaque continuation state only
- `turn` is the runtime execution boundary
- all relation tables use the `r_` prefix

## Snapshot construction

Provider request snapshots are assembled from:

- public `messages` as content truth
- `r_session_messages` as public ordering truth
- runtime objects referenced through `r_soul_session_messages`
- soul/session runtime memory
- runtime-generated meta blocks

Assembly rules:

- public messages enter provider snapshot only when referenced by `r_soul_session_messages` with `target_type=message`
- `r_session_messages` alone does not define provider assembly order
- `r_soul_session_messages` is the only ordering source for provider snapshot construction
- `compact`, `tool_call`, and `tool_result` enter provider snapshot only through `r_soul_session_messages`
- relation rows are assembly material, not snapshot rows

The snapshot is a projection, not the source of truth.

## Open edges

- exact `system.actor_id` persistence shape
- exact size limits and storage policy for `image.data_base64`
- final audit/reference link between public messages and runtime traces
