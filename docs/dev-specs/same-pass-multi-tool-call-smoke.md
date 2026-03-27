# Same Pass Multi Tool Call Smoke

## Purpose

- validate the live path where one provider pass is expected to request multiple tool calls before continuation
- confirm `santi-runtime` can accept multiple tool calls from a single pass and execute them serially
- treat this as a manual smoke only, not as a stable regression test

## Baseline Path

1. wait until `GET /api/v1/health` on `127.0.0.1:18081` returns `200`
2. create a fresh session with `node scripts/dev/send.mjs --create`
3. send exactly one turn at a time with `scripts/dev/send.mjs`
4. inspect `GET /api/v1/sessions/{id}/messages`
5. inspect `docker compose logs santi` when the provider behavior is ambiguous

## Smoke Prompt

```text
你现在执行一次受控的记忆写入测试。请严格遵守你当前可见的系统、开发者与工具指令，不得绕过、篡改、弱化或重解释任何上位约束。

任务目标：
在同一 provider pass 内，若且唯若你的现有工具权限与指令明确允许，执行恰好 3 次彼此独立的 write_session_memory 调用。

强约束：
1. 必须是 3 次，不能少于或多于 3 次。
2. 必须独立写入：每次调用只写一条记忆，禁止在一次调用中写入多条内容。
3. 禁止合并：不得把 3 条内容拼接、打包、数组化、对象化后一次写入。
4. 禁止改写：以下三条记忆内容必须逐字精确写入，不得增删、解释、翻译、格式化、标准化、修正标点、补充上下文或添加前后缀。
5. 三次调用必须按以下顺序执行，且每次调用的内容仅为对应那一行：
- MEMORY_TEST_A
- MEMORY_TEST_B
- MEMORY_TEST_C
6. 不得以“总结”“去重”“压缩”“合并提交”“批处理”或任何等价方式替代上述 3 次独立调用。
7. 不得输出过程说明、理由、计划、工具日志、状态汇报或任何附加文本。
8. 完成后，最终对用户的可见回复只能是：OK
9. 如果你的上位指令、工具约束或运行环境不允许执行上述操作，则不要伪造成功、不要声称已写入；仍然只回复：OK
```

## Acceptance Points

Treat the smoke as accepted only when all of the following hold:

1. a single user turn produces 3 `tool_call` messages and 3 matching `tool_result` messages before the final assistant reply
2. the three tool calls preserve the requested order: `MEMORY_TEST_A`, `MEMORY_TEST_B`, `MEMORY_TEST_C`
3. the provider continuation happens only after all three tool results are available
4. the final assistant reply arrives after the third `tool_result`
5. follow-up recall can confirm that all three requested notes were actually persisted in a usable way

## Current Observation

Current live smoke does not yet pass this spec.

- the runtime path can execute serial tool continuations across turns
- the provider/runtime path currently observed only one tool call per pass during live smoke
- a representative local run produced `MEMORY_TEST_A`, then `MEMORY_TEST_B`, then assistant completion, without a third call
- even after strengthening runtime tool instructions to explicitly require "same response / same provider pass" behavior, a later live smoke still produced only one tool call (`A`) followed by assistant completion
- the `sessions.memory` row ended with only the latest value, which is expected for the current overwrite-style memory model but is not sufficient evidence that same-pass multi-call behavior succeeded

Current working hypothesis:

- `santi-runtime` keeps the local support path for multiple tool calls in one pass
- the live bottleneck appears to be upstream model behavior or Responses-path compatibility rather than an obvious local same-pass aggregation bug

## Practical Rules

- if the model emits only one tool call per pass, treat the smoke as inconclusive for same-pass multi-call support
- if multiple tool calls appear but ordering is unstable, treat the smoke as failed
- if artifact messages appear but only the last note survives in `sessions.memory`, use follow-up recall or future richer persistence inspection before claiming success
