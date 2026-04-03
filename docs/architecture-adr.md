# Architecture ADR

## Status

Accepted.

## Context

`santi` is converging on a single self-contained HTTP service, with a standalone `santi-cli` as the agent-friendly command-line entrypoint.

The runtime and client boundary needs to be explicit so transport, persistence, and compatibility rules stay stable during the refactor.

## Decision

- `santi` no longer provides an embedded CLI; the old internal CLI host is removed.
- `santi` is the only closed-loop HTTP service.
- `local` and `hosted` modes are owned by `santi` itself.
- `santi-cli` talks only to the `santi` HTTP API.
- `local` mode uses sqlite and stays strictly single-process.
- `--backend` is removed.
- All APIs use `/api/v1`.
- `santi-cli` defaults to the local URL and may be overridden by config or env.
- `santi-cli` never auto-starts `santi`.
- `santi` and `santi-cli` follow `X.Y` compatibility matching.

## Consequences

- The service boundary is simpler: HTTP is the only supported integration path for the CLI.
- Local development becomes a direct service-plus-client flow, not a backend-selection flow.
- Backend-specific CLI routing is deleted instead of preserved behind compatibility shims.
- Single-process sqlite local mode keeps the operational model explicit and constrained.

## Compatibility

- `santi-cli` must only target `santi` HTTP endpoints under `/api/v1`.
- A `santi-cli` release is compatible with `santi` when the `X.Y` version pair matches.
- Config and env may override the default local URL, but they do not change the protocol contract.

## Migration

- Move all CLI entrypoints to the standalone `santi-cli`.
- Route CLI calls to `santi` HTTP only.
- Remove `--backend` paths and any embedded backend selection logic.
- Normalize all exposed endpoints to `/api/v1`.
- Keep local mode single-process with sqlite.
- Remove the old internal CLI host after the new separation is complete.
