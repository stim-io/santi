# Local Dev Troubleshooting

## `ECONNREFUSED` or `ECONNRESET` right after restart

Usually `santi` is still compiling or starting.

Check:

1. `docker compose ps santi`
2. `docker compose logs -f santi`
3. `curl http://127.0.0.1:18081/api/v1/health`

## `port is already allocated`

Usually another local stack is still holding the port.

Common ports:

- `18081`
- `18082`

Check:

1. `docker ps --format '{{.Names}} {{.Ports}}'`
2. stop the conflicting stack

## what is the default standalone smoke path now?

Use the installed CLI instead of root-level smoke scripts.

1. `docker compose up -d --build`
2. `./scripts/cli/setup.sh`
3. `santi-cli health`
4. `santi-cli chat 'hello'`
5. `printf 'compact summary' | santi-cli session compact <session_id>`

## how do I inspect standalone runtime state inside docker?

Use `docker compose exec`.

Examples:

1. `docker compose exec santi test -f /data/santi-standalone.sqlite`
2. `docker compose exec santi ls -l /data /runtime`
3. `docker compose logs -f santi`
4. `docker compose logs -f santi-link`

## how do I get back to a clean standalone compose state?

If old sqlite/runtime state is polluting local verification, reset the compose volumes instead of trying to preserve old standalone-only dev data.

1. `docker compose down -v`
2. `docker compose up -d --build`

Typical symptom that means you should do this reset first:

- `500` from `/api/v1/stim/envelopes` or normal send paths with sqlite errors like `table session_messages has no column named updated_at`

That usually means the compose sqlite volume still contains stale standalone-era schema/data that is no longer worth preserving for local verification.
