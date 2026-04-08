# 单机拓扑

## Goal and non-goals

Goal: define 单机 as an internal `santi` assembly path for single-process development and verification.

Non-goals:

- do not add any 单机-only HTTP API
- do not describe a separate runtime host outside `santi`
- do not preserve the old postgres/redis middleware wiring as a target shape

## Boundary

- `santi` is the only HTTP host.
- 单机 is an internal `santi` assembly path, not a separate service.
- `santi-cli` is a pure HTTP client.
- `santi-cli` does not auto-start `santi`.
- the default standalone URL is what `santi-cli` connects to in 单机 usage, and it may be overridden by configuration.

## Invariants

- all API routes stay under `/api/v1`
- 单机 uses sqlite
- 单机 is strictly single-process
- 单机 and 分布式 use the same resource model and compatibility rules because both terminate in the same `santi` service boundary
- concurrent `POST /api/v1/sessions/{id}/send` on the same session remains fail-fast `409 Conflict`

## Runtime model

- startup begins in the `santi` composition root
- the root selects a topology and assembles dependencies
- HTTP starts after the core graph is built
- 单机 changes adapter and dependency topology, not service semantics
- runtime behavior stays aligned with the shared `santi` contract

## Storage and directories

- sqlite stores the 单机 durable state
- normal directories remain the resource model for runtime state
- `soul_dir` and `session_dir` stay ordinary directories, not special mount points
- 单机 does not require postgres or redis to function

## Single-process rule

- one `santi` process owns the 单机 runtime
- do not queue, serialize, or fan out same-session sends across multiple 单机 processes
- if 单机 concurrency conflicts with the same session, fail fast with `409`

## API and compatibility

- 单机 uses the same `/api/v1` contract as 分布式
- 单机 does not introduce compatibility exceptions or extra endpoints
- `health`, `meta`, `sessions`, `messages`, `effects`, `compacts`, `memory`, and admin hooks keep the same meaning
- compatibility is checked through the shared service and client version rules, not via 单机-only routes

## Startup and failure behavior

- startup should fail clearly if sqlite, directories, or the required 单机 graph cannot be assembled
- missing or invalid 单机 configuration should fail before serving traffic
- runtime failures should surface through the same HTTP contract used everywhere else
- `santi-cli` should report connection or compatibility failures instead of starting a hidden backend

## Validation

- verify 单机 startup reaches `/api/v1/health`
- verify `/api/v1/meta` reports the expected service and compatibility fields
- verify the real 单机 HTTP smoke chain can complete `create session -> send -> messages -> fork -> compact -> memory -> admin hook reload -> soul`
- verify same-session concurrent `send` returns `409`
- verify 单机 runs without postgres and redis present
- verify `santi-cli` connects to the configured URL and does not auto-launch `santi`
