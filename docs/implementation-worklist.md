# Implementation Worklist

## Current status snapshot

- Slice 1 is effectively landed: `/api/v1/meta` exists and the shared error envelope is in active use.
- Slice 3 and slice 4 have advanced materially: startup assembly is split by topology inside `santi`, and 单机 now closes the main sqlite-backed HTTP loop.
- recent 单机 convergence also pushed query/memory/soul, fork/compact, and admin hook reload below `santi-api`.
- the next planned order is: finish wrap-up cleanup first, then start the larger runtime-semantics convergence pass.

## Scope for this slice

This is the execution map for the current phase of code change.

- It stays below `crate-refactor-plan.md` and `migration-checklist.md`.
- It groups work by capability slice, not by crate inventory.
- It focuses on the minimum code path needed to converge the current host/client split and stabilize 单机 / 分布式 assembly.

## Change units

### 1. API metadata and error envelope

- Goal: add `/api/v1/meta` and make error responses use one stable envelope.
- Crate/module: `santi-api`.
- Key files: route handlers, response/error types, API contract wiring.
- Prerequisites: current `/api/v1` routing stays intact.
- Minimum validation: `GET /api/v1/meta` returns stable service metadata; one representative error path returns the shared `code` + `message` envelope.

### 2. Standalone CLI becomes HTTP-only

- Goal: keep `santi-cli` as a pure client.
- Crate/module: standalone `santi-cli` entrypoint and config/output path.
- Key files: CLI arg parsing, request client, config resolution, output formatting.
- Prerequisites: `santi-api` exposes the normalized HTTP surface the client targets.
- Minimum validation: CLI connects through HTTP only, and no startup path tries to host `santi`.

### 3. `santi` topology/bootstrap convergence

- Goal: make topology selection and startup assembly internal to `santi`.
- Crate/module: `santi` bootstrap/composition layer.
- Key files: service config, assembly resolution, startup assembly, listener bootstrap.
- Prerequisites: API surface and client target are stable enough to separate transport from assembly.
- Minimum validation: topology is resolved before serving traffic; invalid config fails fast; no duplicate host startup path remains.

### 4. 单机 adaptor lands as sqlite + single process

- Goal: define 单机 as one-process `santi` with sqlite-backed storage.
- Crate/module: 单机 assembly and storage adaptor path inside `santi`.
- Key files: 单机 adapter wiring, sqlite pool/storage setup, 单机 startup checks.
- Prerequisites: internal topology/bootstrap convergence is in place.
- Minimum validation: 单机 starts with sqlite, runs in one process, and rejects same-session concurrency with the existing fail-fast rule.

### 5. Final removal of the legacy internal CLI crate

- Goal: delete the old internal crate after the standalone client path is complete.
- Crate/module: workspace cleanup around the removed internal CLI host.
- Key files: workspace manifests, references, docs that still point at the embedded crate.
- Prerequisites: standalone CLI is fully usable and no runtime path depends on the old crate.
- Minimum validation: workspace builds and tests without the removed internal CLI crate; no code references remain.

## Code-change map

- `santi-api`: normalize `/meta`, unify error envelope, keep route contract stable.
- `santi-cli`: bind to HTTP target only, keep UX output minimal.
- `santi`: own topology selection, bootstrap validation, and runtime assembly.
- 单机 adaptor layer: wire sqlite and single-process assumptions directly.
- workspace root: remove the final embedded CLI crate when the new client path is complete.

## Execution order

1. Normalize `santi-api` contract surface first.
2. Convert standalone `santi-cli` to HTTP-only.
3. Collapse `santi` topology/bootstrap into one internal assembly path.
4. Land 单机 sqlite and single-process adaptor wiring.
5. Delete the legacy internal CLI crate after the new path is fully in use.

## Validation per unit

- API metadata and error envelope: `/api/v1/meta` smoke and one error-path contract check.
- HTTP-only CLI: invoke against a running service with no alternate transport-selection path.
- Topology/bootstrap: startup fail-fast coverage for invalid config and resolved topology.
- 单机 adaptor: sqlite-backed startup and same-session `409` concurrency check.
- CLI crate deletion: workspace build plus reference search for the removed crate.

## Deferred items

- Any broader crate flattening beyond the split described here.
- New transport abstractions.
- Nonessential compatibility shims.
- Future scope/tenant work.
- Any API expansion outside the current resource set.
