# 单机 Adaptor First Pass

> Status: this document is now mainly historical context.
> The first-pass boundary described here has been exceeded by subsequent 单机 convergence work.
> Current source of truth for active 单机 behavior is `docs/standalone-mode.md`, with current task state tracked in `.task/MAIN.md`.

## Scope

This document defines the first-pass 单机 adaptor boundary for `santi`.
It is a narrow execution target for the 单机 path, not the final 单机 runtime design.

## First-pass goal

Get a minimal 单机 adaptor working without a large rewrite of db, lock, or runtime layers.
The goal is to preserve the 分布式 architecture shape while adding a small 单机 path that can serve the existing session/message/send query loop.

## Minimal capabilities

- 单机 runs in a single process.
- 单机 is HTTP-only.
- 单机 uses sqlite for persistence.
- 单机 can support the current session/message/send read-write loop end to end.
- 单机 does not need to cover every 分布式 capability in the first pass.

## What has landed since this first pass

- 单机 query / memory / soul reads and writes were pushed below `santi-api`
- 单机 fork / compact now work through sqlite-backed adaptors instead of API-layer stubs
- 单机 admin hook reload now works through a registry-backed 单机 admin service
- real 单机 HTTP smoke now covers health, meta, create session, send, messages, fork, compact, memory, admin hook reload, and soul

## 单机 adaptor boundary

- Treat the 单机 adaptor as an execution adapter, not a new runtime architecture.
- Keep the boundary thin: route existing session/message/send flows through the 单机 path with the smallest possible surface.
- Do not introduce broad new abstractions just to support 单机.
- Do not split the current runtime into a separate long-lived 单机 stack in this pass.

## Minimal sqlite persistence strategy

- Persist only what is needed to close the current query loop for sessions, messages, and sends.
- Prefer the smallest stable shape that preserves current behavior and recovery.
- Do not define a full long-term schema up front.
- Add only the tables, keys, and indexes required by the first-pass flows and basic lookup/replay needs.
- Keep the schema easy to evolve later without locking in 分布式-only assumptions.

## Single-process locking strategy

- Assume one 单机 process owns the sqlite file.
- Use process-scoped serialization for conflicting session turns.
- Keep `session/send` fail-fast on concurrent use for the same session.
- Do not add distributed locking or cross-process coordination in this pass.
- Do not queue or silently retry conflicting sends.

## Recovery stance

- Recovery only needs to support restart after a normal 单机 process stop.
- On restart, reload state from sqlite and continue the minimal query loop.
- Do not promise crash-safe semantics beyond the persisted minimal state.
- Do not block the first pass on perfect replay, compaction, or migration machinery.

## Deferred items

- Multi-process 单机 support.
- Non-HTTP 单机 interfaces.
- Full 分布式 capability parity.
- Final schema design.
- Cross-process locking.
- Advanced replay, compaction, or durability hardening.
- Any broad runtime refactor that is not needed for the first-pass loop.
- full 分布式 turn/hook runtime parity for 单机 send execution still remains deferred

## Done criteria

- 单机 runs in one process and serves HTTP only.
- sqlite backs the minimal session/message/send path.
- Existing session/message/send queries complete end to end in 单机.
- Concurrent `session/send` on the same session remains fail-fast.
- The implementation stays small and does not force a db/lock/runtime redesign.
