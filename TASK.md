# TASK

## Current Focus

Session/message docs are being reconciled around the canonical public-ledger plus soul-runtime split.

## Canonical Source

- `docs/session-message-model-spec.md` is the only canonical source for the rebuilt session/message schema, lifecycle, mutation rules, and provider-assembly model.
- `TASK.md` tracks active reconciliation work and durable framing only; it should not restate tables, fields, enums, or invariants from the spec.

## Durable Framing

- The stable split is: public session ledger for shared actor-authored facts; soul runtime for provider-facing assembly, tool execution, and per-soul session state.
- High-level rationale lives in `docs/system-model.md` and `docs/stim-santi-boundary.md`.

## Key Docs

- `docs/stim-santi-boundary.md`
- `docs/session-message-actor-model.md`
- `docs/session-message-model-spec.md`
- `docs/runtime-primitives.md`
- `docs/design-notes.md`

## Current Refinement Focus

- align older docs to the canonical spec and remove duplicated model wording
