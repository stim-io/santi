# Hook Reload Boundary

This file defines the runtime contract for hook loading and reload.

## Purpose

`santi` owns:

- parsing a hook source input
- resolving it into a whole `HookSpec` set
- compiling that set into runtime evaluators
- atomically replacing the active hook registry

`santi` does not own:

- file watching
- config orchestration policy
- deciding when reload should happen

Those belong to an upper layer.

## Accepted inputs

Both startup and reload can receive hook configuration as one of:

- `value`: inline hook array payload
- `path`: local file path to read once
- `url`: remote URL to fetch once

The resolved result is always a whole `Vec<HookSpec>`.

## Runtime rule

- reload is whole-set replacement, not patch/merge
- the registry holder swaps the active registry atomically
- a running turn sees one stable registry snapshot for its evaluation pass
- later turns see the new registry after replacement

## Current management paths

- startup:
  - API service: `HOOK_SPECS_JSON` / `HOOK_SPECS_FILE` / `HOOK_SPECS_URL`
- reload:
  - admin API: `PUT /api/v1/admin/hooks`
  - CLI wrapper: `santi-cli admin hooks reload`

## Non-goals

- no watcher inside `santi`
- no persistent action-audit store beyond tracing/logs
- no partial hook mutation protocol
- no hook DSL

## Expected upper-layer behavior

An upper layer may:

- watch files
- render hook config dynamically
- host remote config
- decide when to call reload

But it should hand `santi` a single resolved source input and let `santi` perform one atomic replacement.
