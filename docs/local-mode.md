# Local Mode

## Goal and non-goals

Goal: define `local` as an internal `santi` mode that assembles the same stable HTTP service for single-process development and verification.

Non-goals:

- do not add any local-only HTTP API
- do not describe a separate runtime host outside `santi`
- do not preserve the old postgres/redis middleware wiring as a target shape

## Boundary

- `santi` is the only HTTP host.
- `local` is a mode inside `santi`, not a separate service.
- `santi-cli` is a pure HTTP client.
- `santi-cli` does not auto-start `santi`.
- the default local URL is what `santi-cli` connects to, and it may be overridden by configuration.

## Invariants

- all API routes stay under `/api/v1`
- local mode uses sqlite
- local mode is strictly single-process
- local mode uses the same resource model and compatibility rules as hosted mode
- concurrent `POST /api/v1/sessions/{id}/send` on the same session remains fail-fast `409 Conflict`

## Runtime model

- startup begins in the `santi` composition root
- the root selects `local` or another mode and assembles dependencies
- HTTP starts after the core graph is built
- `local` changes assembly, not service semantics
- runtime behavior stays aligned with the shared `santi` contract

## Storage and directories

- sqlite stores the local durable state
- normal directories remain the resource model for runtime state
- `soul_dir` and `session_dir` stay ordinary directories, not special mount points
- local mode does not require postgres or redis to function

## Single-process rule

- one `santi` process owns the local runtime
- do not queue, serialize, or fan out same-session sends across multiple local processes
- if local concurrency conflicts with the same session, fail fast with `409`

## API and compatibility

- local uses the same `/api/v1` contract as hosted mode
- local does not introduce compatibility exceptions or extra endpoints
- `health`, `meta`, `sessions`, `messages`, `effects`, `compacts`, `memory`, and admin hooks keep the same meaning
- compatibility is checked through the shared service and client version rules, not via local-only routes

## Startup and failure behavior

- startup should fail clearly if sqlite, directories, or the required local graph cannot be assembled
- missing or invalid local configuration should fail before serving traffic
- runtime failures should surface through the same HTTP contract used everywhere else
- `santi-cli` should report connection or compatibility failures instead of starting a hidden backend

## Validation

- verify local startup reaches `/api/v1/health`
- verify `/api/v1/meta` reports the expected service and compatibility fields
- verify the real local HTTP smoke chain can complete `create session -> send -> messages -> fork -> compact -> memory -> admin hook reload -> soul`
- verify same-session concurrent `send` returns `409`
- verify local runs without postgres and redis present
- verify `santi-cli` connects to the configured URL and does not auto-launch `santi`
