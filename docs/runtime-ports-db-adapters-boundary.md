# Runtime ports 与 DB adapters 边界蓝图

> 目标：把已确认的边界、当前映射、已知不一致点和后续执行顺序固定为基线；只记录当前源代码事实，不扩展到代码逻辑。

## 已成立的边界

- **runtime-agnostic ports**：`crates/santi-core/src/port/*`
- **db adapters**：`crates/santi-db/src/adapter/{local,postgres}`

这条边界当前是后续落地的唯一参考线：port 负责抽象能力，adapter 负责具体存储实现。

## 当前 trait -> impl 映射表

> 说明：这里只记录当前代码中已经存在的明确实现关系。

| trait / port | local impl | postgres impl |
| --- | --- | --- |
| `SessionLedgerPort` | `LocalSessionStore` | `DbSessionLedger` |
| `EffectLedgerPort` | `LocalEffectLedger` | `DbEffectLedger` |
| `SoulPort` | `LocalSoulStore` | `DbSoul` |
| `SoulRuntimePort` | `LocalSoulRuntime` | `DbSoulRuntime` |

相关路径：

- ports：`crates/santi-core/src/port/{session_ledger,effect_ledger,soul,soul_runtime}.rs`
- local impls：`crates/santi-db/src/adapter/local/*`
- postgres impls：`crates/santi-db/src/adapter/postgres/*`

## 当前不一致点

- **命名混杂**：`Store / Ledger / Runtime` 混用，例如 `LocalSessionStore` 实现的是 `SessionLedgerPort`，`LocalSoulStore` 实现的是 `SoulPort`。
- **local partial impl**：`LocalSessionStore::apply_message_event()` 仍返回 unavailable；local 的 compact / fork 能力已拆到各自的本地专用路径，而不是停留在一个复合 helper 里，说明 trait 边界已进一步收窄，但 local 能力仍未完全对齐到同一组 ports。
- **local helper 已拆分**：当前没有 `LocalSessionForkCompactStore` 这类合并式 helper；fork 与 compact 分别由独立的 local 实现承载，仍是本地专用入口，而不是 hosted runtime ports 同构实现。

## 当前源代码事实补充

`SoulRuntimePort` 目前已经收敛到以 `acquire_soul_session` 为入口的运行时获取路径；旧的 `get_or_create_soul_session`、`load_turn_context`、`list_assembly_items` 不再是当前 trait 表面的存在项。

runtime 的 send 路径现在按 `SessionLedgerPort::list_messages` 取消息序列，并通过 `SessionLedgerPort::get_message` 补齐单条消息，再由 runtime 组装 assembly items。

`SessionLedgerPort` 已包含 `get_message`，因此 assembly item 组装所需的 message 读取能力已经落在 ledger port 上，而不是保留在 `SoulRuntimePort` 上。

当前还需要继续盯住的 seam 主要变成两类：

- local 的 fork / compact 各自实现是否还需要继续收敛为更原子的 query seam
- compact / assembly 这类读取能力是否需要新的更原子的 query seam，而不是重新把 composite read 塞回 runtime port

## 最安全的后续执行顺序

1. **先确认 canonical path**：把 `adapter/local/*` 与 `adapter/postgres/*` 作为唯一主入口的事实完全收口。
2. **再清理残余 shim**：确认 root-level compatibility exports、旧 helper 文件和 re-export 壳是否还能删除。
3. **最后再看 contract 细节**：在 current-path 收口后，再决定是否还要继续拆分或补齐能力。

## 当前最安全的下一执行段

- 继续做**contract fact update / runtime query seam 收口**。
- canonical path 已经收口到 `adapter/local/*` 与 `adapter/postgres/*`；root-level 旧实现文件和遗留 helper 已不再是事实入口。
- 目标转为：在不放宽 `SoulRuntimePort` 的前提下，补齐仍真正影响主路径的能力，并决定 compact/list 这类读取要落在哪个更小的边界上。

## adapter root 剩余平铺文件归类

### 当前唯一实现入口

- local:
  - `crates/santi-db/src/adapter/local/session_store.rs`
  - `crates/santi-db/src/adapter/local/effect_ledger.rs`
  - `crates/santi-db/src/adapter/local/soul_store.rs`
  - `crates/santi-db/src/adapter/local/soul_runtime.rs`
  - `crates/santi-db/src/adapter/local/session_fork.rs`
  - `crates/santi-db/src/adapter/local/session_compact.rs`
- postgres:
  - `crates/santi-db/src/adapter/postgres/session_ledger.rs`
  - `crates/santi-db/src/adapter/postgres/effect_ledger.rs`
  - `crates/santi-db/src/adapter/postgres/soul.rs`
  - `crates/santi-db/src/adapter/postgres/soul_runtime.rs`

`adapter/local/*` 与 `adapter/postgres/*` 现在已经是唯一事实入口；不再通过 root-level 实现文件承载真实逻辑。

### 导出枢纽

- `crates/santi-db/src/adapter/mod.rs`

它当前承担：

- 暴露 `adapter::local` 与 `adapter::postgres`
- 不再承担 flat compatibility exports

### 已删除的遗留 / 辅助文件

- `crates/santi-db/src/adapter/turn_store.rs`
- `crates/santi-db/src/adapter/session_query.rs`
- `crates/santi-db/src/adapter/memory_store.rs`

这些旧 helper / 旧 port 遗留已经不再保留在当前 adapter 边界内。

## 最安全的模块收口顺序

1. **保持 consumer 入口只走 canonical path**
   - 所有调用方继续只从 `adapter/local/*` 与 `adapter/postgres/*` 进入。
2. **继续收 contract seam**
   - 把仍会影响主路径的 unsupported / partial impl 收到最小必要集合。
3. **把读取能力缺口放到单独 seam 处理**
   - 尤其是 compact/list 这类 hosted query，不要回退成大而全的 composite runtime port。

### 应最后处理的类别

- `santi-api` / `santi-cli` 这类组合根入口
- 集成测试 / smoke 验证相关路径

## 短期内不要做的事

- 不要先做大规模重命名后再回头收 contract。
- 不要把 flat compatibility layer 继续扩展成新的事实标准。
- 不要因为 postgres 能力更完整，就反向放宽 port contract。
- 不要把 local 的 partial impl 当作已经完成的对等实现。
- 不要在没有 canonical path 的前提下新增新的入口模块。
