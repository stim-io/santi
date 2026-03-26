# Session Message Actor Model

This file records the first-pass basic structure for the actor-based `session + message` refactor.

It is a model document, not an implementation plan.

## Goal

Reframe `session` as a shared message space for multiple actors, while moving provider-facing runtime assembly into a separate `soul × session` internal space.

## Core Shift

The old shape mixed several different concerns into one session transcript:

- public session conversation
- provider-facing transcript assembly
- tool-call internal runtime behavior
- provider-oriented `role`

The new shape separates them.

## Actors

An `actor` is anything that can participate in a session message stream.

Current actor kinds:

- `account`
- `soul`
- `system`

Working actor identity shape:

- `actor_type`
- `actor_id`

This identity shape should stay logically uniform across actor kinds.

### `account`

First-pass shape:

- `account_id`
- `account_name`
- other account fields may be added later

### `soul`

`soul` remains a long-lived agent subject.

Additional direction:

- `fork soul` is allowed
- a fork creates `new soul_id + copied memory`
- lineage details can be added later

### `system`

`system` is treated as an actor kind for session-message authorship.

`system` should still use the same `actor_type + actor_id` shape.

Working interpretation:

- `system.actor_id` identifies the concrete trigger or mechanism that emitted the message
- it is not just a generic placeholder for "the platform"

Example:

- the runtime detects token usage beyond a configured threshold
- a system-side trigger decides to fork a session and emit a `suggest compact` message
- that session message should be authored by `actor_type=system`
- its `actor_id` should identify the responsible trigger, not an anonymous global system actor

This keeps system-authored public messages auditable and consistent with other actor kinds.

Current direction:

- keep open the possibility of a future `system_triggers` persistence object
- prioritize code-level identity consistency first
- decide later whether lifecycle/hooks and trigger audit need a dedicated table

## Session

`session` is the shared external message truth for a set of actors.

Working interpretation:

- a session may contain `M * account + N * soul`
- `M + N >= 1`
- session is not owned by a single `soul`
- session is the durable public message source of truth

This means session is no longer modeled as a single-soul chat thread.

It is a shared actor conversation space.

## Session Message

`session_message` is the public durable message fact inside a session.

Canonical first-pass structure:

- `messages.id`
- `messages.actor_type`
- `messages.actor_id`
- `messages.content.parts[]`
- `messages.state`
- `messages.version`
- `messages.created_at`
- `r_session_messages(session_id, message_id, session_seq, ...)`

Strict rule:

- every session message must specify exactly one actor source

Current stance:

- do not introduce `session_participant` yet
- participation is currently treated as a view over `session_message`
- add a dedicated participant structure later only if real query or management friction appears

## Content

`session_message.content` should directly use a `parts` structure.

Reason:

- it gives a stronger compatibility base with OpenAI-style message content
- it also gives a natural compatibility base for traditional IM message forms
- it preserves a single source of truth while allowing multiple projection systems above it

Working stance:

- the ledger owns one neutral `parts[]` payload
- IM-facing product views interpret those parts one way
- soul/runtime/provider-facing views interpret those parts another way

First-pass part types:

- `text`
- `image`

Suggested shapes:

- `{"type":"text","text":"..."}`
- `{"type":"image","mime_type":"image/png","data_base64":"..."}`

This is intentional.

The same message truth may support multiple projections without requiring multiple message bodies.

Current stance:

- public message content modality lives only in `parts[]`
- there is no separate public `message.type` field for `text` / `image` style distinctions

## Message Lifecycle

Public session messages should support two base lifecycle states:

- `pending`
- `fixed`

Current stance:

- lifecycle support belongs in the public ledger model itself
- business layers may choose stricter rules for which actors or flows may use `pending`
- early usage is expected to be most valuable for soul-facing product behavior rather than full public compatibility

## Message Events

Public message mutation should be event-oriented.

Working interpretation:

- `session_message` stores the current public message state
- `session_message_event` stores how that message changed over time
- event semantics should operate directly on `content.parts[]`
- public ordering stays in `r_session_messages`, not on the message row itself

First-pass event actions:

- `patch`
- `insert`
- `remove`
- `fix`
- `delete`

### `patch`

`patch` applies merge-style updates to existing parts.

Working shape:

- `patch([(index, obj), (index, obj), ...])`

Meaning:

- each `index` must already exist
- each `obj` merges into the target part at that index
- `patch` does not create new parts and does not change ordering

### `insert`

`insert` inserts new parts at explicit positions.

Working shape:

- `insert([(index, part), (index, part), ...])`

Meaning:

- insertion is explicit
- ordering changes are intentional and visible

### `remove`

