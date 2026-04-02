# Runtime Port Contract 审计基线

> 这是一份**代码行为不变**之前的审计基线。它只固定当前 `runtime ports` 的 capability matrix / dependency ledger，不推动实现变更，也不代表 contract 已经理顺。

## 范围

只覆盖以下四个 trait：

- `SessionLedgerPort`
- `EffectLedgerPort`
- `SoulPort`
- `SoulRuntimePort`

## 判定口径

- **local 完整支持**：local 实现对该方法有可用实现，且不是显式 unsupported/unavailable。
- **postgres 完整支持**：postgres 实现对该方法有可用实现，且不是显式 unsupported/unavailable。
- **稳定公共能力**：local 与 postgres 都完整支持，且可视为当前对外可依赖的共同 contract。
- **条件/部分能力**：方法在多后端都可调用，但支持范围或语义边界并不完全对等，不能直接按 full core 对待。
- **contract leakage**：postgres 完整，但 local 明确 unavailable / unsupported，说明 trait 表面统一，但能力实际上只在部分后端成立。

## Capability matrix

### 1) `SessionLedgerPort`

| 方法 | local | postgres | 结论 |
| --- | --- | --- | --- |
| `create_session(&self, session_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `get_session(&self, session_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `list_messages(&self, session_id: &str, after_session_seq: Option<i64>)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `append_message(&self, input: AppendSessionMessage)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `apply_message_event(&self, input: ApplyMessageEvent)` | **unavailable**（返回 local mode unavailable） | 完整支持 | **contract leakage** |

### 2) `EffectLedgerPort`

| 方法 | local | postgres | 结论 |
| --- | --- | --- | --- |
| `list_effects(&self, session_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `get_effect(&self, session_id: &str, effect_type: &str, idempotency_key: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `create_effect(&self, input: CreateSessionEffect)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `update_effect(&self, input: UpdateSessionEffect)` | 完整支持 | 完整支持 | 稳定公共能力 |

### 3) `SoulPort`

| 方法 | local | postgres | 结论 |
| --- | --- | --- | --- |
| `get_soul(&self, soul_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `write_soul_memory(&self, soul_id: &str, text: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |

### 4) `SoulRuntimePort`

| 方法 | local | postgres | 结论 |
| --- | --- | --- | --- |
| `get_or_create_soul_session(&self, soul_id: &str, session_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `get_soul_session(&self, soul_session_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `load_turn_context(&self, soul_id: &str, session_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `write_session_memory(&self, soul_session_id: &str, text: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `start_turn(&self, input: StartTurn)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `append_message_ref(&self, input: AppendMessageRef)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `append_tool_call(&self, input: AppendToolCall)` | **unsupported**（local mode 不实现） | 完整支持 | **contract leakage** |
| `append_tool_result(&self, input: AppendToolResult)` | **unsupported**（local mode 不实现） | 完整支持 | **contract leakage** |
| `append_compact(&self, input: AppendCompact)` | **unsupported**（local mode 不实现） | 完整支持 | **contract leakage** |
| `complete_turn(&self, input: CompleteTurn)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `fail_turn(&self, input: FailTurn)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `get_soul_session_by_session_id(&self, session_id: &str)` | 完整支持 | 完整支持 | 稳定公共能力 |
| `fork_soul_session(&self, parent_soul_session_id: &str, fork_point: i64, new_soul_session_id: &str, new_session_id: &str)` | **unsupported**（local mode 不实现） | 完整支持 | **contract leakage** |
| `list_assembly_items(&self, soul_session_id: &str, after_soul_session_seq: Option<i64>)` | 条件支持（当前仅稳定覆盖 message target） | 完整支持 | **条件/部分能力** |

## 当前稳定公共能力

当前可以视为两套后端共同成立的公共 contract：

- `SessionLedgerPort` 除 `apply_message_event` 之外的全部方法
- `EffectLedgerPort` 全部方法
- `SoulPort` 全部方法
- `SoulRuntimePort` 中除 `append_tool_call` / `append_tool_result` / `append_compact` / `fork_soul_session` 之外的大部分基础方法

## 当前条件/部分能力

- `SoulRuntimePort::list_assembly_items`

说明：

- postgres 侧按完整 assembly item 语义工作
- local 侧当前只对 `message target` 有稳定支持
- 因此它可以继续保留在统一 trait 中，但在 contract 分层上不应直接被视作 full core

## 明显 contract leakage

以下方法属于“postgres 完整，但 local 不可用/不支持”的泄漏点：

- `SessionLedgerPort::apply_message_event`
- `SoulRuntimePort::append_tool_call`
- `SoulRuntimePort::append_tool_result`
- `SoulRuntimePort::append_compact`
- `SoulRuntimePort::fork_soul_session`

它们说明：trait 层已经把能力合并进同一接口，但 local 端并没有同等承诺。

## 最安全的后续处理顺序

1. **先 core contract**：先把必须在 local / postgres 两端都成立的公共能力边界收紧，明确哪些方法属于稳定 contract。
2. **再 optional capability**：把只在部分后端成立的能力单独标成 optional capability，不要继续伪装成统一主合同。
3. **最后决定补齐或拆分**：等 contract 边界稳定后，再决定是补齐 local 能力，还是把 trait 拆成更精确的接口。

## 备注

- 本文只做审计归档，不改变任何代码逻辑。
- 这份文档的作用是冻结当前认知：它是后续收口、拆分、补齐之前的基线，而不是结论终稿。
