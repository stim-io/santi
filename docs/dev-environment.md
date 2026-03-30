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

- `postgres`: `127.0.0.1:15432`
- `redis`: `127.0.0.1:16379`
- `santi`: `127.0.0.1:18081`
- provider service (`openai-codex-server`): `127.0.0.1:18082`

Note:

- these are the host-facing stack ports used by local CLI/config examples
- `santi-api` crate defaults are container-oriented (`postgres:5432`, `redis:6379`, `127.0.0.1:8080`) and are expected to be overridden by compose/env in the stack

## CLI

- install: `./scripts/cli/setup.sh`
- reset: `./scripts/cli/reset.sh`
- top-level commands default to the configured backend
- backend selection priority: `--backend` > `SANTI_CLI_BACKEND` > `config.json` > default `local`
- explicit API compatibility form: `santi-cli api ...`

Typical `~/.santi-cli/config.json`:

```json
{
  "backend": "local",
  "base_url": "http://127.0.0.1:18081",
  "database_url": "postgres://santi:santi@127.0.0.1:15432/santi?sslmode=disable",
  "redis_url": "redis://127.0.0.1:16379/0",
  "openai_base_url": "http://127.0.0.1:18082/openai/v1",
  "openai_api_key": "codex-local-dev",
  "openai_model": "gpt-5.4"
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
