# Smoke Harness

This directory hosts the current TypeScript smoke/integration harness for `santi`.

Principles:

- `spec/` holds main-path smoke/integration scenarios.
- `lib/` holds test harness code only.
- `resources/` holds minimal fixtures and helper scripts.
- `tmp/` is local-only scratch space and should not be committed.

The first target is the session main path:

- create session
- send session
- SSE completion
- persistence verification

These checks are intended to hit the real local stack instead of mocking the provider.

Despite the legacy directory name, this is not the project's stable end-to-end truth. The shared root compose plus `scripts/smoke/*` are the primary local entrypoints.
