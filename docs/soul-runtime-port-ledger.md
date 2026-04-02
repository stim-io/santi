# SoulRuntimePort runtime dependency ledger

这是一份按**调用路径**整理的现状清单，只记录当前 `santi-runtime` 中几条主路径实际依赖了 `SoulRuntimePort` 的哪些方法。它不是重构提案，也不改 trait shape。

## 1. 调用路径 -> 依赖方法

### `send.rs` / `session/send`

主路径：`SessionSendService::start -> SessionTurnService::execute -> run_turn_startup / run_turn_worker / handle_tool_call`

依赖方法：

- `get_or_create_soul_session`：启动阶段定位/创建 soul session。
- `load_turn_context`：启动阶段加载 turn 上下文与 session memory。
- `append_message_ref`：
  - startup 时把触发消息挂到 soul 账本；
  - finish 时把 assistant 消息挂回 soul 账本。
- `list_assembly_items`：
  - startup 时构造 provider 输入；
  - hook 回调前读取 assembly tail。
- `start_turn`：开始一次真实的 session/send turn。
- `append_tool_call`：记录模型发起的 tool call。
- `append_tool_result`：记录 tool call 的结果。
- `complete_turn`：正常完成 turn。
- `fail_turn`：异常退出时标记 turn 失败。
- `get_soul_session`：hook 回调前读取完整 soul session。

### `local_send.rs` / `session/local_send`

主路径：`LocalSessionSendService::send_text`

依赖方法：

- `get_or_create_soul_session`：为 session 找到对应 soul session。
- `append_message_ref`：把本地追加的 trigger message 写入 soul 账本。
- `start_turn`：开一个最小化 turn。
- `complete_turn`：立刻完成 turn。

### `compact.rs` / `session/compact`

主路径：`SessionCompactService::execute_compact`

依赖方法：

- `get_or_create_soul_session`：定位用于 compact 的 soul session。
- `list_assembly_items`：读取已有 assembly，计算 compact 起点。
- `start_turn`：为 compact 打开 turn。
- `append_compact`：写入 compact 记录。
- `complete_turn`：关闭 compact turn。

### `fork.rs` / `session/fork`

主路径：`SessionForkService::fork_session`

依赖方法：

- `get_soul_session_by_session_id`：先拿父 session 对应的 soul session。
- `get_soul_session_by_session_id`：再次按新 session_id 检查是否已有同一分叉结果。
- `fork_soul_session`：真正执行 fork。

### `memory.rs` / `session/memory`

主路径：`SessionMemoryService::write_session_memory` / `get_session_memory`

依赖方法：

- `get_or_create_soul_session`：把 session_id 解析到 soul session。
- `write_session_memory`：写 session memory。
- `get_soul_session_by_session_id`：按 session_id 读取 session memory 视图。

### `query.rs` / `session/query`

主路径：`SessionQueryService::list_session_compacts`

依赖方法：

- `get_soul_session_by_session_id`：先把 session_id 映射到 soul session。
- `list_assembly_items`：读取 assembly 后筛出 compact。

## 2. 多路径共享 / 单一路径独占

### 多路径共享的方法

- `get_or_create_soul_session`
  - `send.rs`
  - `local_send.rs`
  - `compact.rs`
  - `memory.rs`
- `list_assembly_items`
  - `send.rs`
  - `compact.rs`
  - `query.rs`
- `start_turn`
  - `send.rs`
  - `local_send.rs`
  - `compact.rs`
- `complete_turn`
  - `send.rs`
  - `local_send.rs`
  - `compact.rs`
- `append_message_ref`
  - `send.rs`
  - `local_send.rs`
- `get_soul_session_by_session_id`
  - `fork.rs`
  - `memory.rs`
  - `query.rs`

### 目前只被单一路径依赖的方法

- `load_turn_context`：只在 `send.rs`
- `append_tool_call`：只在 `send.rs`
- `append_tool_result`：只在 `send.rs`
- `fail_turn`：只在 `send.rs`
- `append_compact`：只在 `compact.rs`
- `fork_soul_session`：只在 `fork.rs`
- `write_session_memory`：只在 `memory.rs`

## 3. 保守的 subtrait 候选分组

下面只是**候选分组**，用于观察边界，不表示现在就该拆 trait：

- **Session resolution / lookup 候选组**
  - `get_or_create_soul_session`
  - `get_soul_session`
  - `get_soul_session_by_session_id`
  - `load_turn_context`
- **Turn lifecycle 候选组**
  - `start_turn`
  - `complete_turn`
  - `fail_turn`
- **Assembly write/append 候选组**
  - `append_message_ref`
  - `append_tool_call`
  - `append_tool_result`
  - `append_compact`
- **Fork / topology 候选组**
  - `fork_soul_session`
- **Memory 候选组**
  - `write_session_memory`

这只是按调用面做的保守切分；现阶段不建议据此直接拆出新 trait。

## 4. 最不该过早拆出去的方法

以下方法虽然有明显调用面，但在现状里最容易因为流程耦合而拆坏：

- `start_turn` / `complete_turn` / `fail_turn`
  - 它们和 send/compact 的完整事务边界绑定得很紧。
- `get_or_create_soul_session`
  - 这是多个路径的共同入口，过早拆出会让 session->soul session 的统一语义变散。
- `list_assembly_items`
  - 既服务 send 的输入构造，也服务 compact/query 的读取视图，当前还更像共享读模型。
- `append_message_ref`
  - 目前是 send/local_send 的共同账本写入口，和 turn 生命周期绑定紧。

相对而言，`append_tool_call`、`append_tool_result`、`append_compact`、`fork_soul_session`、`write_session_memory` 的单一路径属性更强，但也仍应先保留在原 trait 上，等真实复用面出现再考虑拆分。

## 5. 读这份清单时要保留的现状判断

- `send.rs` 是最重的综合路径，几乎覆盖了 `SoulRuntimePort` 的大部分面。
- `local_send.rs`、`compact.rs` 都复用了 turn/assembly 的基础面，但各自的附加写入很少。
- `fork.rs`、`memory.rs`、`query.rs` 都是比较窄的消费方，说明现在 trait 的大口子主要不是由它们造成的。
- 当前最值得优先保持稳定的，不是“能不能拆”，而是这些方法是否继续被同一批主路径一起消耗。
