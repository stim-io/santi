# Self-Assessment Loop

## Purpose

Use this runbook to iterate on whether `santi` can honestly describe its own runtime capability through the real `stim -> santi` conversation path.

This is a high-frequency documentation-level acceptance loop. Keep it easy to revise. Do not turn it into a frozen automated use case until the prompt shape, runtime facts, and recurring failure modes have stabilized.

## Preconditions

- Start support services from the repo root:

```bash
docker compose up -d --build stim-server santi-link
```

- Keep local foreground `santi` running in another repo-root shell:

```bash
scripts/santi local
```

- From `modules/stim`, confirm the local loop sees all required targets:

```bash
cargo run -p stim-dev -- detect
```

Expected target posture:

- `santi` is reachable at the local foreground default on `127.0.0.1:18081`
- `santi-link` is reachable as the provider gateway
- `stim-server` is reachable as the product server support service

## Prompt to run through `stim`

Send this through the renderer conversation, not directly to the provider:

```text
请从你自己的本地运行时视角评估：你现在在 stim 里能完成什么，不能完成什么，下一步最应该补什么？请把回答分成「已接通事实」「不可见或未知」「缺失能力」「下一步交付动作」，每节最多 2 条要点，并且只引用你能从 runtime facts、工具列表和当前对话里看到的信息。
```

The exact wording may change while the loop is being learned. Keep the intent stable: `santi` must assess itself from visible runtime facts instead of producing generic agent marketing copy.

### Tool-backed probe variant

When the goal is to distinguish visible runtime facts from verified local facts, ask for one narrow read-only `bash` probe through the same renderer conversation:

```text
请做一次工具背书自检。必须先调用一次 bash 工具，只做只读探测；不要修改文件，不要调用第二次工具。探测项：pwd；路径 .git、modules/stim、modules/santi、modules/stim-server 是否存在；health: http://127.0.0.1:18081/api/v1/health、http://127.0.0.1:18083/api/v1/health、http://127.0.0.1:18082/openai/v1/health。工具后用中文回答，区分 tool-verified、仅 runtime facts 可见、unknown/下一步，每节最多 2 条。
```

The useful signal is not exact wording. The answer should clearly mark which facts came from tool output, which facts came only from runtime/context exposure, and which facts remain unknown.

## Checklist

A useful answer should:

- identify itself as `santi`, not as a generic assistant
- state that it is participating in the `stim -> santi` product loop
- cite visible runtime facts such as `assembly_mode`, `launch_profile`, provider API/model/gateway, session/soul ids, memory directories, or fallback working directory when those facts are present
- name available tools accurately from the tool list
- distinguish connected facts from unknowns or unavailable state
- for tool-backed probes, label tool-confirmed facts separately from runtime/context-only facts
- stay concise enough for the product round trip; expand only when the user asks for a deeper audit
- avoid claiming `stim-server` durable ledger state, renderer state, host process health, permission isolation, or external service health unless that fact was visible or tool-confirmed
- propose one next delivery action tied to making the real local `stim -> santi` workflow more capable

Treat failures as prompt/contract feedback, not as test failures. Update this checklist when repeated manual runs expose a better acceptance distinction.

## Anti-patterns

- Do not accept a response that only says it is an AI model or language assistant.
- Do not accept claims about local services, files, permissions, or product ledger state that are not visible in the runtime facts or tool results.
- Do not convert this checklist into a deterministic test while the desired answer shape is still changing quickly.
- Do not use direct provider calls as acceptance for this loop; the product path is `stim -> santi`.
- Do not accept an unbounded self-audit that is technically correct but too slow or verbose for the product loop.
- Do not overfit the loop to exact heading obedience; repeated heading drift is a model-admission signal, while grounded tool-vs-runtime separation is the product capability signal.
