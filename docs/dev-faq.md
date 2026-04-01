# Dev FAQ

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

## what is the default local smoke path now?

Use the installed CLI instead of root-level smoke scripts.

1. `docker compose up -d --build`
2. `SANTI_CLI_BACKEND=api ./scripts/cli/setup.sh`
3. `santi-cli health`
4. `santi-cli chat 'hello'`
5. `printf 'compact summary' | santi-cli session compact <session_id>`

## how do I inspect postgres or redis now that they are not host-exposed?

Use `docker compose exec`.

Examples:

1. `docker compose exec postgres pg_isready -U santi -d santi`
2. `docker compose exec postgres psql -U santi -d santi -c 'select 1;'`
3. `docker compose exec redis redis-cli ping`
4. `docker compose exec redis redis-cli info clients`
