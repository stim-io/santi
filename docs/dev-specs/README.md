# Dev Specs

This directory records unstable or manually validated development specs that are useful during iteration but are not yet stable enough to treat as reliable automated tests.

## Rules

- put flaky, environment-sensitive, or provider-sensitive dev scenarios here first
- do not treat entries here as release gates
- when a scenario becomes stable and repeatable, promote it into e2e or focused API tests

## Index

- `tool-loop-session-memory-smoke.md`: manual live smoke for the first real `write_session_memory` tool loop
- `same-pass-multi-tool-call-smoke.md`: manual live smoke for multiple `write_session_memory` calls requested inside one provider pass
- `bash-tool-smoke.md`: manual live smoke for the first real `bash` tool loop

## Known Unstable Areas

- service readiness after `docker compose up` can lag because `santi` starts through `cargo run`
- codex continuation is still a compatibility-sensitive path and should be debugged together with `openai-codex-server`
- live provider behavior may differ across prompts even when the local runtime code is unchanged
- smoke success on one prompt does not prove broader multi-tool or multi-turn stability
