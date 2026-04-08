# 0001 Service Boundary

## Status

Accepted.

## Context

`santi` is converging on a single self-contained HTTP service, with a standalone `santi-cli` as the agent-friendly command-line entrypoint.

The runtime and client boundary needs to be explicit so transport, persistence, adapter topology, and compatibility rules stay stable.

The previous wording was misleading because both paths terminate in the same `santi` service boundary. The real distinction is 中文 `单机` vs `分布式` assembly, with code aligned to `standalone` / `distributed`.

## Decision

- `santi` no longer provides an embedded CLI; the old internal CLI host is removed.
- `santi` is the only closed-loop HTTP service.
- 单机 and 分布式 assembly are both owned by `santi` itself.
- `santi-cli` talks only to the `santi` HTTP API.
- 单机 uses sqlite and stays strictly single-process.
- All APIs use `/api/v1`.
- `santi-cli` defaults to the standalone URL and may be overridden by config or env.
- `santi-cli` never auto-starts `santi`.
- `santi` and `santi-cli` follow `X.Y` compatibility matching.

## Consequences

- The service boundary is simpler: HTTP is the only supported integration path for the CLI.
- 单机 development is a direct service-plus-client flow.
- 单机 and 分布式 differ by dependency topology and adapter family, not by whether a service exists.
- Single-process sqlite 单机 keeps the operational model explicit and constrained.

## Compatibility

- `santi-cli` must only target `santi` HTTP endpoints under `/api/v1`.
- A `santi-cli` release is compatible with `santi` when the `X.Y` version pair matches.
- Config and env may override the default standalone URL, but they do not change the protocol contract.
