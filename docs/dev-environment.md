# Dev Environment

This file records the local development baseline required to run `santi` and its black-box e2e flow.

## Required Tools

- `cargo` / Rust toolchain: build and run `santi/api`
- `node` + `pnpm`: manage and run `santi/e2e`
- `docker` + `docker compose`: run the local stack

## Required Local Files

- `openai-codex-server/auth.json`
  - required by the local `openai-codex-server` service in the root `docker-compose.yml`
  - should stay local and private

## Local Stack Baseline

The default local workflow assumes the root `docker-compose.yml` is the source of truth.

Exposed ports:

- `postgres`: `127.0.0.1:15432`
- `santi`: `127.0.0.1:18081`
- `openai-codex-server`: `127.0.0.1:18082`

## Runtime Environment

Typical values for local development:

- `DATABASE_URL=postgres://santi:santi@127.0.0.1:15432/santi`
- `OPENAI_BASE_URL=http://127.0.0.1:18082/api/v1`
- `OPENAI_API_KEY=codex-local-dev`
- `SANTI_BASE_URL=http://127.0.0.1:18081`

## E2E Notes

- `santi/e2e/.env.example` should mirror the local docker-compose baseline
- `vitest` setup loads environment through `dotenv`
- e2e assumes the local stack is already running unless a test explicitly bootstraps it

## Practical Rule

- if the local stack cannot satisfy `create session -> send session -> SSE -> persistence`, fix the runtime first before expanding test coverage
