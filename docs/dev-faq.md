# Dev FAQ

## Why can e2e fail with `ECONNREFUSED` or `ECONNRESET` right after restarting `santi`?

When `santi` runs through the local root `docker-compose.yml`, the container starts with `cargo run`.
After a rebuild or restart, the service may spend noticeable time recompiling before the HTTP server is ready.

During that window, black-box e2e may fail with:

- `ECONNREFUSED`
- `ECONNRESET`
- early `404` if the request hit the wrong fallback address before `.env` was loaded

Recommended checks:

1. run `docker compose ps santi`
2. inspect `docker compose logs -f santi`
3. wait until `GET /api/v1/health` on `127.0.0.1:18081` succeeds
4. rerun `pnpm test` under `santi/e2e`

Practical rule:

- treat immediate connection failures after restart as service-readiness issues first, not as e2e assertion failures

## Why can `santi/docker-compose.yml up` fail with `port is already allocated`?

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
3. stop the older stack before starting `santi/docker-compose.yml`

Practical rule:

- treat host-port conflicts as local process/environment issues, not as application bugs
