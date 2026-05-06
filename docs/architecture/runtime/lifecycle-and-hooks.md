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

## `session::send` boundary

`session::send` owns one bounded runtime path: take a session-triggered input, assemble provider-visible context, run one turn, and publish the resulting watch/tool side effects.

Keep that boundary split into small local module responsibilities when the file shape requires it:

- service entry and lock/watch orchestration
- turn execution and tool-call loop
- assembly/projection from session and soul-session items into provider input

The split is local to the `session::send` runtime boundary. It does not create a new cross-crate abstraction.

Assembly/projection rules for this boundary:

- public session messages remain the canonical provider-visible history input
- effective compacts may replace covered message ranges
- tool calls and tool results are runtime artifacts, not direct provider input items
- hook execution remains post-turn work rather than part of provider-stream assembly

Provider instruction rules for this boundary:

- provider instructions may include stable self-assessment guidance plus a runtime-facts block for the current process
- the runtime-facts block is part of `santi`'s LLM/runtime projection, not a public product-ledger fact
- non-secret runtime facts may include service name, assembly mode, launch profile, bind address, provider API/model, provider gateway base URL, memory directories, and fallback working directory
- self-assessment guidance must tell the model to ground answers in visible facts, separate unknowns from connected capabilities, and avoid inventing service health, permissions, product-ledger state, or external process state
- automated tests should protect only the stable rendering/contract pieces; real provider self-assessment quality belongs in the local verification runbook until the behavior is stable enough to codify

## `session::watch` boundary

`session::watch` owns the runtime-local observation surface for one session.

Keep that boundary split into small local module responsibilities when file shape requires it:

- watch event and snapshot data shapes
- snapshot projection from canonical query/effect state
- live subscription fanout from the runtime watch hub

The watch boundary is read-oriented. It does not own send/fork/compact execution; it only projects and streams their visible observation state.

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
