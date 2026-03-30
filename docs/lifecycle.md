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

Hooks are still deferred in the current repo.

When they are added, keep them small and non-blocking.

Likely hook points later:

- `session.created`
- `session.forked`
- `message.committed`
- `tool.completed`
- `turn.completed`

## Fork

- fork remains deferred in the current repo
- when added, model it as session copy plus next action
- do not turn hooks into a workflow engine
