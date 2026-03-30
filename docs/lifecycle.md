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

- fork remains deferred in the current repo
- when added, model it as session copy plus next action
- do not turn hooks into a workflow engine
