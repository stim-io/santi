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
santi-cli session create
printf 'hello' | santi-cli session send <session_id>
SANTI_CLI_BACKEND=api santi-cli health
```

## Rule

If the local stack cannot satisfy `create session -> send session -> SSE -> persistence`, fix the runtime first.
