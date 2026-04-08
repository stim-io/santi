# Distributed Provider Gateway

## Boundary

`santi-link/` owns the 分布式 OpenAI-compatible gateway path.

### `santi-link` owns

- upstream auth
- token/header injection
- account routing and health
- `/openai/v1/responses` forwarding surface for 分布式 assembly

### `santi/` owns

- runtime-facing send orchestration
- the narrow distributed caller that talks to `santi-link`

## Rules

`santi-link` decides how 分布式 upstream access is reached.

`santi` should not carry a standalone provider crate anymore.

- `distributed` is the implementation naming for this path inside `santi`
