# Cold-Start Verification

## Purpose

This document is the operational runbook for verifying `santi/` right after a fresh start, restart, or environment reset.

It complements, but does not replace:

- `docs/dev-environment.md` for local setup and baseline commands
- `docs/dev-faq.md` for troubleshooting when a verification step fails

## When to use this runbook

Use this flow when:

- the local stack was just started or rebuilt
- `santi` was just restarted after code changes
- you want a fast confidence check before feature work
- you need a stable main-path verification order for regression checks

## Preconditions

Do not duplicate setup here. Assume the local baseline in `docs/dev-environment.md` is already satisfied.

In particular:

- use the repo-root `docker compose.yml`
- prefer the installed `santi-cli`
- prefer direct CLI-to-HTTP usage against the default standalone stack

## Main-path verification goal

The minimum acceptable cold-start outcome is:

`health -> create session -> send session -> SSE output -> persistence`

If this path is broken, stop and fix the runtime before doing deeper checks.

## Standard verification sequence

### 1. Stack health

```bash
docker compose up -d --build
./scripts/cli/setup.sh
santi-cli health
```

Expected result:

- health returns `ok`

If this fails:

- go to `docs/dev-faq.md`

### 2. Main chat path

```bash
santi-cli chat 'hello'
```

Expected result:

- a new `session:` hint is printed
- assistant text is streamed back

This step validates:

- session creation
- `session/send`
- SSE streaming
- provider path
- basic persistence

If this fails:

- do not continue to compact, hooks, or fork
- fix the main path first

### 3. Continue an existing session

```bash
printf 'hello again' | santi-cli chat --session <session_id>
```

Expected result:

- the same session continues successfully
- no `409` conflict appears when commands are issued sequentially

This step validates:

- continued session send
- same-session persistence continuity

### 4. Inspect persisted messages

```bash
santi-cli session messages <session_id>
```

Expected result:

- the earlier user/assistant messages are present

This step validates:

- ledger persistence is visible through the query path

## Common follow-up verification operations

These are not required to declare the stack minimally healthy, but they are the normal next checks during feature work.

### 5. Manual compact

```bash
printf 'compact summary' | santi-cli session compact <session_id>
santi-cli session compacts <session_id>
```

Expected result:

- `session compact` returns a compact record
- `session compacts` shows at least one compact entry

This step validates:

- compact write path
- compact query path

### 6. Session memory visibility

```bash
santi-cli session memory get <session_id>
```

Expected result:

- session memory is readable without needing direct database inspection

This step validates:

- session memory read path

### 7. Explicit fork

```bash
santi-cli session fork <session_id> --fork-point <n>
```

Expected result:

- a child session id is returned

Useful follow-up:

```bash
santi-cli session get <child_session_id>
santi-cli session messages <child_session_id>
```

This step validates:

- explicit fork path
- child session lineage visibility

### 8. Hook reload and effect visibility

Reload a minimal hook set through the API runtime:

```bash
printf '[{"id":"auto-fork-handoff","enabled":true,"hook_point":"turn_completed","kind":"fork_handoff_threshold","params":{"min_messages_since_last_compact":1}}]' \
  | santi-cli admin hooks reload
```

Then trigger a normal turn and inspect effects:

```bash
printf 'normal reply' | santi-cli chat --session <session_id>
santi-cli session effects <session_id>
```

Expected result:

- hook reload reports a non-zero hook count
- `session effects` shows the effect outcome
- for `hook_fork_handoff`, `result_ref` should point at the child session id

Useful follow-up:

```bash
santi-cli session messages <child_session_id>
santi-cli chat --session <child_session_id> <message>
```

This step validates:

- runtime hook replacement
- effect ledger visibility
- effect-driven fork handoff path

## Failure routing

- health/startup problem: `docs/dev-faq.md`
- main chat path broken: fix runtime first, do not continue
- compact/effects/fork broken while main chat path is healthy: debug the specific subsystem after confirming persistence still works

## Rule of use

Use this document for the verification order and pass/fail criteria.

Use `docs/dev-environment.md` for baseline local commands and setup details.
Use `docs/dev-faq.md` for troubleshooting and recovery steps.
