# Test Construction Standard

## Purpose

This document defines how new automated tests should be added to `santi/`.

It answers one question:

- what kind of test should be added for a given change or regression?

It complements, but does not replace:

- `docs/operations/local-dev/verification.md` for manual verification order
- `docs/contracts/http/api-surface.md` for stable HTTP behavior
- `docs/contracts/runtime/session-locking.md` for conflict semantics

## Core rule

Start from the highest-value executable check on the main path.

Do not start by adding many narrow unit tests for internal details.

Prefer the smallest test that proves the real contract the change is supposed to protect.

## Testing goals

Tests should protect:

- main-path behavior that must keep working
- stable contracts at the HTTP, CLI, and runtime-boundary level
- known regression risks exposed by real failures
- fail-fast behavior for locking, validation, and error handling

Tests should not exist mainly to mirror implementation structure.

## Layer selection rule

Choose the test layer that matches the behavior you need to lock down.

### 1. Smoke or integration tests first

Use smoke or integration tests when the behavior is visible through:

- the HTTP surface
- the CLI surface
- runtime composition across multiple components
- persistence, hooks, fork, compact, or locking flows

This is the default for main-path work.

Examples:

- `health -> create session -> send -> SSE -> persistence`
- same-session concurrent `send` returns `409 Conflict`
- hook reload changes effect behavior visible through `/api/v1`
- CLI output shape matches the live HTTP response contract

### 2. Focused lower-level tests only when needed

Add narrower tests when:

- the main-path check exposed a precise weak spot
- the bug can be locked down more directly at one layer
- the lower-level test removes expensive setup without losing contract confidence

Good candidates:

- response decoding regressions in CLI code
- formatting or serialization rules with stable output expectations
- small runtime helpers with durable behavior and no better outer contract

### 3. Do not drop too low without a reason

Avoid testing private decomposition just because it exists.

If a behavior is only meaningful through the assembled runtime, test the assembled runtime.

## Canonical behavior over implementation detail

Every test should protect a user-visible, operator-visible, or contract-visible fact.

Prefer assertions like:

- returns `409 Conflict`
- emits SSE `response.output_text.delta`
- persists messages in session order
- returns wrapped `effects` payload with stable fields

Avoid assertions like:

- internal helper `x` is called
- transient intermediate state exists only because of current decomposition
- exact internal orchestration steps that are not part of a stable contract

## Real-upstream rule

Automated tests must not depend on real model API calls.

Use deterministic local substitutes instead:

- stub HTTP servers for provider responses
- in-process test assemblies
- controlled fake ports or fake locks where the contract is at that boundary

Real upstream validation belongs in runbooks and manual verification, not committed automated tests.

## Contract-first assertions

When choosing assertions, prefer the strongest external contract available.

### HTTP tests

Assert:

- status code
- response envelope shape
- stable fields
- conflict and validation semantics

For streaming endpoints, assert:

- whether the request fails before stream creation or succeeds with a valid stream
- required SSE event types and terminal behavior

### CLI tests

Assert:

- stdout vs stderr split
- human vs `--json` behavior
- failure exit behavior
- compatibility with the live HTTP response shape

### Runtime tests

Assert:

- stable domain behavior
- lock/conflict discipline
- durable sequencing and persistence semantics

Do not turn runtime tests into a second copy of HTTP or CLI tests unless the runtime boundary itself is the contract under protection.

## Regression-test rule

When a bug is found:

1. reproduce it at the highest meaningful layer
2. decide whether that layer is the right permanent protection point
3. add the narrowest durable test that would have caught the bug
4. fix the code
5. re-run the affected path

If the bug crossed layers, it is normal to add more than one test, but each test must protect a distinct contract.

## Keep tests small and deterministic

Prefer:

- minimal fixtures
- explicit inputs
- stable deterministic responses
- local setup owned by the test itself

Avoid:

- broad shared mutable fixtures
- hidden ordering assumptions between tests
- sleeps without a contract reason
- random data when fixed data would express the behavior more clearly

## Naming rule

