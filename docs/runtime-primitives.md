# Runtime Primitives

## `soul`

`soul` is the long-lived top-level subject in the system.

Product meaning:

- a `soul` is an agent instance with unbounded session growth over time

Non-goal at this layer:

- `santi` does not currently care whether one person has one `soul` or many; that mapping belongs above the core runtime

Relationship:

- `soul` participates in shared `session` ledgers through `soul_sessions`

Design intent:

- keep a stable subject above individual sessions
- let the same continuing personal agent participate in multiple sessions
- avoid treating sessions as the highest-level identity object

## `message`

`message` is the full-fidelity fact record.

In the new direction, the public session-ledger message should be treated as an actor-authored shared fact rather than a provider transcript row.

Current structural direction:

- public message content should be represented directly as `parts[]`
- first-pass public parts should stay minimal: `text` and `image(base64)`
- `role` is not a public message primitive
- `tool_call`, `tool_result`, and `compact` should not be modeled as normal public session-message facts
- those runtime-facing concepts belong to soul-internal runtime assembly instead
- public message mutation should be expressed through event actions over `parts[]`, not through provider-facing transcript semantics

## `session-message pair`

A separate relation layer maps messages into sessions through simple pairs:

- `r_session_messages(session_id, message_id, session_seq, ...)`

Design intent:

- keep `messages` as the full source of message truth
- reduce conceptual load on `session`
- keep public ordering in `r_session_messages` rather than on the message row
- keep public message facts separate from soul-runtime assembly

## `snapshot`

`snapshot` is a model-facing runtime projection.

- `system prompt = snapshot([santi one-line identity] + [soul memory] + [session memory], <meta>)`
- `message block = snapshot([public message or runtime artifact], <meta>)`

This means the system prompt is not a hard-coded string. It is a generated runtime artifact.

`snapshot` is not a separate top-level persistence object. It is an aggregation view over public ledger facts, `r_soul_session_messages`, runtime artifacts, and memory.

`meta` is a loose model-facing block, similar to a tagged section such as `<santi-meta>...</santi-meta>`.

- It is composed on demand from available data.
- It has no strict schema contract.
- It should not become the foundation of core runtime semantics.

Current implementation state:

- the first real runtime snapshot builder now belongs to the session turn path
- it currently renders identity text, memory text, request instructions, and a small `<santi-meta>` block
- current meta facts include runtime-discovery fields such as `session_id`, `soul_id`, memory presence flags, `SANTI_RUNTIME_SOUL_DIR`, `SANTI_RUNTIME_SESSION_DIR`, and `fallback_cwd`

The assembled provider-facing view may mix raw public messages and runtime artifacts, but that complexity belongs in soul-runtime assembly rather than in the public ledger.

## Memory Model

Two memory layers are exposed as simple native tools.

- `memory.soul(text)`: write long-lived memory
- `memory.session(text)`: write current-session memory

Current canonical mapping:

- `memory.soul(...)` writes `souls.memory`
- `memory.session(...)` writes `soul_sessions.session_memory` for the current `soul x session`, not shared public session memory

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

`tool_call` and `tool_result` are soul-internal runtime artifacts.

- `tool_call` records one immutable tool request
- `tool_result` records the terminal outcome of that call
- tool-level failure should be stored directly on `tool_result`
- turn-level failure should stay on `turn`
- do not add status wrappers, recovery objects, or metadata envelopes in the first pass

## Compact Model

- `tools:compact.aggregate(Array<{summary, start_session_seq, end_session_seq}>)`
- `tools:compact.query(Array<[start_session_seq, end_session_seq]>)`

`compact` is a soul-internal runtime artifact, not a public message.

`aggregate(...)` creates one immutable compact record.

- a compact stores plain summary text plus the inclusive interval `[start_session_seq, end_session_seq]`
- a compact belongs to the turn that created it
- a `soul_session` adopts compacts through `r_soul_session_messages`
- memory stays separate from compact
- replacing active compact state means creating new compacts and updating relations
- invalid compact intervals fail

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

Current model direction:

- `session` is the shared public ledger container
- `soul_session` is the per-actor runtime container for provider continuity and assembly

## Runtime Boundary

`santi` should prefer real runtime and operating-system boundaries wherever possible.

Current stance:

- runtime access control should lean on Unix user, process, filesystem, and container boundaries
- `santi` itself should avoid growing an early application-layer permission product
- friendlier enterprise or end-user security products can be layered above `santi` later
- if container isolation is used, treat the container as the host boundary for `santi` itself rather than wrapping each tool call in a second container by default
- `bash` should be modeled as a real subprocess inside the `santi` runtime environment, not as a fake or over-sanitized execution path
- in the local `docker-compose` baseline, `santi` itself runs as a non-root Unix user and `bash` inherits that runtime identity

## Model-Facing Contract

`santi` should treat the model as the primary user of runtime-facing tool contracts.

Current stance:

- prefer tool shapes, defaults, and return structures that fit the model's stable natural calling habits
- do not assume prompt tuning can permanently retrain strong model muscle memory
- if a behavior keeps appearing under plain, reasonable prompting, first treat it as a real usage pattern rather than a model mistake
- when that stable usage pattern does not violate safety, core semantics, or long-term maintainability, adjust the runtime contract to fit it
- only push back on model habits when they create a real boundary problem, safety problem, or meaning drift

Example implication for `bash`:

- if the model repeatedly treats `cwd` as a first-class part of shell execution, `santi` should treat that as a real contract expectation
- evaluation should focus on whether `cwd` resolves safely and predictably, not on whether the model omitted it
- similarly, shell-style habits like explicit path anchoring or `cd && ...` should be judged first by safety and runtime clarity, not by whether they match an idealized calling style
