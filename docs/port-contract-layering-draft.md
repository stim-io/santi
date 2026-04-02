# Runtime Port Contract 分层草案

> 这是**代码行为不变**前提下的 contract layering 草案。它只整理当前 4 个核心 ports 的能力分层，不修改 trait 形状，不调整 runtime 主流程，也不要求 local 立即补齐能力。

## 目标

在继续做命名收口、模块收口、trait 拆分或 local 能力补齐之前，先把当前 ports 的能力分成三层：

- **core contract**：runtime 直接依赖的原子边界能力，local / postgres 都应可稳定承诺
- **conditional / partial capability**：方法在多后端都可调用，但支持范围或语义边界不完全对等
- **optional capability**：当前只有部分后端完整承诺，不能当作统一主合同

## 分层结果

### 1) Core contract

#### `SessionLedgerPort`

- `create_session`
- `get_session`
- `list_messages`
- `append_message`

#### `EffectLedgerPort`

- `list_effects`
- `get_effect`
- `create_effect`
- `update_effect`

#### `SoulPort`

- `get_soul`
- `write_soul_memory`

#### `SoulRuntimePort`

- `get_or_create_soul_session`
- `get_soul_session`
- `load_turn_context`
- `write_session_memory`
- `start_turn`
- `append_message_ref`
- `complete_turn`
- `fail_turn`
- `get_soul_session_by_session_id`

这些方法是当前最适合被当作 runtime 主流程稳定依赖面的部分。

### 2) Conditional / partial capability

#### `SoulRuntimePort`

- `list_assembly_items`

当前口径：

- postgres 侧按完整 assembly item 语义工作
- local 侧当前仅对 `message target` 有稳定支持

因此它可以继续保留在统一 trait 中，但在合同分层上不应被写成 full core。

### 3) Optional capability

#### `SessionLedgerPort`

- `apply_message_event`

#### `SoulRuntimePort`

- `append_tool_call`
- `append_tool_result`
- `append_compact`
- `fork_soul_session`

当前这些能力都是 postgres 完整、local unavailable / unsupported 的状态，不能当作所有 adapter 都应默认承诺的主合同。

## 当前最安全的解释口径

- **core contract**：runtime 可直接依赖的原子边界能力
- **conditional / partial capability**：runtime 只能在已知边界内依赖，文档必须写清支持范围
- **optional capability**：不应在不加判断的前提下成为 runtime 主流程前提

## 最安全的后续顺序

1. **先固定三层口径与措辞**
   - 把 `unsupported / unavailable / partial` 的解释写准
2. **再决定 canonical contract**
   - 明确 runtime 主流程只依赖哪一层
3. **最后再决定后续动作**
   - 是拆 trait
   - 还是补 local
   - 还是把部分能力显式标成 hosted-only / non-local

## 这份草案现在不做的事

- 不改 trait 定义
- 不改 adapter 实现
- 不改 runtime 主流程
- 不改变 local / postgres 现有行为
- 不提前决定最终 trait 拆分方案
