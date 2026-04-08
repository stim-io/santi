# Service Config and Bootstrap

## Scope

This document defines the configuration and bootstrap boundary for the `santi` service host.

- `santi` is the only HTTP host in this layer.
- All public API endpoints live under `/api/v1`.
- the primary conceptual axis is topology: 中文 `单机` vs `分布式`.
- runtime identifiers are `standalone`, `distributed`, and `test` for assembly selection.
- `santi-cli` is a pure HTTP client. It connects to a configured URL and does not start `santi` automatically.

## Bootstrap responsibilities

Service startup must complete configuration validation and dependency assembly before the HTTP listener begins accepting traffic.

Bootstrap is responsible for:

- loading configuration from all supported sources
- resolving the active assembly path
- validating required dependencies for that topology
- assembling the runtime dependencies for the selected topology
- failing fast when required inputs are missing or inconsistent

Bootstrap is not responsible for masking missing configuration or starting with partial dependencies.

## Configuration sources and precedence

Configuration is resolved in this order:

1. CLI flags
2. environment variables
3. configuration file
4. defaults

Later sources only apply when earlier sources do not provide a value.

## Topology selection

Configuration selects an internal `santi` assembly path.

- 单机: single-process `santi` assembly with sqlite-backed storage
- 分布式: `santi` assembly that depends on external adapter families and deployment-managed dependencies
- test: test-oriented assembly for controlled validation

The selected topology determines required configuration, dependency checks, and startup validation.

## Required config by topology

### 单机 (`standalone`)

单机 requires:

- SQLite configuration
- the directory or path needed to create and use the standalone sqlite database

If the SQLite configuration or required directory is missing, startup must fail.

单机 is strictly single-process.

### 分布式 (`distributed`)

分布式 requires the external dependency configuration for the selected adapter family and deployment topology.

If any required external dependency configuration is missing, startup must fail.

### test

`test` must use a controlled, explicit assembly suitable for automated validation and should still fail fast on missing required inputs.

## Addressing defaults

The service should use stable addressing defaults that match the current boundary:

- server APIs are served from `/api/v1`
- CLI defaults point to the 单机 service URL

`santi-cli` defaults to the standalone HTTP endpoint unless overridden by config, environment, or CLI flags.

## Startup validation and fail-fast

Startup must validate the active configuration before opening the listener.

Validation includes:

- detecting the effective assembly identifier and topology
- checking required config for the selected topology
- verifying that required directories, paths, and dependencies exist or are reachable
- rejecting unsupported or incomplete combinations early

If validation fails, the process must exit before serving HTTP.

## Observability at bootstrap

Bootstrap should expose enough visibility to explain startup success or failure without relying on runtime traffic.

Focus on:

- selected assembly identifier
- selected topology
- resolved config source order
- validated dependency set
- bootstrap failures and the reason for exit

Keep bootstrap observability concise and operational.

## Smoke validation

Smoke checks should verify the service boundary after bootstrap:

- the process starts only after config validation completes
- the selected topology matches the expected assembly
- `/api/v1` is reachable on the configured service address
- `santi-cli` can connect to the configured HTTP endpoint without starting the service itself

Smoke validation should confirm boundary behavior, not implementation internals.
