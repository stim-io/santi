# Design Notes

This file collects high-level design fragments that are worth keeping, but are not yet stable enough to treat as current model truth or implementation direction.

Use it for ideas that may later affect product, architecture, or runtime semantics.

## Notes

### The Three-Body Person Motif

Idea:

- the "Three-Body person" image should be treated as a long-running design motif for `stim + santi`
- it is not only a metaphor for `soul`
- it is a way to think about how public communication and internal cognition may coexist, diverge, and sometimes become observable

Why it matters:

- many later product and architecture decisions will depend on how we think about the relationship between external speech and internal thought
- this motif gives a durable intuition for why `stim` should not collapse into a traditional IM product
- it also gives a durable intuition for why `santi` should not collapse into a hidden internal runtime with no meaningful projection path
- it helps explain why actor-internal traces may eventually become product-visible without being identical to public session messages

Current working value:

- public session ledger remains shared truth
- actor-internal cognition may still have its own truth space
- some internal truth may eventually be projected in read-only form
- `soul` is simply the first and strongest place where this pressure appears

Not now because:

- the motif is strategically important, but it is still too high-level to turn directly into fixed schema rules
- the current job is to let it guide model boundaries and product direction without forcing premature low-level abstractions

Related docs:

- `docs/stim-santi-boundary.md`
- `docs/session-message-actor-model.md`
- `docs/runtime-primitives.md`

Reference fragment:

```text
字幕：不只是面对面，我们可以在相当远的距离上交流。另外，欺骗和撒谎这两个词我们一直难以理解。

伊文斯：“一个思想全透明的社会是怎样的社会？会产生怎样的文化、怎样的政治？你们没有计谋，不可能伪装。”

字幕：计谋和伪装是什么？

---- 摘自《三体》刘慈欣老师
```

### Compact May Be Actor-Level, Not Soul-Only

Idea:

- `compact` may not be fundamentally soul-exclusive
- it may be better understood as an actor-level thinking snapshot
- it only appears soul-heavy right now because current runtime intelligence is concentrated in the soul system

Why it matters:

- this changes how future read-only thought traces might be explained in `stim`
- it may eventually move `compact` from a soul-specific mechanism to a more general actor-side cognition primitive
- it keeps open the possibility that multiple actor kinds could accumulate and expose internal working-state snapshots

Not now because:

- current implementation and current runtime design pressure still live overwhelmingly in the soul system
- treating `compact` as soul-internal remains the simplest working model for now
- the actor-level generalization is conceptually promising, but not yet necessary for the first clean rebuild
- the canonical first-pass model should still treat `compact` as a `soul_session` runtime artifact, not as a public or actor-generic primitive

Related docs:

- `docs/stim-santi-boundary.md`
- `docs/session-message-actor-model.md`
- `docs/runtime-primitives.md`