`remove` removes existing parts by index.

Working shape:

- `remove([index, index, ...])`

Meaning:

- removal changes the current visible content shape
- removal is part-level, not message-level

### `fix`

`fix` transitions a message from `pending` to `fixed`.

### `delete`

`delete` is message-level soft deletion.

It does not physically erase the public ledger object.

## Hard Constraints

The current hard constraints are:

1. `delete` is soft delete only
2. a `fixed` message does not allow further operations
3. `actor_type + actor_id` restricts who may operate on the message

Additional first-pass operational rules:

- `patch`, `insert`, and `remove` must apply against the latest message version
- indexes must be strictly valid; the system should not auto-create missing parts or implicit gaps
- `system` should normally emit directly as `fixed` unless a later product rule explicitly requires otherwise

## What Leaves Session Message

### `role`

`role` must be removed from the session-message model.

Why:

- `role` is not a durable session fact
- `role` depends on which actor is currently building a provider-facing view
- if two sides of a session are both `soul`, `user` vs `assistant` is not a stable truth

Conclusion:

- `role` is snapshot-only
- `role` belongs to provider-facing transcript construction, not to session persistence

### `tool_call`

`tool_call` leaves the session-message model.

### `tool_result`

`tool_result` leaves the session-message model.

Reason:

- tool-call cycles are internal runtime behavior of a specific `soul`
- they are not part of the shared external session truth by default
- the same principle applies to internal system runtime activity: not every internal system event should become a public session message

### `compact`

`compact` also leaves the public session-message model.

Reason:

- `compact` belongs to soul-internal runtime assembly
- humans may read a plain IM-like session stream while a soul reads a compacted internal working view
- public session truth and soul runtime truth must stay distinct even when they are related

## Soul × Session Internal Space

Each `soul` participating in a session may maintain its own internal runtime space for provider interaction.

Working interpretation:

- this is a `soul × session` message space
- it is independent from `session_message`
- it is the provider-facing assembly space for that soul

This internal space may contain:

- provider transcript state
- tool-call cycle state
- tool-call messages
- tool-result messages
- provider-oriented `role`

It may also hold internal runtime events that should not become public session messages.

This is where runtime and provider continuity belong.

It is not the same thing as the public session conversation.

Canonical first-pass runtime structure:

- `soul_sessions` is the durable runtime container for one `soul x session`
- `provider_state` is optional opaque provider continuation state owned by `soul_sessions`
- `turn` is one execution attempt for one `soul_session`, with completion or failure recorded on the `turn`
- `tool_call`, `tool_result`, and `compact` are runtime artifacts, not public messages

Working implementation-facing interpretation:

- the unique assembly truth for a given `soul × session` should live in `r_soul_session_messages`
- that relation should order weak references to public `message` objects and soul-internal runtime objects
- public `session_message` facts are not replayed into provider input directly; they enter only through soul-side assembly projection

## Snapshot Rule

Provider-facing transcript assembly becomes a projection.

It is built from:

- public `session_message` facts
- current `soul × session` internal runtime state
- runtime memory and meta blocks

More precisely:

- public messages provide shared content truth
- `r_session_messages` provides public session ordering only
- `r_soul_session_messages` is the only provider-assembly ordering truth
- compact, tool-call, and tool-result objects enter the provider-facing view only through `r_soul_session_messages`

Only at this stage may the system derive:

- `role=user`
- `role=assistant`
- provider-facing tool transcript shape

So:

- `session_message` is source of truth
- provider transcript is a view
- `r_soul_session_messages` is the unique assembly truth for provider snapshot construction

## Current First-Pass Structure

At the basic-structure level, the model now looks like this:

- `account`
- `soul`
- `session`
- `message`
- `message_event`
- `r_session_messages`
- `soul_session`
- `turn`
- `r_soul_session_messages`

With these rules:

- session stores shared actor messages only
- every session message has an explicit actor source
- content is `parts[]`
- message lifecycle is ledger-native
- public message modality is carried only by parts, not by a separate message type field
- role is derived later
- compact is soul-internal
- tool-call cycles stay inside the relevant `soul × session` runtime space
- `provider_state` is optional opaque provider continuation state, not truth
- `turn` is one execution attempt for one `soul_session`, with completion or failure recorded on the `turn`
- `r_soul_session_messages` maintains weak references plus provider-facing assembly order
- fork acts on `soul`, not on session-message role semantics

## Deferred Details

The following are intentionally left for later discussion:

- whether actor references use a unified id scheme or kind-specific fields
- the exact persistence shape for runtime audit links back into public messages
- how public session messages reference internal soul runtime traces for debugging or audit
