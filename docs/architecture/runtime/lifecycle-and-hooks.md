# Runtime Lifecycle and Hooks

## Core runtime objects

- `session`: public shared ledger container
- `soul_session`: runtime container for one `soul × session`
- `turn`: one execution attempt for one `soul_session`

The exact schema lives in `docs/contracts/data-model/session-message-model.md`.

## Turn lifecycle

- `turn` starts from a known public frontier and runtime frontier
- runtime work, tool activity, and runtime artifacts belong to that `turn`
- a turn completes with a final public assistant message or fails on the turn itself
- keep lifecycle split as `start_turn` / `complete_turn` / `fail_turn`
- do not collapse turn lifecycle into a single `mark_turn` abstraction

## Fork rules

- fork is an explicit session API, not a hook-defined semantic
- fork copies context, not provider continuity
- copied context is:
  - latest `soul_sessions.session_memory`
  - reference prefix from `r_soul_session_messages` through the requested `fork_point`
- fork does not copy `provider_state`
- child ordering is rebuilt in its own sequence space
- parent and child compaction diverge immediately after fork
- lineage is `parent_session_id + fork_point`
- fork, send, and explicit compact share the same parent-session lock family and fail fast on conflict

## Hook points

- `turn.completed`
- `session.created`
- `session.forked`
- `message.committed`
- `tool.completed`

## Hook execution rules

- hook evaluation is read-only and produces runtime actions
- runtime actions execute after `turn.completed` and fail open
- action execution does not recursively trigger more hooks
- `session_effects` is the source of truth for hook idempotency and recursion control
- runtime actions may include `compact`
- hook kinds stay typed in Rust even when instances come from config

## Reload boundary

`santi` owns:

- parsing hook source input
- resolving it into a whole `Vec<HookSpec>`
- compiling runtime evaluators
- atomically replacing the active registry

`santi` does not own:

- file watching
- config orchestration policy
- deciding when reload happens

Those remain upper-layer concerns.

Accepted hook inputs:

- inline `value`
- local `path`
- remote `url`

Reload rules:

- reload is whole-set replacement, not patch or merge
- the registry swap is atomic
- one running turn sees one stable registry snapshot for its evaluation pass
- later turns see the new registry after replacement
- management happens through startup config and `PUT /api/v1/admin/hooks`
