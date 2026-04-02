# Local Adaptor First Pass

> Status: this document is now mainly historical context.
> The first-pass boundary described here has been exceeded by subsequent local convergence work.
> Current source of truth for active local-mode behavior is `docs/local-mode.md`, with current task state tracked in `.task/MAIN.md`.

## Scope

This document defines the first-pass local adaptor boundary for `santi`.
It is a narrow execution target for local mode work, not the final local runtime design.

## First-pass goal

Get a minimal local adaptor working without a large rewrite of db, lock, or runtime layers.
The goal is to preserve the hosted architecture shape while adding a small local path that can serve the existing session/message/send query loop.

## Minimal capabilities

- Local runs in a single process.
- Local is HTTP-only.
- Local uses sqlite for persistence.
- Local can support the current session/message/send read-write loop end to end.
- Local does not need to cover every hosted capability in the first pass.

## What has landed since this first pass

- local query / memory / soul reads and writes were pushed below `santi-api`
- local fork / compact now work through sqlite-backed adaptors instead of API-layer stubs
- local admin hook reload now works through a registry-backed local admin service
- real local HTTP smoke now covers health, meta, create session, send, messages, fork, compact, memory, admin hook reload, and soul

## Local adaptor boundary

- Treat the local adaptor as an execution adapter, not a new runtime architecture.
- Keep the boundary thin: route existing session/message/send flows through the local path with the smallest possible surface.
- Do not introduce broad new abstractions just to support local mode.
- Do not split the current runtime into a separate long-lived local stack in this pass.

## Minimal sqlite persistence strategy

- Persist only what is needed to close the current query loop for sessions, messages, and sends.
- Prefer the smallest stable shape that preserves current behavior and recovery.
- Do not define a full long-term schema up front.
- Add only the tables, keys, and indexes required by the first-pass flows and basic lookup/replay needs.
- Keep the schema easy to evolve later without locking in hosted-only assumptions.

## Single-process locking strategy

- Assume one local process owns the sqlite file.
- Use process-local serialization for conflicting session turns.
- Keep `session/send` fail-fast on concurrent use for the same session.
- Do not add distributed locking or cross-process coordination in this pass.
- Do not queue or silently retry conflicting sends.

## Recovery stance

- Recovery only needs to support restart after a normal local process stop.
- On restart, reload state from sqlite and continue the minimal query loop.
- Do not promise crash-safe semantics beyond the persisted minimal state.
- Do not block the first pass on perfect replay, compaction, or migration machinery.

## Deferred items

- Multi-process local support.
- Non-HTTP local interfaces.
- Full hosted capability parity.
- Final schema design.
- Cross-process locking.
- Advanced replay, compaction, or durability hardening.
- Any broad runtime refactor that is not needed for the first-pass loop.
- full hosted turn/hook runtime parity for local send execution still remains deferred

## Done criteria

- Local mode runs in one process and serves HTTP only.
- sqlite backs the minimal session/message/send path.
- Existing session/message/send queries complete end to end in local mode.
- Concurrent `session/send` on the same session remains fail-fast.
- The implementation stays small and does not force a db/lock/runtime redesign.
