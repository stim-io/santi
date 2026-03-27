# Dev FAQ

## Does `santi` development always require `docker compose` from the host?

No.

There are two different workflows:

- in-container iteration: a running `santi` container can inspect code, edit files, run `cargo`, use `node` helpers, and eventually drive a small PR flow
- external root-stack integration: the repository root `docker-compose.yml` is still the baseline for validating the full local stack

Use the compose-based workflow when the question is about end-to-end integration across services.
Use the in-container workflow when the question is about helping `santi` itself iterate on its own codebase.

## Why can smoke or integration checks fail with `ECONNREFUSED` or `ECONNRESET` right after restarting `santi`?

When `santi` runs through the root `docker-compose.yml`, the container starts with `cargo run`.
After a rebuild or restart, the service may spend noticeable time recompiling before the HTTP server is ready.

During that window, smoke or integration checks may fail with:

- `ECONNREFUSED`
- `ECONNRESET`
- early `404` if the request hit the wrong fallback address before `.env` was loaded

Recommended checks:

1. run `docker compose ps santi`
2. inspect `docker compose logs -f santi`
3. wait until `GET /api/v1/health` on `127.0.0.1:18081` succeeds
4. rerun the relevant smoke script or harness command

Practical rule:

- treat immediate connection failures after restart as service-readiness issues first, not as product-behavior failures

## Why can `docker compose up` fail with `port is already allocated`?

This usually means an older local stack is still holding the same host ports.

Common conflicts:

- `15432` for PostgreSQL
- `18081` for `santi`
- `18082` for `openai-codex-server`
- `16379` for Redis

Typical cause:

- an older compose project is still running from another compose file or project root

Recommended checks:

1. run `docker ps --format '{{.Names}} {{.Ports}}'`
2. identify the container already binding the conflicting port
3. stop the older stack before starting the root compose project

Practical rule:

- treat host-port conflicts as local process/environment issues, not as application bugs
