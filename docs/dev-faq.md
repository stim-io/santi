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
