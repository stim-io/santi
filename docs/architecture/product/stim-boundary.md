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

## Product boundary

`stim` treats `session` as the public IM ledger and may expose selected `soul`-side mechanisms as read-only alternate views.

The detailed runtime split lives in `docs/contracts/data-model/session-message-model.md`.

At the product layer, the important rule is simpler:

- the shared session ledger remains the public truth
- product views may project that truth differently for different actors
- `soul` may expose internal read-only mechanism views without changing public ledger semantics

## Design Rule

The same underlying truth may be interpreted differently by different layers.

That is intentional.

- public IM view and soul-runtime view are both projections
- neither projection should replace the shared ledger truth
- provider-facing snapshot is another projection again

This is one of the main reasons the session/message model stays actor-based and provider-neutral.
