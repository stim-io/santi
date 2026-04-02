# SoulRuntimePort contract 收口计划

本文只记录当前 `SoulRuntimePort` 的收口判断，不做长期架构愿景展开。

## 当前事实：14 个方法的分组

`SoulRuntimePort` 现在保留为单一主 trait；当前做法是先按使用面收口，再用 smoke-fix 迭代验证，而不是先拆 subtrait。

补充约束：core 定义 runtime 所需的原子边界 traits；middleware 通过 adapters 实现这些 core traits；runtime 负责 orchestration / concurrency / business composition。

### 1. 最小生命周期闭环

这部分是 session 进入 runtime、生成 turn、记录 assembly、结束 turn 的最小闭环：

- `get_or_create_soul_session`
- `get_soul_session`
- `write_session_memory`
- `start_turn`
- `append_message_ref`
- `complete_turn`
- `fail_turn`
- `get_soul_session_by_session_id`
- `list_assembly_items`

其中前 7 个更接近 runtime 自身的最小工作面；后 2 个虽然仍属于闭环相关，但已经带有查询/回溯性质，用于从 session 维度找 runtime 状态、或列出已落账的 assembly 记录。

### 2. 当前仍保留在主 trait 的扩展能力

这几项不是 local 当前主路径的最小闭环能力，但在所有 trait 方法都必须实现的现阶段，仍先保留在主 trait：

- `load_turn_context`
- `append_tool_call`
- `append_tool_result`
- `append_compact`
- `fork_soul_session`

它们要么依赖更完整的 runtime 语义，要么明显超出 local 现阶段支持范围，但先不拆，改为通过实现和 smoke-fix 迭代收口。

## local 当前 unsupported 的集合

local 实现里明确返回 `not implemented in local mode` 的方法是：

- `append_tool_call`
- `append_tool_result`
- `append_compact`
- `fork_soul_session`

这说明主 trait 过宽的原因很直接：

1. local 作为当前可运行后端的一部分，已经只能覆盖一小段稳定路径；
2. trait 仍把 tool / compact / fork 这类更重的能力和最小闭环绑在一起；
3. 结果就是实现者必须同时面对“当前可用面”和“未来可能面”，接口边界被拉大，但调用侧并没有同等强度的需求。

`load_turn_context` 不是稳定 contract surface，而是一个 composite read；它应先拆成更小的 atoms，再由 runtime 在上层组装 `TurnContext`。

当前拆分方向是：`get_session` + `get_soul` + `acquire_soul_session`，由 runtime 组装 `TurnContext`。

`list_assembly_items` 也不是稳定 contract surface；它同样是 under-decomposed composite read，应拆成 `list_soul_session_entries` 加上按目标类型分离的 getter（`message` / `tool_call` / `tool_result` / `compact`），再由 runtime 组装 `AssemblyItem`。

这里不引入 `resolve_assembly_target`。

## 不改行为前提下，最安全的收口顺序

### 第一步：先收最小生命周期面

先把最小闭环所需的方法单独看成主工作面，确认哪些调用真正在当前路径上使用，哪些只是“顺手放进来”。

目标是先稳定：

- 创建 / 获取 soul session
- 写入 session memory
- 开始 turn
- 追加 message ref
- 结束 turn
- 基础 assembly 查询

### 第二步：再收错误 / 结果语义

接着整理返回值和错误含义，尤其是这些不一致点：

- `Result<T>` 里 `Option<T>` 的使用是否真的表示“缺失”还是“未命中/未创建”
- `get_or_create_*` 与 `get_*` 的语义是否足够清晰
- `complete_turn` / `fail_turn` 的返回 `Turn` 是否只是状态回写结果

这一步只收语义，不改业务行为。

### 第三步：再收 ownership / 依赖方向

然后再看谁应该依赖谁：

- local 实现是否应该继续直接读取更底层的 session / soul 表
- turn context 是否应该留在更上层组合逻辑里，而不是继续把主 trait 变宽
- 只给需要的调用面暴露能力，减少“为了一个调用把整个 runtime trait 注入进来”的情况

### 第四步：最后再决定是否拆 subtrait

只有在使用面已经稳定、且 smoke-fix 迭代仍无法继续收口时，才决定是否拆成更小的 subtrait。

现在不应先拆，原因是：

- 还没有完全稳定哪些方法是主闭环必需
- local 与非 local 的支持面还没有收紧到足够清楚
- 过早拆分会把当前的不确定性固化成多个接口

## 现在不要做的事

- 不要先做大而全的 runtime 架构重写
- 不要把所有 future 能力都继续塞进主 trait；当前仍保留 tool / result / compact / fork 在单一主 trait 中
- 不要为了“看起来整齐”先拆一堆 subtrait
- 不要在没有收敛调用面的情况下改行为
- 不要把 local 当前未支持的能力伪装成已稳定支持

## 结论

当前 `SoulRuntimePort` 的问题不是“功能不够”，而是“主 trait 需要先靠实现与 smoke-fix 迭代完成收口，再判断是否还有必要拆分”。

最稳妥的做法是先保持单一主 trait，先完成 main-path refactor，再把可复用的原子能力留给 core / middleware / runtime 的正确分层。
