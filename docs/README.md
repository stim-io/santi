# Docs Map

`docs/` is organized by question, not by implementation phase.

- `architecture/`: what the system is and how responsibility is divided
- `contracts/`: stable external and internal contracts that code must obey
- `operations/`: how to run, verify, and troubleshoot the workspace locally

Read `docs/operations/documentation.md` before doing structural docs updates.

Preferred sub-structure:

- `architecture/overview.md`: top-level model and design principles
- `architecture/runtime/`: runtime vocabulary and lifecycle
- `architecture/topology/`: service boundary, assemblies, and upstream gateway split
- `architecture/layers/`: layering rules and crate ownership
- `architecture/product/`: product-facing boundary notes when they add distinct value
- `architecture/decisions/`: durable ADR-style decisions
- `contracts/data-model/`: canonical object and relation specs
- `contracts/http/`: HTTP surface and envelope contracts
- `contracts/runtime/`: runtime-facing behavioral contracts and boundaries
- `operations/local-dev/`: setup, verification, troubleshooting

Documentation governance:

- `operations/documentation.md`: canonical docs update guide and anti-duplication rules

If two docs repeat the same fact, move that fact to one canonical document and link to it.
If a document is only useful as history, delete it and rely on git history instead.
