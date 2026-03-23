# Lifecycle

## `soul` Lifecycle

`soul` is the durable subject that survives across unbounded session growth.

At the current stage, the important lifecycle fact is simple:

- a `soul` persists as the stable owner of many sessions over time

This model intentionally avoids introducing complex status machines too early.

## `session` Lifecycle

`session` is a runtime concept first. User conversation is only one special case of session usage.

The model may also allow `soul`-to-`soul` interaction through sessions.

Channel-specific conversation semantics should be handled above `santi`.

It should carry:

- message history
- current task context
- tool activity history
- session-scoped memory
- related session resources

Current-stage rule:

- `session` is a work unit, not a security boundary

## Fork Through Event Hooks

Forking is a native capability, but it should be driven through lifecycle hooks rather than treated as a separate workflow engine.

Core idea:

- at the right moment, copy the current session and do something next

Mechanism:

- `fork` should be triggered through `eventbus` hooks
- a hook observes a lifecycle event or runtime threshold
- the hook creates a new session by copying current `messages` and `session` state into a new `session_id`
- the forked session then receives a new independent `user` message and continues like any other session

This keeps fork aligned with its core meaning: session copy plus a next action.

At the current stage, `fork` itself is a normal runtime action. The hook layer mainly contributes observation value around that action.

## Eventbus Model

The `eventbus` should stay fact-oriented and minimal.

- events are internal runtime facts, not workflow graph nodes
- hooks are reactions to those facts
- hook handlers should write through normal system paths such as creating messages, writing memory, or creating a forked session
- hooks should not gain privileged access to hidden runtime state

Default stance:

- prefer post-commit facts over pre-action interception
- keep hooks observational by default

## `turn`

`turn` means a complete agent response cycle ending in the final finished `assistant` message.

It may include the full tool-calling cycle inside it, for example:

- `assistant -> tool_call -> tool -> assistant`

`turn.completed` should fire only when that whole cycle has reached its final assistant completion.

This makes `turn` the most useful current hook boundary for reminder-style system interventions.

Current implemented shape already includes the first real tool path:

- `assistant -> tool_call -> tool_result -> assistant`

Current limitation:

- the first real loop is intentionally narrow and currently supports only one tool call per turn for `write_session_memory`

Current implemented scope has already expanded to a small set of real tools:

- `write_session_memory`
- `write_soul_memory`
- `bash`

Current interface stance:

- canonical runtime turns should move through `session/send`
- low-level provider-shaped compatibility paths should stay outside the main lifecycle surface

Current direction:

- future real tools such as `bash` should continue to follow the same turn shape, but execute inside the normal `santi` runtime boundary rather than through a second per-call isolation layer by default
- fork should eventually copy session-local state and relation views while shared facts remain shared

### Event Envelope

Use a small reference-oriented event shape:

```json
{
  "id": "evt_123",
  "type": "message.committed",
  "ts": "2026-03-20T12:34:56Z",
  "soul_id": "soul_1",
  "session_id": "sess_1",
  "message_id": "msg_9",
  "data": {}
}
```

Guidelines:

- keep full truth in the underlying tables, especially `messages`
- use ids and small references in events
- let `data` carry only minimal event-specific extras

## Built-in Hook Points

The first built-in hook points should stay small:

- `session.created`
- `session.forked`
- `message.committed`
- `tool.completed`
- `turn.completed`

Why these are enough:

- `session.created` supports light session initialization
- `session.forked` records completed fork facts as a derived event
- `message.committed` is the main durable fact stream and leaves room for message-level hooks when needed later
- `tool.completed` supports follow-up reactions without exposing too many tool phases
- `turn.completed` is the natural place for threshold-based suggestions such as `memory` or `compact`

At the current stage, `turn.completed` is expected to carry most of the practical hook value.

## Blocking Defaults

Blocking behavior should be avoided for now.

- non-blocking: `session.created`
- non-blocking: `session.forked`
- non-blocking: `message.committed`
- non-blocking: `tool.completed`
- non-blocking: `turn.completed`

This keeps control flow legible and avoids turning hooks into a hidden runtime engine.

## Fork Use Cases

Two core use cases remain in scope:

1. System-triggered fork at specific hard-coded moments to remind the agent to take helpful actions without forcing them.
2. Agent-initiated fork for branch tasks, exploration, verification, or side work.

At the current stage, agent initiative should remain intentionally unconstrained. The goal is to expose the real core problems before adding artificial behavior limits.

Early examples of system-triggered behavior:

- when repetition becomes heavy, suggest `memory`
- when session token usage crosses a threshold, suggest `compact`

Example:

- when total session tokens exceed `100k`, insert an `assistant` message reminding the agent that compaction may be appropriate

Specific strategies are secondary. The main design point is that fork belongs to lifecycle/event handling, not to a special branch object model.

In the current model, fork has observation value through events, but little standalone business value as a hook boundary.

Compact timing direction:

- `compact` should be triggered with stage-awareness, not only by raw token count
- token pressure is a useful signal, but not the whole trigger policy
- good compact windows usually appear when the task is already clear enough to summarize but not yet so noisy that state extraction becomes lossy or urgent
- `turn.completed` remains the most natural current place to surface fork-based reminders such as “token is getting large; consider compact if this feels like a good boundary”

## Fork Persistence and Visibility

- forks should be persisted as normal data
- user-facing visibility can remain read-only for now
- post-fork growth remains unconstrained; the new session evolves like any other session

## Traps To Avoid

- do not turn hooks into a workflow language with steps, graphs, priorities, or retry DSLs
- do not duplicate full `message` or `session` payloads inside events
- do not create a separate branch object if normal sessions already express the result
- do not let hooks bypass normal persistence and message-writing paths
