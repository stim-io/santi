# Crate Architecture

This file records the stable crate-layering direction for `santi`.

The exact crate count may still grow, but the layering model should remain stable.

## Layering Goal

`api` should not directly know PostgreSQL, Redis, OpenAI-compatible wire details, or future alternative implementations.

The system should instead separate:

- core runtime model and ports
- infrastructure implementations of those ports
- transport and composition

## Stable Layering

```text
santi-core
  - core data models
  - runtime kernel logic
  - ports / interfaces

santi-db / santi-lock / santi-provider
  - infrastructure adapters
  - concrete implementations of core ports

santi-runtime
  - application/usecase orchestration
  - depends on core ports, not transport

santi-api
  - HTTP/SSE transport
  - schema / openapi
  - config
  - AppState / composition root
```

## Current Meaning Of Each Layer

### `santi-core`

- owns the stable runtime model
- defines the contracts that upper layers depend on
- should not depend on HTTP, SQL, Redis, or provider-specific protocols

Typical contents:

- `model/*`
- `kernel/*`
- `port/*`

### `santi-db`

- implements persistence-facing ports
- owns PostgreSQL/sqlx details today
- may later host SQLite or other storage adapters without changing `api`

Typical contents:

- `db/*`
- `repo/*`
- store adapters such as `turn_store`

### `santi-lock`

- implements lock-facing ports
- owns Redis details today
- may later add local in-process lock implementations

### `santi-provider`

- implements provider-facing ports
- owns OpenAI-compatible wire details today
- may later add mock or alternate provider implementations

### `santi-runtime`

- is the home of usecase orchestration
- should depend on `santi-core` ports rather than concrete infra crates
- should not know HTTP or OpenAPI

Typical future contents:

- `session/send`
- `session/query`
- `session/memory`

### `santi-api`

- should stay transport-focused
- should expose HTTP/SSE routes and map errors/status codes
- should assemble concrete lower-layer implementations into `AppState`
- should not become the long-term home of heavy usecase logic

## Evolution Rule

We may add more infrastructure crates over time, for example:

- MQ
- RAG / retrieval
- background job execution
- alternate provider adapters

But those additions should respect the same layering model:

- define stable ports close to `santi-core`
- keep implementations in lower infrastructure crates
- keep transport-specific concerns in `santi-api`

## Refactor Rule

When a change clearly crosses layers, prefer a decisive refactor over compatibility glue.

In practice:

- do not over-optimize for backward compatibility inside the codebase
- do not preserve old internal module shapes just to reduce churn
- if a boundary is wrong, move the code to the right layer and fix imports decisively

The goal is a cleaner long-term architecture, not a perfectly smooth internal migration.

## Immediate Guidance

- treat `santi-core` as the stable contract layer
- treat `santi-db`, `santi-lock`, and `santi-provider` as replaceable adapter layers
- continue shrinking `santi-api` toward transport + wiring only
- keep `santi-runtime` as the application layer so `santi-api` does not grow back into a permanent application layer

## Runtime-Local Contracts

`santi-runtime` should consume `santi-core` contracts, but `santi-core` should not be forced to model every runtime-local dependency.

Use this rule:

- put stable cross-runtime models and ports in `santi-core`
- let `santi-runtime` define internal contracts when a dependency is clearly runtime-local
- only promote a runtime-local contract upward after real repeated friction shows that it is stable and broadly reusable

Example:

- `bash` is not a `santi-core` concern
- `bash` is a runtime-specific local capability that belongs to runtime strategy and orchestration
- if `santi-runtime` needs an abstraction around local command execution, that abstraction should start as a runtime-local internal contract rather than a `santi-core` port

This avoids two failure modes:

- polluting `santi-core` with premature execution-specific abstractions
- forcing `santi-runtime` to depend directly on concrete lower-layer implementations just because a capability is not yet stable enough for `santi-core`
