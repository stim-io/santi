# Phase B implementation-move baseline

This document is a baseline only. It records the current safe migration shape for moving real implementation into canonical wrapper modules.

## Current canonical wrapper -> root-level implementation map

Use this map as the current reference point for Phase B.

### `crates/santi-db/src/adapter/local/mod.rs`
- Canonical wrapper for local adapter surface.
- Root-level implementation sources still in use:
  - `crates/santi-db/src/adapter/local_soul_runtime.rs`
- Already moved into canonical wrapper:
  - `crates/santi-db/src/adapter/local/effect_ledger.rs`
  - root-level `crates/santi-db/src/adapter/local_effect_ledger.rs` now acts as shim/re-export
  - `crates/santi-db/src/adapter/local/session_fork_compact.rs`
  - root-level `crates/santi-db/src/adapter/local_session_fork_compact.rs` now acts as shim/re-export
  - `crates/santi-db/src/adapter/local/session_store.rs`
  - root-level `crates/santi-db/src/adapter/local_session_store.rs` now acts as shim/re-export
  - `crates/santi-db/src/adapter/local/soul_store.rs`
  - root-level `crates/santi-db/src/adapter/local_soul_store.rs` now acts as shim/re-export

### `crates/santi-db/src/adapter/postgres/mod.rs`
- Canonical wrapper for postgres adapter surface.
- Root-level implementation sources still in use:
  - `crates/santi-db/src/adapter/soul_runtime.rs`
- Already moved into canonical wrapper:
  - `crates/santi-db/src/adapter/postgres/effect_ledger.rs`
  - root-level `crates/santi-db/src/adapter/effect_ledger.rs` now acts as shim/re-export
  - `crates/santi-db/src/adapter/postgres/session_ledger.rs`
  - root-level `crates/santi-db/src/adapter/session_ledger.rs` now acts as shim/re-export
  - `crates/santi-db/src/adapter/postgres/soul.rs`
  - root-level `crates/santi-db/src/adapter/soul.rs` now acts as shim/re-export

## Safest migration order

1. Move small, non-runtime implementation units first.
2. Repoint canonical wrappers to own the moved code directly.
3. Keep old root-level modules as shim/re-export until wrapper ownership is complete.
4. Move high-coupling runtime files only after the smaller modules are stable.
5. Recheck the wrapper-to-impl map before any root-level deletion.

## Risk classes

### Low risk

Small, focused files with limited surface area and little orchestration logic.

Examples:
- `crates/santi-db/src/adapter/local_effect_ledger.rs`
- `crates/santi-db/src/adapter/local_session_store.rs`
- `crates/santi-db/src/adapter/local_soul_store.rs`
- `crates/santi-db/src/adapter/effect_ledger.rs`
- `crates/santi-db/src/adapter/session_ledger.rs`

### Medium risk

Files that carry adapter coordination or multiple call sites, but are not the core runtime boundary.

Examples:
- `crates/santi-db/src/adapter/soul.rs`
- `crates/santi-db/src/adapter/local_session_fork_compact.rs`
- `crates/santi-db/src/adapter/local_soul_runtime.rs`
- `crates/santi-db/src/adapter/soul_runtime.rs`

### High risk

Runtime-facing files and files that shape execution behavior, lifecycle, or session flow.

Examples:
- `crates/santi-runtime/src/session/*.rs`
- `crates/santi-runtime/src/runtime/*.rs`
- `crates/santi-api/src/*.rs`
- `crates/santi-cli/src/*.rs`

## Safest stop point

The safest point to stop is when:

- canonical wrappers contain the real implementation,
- old root-level modules still exist only as shim/re-export layers,
- no root-level deletion has been attempted yet,
- and the system still compiles through the wrapper paths.

At that point, Phase B is complete enough to pause without exposing a partially removed root-level implementation boundary.

## Current pause point

Phase B is now complete for:

- `SoulPort`
- `EffectLedgerPort`
- `SessionLedgerPort`
- local `SessionForkCompact`

The remaining unmoved high-risk implementation is `SoulRuntimePort` (`local_soul_runtime.rs` / `soul_runtime.rs`).

That pair still sits directly on runtime lifecycle and contract-leakage boundaries, so it is the right pause point before any deeper move.

## Baseline rule

Do not change code in this phase. Record the move path first, then execute migration in small steps.
