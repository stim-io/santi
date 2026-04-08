# Documentation Update Guide

This file defines how `santi/docs` should evolve.

## Goal

Keep the docs system easy to navigate, low in repetition, and high in durable value.

The target is not fewer files. The target is a clearer information architecture.

## Organizing principle

Organize docs by **question** and **ownership boundary**, not by implementation phase.

- `architecture/`: what the system is, how responsibilities are divided, and where boundaries live
- `contracts/`: stable rules and shapes that code must obey
- `operations/`: how to run, verify, troubleshoot, and maintain the workspace locally

Subdirectories are encouraged when they reduce ambiguity or repetition.

## Canonical-source rule

Each important fact should have one canonical home.

- define a concept once
- in other docs, link to it instead of redefining it
- short navigational summaries are fine
- parallel full definitions are not

If two docs explain the same rule in full, the structure is wrong and should be corrected.

## Split / merge rule

Do not optimize for file count.

Split a document when it mixes different question types, such as:

- architecture + contract
- product boundary + runtime mechanics
- stable rules + operational runbook
- durable guidance + transient implementation inventory

Merge documents when they repeatedly describe the same boundary and one canonical document would be clearer.

## Durable-content rule

Keep durable information. Remove process noise.

Keep:

- stable boundaries
- object models
- invariants
- API and runtime contracts
- operational procedures that remain useful across changes
- ADR-style decisions that still govern the repo

Do not keep active docs for:

- migration plans
- phase notes
- worklists
- baselines
- historical cleanup notes
- transitional compatibility narratives

If something is only useful as history, delete it and rely on git history.

## Wording rule

Write docs as current truth, not as a transition diary.

Avoid phrases like:

- `first-pass`
- `current stance`
- `near-term`
- `migration`
- `legacy`
- `historical`

Prefer:

- explicit rules
- current boundaries
- stable invariants
- clearly named open edges when something is intentionally unresolved

## Document-boundary rule

Each document should answer one primary question.

Examples:

- `architecture/topology/service-and-assemblies.md` answers service-boundary and assembly questions
- `contracts/data-model/session-message-model.md` answers canonical model questions
- `operations/local-dev/verification.md` answers local verification flow questions

If a document starts answering multiple primary questions, split it.

## Update process

When changing docs:

1. identify the canonical home for the fact you are changing
2. update that canonical document first
3. trim or relink any duplicated explanation elsewhere
4. if no good home exists, create one in the correct bucket/subdirectory
5. prefer renaming, splitting, or merging docs over layering on another overlapping file
6. if the content is only historical, delete it instead of archiving it

## Directory evolution rule

The current top-level buckets are stable:

- `architecture/`
- `contracts/`
- `operations/`

New top-level buckets should be rare.

Prefer refining subdirectories inside an existing bucket before adding a new top-level category.

## Quality check before finishing

Before considering a docs change done, ask:

1. Is there exactly one canonical place for each important fact I touched?
2. Did I remove repeated explanations instead of adding another one?
3. Did I keep only durable, high-value information?
4. Does the file location match the question the document answers?
5. Would a new reader know where to look first?
