# SoulRuntimePort capability matrix

> 本文是 `SoulRuntimePort` 的证据化基线。它只固定当前方法、调用面、支持面与保守分类，不改代码，不提前拆 trait。

## 判定口径

- **core**：当前运行主路径直接依赖，且 local / postgres 都有可工作的实现
- **conditional**：当前有调用面，但支持范围、依赖前提或语义成熟度需要保守表述
- **optional**：只有部分后端完整支持，或明显超出当前最小生命周期闭环

## Matrix

| 方法 | 当前主调用面 / 文件 | local | postgres | 分类 | 失败 / 降级备注 |
| --- | --- | --- | --- | --- | --- |
| `get_or_create_soul_session` | `crates/santi-runtime/src/session/{send,memory,compact}.rs` | 支持 | 支持 | core | 主路径直接依赖 |
| `get_soul_session` | `crates/santi-runtime/src/session/send.rs` | 支持 | 支持 | core | 主路径直接依赖 |
| `load_turn_context` | `crates/santi-runtime/src/session/send.rs` | 支持 | 支持 | conditional | local 依赖 `sessions` / `souls` 表，语义来源仍偏混合 |
| `write_session_memory` | `crates/santi-runtime/src/session/memory.rs`, `crates/santi-api/src/surface.rs`, `crates/santi-cli/src/backend/local.rs` | 支持 | 支持 | conditional | 能力稳定，但更像附属 memory 面，不是 turn 闭环最小必需 |
| `start_turn` | `crates/santi-runtime/src/session/{send,compact}.rs`, `local_send.rs` | 支持 | 支持 | core | 主路径直接依赖 |
| `append_message_ref` | `crates/santi-runtime/src/session/send.rs`, `local_send.rs` | 支持 | 支持 | core | 主路径直接依赖 |
| `append_tool_call` | `crates/santi-runtime/src/session/send.rs` | **unsupported** | 支持 | optional | local 返回 unsupported |
| `append_tool_result` | `crates/santi-runtime/src/session/send.rs` | **unsupported** | 支持 | optional | local 返回 unsupported |
| `append_compact` | `crates/santi-runtime/src/session/compact.rs` | **unsupported** | 支持 | optional | local 返回 unsupported |
| `complete_turn` | `crates/santi-runtime/src/session/{send,compact}.rs`, `local_send.rs` | 支持 | 支持 | core | 主路径直接依赖 |
| `fail_turn` | `crates/santi-runtime/src/session/send.rs` | 支持 | 支持 | conditional | send 主路径使用，但失败语义需单独收紧 |
| `get_soul_session_by_session_id` | `crates/santi-runtime/src/session/{fork,memory,query}.rs` | 支持 | 支持 | conditional | 查询/辅助定位能力，不属于最小 turn 闭环 |
| `fork_soul_session` | `crates/santi-runtime/src/session/fork.rs` | **unsupported** | 支持 | optional | local 返回 unsupported |
| `list_assembly_items` | `crates/santi-runtime/src/session/{send,compact,query}.rs` | 条件支持 | 支持 | conditional | local 当前仅稳定覆盖 message target |

## 当前运行主路径直接依赖的方法

以下方法已经被 `send` / `compact` / `local_send` 等主路径直接使用：

- `get_or_create_soul_session`
- `get_soul_session`
- `load_turn_context`
- `start_turn`
- `append_message_ref`
- `append_tool_call`
- `append_tool_result`
- `complete_turn`
- `fail_turn`
- `list_assembly_items`

其中需要特别保守对待的是：

- `append_tool_call`
- `append_tool_result`
- `list_assembly_items`

因为它们已经在主路径被调用，但 local 支持面并不等于 postgres。

## 当前最小 core contract 候选

在保持保守口径的前提下，当前最接近最小闭环核心面的候选是：

- `get_or_create_soul_session`
- `get_soul_session`
- `start_turn`
- `append_message_ref`
- `complete_turn`

如需再扩大，优先下一层是：

- `fail_turn`
- `load_turn_context`
- `list_assembly_items`

但这三项都需要继续收紧语义后再决定是否纳入同一主合同。

## 当前明确 optional / leakage 集合

- `append_tool_call`
- `append_tool_result`
- `append_compact`
- `fork_soul_session`

这些方法当前不应被当成所有 adapter 都默认承诺的统一主合同。
