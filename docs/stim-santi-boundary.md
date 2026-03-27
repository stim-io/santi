# Stim Santi Boundary

This file records the current high-level product boundary between `stim` and `santi`.

## Product Direction

`stim` is the intended product surface above `santi`.

Working shorthand:

- `stim = santi im`

This means `stim` is not just another chat shell around `santi`.

It is the product layer that treats `session` as a high-bandwidth IM ledger while exposing selected `soul`-side mechanisms as alternate read-only views.

## Two Core Product Differences

### 1. Multi-View Session Reading

The same session may be read from different actor viewpoints.

Implication:

- the public session ledger is the single shared truth
- different actors may project that same truth differently
- `soul` may additionally expose internal mechanism views such as thought-like runtime traces

At the product level, this is the basis for capabilities like reading a conversation from another actor's point of view or inspecting a `soul`'s internal working path in read-only form.

### 2. Message Lifecycle

Public messages support two lifecycle states:

- `pending`
- `fixed`

Messages may be revised while pending.

That revision history should be managed through event-oriented ledger semantics rather than ad hoc overwrite behavior.

## Boundary

The system is intentionally split into two layers.

### Public Session Ledger

This is the IM-facing layer.

It owns:

- actor-authored shared session messages
- message ordering
- message lifecycle such as `pending` and `fixed`
- ledger/event semantics for public message changes

It does not own:

- provider-facing `role`
- tool-call cycles
- compact state
- provider transcript continuity

### Soul Runtime

This is the actor-internal runtime layer.

It owns:

- provider-facing assembly state
- compact
- tool-call cycles
- provider continuity state
- runtime execution boundaries such as `turn`
- internal runtime state and future thought-trace style views

`santi` should keep these runtime concerns inside the `soul` system rather than leaking them into the public session ledger.

Current first-pass runtime shape:

- `soul_sessions` as the durable `soul x session` runtime container
- `turns` as one execution attempt for one `soul_session`, with success or failure recorded on the `turn`
- runtime artifacts such as `tool_call`, `tool_result`, and `compact`
- `provider_state` as optional opaque provider continuation state rather than canonical transcript truth

## Design Rule

The same underlying truth may be interpreted differently by different layers.

That is intentional.

- public IM view and soul-runtime view are both projections
- neither projection should replace the shared ledger truth
- provider-facing snapshot is another projection again

This is one of the main reasons the session/message model must stay actor-based and provider-neutral.