Test names should describe the protected behavior, not the implementation step.

Prefer:

- `standalone_http_concurrent_send_returns_conflict`
- `session_send_fails_on_sse_error_payload`

Avoid:

- `test_send_works_2`
- `uses_new_flow`

## Duplication rule

Do not add multiple tests that prove the same fact at the same layer.

If two tests differ only in setup style but assert the same contract, keep the clearer one.

If a broader test already locks the behavior adequately, do not add a narrower duplicate unless it materially improves failure localization.

## Growth-control rule

When adding a new test, also inspect the nearby existing tests and ask whether the suite should be reshaped instead of only expanded.

Prefer to:

- split oversized tests that protect multiple unrelated facts
- extract repeated setup when it improves clarity without hiding the contract
- regroup tests so each one protects one primary behavior
- keep neighboring tests logically orthogonal instead of overlapping heavily

The goal is controlled test-suite growth.

Do not treat every new regression as permission to append another loosely related case onto an already bloated test.

If the existing test layout is becoming hard to reason about, refactor the tests first or as part of the same change.

## Orthogonality rule

Treat `keep tests as orthogonal as possible` as a practice to refine over time, not a slogan to enforce abstractly.

Do not invent purity rules detached from the codebase.

Instead:

- use real regressions and real maintenance pain to decide when tests are overlapping too much
- capture good and bad examples from this repository as they appear
- update this document incrementally from actual practice

## Golden cases from real practice

These examples are intentionally grounded in tests that already exist in this repo.

### Good: one runtime contract, one test

- `crates/santi-api/tests/standalone_http_smoke.rs`:
  - `standalone_http_concurrent_send_returns_conflict`

Why this is good:

- it protects one contract: same-session concurrent `send` must fail as HTTP `409 Conflict`
- it uses the assembled standalone HTTP surface where that contract is actually visible
- it does not also try to validate unrelated persistence, compaction, or CLI formatting facts

### Good: one CLI decode/error contract, one test

- `santi-cli/app/tests/cli_contract.rs`:
  - `session_send_fails_on_sse_error_payload`
  - `session_effects_json_decodes_wrapped_effects_response`

Why this is good:

- each test protects one CLI-facing contract
- one is about stream error handling
- one is about JSON response decoding shape
- neither test tries to re-prove the full standalone runtime path

Together they are complementary, not redundant.

### Bad: appending unrelated regressions onto a broad smoke test

- `crates/santi-api/tests/standalone_http_smoke.rs`:
  - `standalone_http_session_create_get_and_meta_smoke`

Why this is a caution case:

- it is a valid broad smoke test
- but it already covers many facts in one flow
- when a new regression is unrelated to that main path, the default should not be to keep stuffing more assertions into this test

Preferred response:

- preserve this test as a broad confidence check
- add or split out a separate focused test for the new contract

### Bad: using one test to protect multiple layers at once

Recent practice showed that concurrent-send failure touched both:

- runtime/HTTP conflict semantics
- CLI handling of stream-carried errors

The better shape was:

- one `santi-api` integration test for HTTP `409`
- one `santi-cli` contract test for SSE error handling

The worse shape would have been one oversized end-to-end test trying to lock both facts at once.

That would have made failures harder to localize and future growth harder to control.

## Maintenance note

Keep extending the golden cases in this document only when the repository teaches something concrete.

Do not add hypothetical examples just to make the guideline look complete.

## Minimal expectation for new work

Most non-trivial behavior changes should land with one of these:

- a smoke or integration test for the changed contract
- a focused regression test for a specific bug
- both, when the bug revealed a broader contract gap and a local decode/format gap

Pure refactors do not need new tests unless they change risk at a contract boundary.

## Done check

Before considering test work complete, ask:

1. Am I testing the real contract, not just the current implementation?
2. Did I choose the highest-value layer first?
3. Is the test deterministic and local?
4. Would this test have caught the bug or regression I care about?
5. Did I avoid adding duplicate coverage for the same fact?
6. Did I check whether existing tests should be split or regrouped so the suite stays understandable and growth stays controlled?
