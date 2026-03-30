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

## Cold Start Flow

1. start the root stack with `docker compose up -d --build`
2. run `./scripts/cli/setup.sh`
3. run installed `santi-cli` directly, starting with `santi-cli health`
4. use `santi-cli session create` / `santi-cli session send ...` for the normal loop
5. use `--backend api` or `SANTI_CLI_BACKEND=api` when you want the HTTP backend explicitly
6. use `./scripts/cli/reset.sh` only when you want to remove the local install and default CLI state

## Key File Index

- `src/main.rs`: CLI entrypoint and shared command dispatch
- `src/cli.rs`: command-line contract for top-level and `api` namespaces
- `src/config.rs`: config-file and `SANTI_CLI_*` fallback rules
- `src/backend/api.rs`: HTTP/SSE-backed adaptor to `santi-api`
- `src/backend/local.rs`: true local backend wired directly to runtime services
- `src/output.rs`: stdout/stderr rendering behavior
