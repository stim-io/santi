# Crate Refactor Plan

## Goal

Converge on a single HTTP host: `santi`.

For the host boundary, `santi` is the external service boundary, while `santi-api` is the only HTTP host crate at the code layer.

The refactor keeps composition at the root, moves mode-specific concerns inward, deletes the legacy internal CLI crate, and leaves `santi-api` as the only HTTP host crate.

## Ordering rules

1. Stabilize the composition root.
2. Move mode selection and assembly inward.
3. Localize local-mode adaptors.
4. Remove embedded CLI hosting paths.
5. Delete the legacy internal CLI crate.
6. Finish with `santi-api` as the only HTTP host.

Do not collapse these steps into one sweeping rewrite.

## Phase 1: Composition root

- Make the `santi` root the only place that assembles runtime graphs.
- Keep startup, shutdown, and mode selection at the root boundary.
- Ensure the HTTP contract is registered once and only once.

Completion criteria:

- there is a single runtime assembly entrypoint
- route registration is centralized
- no secondary crate owns host startup

## Phase 2: Mode is internal

- Move mode branching behind the composition root.
- Keep `local`, `hosted`, and `test` as assembly choices, not separate ownership domains.
- Avoid mode-specific transport logic leaking into shared code.

Completion criteria:

- mode selection changes dependencies, not API shape
- runtime semantics stay inside `santi`
- no duplicated startup path remains

## Phase 3: Local adaptors

- Keep local-mode adaptors narrow and explicit.
- Prefer direct adapters over compatibility glue.
- Preserve sqlite and single-process assumptions in local mode.

Completion criteria:

- local adaptors are clearly bounded
- local mode does not depend on CLI hosting behavior
- no adapter exists only to preserve an old shape

## Phase 4: Remove the legacy internal CLI crate

- Delete the embedded CLI host after the standalone client path is complete.
- Remove any remaining startup or transport ownership from that crate.
- Keep CLI behavior client-only.

Completion criteria:

- the legacy internal CLI crate is gone
- no runtime hosting code references it
- CLI responsibilities are purely outbound

## Phase 5: Single HTTP host

- Keep `santi-api` as the only HTTP host crate.
- Ensure all `/api/v1` routes resolve through that host.
- Avoid reintroducing parallel HTTP roots.

Completion criteria:

- only one crate owns HTTP serving
- the contract is not split across hosts
- local and hosted modes share the same service boundary

## Anti-patterns

- preserving two host entrypoints during migration
- moving CLI behavior back into runtime crates
- keeping backend-selection branching after the host split
- adding temporary compatibility layers that become permanent
- duplicating route registration across crates
- introducing new host-level abstractions before the current split is complete

## Finish line

The refactor is done when `santi` owns runtime assembly, `santi-api` owns the only HTTP host, and the CLI is purely a client.
