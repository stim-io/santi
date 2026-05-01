# Stim Boundary

This file records the durable product boundary between `stim`, `stim-server`, `stim-proto`, and `santi` from the `santi` side.

## Ownership rule

`stim-server` owns the durable product IM message ledger for the whole `stim` product system.

`santi` owns agent/runtime ledger views that let agents participate in that system and assemble provider-facing turns. It does not own the long-term product IM ledger, and it should not make its local session ids or message ids the durable product ownership model.

`stim-proto` owns the shared protocol semantics that need to survive across runtime differences. It should absorb durable cross-repo message, discovery, acknowledgement, and reply contracts, but it should not force identical internal table structure across repos.

## Santi ledger views

`santi` keeps at least two distinct message-ledger views:

1. **IM-facing ledger view**
   - carries the protocol/session facts `santi` needs to participate in IM conversations
   - records actor-authored message content, mutation lifecycle, ordering, and acknowledgement facts relevant to agent participation
   - may correlate with `stim-server` product-ledger ids, but is not the product ledger itself

2. **LLM/runtime ledger view**
   - carries turns, provider assembly, tool calls/results, compacts, runtime memory, and provider-facing snapshots
   - is optimized around agent execution and auditability, not product transcript ownership
   - may reference IM-facing messages as input material, but should not store provider snapshots as public IM truth

These views must not collapse into one universal `messages` table. Keep relation rows, ids, references, correlation ids, and causation ids explicit whenever facts cross from the product ledger into `santi`'s IM-facing view or from that view into the LLM/runtime view.

## Protocol boundary

For `stim-proto` participation, `santi` should expose a narrow protocol-shaped surface rather than forcing `stim` controller or `stim-server` to couple to the current `santi` session HTTP surface.

That means:

- `santi` may internally bridge incoming `stim-proto` envelopes into its IM-facing session model
- the cross-repo contract should remain shared `stim-proto` envelope, acknowledgement, and reply semantics
- product message-ledger ownership stays outside `santi`, even when `santi` mirrors enough IM facts to run an agent turn

The same rule applies to reply semantics:

- durable assistant-reply semantics shared with `stim` belong in `stim-proto`
- `santi` runtime/provider details belong in the LLM/runtime ledger view
- do not preserve a separate cross-repo private reply path just because it appeared first during execution
- do not copy every current `santi` product-route detail into `stim-proto`; only the durable shared reply boundary belongs there

## Projection rule

The same underlying interaction may have several projections:

- `stim-server` product transcript projection over durable product-ledger facts
- `santi` IM-facing projection for protocol participation
- `santi` LLM/runtime projection for provider assembly and turn audit
- controller/debug projections for local operation coverage in `stim`

These projections may carry overlapping text and ids, but overlap is not ownership. Treat cross-layer ids as references unless the contract explicitly says otherwise.

## Client platform rule

`stim` should treat cross-platform rendering differences as a client-side UI concern, not as a reason to move heavy product or agent logic into the client.

Implications:

- durable IM message-ledger facts belong in `stim-server`
- heavy agent/runtime behavior belongs in `santi`
- client-local controller events may improve local coverage and acceptance, but they are not product-ledger events
- platform differences should be made explicit rather than hidden: shared UI framework, shared base CSS, and clearly separated platform-specific CSS are all acceptable
- if WebKit and Chromium need different presentation treatment, expose that split deliberately at build time instead of letting compatibility hacks spread implicitly through product logic

This keeps platform variance local to presentation while preserving stable server-side communication and runtime boundaries.
