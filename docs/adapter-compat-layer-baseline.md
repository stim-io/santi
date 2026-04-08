# adapter/mod.rs compat-layer shrink baseline

This document is a baseline, not an implementation change.

## Module roles

### Canonical submodules

These are the stable module entry points that should own the public shape of the adapter layer. They are the names other code should treat as canonical.

### Root-level implementation modules

These are the actual implementation sources currently exposed from the adapter root. They are still the real implementation owners.

`adapter/standalone/*` and `adapter/postgres/*` continue to depend on these root-level `pub mod ...` items as their live source of implementation.

### `*_mod` alias re-exports

These are compat-only aliases that re-export existing modules to preserve old paths.
They are the only items that are currently the minimum-risk removal target.

## Current minimum-risk deletion scope

The only safe immediate shrink target is the `*_mod` alias layer.

Do not treat root-level `pub mod ...` items as removable yet.

## Why root-level modules must stay for now

Root-level `pub mod ...` modules cannot be deleted yet because `adapter/standalone/*` and `adapter/postgres/*` still rely on them as the true implementation source.

Removing them now would move the implementation boundary before the wrapper layer has been repointed.

## Safest three-phase order

### Phase A: delete alias re-exports

Remove only the `*_mod` alias layer first.

Status: completed. The `*_mod` alias re-exports were removed from `crates/santi-db/src/adapter/mod.rs`, and core crates still compile through canonical `adapter::{standalone,postgres}` paths.

### Phase B: move real implementation into canonical wrappers

Repoint canonical wrapper modules so they own the implementation directly.

### Phase C: reassess root-level module deletion

Only after Phase B is complete should root-level module removal be reconsidered.

## Rule of thumb

If a change affects the compat surface, shrink aliases first, then move ownership, then evaluate root-level deletion.
