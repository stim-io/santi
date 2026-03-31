# Lifecycle

## `soul`

- durable subject
- spans many sessions over time

## `session`

- public shared ledger container
- carries public messages, ordering, and lifecycle
- is not the home of tool artifacts or session-scoped runtime memory

## `soul_session`

- runtime container for one `soul × session`
- owns `session_memory`, provider continuity, and runtime frontier state

## `turn`

- one execution attempt for one `soul_session`
- contains runtime tool activity
- completes with the final public assistant message, or fails on the turn itself

## Hooks

Hooks now exist in a minimal runtime form.

Keep them small and non-blocking.

Current and likely hook points:

- `turn.completed` (current first-pass hook point)
- `session.created`
- `session.forked`
- `message.committed`
- `tool.completed`

Current stance:

- hook evaluation is read-only and produces runtime actions
- runtime actions run after `turn.completed` and must fail-open
- action execution does not recursively trigger more hooks
- first runtime action is `compact`
- hook instances are dynamically loaded from config/env, while hook kinds stay typed in Rust
- hot-reload shape is still deferred, but the registry holder is designed for atomic whole-set replacement
- current explicit management path is whole-set reload through the admin API; file watching remains deferred
- startup and reload inputs may arrive as inline value, file path, or URL; watcher/orchestration remains an upper-layer concern

## Fork

- first shipping shape is an explicit session API, not a hook-driven action
- fork copies context, not runtime model continuity
- copied context is:
  - latest `soul_sessions.session_memory` value at fork execution time
  - prefix references from `r_soul_session_messages` through the requested `fork_point`
- fork does not copy `provider_state`
- fork acquires the same session lock family currently used by `session/send` so fork and send do not race on the same parent session
- manual compact now uses that same session lock family too, so `send`, `fork`, and explicit `compact` fail fast instead of mutating the same parent session concurrently
- child session ordering is rebuilt in its own space; it does not inherit parent seq numbers
- parent and child compact state diverge immediately after fork; later compaction is independent per session
- first-pass lineage is `parent_session_id + fork_point`
- explicit API is the source of truth; hook integration, when added, should reuse the same fork service instead of redefining fork semantics
- do not turn hooks into a workflow engine
