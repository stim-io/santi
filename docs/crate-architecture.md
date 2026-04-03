# Crate Architecture

## Layering

```text
santi-core
  - models
  - ports

santi-db / santi-ebus / santi-lock
  - infrastructure adapters

santi-runtime
  - usecase orchestration

santi-api
  - HTTP/SSE transport
  - config
  - wiring
  - santi-link gateway client for hosted upstream calls
```

## Rules

- `santi-core` should not know HTTP, SQL, Redis, or provider wire details
- infrastructure crates implement core ports
- `santi-runtime` depends on ports, not transport
- `santi-api` stays transport-focused

## Refactor rule

If a boundary is wrong, move the code decisively instead of preserving compatibility glue.
