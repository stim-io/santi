# HTTP API Surface

## Scope

This document defines the minimal stable HTTP contract for `santi`.

It keeps the current resource shape and only normalizes how the surface is named and versioned.

## Base path

- All API routes use the `/api/v1` prefix.
- No new top-level resources are introduced here.
- Existing resources remain the source of truth: `health`, `meta`, `soul`, `sessions`, `messages`, `effects`, `compacts`, `memory`, and admin hooks.
- `stim` protocol participation may add a narrow `stim`-scoped entrypoint when the shared `stim-proto` envelope/ack contract needs a real HTTP landing surface.

## Resource map

- `GET /api/v1/health`
  - process and transport liveness
  - cheap readiness-style check for routing and service availability
- `GET /api/v1/meta`
  - stable service metadata
  - version, build, and capability information
  - not a health probe
  - minimal required fields should include `api_version`, `service_name`, `service_version`, `compatible_cli_xy` or an equivalent compatibility field, and `capabilities`
- `GET /api/v1/soul`
  - soul-scoped lifecycle and resource operations
- `PUT /api/v1/soul/memory`
  - soul memory updates
- `POST /api/v1/sessions`
  - session ledger operations and session-scoped reads
- `GET /api/v1/sessions/{id}`
  - session lookup and session-scoped reads
- `POST /api/v1/sessions/{id}/send`
  - append a turn into the active session flow
- `POST /api/v1/sessions/{id}/fork`
  - fork a session into a new branch
- `POST /api/v1/sessions/{id}/compact`
  - compact session state
- `GET /api/v1/sessions/{id}/memory`
  - read session memory
- `PUT /api/v1/sessions/{id}/memory`
  - update session memory
- `GET /api/v1/sessions/{id}/messages`
  - message reads and message-shaped projections
- `GET /api/v1/sessions/{id}/effects`
  - effect records produced by runtime actions
- `GET /api/v1/sessions/{id}/compacts`
  - compacted session state views
- `PUT /api/v1/admin/hooks`
  - operational hooks used by trusted standalone/admin flows
- `POST /api/v1/stim/envelopes`
  - narrow `stim-proto` protocol participation surface
  - accepts a shared `MessageEnvelope`
  - returns a shared `ProtocolSubmission` containing `ProtocolAcknowledgement` and an optional reply handle
  - should stay protocol-shaped rather than turning into a second product chat API

## `/health` vs `/meta`

- `/health` answers: is the service up enough to accept traffic?
- `/meta` answers: what exact service build and contract version is this?
- `/health` should stay minimal and inexpensive.
- `/meta` may include richer static information, but should not require live domain reads.

## Common conventions

- Responses are JSON unless a route is explicitly defined otherwise.
- Errors use the same stable envelope across resources, with at minimum `code` and `message`.
- Route shapes should stay resource-oriented and predictable.
- `POST /api/v1/sessions/{id}/send` remains sequential per session; concurrent sends on the same session return `409 Conflict`.

## Versioning and compatibility

- `/api/v1` is the stable contract prefix.
- Compatibility is judged by `X.Y` service/client version matching, not by route discovery.
- New fields may be added in a backward-compatible way.
- Removing or renaming existing fields requires a new contract version.

## Stable fields

- Keep identifiers, resource names, timestamps, and version markers stable where they already exist.
- Prefer additive fields over reshaping existing payloads.
- Documented fields are stable unless explicitly marked as experimental.

## Allowed leave-outs

- A response may omit optional or not-yet-derived data.
- Empty lists are preferred over ad hoc nulls when no items exist.
- Fields that depend on unavailable runtime state may be left blank or omitted if the route contract already allows it.
- Leave-outs must not change the meaning of required fields.

## Stability rule

Do not introduce new primary resources in the HTTP contract until the runtime model needs them.

`/api/v1/stim/envelopes` is allowed because `santi` now needs one explicit protocol-shaped landing surface for `stim-proto` participation rather than forcing `stim` to couple directly to the existing product session routes.

Keep this surface aligned with the current resource set and avoid speculative expansion.
