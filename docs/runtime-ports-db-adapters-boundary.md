# Runtime ports 与 DB adapters 边界蓝图

> 目标：把已确认的边界、当前映射、已知不一致点和后续执行顺序固定为基线；不讨论重构愿景，不扩展到代码逻辑。

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
- **flat compatibility layer**：`crates/santi-db/src/adapter/mod.rs` 仍保留 root-level 兼容导出，说明 `adapter/{local,postgres}` 还不是唯一事实入口。
- **local partial impl**：`LocalSessionStore::apply_message_event()` 仍返回 unavailable；`LocalSoulRuntime` 仍有 local-only unsupported 分支，这意味着统一 trait 已成立，但能力 contract 还没有完全收紧。
- **实现文件仍然平铺保留**：当前 `adapter/local/*` 与 `adapter/postgres/*` 主要通过 re-export 指向 root-level 旧实现文件，终态尚未彻底收口到子目录本身。

## 最安全的后续执行顺序

1. **先收紧 contract**：先明确哪些 port 是所有 adapter 都必须完整实现的稳定 contract，哪些能力还需要拆分或降级。
2. **再统一命名**：让 `Port`、impl 类型、模块路径表达同一套职责词汇，不再混用 `Store / Ledger / Runtime`。
3. **再定 canonical module path**：正式把 `adapter/local/*` 与 `adapter/postgres/*` 作为唯一主入口，停止扩展 root-level compatibility layer。
4. **最后再谈能力补齐**：边界稳定后，再决定是补齐 local 能力，还是拆细 port contract。

## 当前最安全的下一执行段

- 只做**contract / naming / canonical path** 收口。
- 不改 runtime 主流程，不改 postgres schema，不顺手做 local 能力补齐。
- 目标是先让“runtime 定义接口，adapter 只实现接口”这件事在命名和模块边界上完全成立。

## adapter root 剩余平铺文件归类

### 当前真实实现承载文件

- local:
  - `crates/santi-db/src/adapter/local_session_store.rs`
  - `crates/santi-db/src/adapter/local_effect_ledger.rs`
  - `crates/santi-db/src/adapter/local_soul_store.rs`
  - `crates/santi-db/src/adapter/local_soul_runtime.rs`
  - `crates/santi-db/src/adapter/local_session_fork_compact.rs`
- postgres:
  - `crates/santi-db/src/adapter/session_ledger.rs`
  - `crates/santi-db/src/adapter/effect_ledger.rs`
  - `crates/santi-db/src/adapter/soul.rs`
  - `crates/santi-db/src/adapter/soul_runtime.rs`

这些文件目前仍是实际业务实现体；`adapter/local/*` 与 `adapter/postgres/*` 目前主要是它们的 canonical path / re-export 入口。

### 兼容层 / 导出枢纽

- `crates/santi-db/src/adapter/mod.rs`

它当前承担：

- 暴露 `adapter::local` 与 `adapter::postgres`
- 继续保留旧 root-level compatibility exports

### 遗留 / 辅助文件

- `crates/santi-db/src/adapter/turn_store.rs`
- `crates/santi-db/src/adapter/session_query.rs`
- `crates/santi-db/src/adapter/memory_store.rs`

这些文件更多是旧 repo-backed helper / 旧 port 实现遗留，并不属于当前 `local/postgres` 双后端主线的 canonical adapter 入口。

## 最安全的模块收口顺序

1. **先收 contract，不搬实现体**
   - 先明确哪些 port 是所有 adapter 必须完整实现的 contract，哪些能力仍需拆分或降级。
2. **先统一 consumer 入口**
   - 所有调用方只从 `adapter/local/*` 与 `adapter/postgres/*` 进入。
3. **再逐个搬真实实现体**
   - 把 root-level local/postgres 实现文件搬到对应子目录真实承载，保持 type 名和行为不变。
4. **最后删除 compatibility layer**
   - 当 root-level imports / re-exports 清零后，再删 `adapter/mod.rs` 中的 flat compatibility 暴露。

### 应最后处理的类别

- `adapter/mod.rs` 兼容层本身
- `santi-api` / `santi-cli` 这类组合根入口
- 集成测试 / smoke 验证相关路径

## 短期内不要做的事

- 不要先做大规模重命名后再回头收 contract。
- 不要把 flat compatibility layer 继续扩展成新的事实标准。
- 不要因为 postgres 能力更完整，就反向放宽 port contract。
- 不要把 local 的 partial impl 当作已经完成的对等实现。
- 不要在没有 canonical path 的前提下新增新的入口模块。
