# Architecture Overview

## Goal

We are building a customized personal agent service.

The goal is not to replicate reference projects. The goal is to define a small, stable runtime model that can grow from chat into an agent with session, tools, memory, compacting, and branching.

## Direction

`santi` should operate through its own canonical runtime paths so it can inspect, explain, and improve its own behavior.

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

- infinite context should not be treated as an available design foundation
- longer context windows do not remove the need for state management, retrieval, and compaction
- runtime design should stay conservative and layered rather than assuming raw history can be carried forever

## Product Route

The system grows from streaming chat into a persistent personal-agent runtime with sessions, tools, memory, compacting, and workspace resources. The concrete object model and constraints live in `docs/contracts/data-model/session-message-model.md`.

## Core Principles

- Prefer a small number of stable primitives over early abstraction growth.
- Separate shared public-ledger facts from soul-internal runtime state.
- Treat provider snapshots as projections assembled from canonical state, not as source-of-truth objects.
- Assume finite working capacity; memory, retrieval, and compaction remain core runtime responsibilities.
- Keep upper-layer product mappings and tenant logic above the core runtime until they are actually needed.
- Prefer real runtime and OS boundaries, with normal directories and ordinary process execution, over early artificial permission machinery.

See:

- `docs/architecture/runtime/glossary.md`
- `docs/architecture/runtime/lifecycle-and-hooks.md`
- `docs/architecture/product/stim-boundary.md`
- `docs/contracts/data-model/session-message-model.md`

## HTTP Exposure

At the current stage, HTTP capabilities are intentionally open and `session` is a work unit rather than an isolation boundary. Future `scope` / `tenant` support may add isolation later, but that layer should not distort the core runtime model.

## Reference-Project Lens

Reference projects help us choose principles, not implementations.

### `opencode`

- Teaches focused agent behavior inside a task environment.
- Most relevant for tool use, workspace flow, and coding-agent UX.

### `openclaw`

- Teaches long-lived personal-agent framing across channels and capabilities.
- Most relevant for gateway thinking, persistent agent identity, and extensibility.
