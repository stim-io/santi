# E2E

This directory hosts black-box end-to-end tests for `santi`.

Principles:

- `spec/` holds user-path scenarios only.
- `lib/` holds test harness code only.
- `resources/` holds minimal fixtures and helper scripts.
- `tmp/` is local-only scratch space and should not be committed.

The first target is the session main path:

- create session
- send session
- SSE completion
- persistence verification

These tests are intended to hit the real local stack instead of mocking the provider.
