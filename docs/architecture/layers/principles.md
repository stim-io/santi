# Layering Principles

## Mental model

- `core` = stable atoms and minimal primitives
- `runtime` = orchestration, semantic execution, and business composition
- middleware (`db` / `ebus` / `lock`) = concrete infrastructure engines
- `api` = transport, config wiring, and boundary-facing integration

## Ownership rules

- `core` exposes only the smallest durable capability set
- `runtime` composes higher-level behavior from those atoms
- middleware implements infrastructure behavior without absorbing runtime policy
- transport layers depend on runtime and contracts, not the reverse

## Decision rule

When placing a capability, ask:

1. Is it a durable atomic primitive?
2. Or is it recomposable behavior?
3. If recomposable, can runtime own it without semantic loss?

Default to keeping atoms in `core` and composition in `runtime`.

## Change rule

If a boundary is wrong, move the code decisively instead of preserving compatibility glue.
