# Runtime Primitives

This file is a short glossary. For exact schema and invariants, use `docs/session-message-model-spec.md`.

## `soul`

- long-lived agent subject
- participates in shared sessions through `soul_sessions`
- owns long-lived `souls.memory`

## `session`

- public shared ledger container
- owns public message ordering through `r_session_messages`
- is not the place for tool artifacts or session-scoped runtime memory

## `message`

- public actor-authored ledger fact
- uses neutral `content.parts[]`
- does not expose provider `role`
- does not store `tool_call` or `tool_result`

## `soul_session`

- runtime container for one `soul × session`
- owns `session_memory`
- owns provider continuity and runtime frontier state

## `turn`

- one execution attempt for one `soul_session`
- contains runtime tool activity
- records completion or failure at the runtime boundary

## Memory

- `memory.soul(...)` maps to `souls.memory`
- `memory.session(...)` maps to `soul_sessions.session_memory`
- both are replace-whole indexes, not append-only note stores
- richer memory material can live in `SANTI_SOUL_MEMORY_DIR` and `SANTI_SESSION_MEMORY_DIR`

## Tool artifacts

- `tool_call` and `tool_result` are runtime artifacts
- they belong to soul runtime, not to the public session ledger

## Directories

- `SANTI_SOUL_MEMORY_DIR` and `SANTI_SESSION_MEMORY_DIR` are normal directories
- they are free-form resource spaces, not special storage systems
