# Composition Root

## Goal and non-goals

Goal: define the assembly boundary for `santi` as the only HTTP runtime host in the system.

Non-goals:

- do not describe transport internals or storage implementation details
- do not preserve standalone runtime hosting inside any internal CLI host
- do not introduce alternative HTTP roots or duplicated startup paths

## Top-level components

- `santi` runtime host: owns HTTP serving, topology assembly, and shutdown coordination
- 单机 assembly: in-process `santi` runtime with sqlite and strict single-process execution
- 分布式 assembly: deployed `santi` runtime with the same HTTP contract but different dependency and adapter topology
- `test` assembly: composition for deterministic non-network verification
- standalone `santi-cli`: HTTP client only; it connects to `santi` and does not host runtime state

## Composition root rules

- `santi` is the sole HTTP runtime host
- all HTTP surface area uses the unified `/api/v1` prefix
- 单机 and 分布式 both assemble into `santi`; they do not branch into separate runtime owners
- 单机 uses sqlite and must remain strictly single-process
- the old internal CLI host is gone; standalone CLI ownership lives in `../santi-cli/`

## Dependency direction

- UI and CLI code depend on `santi` HTTP, not on runtime internals
- runtime internals depend inward on stable domain and service primitives
- topology assembly depends on shared contracts, not on ad hoc cross-topology hooks
- 分布式 deployment adds infrastructure concerns around `santi`, but does not own runtime semantics

## Startup and shutdown flow

- startup begins at the `santi` composition root
- the root selects a topology, builds dependencies, and starts HTTP last
- shutdown is coordinated from the root and propagates downward in reverse order
- `test` should assemble the same core graph shape without requiring external services

## Topology-specific assembly

### 单机 (`standalone`)

- assemble `santi` in-process
- use sqlite
- enforce single-process execution and fail fast on conflicting concurrency

### 分布式 (`distributed`)

- assemble the same `santi` HTTP host behind deployment infrastructure
- keep runtime ownership inside `santi`
- avoid topology-specific API drift

### test

- build a minimal deterministic graph
- prefer injected fakes and controlled error paths over network dependencies
- keep test assembly aligned with the same dependency direction as production

## Observability and error injection

- observability should be rooted at composition so startup, shutdown, and topology selection are visible
- error injection should target boundary points in the assembled graph
- prefer deterministic failure surfaces over broad global hooks
- do not rely on runtime internals leaking topology-specific test controls

## Crate refactor constraints

- preserve `santi` as the composition owner for runtime hosting
- keep CLI ownership outside `santi/` and do not reintroduce an internal CLI host
- do not move HTTP hosting back into the CLI crate
- do not split the unified `/api/v1` contract across crates
