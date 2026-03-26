# Tool Loop Session Memory Smoke

## Purpose

- validate the current live path for `create session -> session/send -> tool_call -> tool_result -> continuation -> assistant`
- confirm `write_session_memory` persists durable session memory and that the next turn can recall it
- use this as a manual smoke only, not as a stable regression test

## Baseline Path

1. wait until `GET /api/v1/health` on `127.0.0.1:18081` returns `200`
2. create a fresh session with `node scripts/dev/send.mjs --create`
3. send exactly one turn at a time with `scripts/dev/send.mjs`
4. verify SSE reaches `response.completed`
5. inspect `GET /api/v1/sessions/{id}/messages` for persistence

## Smoke Prompt

```text
请把下面这句话作为当前 session 的 durable memory note 立即写入 write_session_memory：`用户偏好：回答默认用中文，先给结论再给要点，非必要不展开。` 只执行写入，不要口头确认，不要复述内容。
```

## Acceptance Points

Treat the smoke as accepted only when all of the following hold:

1. `session/send` returns SSE successfully and reaches `response.completed`
2. `GET /api/v1/sessions/{id}/messages` shows `user -> tool_call -> tool_result -> assistant`
3. the intended happy path writes a `tool_result` with non-null `output` and null `error_text`
4. the target `soul_sessions.session_memory` row contains the expected durable note
5. a follow-up turn can correctly recall the saved memory from the same session

## Practical Rules

- if SSE succeeds but `soul_sessions.session_memory` is wrong or recall fails, treat the smoke as failed
- if artifact messages exist but continuation fails, treat it as a tool-loop integration failure, not a persistence success
- inspect recent `docker compose logs santi` when the provider path behaves unexpectedly
- always issue session turns strictly sequentially against the same session
