# Service Config and Bootstrap

## Scope

This document defines the configuration and bootstrap boundary for the `santi` service host.

- `santi` is the only HTTP host in this layer.
- All public API endpoints live under `/api/v1`.
- `mode` is an internal assembly choice: `local`, `hosted`, or `test`.
- `santi-cli` is a pure HTTP client. It connects to a configured URL and does not start `santi` automatically.

## Bootstrap responsibilities

Service startup must complete configuration validation and dependency assembly before the HTTP listener begins accepting traffic.

Bootstrap is responsible for:

- loading configuration from all supported sources
- resolving the active mode
- validating required dependencies for that mode
- assembling the runtime dependencies for the selected mode
- failing fast when required inputs are missing or inconsistent

Bootstrap is not responsible for masking missing configuration or starting with partial dependencies.

## Configuration sources and precedence

Configuration is resolved in this order:

1. CLI flags
2. environment variables
3. configuration file
4. defaults

Later sources only apply when earlier sources do not provide a value.

## Mode selection

`mode` selects the internal service assembly.

- `local`: single-process service with local SQLite storage
- `hosted`: service configured for external hosted dependencies
- `test`: test-oriented assembly for controlled validation

The selected mode determines required configuration, dependency checks, and startup validation.

## Required config by mode

### local

`local` mode requires:

- SQLite configuration
- the directory or path needed to create and use the local database

If the SQLite configuration or required directory is missing, startup must fail.

`local` mode is strictly single-process.

### hosted

`hosted` mode requires the external dependency configuration needed by the hosted deployment.

If any required external dependency configuration is missing, startup must fail.

### test

`test` mode must use a controlled, explicit assembly suitable for automated validation and should still fail fast on missing required inputs.

## Addressing defaults

The service should use stable addressing defaults that match the current boundary:

- server APIs are served from `/api/v1`
- CLI defaults point to the local service URL

`santi-cli` defaults to the local HTTP endpoint unless overridden by config, environment, or CLI flags.

## Startup validation and fail-fast

Startup must validate the active configuration before opening the listener.

Validation includes:

- detecting the effective mode
- checking required config for the selected mode
- verifying that required directories, paths, and dependencies exist or are reachable
- rejecting unsupported or incomplete combinations early

If validation fails, the process must exit before serving HTTP.

## Observability at bootstrap

Bootstrap should expose enough visibility to explain startup success or failure without relying on runtime traffic.

Focus on:

- selected mode
- resolved config source order
- validated dependency set
- bootstrap failures and the reason for exit

Keep bootstrap observability concise and operational.

## Smoke validation

Smoke checks should verify the service boundary after bootstrap:

- the process starts only after config validation completes
- the selected mode matches the expected assembly
- `/api/v1` is reachable on the configured service address
- `santi-cli` can connect to the configured HTTP endpoint without starting the service itself

Smoke validation should confirm boundary behavior, not implementation internals.
