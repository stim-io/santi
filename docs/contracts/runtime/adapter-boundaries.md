# Adapter Boundaries

## Boundary

- runtime-agnostic traits live in `crates/santi-core/src/port/*`
- db adapters live in `crates/santi-db/src/adapter/{standalone,postgres}`
- ports define capability shape; adapters implement storage-specific behavior
- topology differences change adapter family and dependency graph, not service ownership

## Canonical paths

- consumers should enter through `adapter::standalone::*` and `adapter::postgres::*`
- `adapter/mod.rs` is the export hub for those canonical families
- do not reintroduce flat compatibility exports as a second public surface

## Naming rules

- `Port` is reserved for trait names
- `Ledger` is reserved for ledger semantics
- `Runtime` is reserved for execution-state and continuity semantics
- do not use `Store` as a competing synonym for `Port`, `Ledger`, or `Runtime`
- `get` reads an existing value, `create` creates only, and `acquire` is the sanctioned get-or-create atom without lock semantics

## Contract rules

- keep ports atomic; if a capability can be recomposed without semantic loss, runtime should compose it instead of widening the trait
- do not put composite reads such as `load_turn_context` or `list_assembly_items` back into stable port contracts
- when runtime needs richer reads, introduce smaller seams and compose them in runtime
- do not widen contracts merely because one adapter family has a more convenient implementation

## Current implementation map

| port | standalone | postgres |
| --- | --- | --- |
| `SessionLedgerPort` | `StandaloneSessionStore` | `DbSessionLedger` |
| `EffectLedgerPort` | `StandaloneEffectLedger` | `DbEffectLedger` |
| `SoulPort` | `StandaloneSoulStore` | `DbSoul` |
| `SoulRuntimePort` | `StandaloneSoulRuntime` | `DbSoulRuntime` |

## Known pressure points

- some implementation names still use `Store` and should eventually converge on the naming rules above
- standalone fork and compact remain local adapter seams rather than symmetric runtime-port implementations
- unsupported or partial adapter behavior should be fixed by tightening seams, not by broadening composite contracts
