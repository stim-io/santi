# Service and Assemblies

## Service boundary

- `santi` is the only HTTP runtime host
- all public API endpoints live under `/api/v1`
- 单机 and 分布式 are assembly choices inside the same `santi` service boundary
- `santi-cli` is an HTTP client only and never starts `santi` automatically

## Assembly set

- `standalone`: in-process `santi` assembly with sqlite and strict single-process execution
- `distributed`: `santi` assembly behind deployment-managed infrastructure and external adapter families
- `test`: deterministic assembly for controlled validation

## Bootstrap responsibilities

Bootstrap must finish before the listener accepts traffic.

It is responsible for:

- loading config from supported sources
- resolving the active assembly
- validating required inputs for that assembly
- assembling dependencies for the selected topology
- failing fast on missing or inconsistent inputs

It is not responsible for masking missing configuration or starting with partial dependencies.

## Configuration precedence

Configuration resolves in this order:

1. CLI flags
2. environment variables
3. configuration file
4. defaults

## Topology rules

### `standalone`

- uses sqlite-backed durable state
- is strictly single-process
- does not require postgres or redis
- changes adapter/dependency topology, not service semantics

### `distributed`

- keeps runtime ownership inside `santi`
- depends on external infrastructure and adapter families
- must not introduce topology-specific API drift

### `test`

- builds the same core graph shape with controlled dependencies
- should fail fast on missing required inputs

## Startup and shutdown

- startup begins at the composition root
- the root selects topology, builds dependencies, and starts HTTP last
- shutdown is coordinated from the root and propagates downward in reverse order
- startup must fail before serving traffic if required config, directories, or dependencies are missing

## Standalone-specific invariants

- sqlite is the durable store
- `soul_dir` and `session_dir` stay ordinary directories
- same-session concurrent `send` remains fail-fast `409 Conflict`
- `fork` and explicit `compact` follow the same conflict discipline on the parent session

## Compatibility rules

- `standalone` and `distributed` use the same resource model and HTTP contract
- compatibility is checked through shared service/client version rules, not topology-specific routes
- the CLI default target is the standalone service URL unless overridden by config, env, or CLI flags
