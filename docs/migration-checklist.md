# Migration Checklist

## Current status snapshot

- API metadata/error envelope and `/api/v1` normalization are in place.
- local mode now runs as a sqlite-backed single-process `santi` HTTP service.
- recent real local smoke verified: `health -> meta -> create session -> send -> messages -> fork -> compact -> memory -> admin hook reload -> soul`.
- this checklist is still the target-state tracker; remaining unchecked items should be treated as cross-phase cleanup, not as proof that the recent local convergence work is absent.

## Target State

- `santi` runs as the only self-contained HTTP service.
- `santi-cli` is a standalone command-line client for `santi`.
- Local mode uses sqlite and runs in a single process.
- All API routes are under `/api/v1`.
- `--backend` is gone.
- `santi-cli` does not auto-start `santi`.
- `santi-cli` defaults to the local URL and allows config/env override.
- `santi` and `santi-cli` stay aligned by `X.Y` compatibility.

## Phase 1: Service Boundary

- [ ] Confirm `santi` owns all HTTP serving paths.
- [ ] Remove any embedded CLI assumptions from `santi` runtime wiring.
- [ ] Normalize route registration to `/api/v1`.

## Phase 2: CLI Extraction

- [ ] Move user-facing command entrypoints to standalone `santi-cli`.
- [ ] Remove `--backend` and all backend switching branches.
- [ ] Point `santi-cli` at the `santi` HTTP API only.
- [ ] Make the default target the local URL.
- [ ] Keep config/env overrides limited to the target URL.

## Phase 3: Local Mode Simplification

- [ ] Lock local mode to sqlite.
- [ ] Enforce strict single-process operation for local mode.
- [ ] Remove startup behavior that tries to manage `santi` from the CLI.

## Phase 4: Cleanup

- [x] Delete the legacy internal CLI crate after the standalone client is in place.
- [ ] Remove stale compatibility code and dead backend abstractions.
- [ ] Update docs and runbooks that still mention the embedded CLI or `--backend`.

## Verification

- [ ] `santi` serves the expected `/api/v1` endpoints.
- [ ] `santi-cli` connects successfully using the default local URL.
- [ ] `santi-cli` connects successfully with config/env overrides.
- [ ] Local mode starts only as a single process with sqlite.
- [ ] No code path still depends on `--backend`.
- [ ] Version-matched `santi` and `santi-cli` interoperate as expected.
