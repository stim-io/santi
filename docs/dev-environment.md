# Dev Environment

This file records two different baselines:

- the minimum tooling for iterating on `santi` from inside the `santi` container itself
- the external root-stack setup used to run the full local integration path

## In-Container Iteration Baseline

The `santi` container should be able to support a narrow self-iteration loop such as:

- clone or copy a `santi` worktree
- edit files through agent tooling

> Note: Inside-container PR workflows require GH_TOKEN or GITHUB_TOKEN for gh.
- run `cargo check`
- use `node` for `scripts/dev/send.mjs` when helpful
- use `gh` for the eventual first PR flow

Minimum in-container tools:

- `cargo` / Rust toolchain
- `git`
- `node`
- `pnpm`
- `gh`
- `bash`

Useful in-container defaults:

- `SANTI_BASE_URL=http://127.0.0.1:8080` so `scripts/dev/send.mjs` can talk to the API from inside the `santi` container without relying on host-mapped port `18081`

This baseline is intentionally smaller than a full general-purpose dev image.

## External Root-Stack Baseline

Required host-side tools for the shared integration stack:

- `cargo` / Rust toolchain: build and run `santi/api`
- `node` + `pnpm`: run the current TypeScript smoke/integration harness under `santi/e2e`
- `docker` + `docker compose`: run the local stack

## Required Local Files

- `providers/auth.json`
  - required by the local `openai-codex-server` service in the root `docker-compose.yml`
  - should stay local and private

## Local Stack Baseline

The default local workflow assumes the repository root `docker-compose.yml` is the single source of truth.

Exposed ports:

- `postgres`: `127.0.0.1:15432`
- `santi`: `127.0.0.1:18081`
- `openai-codex-server`: `127.0.0.1:18082`

## Runtime Environment

Typical values for local development:

- `DATABASE_URL=postgres://santi:santi@127.0.0.1:15432/santi?sslmode=disable`
- `REDIS_URL=redis://127.0.0.1:16379/0`
- `OPENAI_BASE_URL=http://127.0.0.1:18082/openai/v1`
- `OPENAI_API_KEY=codex-local-dev`
- `SANTI_BASE_URL=http://127.0.0.1:18081`

## Smoke Entry Points

- start the shared stack from the repository root: `docker compose up --build`
- use root smoke scripts for the first-pass checks:
  - `scripts/smoke/codex-server.sh`
  - `scripts/smoke/stack.sh`
- the current `santi/e2e` package is legacy harness code and should be treated as smoke/integration scaffolding, not as stable end-to-end truth

## Practical Split

- use the in-container baseline when the goal is to let a running `santi` instance inspect, modify, build, and eventually submit a small PR against `santi`
- use the external root-stack baseline when the goal is to validate the full integration path across postgres / redis / provider / api

## Practical Rule

- if the local stack cannot satisfy `create session -> send session -> SSE -> persistence`, fix the runtime first before expanding test coverage
