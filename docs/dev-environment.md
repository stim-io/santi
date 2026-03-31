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
- top-level commands default to the configured backend
- backend selection priority: `--backend` > `SANTI_CLI_BACKEND` > `config.json` > default `local`
- explicit API compatibility form: `santi-cli api ...`
- with the default compose stack, host-side CLI usage should prefer `api` backend because `postgres` and `redis` are not host-exposed

Typical `~/.santi-cli/config.json`:

```json
{
  "backend": "api",
  "base_url": "http://127.0.0.1:18081",
  "openai_base_url": "http://127.0.0.1:18082/openai/v1",
  "openai_api_key": "codex-local-dev",
  "openai_model": "gpt-5.4"
}
```

Examples:

```bash
docker compose up -d --build
./scripts/cli/setup.sh
santi-cli --backend api health
santi-cli --backend api chat 'hello'
printf 'hello again' | santi-cli --backend api chat --session <session_id>
printf 'compact summary' | santi-cli --backend api session compact <session_id>
santi-cli --backend api chat 'hello from api backend'
printf 'compact summary' | santi-cli --backend api session compact <session_id>
SANTI_CLI_BACKEND=api santi-cli health

# optional typed hook instances for local CLI runtime
export SANTI_CLI_HOOKS_JSON='[{"id":"auto-compact-threshold","enabled":true,"hook_point":"turn_completed","kind":"compact_threshold","params":{"min_messages_since_last_compact":2}}]'
santi-cli chat 'auto compact local'

# or point startup at a file / url instead of inline JSON
export SANTI_CLI_HOOKS_FILE='/absolute/path/to/hooks.json'
export SANTI_CLI_HOOKS_URL='https://example.com/hooks.json'

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
  | santi-cli --backend api admin hooks reload

# path/url modes for the same reload entrypoint
printf '{"source":"path","path":"/app/tmp/hooks.json"}' | santi-cli --backend api admin hooks reload
printf '{"source":"url","url":"http://host.docker.internal:18765/hooks.json"}' | santi-cli --backend api admin hooks reload
```

## Rule

If the local stack cannot satisfy `create session -> send session -> SSE -> persistence`, fix the runtime first.
