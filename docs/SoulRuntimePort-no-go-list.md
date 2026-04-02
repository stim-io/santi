# SoulRuntimePort stricter exclusion / no-go list

This note keeps the negative boundary explicit while the port is still under active refactor pressure.
The goal is not to split `SoulRuntimePort` now, but to keep the single main trait focused and avoid baking in convenience or layering mistakes.

## Why lock the negative boundary first

- It is easier to rule out bad shapes than to recover from a trait that already encodes them.
- A premature trait sketch usually reflects today’s implementation pressure, not the durable runtime boundary.
- Once a port is published, every extra method, parameter, or cross-layer dependency becomes expensive to remove.
- A no-go list keeps the design conversation focused on what the port must not absorb from adjacent layers.

## No-go items by type

### 1. 职责越界

Do not let `SoulRuntimePort` become a grab bag for concerns that belong elsewhere.

- Do not mix runtime execution with product-level session ledger policy.
- Do not move CLI orchestration, command parsing, or transport selection into the port.
- Do not fold auth gateway responsibilities, tenant policy, or upstream account handling into the port.
- Do not make the port own persistence schema evolution, migration strategy, or storage layout policy.

### 2. 层级泄漏

Do not let internal layering details escape upward through the port surface.

- Do not expose crate-private runtime internals as part of the public contract.
- Do not force callers to understand hook reload mechanics, file layout, or internal resource wiring.
- Do not leak storage backend choices, lock primitives, or process-mode assembly details through trait methods.
- Do not make the port depend on implementation-local types whose only purpose is to mirror current code structure.

### 3. 拆分破坏

Do not use the port to glue together things that should remain separable.

- Do not merge soul execution, session ledger mutation, and external transport concerns into one all-purpose call path.
- Do not require unrelated capabilities to be passed together just because the current implementation happens to bundle them.
- Do not make the port couple cold-start bootstrap, per-turn execution, and lifecycle hooks into a single mandatory shape.
- Keep a single main `SoulRuntimePort` trait for now.
- Do not introduce subtraits before the runtime boundary has been fully decomposed and validated.

### 4. 时间耦合

Do not encode sequencing assumptions that make the port brittle.

- Do not require hidden ordering dependence on startup side effects.
- Do not assume a specific session turn timing model beyond what the runtime contract already guarantees.
- Do not bake in retry, queueing, or implicit serialization semantics that belong to higher-level orchestration.
- Do not tie the port to transient initialization order or background task scheduling details.

### 5. 便利性膨胀

Do not add helpers just because they are easy to call from today’s code.

- Do not add “nice to have” methods that merely wrap existing internals.
- Do not widen the surface to save a small amount of call-site plumbing.
- Do not add convenience accessors for data that callers should not own directly.
- Do not accept optional arguments or catch-all structs that hide unclear responsibility boundaries.

### 6. 未来锁死

Do not choose shapes that make later narrowing or split-out impossible.

- Do not commit to a method set that assumes the current runtime will stay monolithic.
- Do not encode a return type that forces downstream code to depend on internal representation details.
- Do not introduce generic abstraction knobs before we know which variation actually matters.
- Do not blur the boundary where core defines atomic traits, middleware implements them through adapters, and runtime composes them.

## Concrete prohibitions for the current `SoulRuntimePort` situation

Given the current `santi/` boundary, the port must not:

- become the place where session ledger, soul runtime, and CLI behavior are all unified;
- expose storage-specific controls for the sake of implementation convenience;
- carry hook reload or file-system concerns as part of its public contract;
- absorb concurrency policy beyond the already established fail-fast boundary;
- require real-model-call behavior for validation or testing purposes;
- be shaped around current crate boundaries instead of the durable runtime boundary.

## Conditions before entering trait sketching

Do not start a trait sketch until all of the following are true:

- the negative boundary is agreed and stable enough to survive implementation churn;
- the runtime role of `SoulRuntimePort` is described in terms of responsibility, not method names;
- the port’s callers and owners are identified without crossing into adjacent layers;
- the minimal stable primitives are known and the convenience-only items have been excluded or decomposed;
- the design can be reviewed without depending on current internal types or crate layout;
- the sketch can be evaluated against concrete no-go items above, not just intuition.

## Reminder

This document is intentionally exclusion-first.
When the port shape is ready, it should still be a single main trait unless and until the runtime boundary itself justifies narrower traits.
