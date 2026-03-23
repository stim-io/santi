# Runtime Primitives

## `soul`

`soul` is the long-lived top-level subject in the system.

Product meaning:

- a `soul` is an agent instance with unbounded session growth over time

Non-goal at this layer:

- `santi` does not currently care whether one person has one `soul` or many; that mapping belongs above the core runtime

Relationship:

- `<soul> 1:N <session>`

Design intent:

- keep a stable subject above individual sessions
- let multiple sessions belong to the same continuing personal agent
- avoid treating sessions as the highest-level identity object

## `message`

`message` is the full-fidelity fact record.

It stores the complete message-layer information for user, assistant, compact, tool, and tool-call events.

`messages` keeps `compact` as a normal message type.

## `session-message pair`

A separate relation layer maps messages into sessions through simple pairs:

- `r_session_messages(session_id, message_id, session_seq, ...)`

Design intent:

- keep `messages` as the full source of message truth
- reduce conceptual load on `session`
- let one message be associated with one or more sessions
- support compact and fork without forcing a 1:1 session-message model

## `snapshot`

`snapshot` is a message-row field aggregation used as a model-facing runtime view.

- `system prompt = snapshot([santi one-line identity] + [soul memory] + [session memory], <meta>)`
- `message = snapshot([raw:(user / assistant / compact / tool / tool_call)], <meta>)`
- `compact message = snapshot([summary], <meta>)`

This means the system prompt is not a hard-coded string. It is a generated runtime artifact.

`snapshot` is not a separate top-level persistence object. It is an aggregation view over stored data, centered on message-row fields.

`meta` is a loose model-facing block, similar to a tagged section such as `<santi-meta>...</santi-meta>`.

- It is composed on demand from available data.
- It has no strict schema contract.
- It should not become the foundation of core runtime semantics.

Current implementation state:

- the first real runtime snapshot builder now belongs to the session turn path
- it currently renders identity text, memory text, request instructions, and a small `<santi-meta>` block
- current meta facts include runtime-discovery fields such as `session_id`, `soul_id`, memory presence flags, `SANTI_RUNTIME_SOUL_DIR`, `SANTI_RUNTIME_SESSION_DIR`, and `fallback_cwd`

Compact introduces non-1:1 mapping between sessions and messages. That complexity remains, but it is isolated in the session-message relation layer rather than being pushed into `session` itself.

## Memory Model

Two memory layers are exposed as simple native tools.

- `memory.soul(text)`: write long-lived memory
- `memory.session(text)`: write current-session memory

At the concept level, these primitives stay minimal and stable.

At the provider/tool wire level, use explicit action-oriented names when helpful for model behavior, for example:

- `write_soul_memory`
- `write_session_memory`

This keeps the conceptual model clean while giving the model clearer executable tool names.

Current implementation state:

- the first real provider-facing memory tool is `write_session_memory`
- tool targeting uses runtime-injected context such as `session_id`, not model-chosen identifiers
- provider/tool wire names may be more action-oriented than concept-layer primitive names

`soul-memory` is fundamentally a text field on `soul`.

At the current stage, there is no hard content constraint on what `soul-memory` should contain.

Its useful boundary should emerge through real usage, especially through the fork-driven workflow.

The core `text` fields in `soul-memory` and `session-memory` are primarily entrusted to model-side management.

The system should focus less on over-structuring memory content and more on identifying which missing resources should be supplied back to the model through context or `meta`.

Design intent:

- keep the tool API minimal and stable
- let the system own storage, shaping, compaction, and retrieval
- separate long-lived identity memory from current work memory

## Tool Artifacts

`tool_call` and `tool_result` are now persisted as normal message facts.

Current implementation stance:

- tool artifacts live in `messages` alongside other message facts
- tool artifacts are persisted for audit and runtime continuity
- tool artifacts are not replayed into provider conversational `input`; only normal `user` and `assistant` messages are replayed
- bash tool results are layered: tool-call metadata such as `feedback_msg` and `duration_ms` wrap a `bash_result` object with `exit_code`, `stdout`, and `stderr`

## Compact Model

- `tools:compact.aggregate(Array<{summary, start_session_seq, end_session_seq}>)`
- `tools:compact.query(Array<[start_session_seq, end_session_seq]>)`

Compact-specific details live in a separate `compact` table, while the corresponding message still lives in `messages` with `type = compact`.

Working interpretation:

- `messages` are the raw durable fact stream
- `query` is the read path over those facts
- `aggregate(Array<{summary, start_session_seq, end_session_seq}>)` is the compact-generation action
- `compact` is the persisted single-layer working-state artifact produced by that action
- `r_compact_messages` is the relation snapshot that records which message facts were folded into a given compact

Core conclusion:

- `compact` is immutable after creation
- `session` may change which compact segments it currently adopts through relations
- `compact` interval semantics use `start_session_seq` and `end_session_seq`; a separate `frontier` object is not required in the core model
- runtime session assembly may legally produce mixed views such as `compact_a + raw_messages + compact_b`
- when compact reaches the provider boundary, its special semantics should be reinforced through `meta` tags rather than a heavy provider-specific object model

Provider-facing shape:

