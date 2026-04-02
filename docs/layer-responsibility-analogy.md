# Layer Responsibility Analogy

Use this as a cold-start mental model for the `santi/` stack:

- `core` = stable atoms and minimal primitives
- `runtime` = semantic execution model, including orchestration and spec-like behavior
- middleware (`db` / `ebus` / `lock`) = concrete backend engines

## How to read the analogy

- `core` should expose only the smallest durable capability set.
- `runtime` is where higher-level behavior gets composed, sequenced, and made operational.
- middleware should provide real infrastructure behavior, not absorb orchestration logic.

The key rule is simple: if a behavior can be recomposed from lower-level atomic operations without meaningful semantic loss, it belongs in `runtime` or a shim layer, not in core traits.

## Limits of the analogy

- It is a boundary guide, not a strict architecture proof.
- Some concerns will cross layers in implementation, but the ownership of behavior should still stay clear.
- “Spec” here means executable semantic shape plus orchestration, not prose documentation alone.
- Concrete engines can be powerful, but they should remain infrastructure, not become the place where runtime policy lives.

## Practical use

When choosing where a new capability belongs, ask:

1. Is this a durable atomic primitive?
2. Or is it a recomposable higher-level behavior?
3. If it is recomposable, can `runtime` own it without losing meaning or performance?

Default to keeping the atom in `core` and the composition in `runtime`.
