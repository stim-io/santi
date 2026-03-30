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

- `15432`
- `18081`
- `18082`
- `16379`

Check:

1. `docker ps --format '{{.Names}} {{.Ports}}'`
2. stop the conflicting stack
