# Provider Gateway

## Boundary

`santi-link/` owns the OpenAI-compatible gateway path used by `santi` assemblies.

### `santi-link` owns

- upstream auth
- token/header injection
- account routing and health
- `/openai/v1/responses` forwarding surface for `santi`

### `santi/` owns

- runtime-facing send orchestration
- the narrow caller that talks to `santi-link`

That caller boundary may be split into small local modules when file shape requires it, but it stays one `santi`-owned repo-local surface responsible for:

- request mapping from runtime provider input into gateway `/responses` calls
- gateway SSE normalization into runtime-facing provider events
- opaque response caching/continuation handling for follow-up tool calls

## Rules

`santi-link` decides how upstream access is reached.

`santi` should not carry a standalone provider crate anymore.

- `standalone` and `distributed` both use the same gateway-facing provider path
