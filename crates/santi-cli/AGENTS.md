# AGENTS

## Purpose

This file records the stable role and local constraints of `santi-cli`.

## Core Constraints

- `santi-cli` exposes one top-level command surface.
- backend selection should be driven by `--backend`, `SANTI_CLI_BACKEND`, and config file defaults.
- `santi-cli api ...` remains a compatibility-style explicit API namespace.
- All externally configurable environment variables for `santi-cli` must use the `SANTI_CLI_*` prefix.
- The stable locator defaults are `SANTI_CLI_HOME=~/.santi-cli` and `SANTI_CLI_CONFIG_FILE=~/.santi-cli/config.json`.
- Runtime configuration should be read from `config.json` by default, with CLI flags overriding env and env overriding file values.
- `scripts/cli/setup.sh` should assume the local root `docker-compose.yml` defaults and produce an out-of-box local-backed config without extra probing or defensive checks.
- Fallback handling should stay simple: if the local stack shape differs, update `~/.santi-cli/config.json` or override with `SANTI_CLI_*` for that run.
- Keep one stable command vocabulary across top-level and `api` forms.
- Do not document or imply a backend selector field unless the code actually reads one.
- The normal operator path is `santi-cli chat <message>`; continuing a session should use `santi-cli chat --session <id> <message>` or stdin with `--session <id>`.
- Backend-specific wrapper scripts should not carry user-facing chat semantics; use backend flags/env on `santi-cli` itself instead.
- Explicit compact verification should use `printf '<summary>' | santi-cli session compact <session_id>` and the same command with `--backend api` for the HTTP path.
- Hook instances are typed in Rust but loaded dynamically from config/env; the current CLI-side env override is `SANTI_CLI_HOOKS_JSON`.
- Startup hook input may come from `SANTI_CLI_HOOKS_JSON`, `SANTI_CLI_HOOKS_FILE`, or `SANTI_CLI_HOOKS_URL`.
- Hook loading is now routed through a registry holder that supports atomic whole-set replacement; file watching and live reload triggers are still deferred.
- The current service-side management entrypoint for hook replacement is `PUT /api/v1/admin/hooks` with a whole hook-set payload.
- The preferred operator path for service-side replacement is now `santi-cli --backend api admin hooks reload` with an inline value / path / url payload on stdin.

## Cold Start Flow

1. start the root stack with `docker compose up -d --build`
2. run `./scripts/cli/setup.sh`
3. run installed `santi-cli` directly, starting with `santi-cli health`
4. use `santi-cli chat ...` for the normal loop; use `--session <id>` when you want to continue an existing session explicitly
5. use `santi-cli --backend api chat ...` or `SANTI_CLI_BACKEND=api santi-cli chat ...` when you want the HTTP backend explicitly
6. use `printf '[...]' | santi-cli --backend api admin hooks reload` when you want to replace the active hook set on a running API service
7. use `./scripts/cli/reset.sh` only when you want to remove the local install and default CLI state

## Key File Index

- `src/main.rs`: CLI entrypoint and shared command dispatch
- `src/cli.rs`: command-line contract for top-level and `api` namespaces
- `src/config.rs`: config-file and `SANTI_CLI_*` fallback rules
- `src/output.rs`: stdout/stderr rendering behavior and chat/session hints
- `src/backend/api.rs`: HTTP/SSE-backed adaptor to `santi-api`
- `src/backend/local.rs`: true local backend wired directly to runtime services