- raw `user` and `assistant` messages should render as `{raw content} + <santi-meta>...</santi-meta>`
- `compact` should render as `{summary} + <santi-meta>...</santi-meta>`
- `compact` should remain a summary block rather than pretending to be a normal conversational turn
- provider input should preserve session-view order even when the assembled view mixes compact segments and raw message gaps
- provider-facing role selection for `compact` should remain an evaluation choice; the current working hypothesis is `user` first, then `assistant`, with `system` treated as the strongest and most cautious fallback

Example session view:

- `compact_a(1..4) + raw_messages(5..6) + compact_b(7..9)`

Example provider-facing assembly:

```text
{summary for compact_a}
<santi-meta>
type: compact
start_session_seq: 1
end_session_seq: 4
</santi-meta>

{raw user or assistant message content}
<santi-meta>
type: message
session_seq: 5
ts: ...
</santi-meta>

{raw user or assistant message content}
<santi-meta>
type: message
session_seq: 6
ts: ...
</santi-meta>

{summary for compact_b}
<santi-meta>
type: compact
start_session_seq: 7
end_session_seq: 9
</santi-meta>
```

Memory and compact priority:

- `memory` belongs to the top-level system message rather than the compact layer
- the final system message should keep a stable structure such as:

```text
<one-line santi identity>

<soul_memory/>

<session_memory/>

<santi_meta/>
```

- `compact` should not replace or absorb this top-level memory structure
- `compact` is responsible for session-history compression, while `memory` remains the higher-priority runtime context

Sequence semantics:

- `session_seq` is immutable
- `session_seq` is session-local, not a global message order
- `compact` is still a normal `message`, and `compact.message.session_seq == compact.start_session_seq`
- `compact` covers the inclusive session-local interval `[start_session_seq, end_session_seq]`
- current session assembly may legally interleave compact segments and raw message gaps, for example `compact_a + raw_messages + compact_b`

This means compact should not try to replace raw history, retrieval, and state transition all at once.

Current implementation direction:

- `query` should first behave like a SQL-like `messages` table variant for precise retrieval rather than a semantic-search product
- `aggregate` should be understood as the act of producing the next compact from selected summary segments, not as a second independent layer beside compact
- `compact` should be treated as current working-state, not as a prose-first transcript summary

Rules:

- `aggregate` takes `Array<{summary, start_session_seq, end_session_seq}>`
- if an aggregate input contains existing compact segments, they must be expanded and re-aggregated
- compacting must remain single-layer; do not keep compact-on-compact chains because that turns working-state continuity into recursive retelling
- a compact result is still a message object, not a side cache
- compact creation is immutable; replacing a summary means creating a new compact rather than mutating an old one
- old compact records are not deleted when newer compact state becomes active
- active compact state should be derived from current session relations rather than mutable `deprecated` fields on shared objects
- runtime assembly should prefer the compact state implied by current relations
- a new compact interval may only be either fully disjoint from an existing compact interval or fully contain it; all partial overlaps are invalid and should fail fast
- if a new compact fully contains existing child compact segments, keep the child compact objects but remove their current session relations
- cold-start behavior should stay strict: do not auto-split, auto-trim, auto-merge, or silently reinterpret invalid compact intervals

Quality direction:

- good compact output should separate confirmed facts from open hypotheses
- good compact output should preserve next actions and current constraints
- good compact output should keep anchors back to raw messages or session-local ranges for later verification
- compact should stay inspectable and replaceable, not silently assume perfect truth

Current relation direction:

- use explicit relation names with `r_` prefixes and fully expanded nouns
- treat `r_session_messages` as the session-local message view
- treat `r_compact_messages` as the compact source snapshot view
- avoid introducing separate active-state tables if current compact state can be derived cleanly from relations

## Directory Model

- `soul_dir` is a normal directory
- `session_dir` is a normal directory

They are not special storage systems. They provide a unified resource space for the agent.

Design intent:

- keep resource access simple and legible
- allow agent-readable and human-readable organization
- avoid inventing a second resource model beside the file system

Current implementation state:

- runtime resource facts are exposed to shell execution through `SANTI_RUNTIME_SOUL_DIR` and `SANTI_RUNTIME_SESSION_DIR`
- those directories are normal resource roots and are conceptually separate from the process cwd
- `santi` may provide a fallback cwd, but `cwd` remains a per-call execution choice rather than a synonym for `session_dir`

## Session Interpretation

`session` should be interpreted here as a core runtime unit.

Product-specific conversation mappings such as chatbot sessions, Feishu conversations, or Slack conversations belong above `santi` and can be adapted by upper layers as needed.

## Runtime Boundary

`santi` should prefer real runtime and operating-system boundaries wherever possible.

Current stance:

- runtime access control should lean on Unix user, process, filesystem, and container boundaries
- `santi` itself should avoid growing an early application-layer permission product
- friendlier enterprise or end-user security products can be layered above `santi` later
- if container isolation is used, treat the container as the host boundary for `santi` itself rather than wrapping each tool call in a second container by default
- `bash` should be modeled as a real subprocess inside the `santi` runtime environment, not as a fake or over-sanitized execution path
- in the local `docker-compose` baseline, `santi` itself runs as a non-root Unix user and `bash` inherits that runtime identity
