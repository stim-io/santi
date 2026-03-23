# System Model

## Goal

We are building a customized personal agent service.

The goal is not to replicate reference projects. The goal is to define a small, stable runtime model that can grow from chat into an agent with session, tools, memory, compacting, and branching.

## Current Stage Goal

The current stage goal is to let `santi` become aware of `santi` and begin self-iteration.

In practical terms, this means:

- `santi` should increasingly understand its own runtime structure, resources, and boundaries
- `santi` should increasingly work through its own canonical paths and primitives rather than ad hoc external steering
- `santi` should gradually become able to identify and articulate the next useful improvements to its own runtime

This stage is expected to involve many hard cold-start problems. The goal is not to avoid them, but to work through them one by one.

## Foundational Assumption

This project assumes a model capable of actively recognizing memory as part of its own runtime behavior.

In other words:

- the runtime depends on models that can notice what should be remembered
- the runtime depends on models that can decide when memory should be written
- the runtime depends on models that can distinguish between long-lived and session-scoped memory

This is not a universal assumption for all models. At the current stage, only a small number of top-tier models appear strong enough to act as this runtime substrate.

This assumption is one of the core reasons the project is worth starting now.

This project also assumes that both human and model-side working capacity remain fundamentally limited.

In other words:

- infinite context should not be treated as an available or near-term design foundation
- longer context windows do not remove the need for state management, retrieval, and compaction
- runtime design should stay conservative and layered rather than assuming raw history can be carried forever

## Product Route

1. Support OpenAI-compatible streaming chat.
2. Introduce `session`.
3. Introduce `tools:bash`.
4. Introduce PostgreSQL persistence via `sqlx`.
5. Introduce `tools:memory.session`.
6. Introduce `tools:memory.soul`.
7. Introduce `tools:compact.aggregate` and `tools:compact.query`.
8. Introduce `soul_dir`.
9. Introduce `session_dir`.

This route moves the system from a stateless chat bot to a stateful personal agent runtime.

## Core Principles

- `santi` should stay focused on its own runtime model and avoid taking on upper-layer product mapping concerns too early.
- `soul` is the long-lived top-level subject, with `<soul> 1:N <session>`.
- `session` is a runtime work unit, not a security boundary.
- `message` is the full-fidelity fact record.
- session-message relations are first-class and should be modeled explicitly.
- `snapshot` is a model-facing aggregation view, not a separate persistence object.
- `soul-memory` is a text field on `soul`.
- `memory.soul(text)` and `memory.session(text)` stay minimal.
- `compact` remains a normal message type with compact-specific detail stored separately.
- `compact` remains single-layer, and active compact state should be derived from relations rather than mutable object flags.
- `soul_dir` and `session_dir` are normal directories.
- `fork` is implemented through lifecycle hooks: copy session, then do something next.
- upper-layer mappings such as "one person has one or many agents" are not current `santi` concerns.
- upper-layer mappings such as chatbot sessions, Feishu conversations, or Slack conversations belong above `santi`, not in the core runtime definition.
- agent initiative is intentionally left open for now so the system can expose the real boundary problems.
- memory text is primarily managed by the model itself; the system's job is to identify and supply missing resources through context and `meta`.
- `compact` should be understood as working-state management, not only as transcript shortening.
- runtime access control should prefer real OS and Unix-user boundaries over early application-layer permission machinery.
- more enterprise-friendly or user-friendly security products may be built above `santi`, but they should not distort `santi`'s core runtime model.
- `santi` should be treated as a real runtime user, so real execution should be preferred over heavily pre-restricted fake execution.
- when container isolation is used, it should usually host the whole `santi` runtime; tools like `bash` should normally execute as ordinary child processes inside that runtime boundary.

See:

- `docs/runtime-primitives.md`
- `docs/lifecycle.md`

## HTTP Exposure

At the current stage, system capabilities are exposed openly at the HTTP layer.

Implications:

- sessions can in practice perceive or interact with other sessions
- session boundaries are organizational, not isolation boundaries
- future `scope` / `tenant` support will add isolation later

Current design stance:

- do not prematurely optimize for multi-tenant isolation
- do leave room for future scope-aware extensions
- keep the core runtime model independent from tenant logic

## Reference-Project Lens

Reference projects help us choose principles, not implementations.

### `opencode`

- Teaches focused agent behavior inside a task environment.
- Most relevant for tool use, workspace flow, and coding-agent UX.

### `openclaw`

- Teaches long-lived personal-agent framing across channels and capabilities.
- Most relevant for gateway thinking, persistent agent identity, and extensibility.

## Current Design Priorities

- Prefer a small number of stable primitives.
- Keep raw messages and compact messages structurally aligned.
- Keep memory APIs tiny.
- Keep directories normal.
- Treat persistence as infrastructure, not as the main abstraction.
- Delay isolation complexity until `scope` / `tenant` becomes necessary.

## Current Runtime State

- The first real single-tool loop is now implemented for `write_session_memory`.
- Current runtime instruction assembly now uses a first real snapshot-style builder on the session turn path.
- Current runtime snapshot includes `soul.memory` + `session.memory` + request instructions, plus runtime facts rendered through a `<santi-meta>` block.
- Tool execution remains narrow on purpose; current implementation proves the runtime path before a broader tool system is introduced.
- Current Codex backend support depends on a local continuation-shim patch in `openai-codex-server` so `santi` can complete its first real tool loop; treat Codex continuation handling as an integration constraint until upstream support lands.
- Local development should use the root `docker-compose.yml` to bring up PostgreSQL and the supporting services together.
- Current bash execution semantics are now real: `bash` runs as a normal child process of `santi` inside the runtime boundary, while `SANTI_RUNTIME_*` paths are exposed separately from the process cwd.
- `/api/v1/sessions/{id}/send` is now the runtime-facing canonical path for submitting a user turn.
- `session/send` is the main runtime interface; legacy low-level provider-shaped APIs should not be treated as part of the primary surface.
- Current compact direction is to keep `query` as the retrieval path and treat `aggregate(Array<{summary, start_session_seq, end_session_seq}>)` as the action that produces the next single-layer compact state.
- Current relation direction is to use explicit `r_` tables with fully expanded names, such as `r_session_messages` and `r_compact_messages`.
