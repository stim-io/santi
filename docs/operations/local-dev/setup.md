# Local Dev Setup

## Required host tools

- `cargo`
- `node` + `pnpm`
- `docker` + `docker compose`

## Required local file

- `santi-link/auth.json`

## Local stack

Use the repository root `docker-compose.yml`.

Ports:

- `santi`: `127.0.0.1:18081`
- `santi-link`: `127.0.0.1:18082`

Note:

- only `santi` and `santi-link` are host-exposed in the default stack
- the default Docker cold start is standalone-only: `santi` uses sqlite at `/data/santi-standalone.sqlite`
- standalone still uses the normal provider path through `santi-link`; the topology change is storage/locking/bootstrap, not send semantics

Probe examples:

```bash
docker compose exec santi test -f /data/santi-standalone.sqlite
docker compose exec santi ls -l /data /runtime
docker compose logs -f santi
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

# path/url modes for the same reload entrypoint
curl -X PUT http://127.0.0.1:18081/api/v1/admin/hooks \
  -H 'content-type: application/json' \
  -d '{"source":"path","path":"/app/tmp/hooks.json"}'

curl -X PUT http://127.0.0.1:18081/api/v1/admin/hooks \
  -H 'content-type: application/json' \
  -d '{"source":"url","url":"http://host.docker.internal:18765/hooks.json"}'
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
