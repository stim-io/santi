# SoulRuntimePort: do-not-split zones & conservative subtrait draft

> 讨论稿，仅用于边界判断。**不改 trait shape，不改代码。**

## 1. 先必须绑定在一起的生命周期耦合簇

下面这些方法当前应视为同一条主路径上的耦合簇，优先保持在同一个 port/trait 语义边界内，不要先拆：

- **Turn lifecycle 簇**
  - `start_turn`
  - `complete_turn`
  - `fail_turn`
  - 原因：这三项直接绑定 send / compact 的 turn 生命周期与收尾语义。
- **Session -> soul session 入口簇**
  - `get_or_create_soul_session`
  - `append_message_ref`
  - `list_assembly_items`
  - 原因：它们共同构成“从 session 进入 soul runtime，并读写 assembly”的最小共享路径。
- **共享定位簇**
  - `get_soul_session_by_session_id`
  - 原因：虽然它不直接进入 turn，但被 fork / memory / query 共同依赖，当前仍更像共享定位面，而不是应立即单拆的能力岛。

## 2. 看似可拆、但现在拆会破坏主路径理解的单一路径方法

以下方法如果单独抽离，表面上像是独立 capability，但现在会削弱对主路径的理解：

- `load_turn_context`
  - 只在 `send.rs` 使用，但它仍属于 send 启动阶段的核心上下文装配。
- `fail_turn`
  - 虽然只有 send 主路径直接显式依赖，但它和 `start_turn / complete_turn` 是同一事务边界的一部分。
- `append_tool_call`
  - 只在 send 主路径出现，但它是 send worker 内部 tool loop 的一部分，不能仅凭“单一路径”就当成可立即外拆。
- `append_tool_result`
  - 同上；它与 `append_tool_call` 以及 send worker 主循环绑定。

## 3. 保守 subtrait 候选方向（仅讨论稿）

> 这里只列保守方向，不表示现在就该拆。

1. **LifecycleControlDraft**
   - 候选范围：`start_turn` / `complete_turn` / `fail_turn`
   - 适用前提：先确认 turn 生命周期确实可独立于 assembly / lookup 面表达。

2. **ExecutionSurfaceDraft**
   - 候选范围：`get_or_create_soul_session` / `append_message_ref` / `list_assembly_items`
   - 适用前提：先确认这是稳定共享读写面，而不是临时绑在 send/local_send/compact 上的组合面。

3. **RuntimeReadOnlyDraft**
   - 候选范围：`get_soul_session` / `get_soul_session_by_session_id` / `load_turn_context`
   - 适用前提：先确认这些读取/定位方法不会继续牵引执行态 contract。

> `append_tool_call` / `append_tool_result` / `append_compact` / `fork_soul_session` 当前更适合作为 future optional capability 候选，而不是立刻形成稳定 subtrait。

## 4. 当前无效的拆分理由

以下理由**不能单独作为拆 trait 的依据**：

- **仅仅因为 capability matrix 里有一行**。
- **仅仅因为方法名看起来不同**，但实际共享同一状态机或句柄。
- **仅仅因为返回类型不同**，但来源仍是同一主流程。
- **仅仅因为“以后可能会独立发展”**，但现在没有真实使用边界。
- **仅仅为了把接口拆得更细**，但会让主路径阅读变差。

## 5. 结论

当前建议：

- 先把 `SoulRuntimePort` 作为一个**主路径优先**的整体理解。
- 先识别生命周期耦合簇，再判断是否存在稳定、可独立命名的边界。
- 在没有明确分层证据前，不要因局部 capability 或局部命名差异进行拆分。

> 本文仅为保守讨论稿，不改变现有 trait 结构与代码实现。
