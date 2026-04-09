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

## Rules

`santi-link` decides how upstream access is reached.

`santi` should not carry a standalone provider crate anymore.

- `standalone` and `distributed` both use the same gateway-facing provider path
