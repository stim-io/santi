# Dev Environment

## Required host tools

- `cargo`
- `node` + `pnpm`
- `docker` + `docker compose`

## Required local file

- `providers/auth.json`

## Local stack

Use the repository root `docker-compose.yml`.

Ports:

- `santi`: `127.0.0.1:18081`
- `providers`: `127.0.0.1:18082`

Note:

- only `santi` and `providers` are host-exposed in the default stack
- `postgres` and `redis` stay internal to Docker; probe them with `docker exec`, not host ports
- `santi-api` crate defaults are container-oriented (`postgres:5432`, `redis:6379`, `127.0.0.1:8080`) and are expected to be overridden by compose/env in the stack

Probe examples:

```bash
docker compose exec postgres pg_isready -U santi -d santi
docker compose exec postgres psql -U santi -d santi -c 'select 1;'
docker compose exec redis redis-cli ping
docker compose exec redis redis-cli info server
```

## CLI

- install: `./scripts/cli/setup.sh`
- reset: `./scripts/cli/reset.sh`
- stable local smoke should go through installed `santi-cli`, not root-level curl wrappers
- top-level commands talk directly to the configured `base_url`
- keep CLI configuration focused on endpoint and auth inputs, not transport-selection indirection

Typical `~/.santi-cli/config.json`:

```json
{
  "base_url": "http://127.0.0.1:18081"
}
```

Examples:

```bash
docker compose up -d --build
./scripts/cli/setup.sh
santi-cli health
santi-cli chat 'hello'
printf 'hello again' | santi-cli chat --session <session_id>
printf 'compact summary' | santi-cli session compact <session_id>
santi-cli chat 'hello from standalone path'
printf 'compact summary' | santi-cli session compact <session_id>

# optional typed hook instances for dockerized API runtime
export HOOK_SPECS_JSON='[{"id":"auto-compact-threshold","enabled":true,"hook_point":"turn_completed","kind":"compact_threshold","params":{"min_messages_since_last_compact":2}}]'
export HOOK_SPECS_FILE='/app/tmp/hooks.json'
export HOOK_SPECS_URL='https://example.com/hooks.json'
docker compose up -d --build santi

# runtime hook reload without restart (whole-set replace)
curl -X PUT http://127.0.0.1:18081/api/v1/admin/hooks \
  -H 'content-type: application/json' \
  -d '{"hooks":[{"id":"auto-compact-threshold","enabled":true,"hook_point":"turn_completed","kind":"compact_threshold","params":{"min_messages_since_last_compact":2}}]}'

# same operation through santi-cli
printf '[{"id":"auto-compact-threshold","enabled":true,"hook_point":"turn_completed","kind":"compact_threshold","params":{"min_messages_since_last_compact":2}}]' \
  | santi-cli admin hooks reload

# path/url modes for the same reload entrypoint
printf '{"source":"path","path":"/app/tmp/hooks.json"}' | santi-cli admin hooks reload
printf '{"source":"url","url":"http://host.docker.internal:18765/hooks.json"}' | santi-cli admin hooks reload
```

Preferred smoke sequence:

```bash
docker compose up -d --build
./scripts/cli/setup.sh
santi-cli health
santi-cli chat 'hello'
printf 'compact summary' | santi-cli session compact <session_id>
```

## Rule

If the local stack cannot satisfy `create session -> send session -> SSE -> persistence`, fix the runtime first.
