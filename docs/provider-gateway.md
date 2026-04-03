# Provider Gateway

## Split

`santi-link/` now owns the hosted OpenAI-compatible gateway path.

### `santi-link` owns

- upstream auth
- token/header injection
- account routing and health
- `/openai/v1/responses` forwarding surface for hosted mode

### `santi/` owns

- runtime-facing send orchestration
- the narrow hosted caller that talks to `santi-link`

## Rule

`santi-link` decides how hosted upstream access is reached.

`santi` should not carry a standalone provider crate anymore.
