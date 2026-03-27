# Bash Tool Smoke

## Purpose

- validate the current live path for `create session -> session/send -> bash tool_call -> tool_result -> continuation -> assistant`
- confirm the runtime actually spawns `/bin/bash` and returns structured output through tool artifacts
- use this as a manual smoke only, not as a stable regression test

## Baseline Path

1. wait until `GET /api/v1/health` on `127.0.0.1:18081` returns `200`
2. create a fresh session with `node scripts/dev/send.mjs --create`
3. send exactly one turn at a time with `scripts/dev/send.mjs`
4. inspect `GET /api/v1/sessions/{id}/messages`
5. inspect `docker compose logs santi` if the provider path is ambiguous

## Smoke Prompt

```text
你现在只执行一个最小化探测任务，并严格遵守以下规则：

1. 如果你当前可用工具里能直接看到 bash 工具：
- 只调用一次 bash
- 只执行这一条命令：
pwd && printf '\n' "$HOME" "$PATH"
- 不要调用任何其他工具
- 不要解释，不要复述命令，不要输出命令结果
- 在 bash 成功执行后，你的最终回复只能是：
OK

2. 如果你当前看不到 bash 工具，或无法直接调用 bash：
- 不要调用任何工具
- 你的最终回复只能是：
NO_BASH
```

## Acceptance Points

Treat the smoke as accepted only when all of the following hold:

1. `GET /api/v1/sessions/{id}/messages` shows `user -> tool_call(bash) -> tool_result(bash) -> assistant`
2. the bash `tool_result` has non-null `output` and null `error_text`
3. the bash output contains the expected working directory shape for the current session
4. container logs show `runtime tool call: bash` followed by `runtime tool call completed: bash`
5. the assistant completes after continuation instead of failing at the provider boundary

## Current Observation

Current live smoke passes for the runtime/tool-loop path.

- the model can see and call `bash`
- `santi-runtime` executes `/bin/bash` and persists both `tool_call` and `tool_result`
- continuation returns a final assistant reply successfully

Known limitation in the current live path:

- the model does not reliably obey the exact requested final token such as `OK`; one representative run replied `Ready.` instead, so the canonical acceptance focuses on the tool loop and runtime evidence rather than exact assistant wording

## Broader Discussion Findings

- the model can avoid `bash` for a pure mental task such as `37 * 19`, so `bash` is not being blindly overused in the simplest no-tool path
- the model often injects an explicit `cwd` argument even when the user asks not to provide one, which makes default-cwd semantics harder to verify through pure black-box prompting
- when asked to explain cwd semantics after a bash call, the model may simply echo `stdout` instead of answering the semantic question
- relative `cwd` currently resolves under the session fallback path as expected, for example `alpha/beta` became `<fallback_cwd>/alpha/beta`
- concurrent probes against the same session correctly surface `409`, which is consistent with the fail-fast session lock contract rather than a bash-specific bug
