# Local Dev Setup

## Required host tools

- `cargo`
- `node` + `pnpm`
- `docker` + `docker compose`

## Required local file

- `santi-link/auth.json`

## Local stack

Use the repository root `docker-compose.yml` for support services and prefer repo-root foreground execution for `santi` during integrated `stim -> santi` iteration.

Ports:

- `santi`: `127.0.0.1:18081`
- `santi-link`: `127.0.0.1:18082`
- `stim-server`: `127.0.0.1:18083`

Note:

- the preferred current loop starts support services with `docker compose up -d --build stim-server santi-link`
- start local foreground `santi` from the repo root with `scripts/santi local`
- local foreground `santi` runs standalone, uses sqlite under `.tmp/local-santi`, binds `127.0.0.1:18081`, and uses the normal provider path through `santi-link`
- for a DeepSeek foreground run, start `santi` from the repo root with `scripts/santi deepseek`; the helper reads provider values from the process environment first, then the ignored repo-root `.env`, then the macOS launch environment. For the required `DEEPSEEK_API_KEY`, it also falls back to the user's shell environment, does not print the secret, sets `SANTI_PROVIDER_API=chat-completions`, and defaults to `deepseek-chat` against `https://api.deepseek.com`
- containerized `santi` remains available through the explicit `container-santi` compose profile

Probe examples:

```bash
docker compose up -d --build stim-server santi-link
```

In another repo-root shell:

```bash
scripts/santi local
```

DeepSeek foreground variant:

```bash
scripts/santi deepseek
```

Then probe from any shell:

```bash
curl http://127.0.0.1:18081/api/v1/health
docker compose logs -f santi-link
```

Use containerized `santi` only when the container path itself is under inspection:

```bash
docker compose --profile container-santi up -d --build santi
```

Reset local foreground state when old sqlite/runtime contents are getting in the way:

```bash
rm -rf .tmp/local-santi
```

Reset container state when old compose volumes are getting in the way:

```bash
docker compose down -v
```

Use reset for local verification hygiene. Do not add a migration layer just to preserve old standalone dev state.

If you hit sqlite/schema mismatch errors during local verification (for example `table session_messages has no column named updated_at`), treat that as a reset-first local state problem rather than as a migration task.

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

Start support services:

```bash
docker compose up -d --build stim-server santi-link
```

Keep local foreground `santi` running in another repo-root shell:

```bash
scripts/santi local
```

Run CLI checks from `modules/santi`:

```bash
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
docker compose --profile container-santi up -d --build santi

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

Start support services and keep local foreground `santi` running as above, then run:

```bash
./scripts/cli/setup.sh
santi-cli health
santi-cli chat 'hello'
printf 'compact summary' | santi-cli session compact <session_id>
```

## Rule

If the local stack cannot satisfy `create session -> send session -> SSE -> persistence`, fix the runtime first.
