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
