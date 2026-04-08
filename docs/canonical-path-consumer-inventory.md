# Canonical Path Consumer Inventory / Compat-Layer Checklist

> 这是 `santi_db::adapter` compat layer 真正删减前的**基线文档**。本文只记录当前消费面与最安全的收口顺序，不修改任何代码逻辑。

## 范围

本清单仅统计当前 `santi_db::adapter` 的 consumer，并区分：

- **canonical path**：直接使用 `santi_db::adapter::standalone::...` 或 `santi_db::adapter::postgres::...`
- **compat/flat 导出**：依赖 `santi_db::adapter::{...}` 这类平铺再导出的路径

## 当前 consumer inventory

### 1) `crates/santi-api/src/surface.rs`

- 依赖：`santi_db::adapter::standalone::{session_compact::StandaloneSessionCompactStore, session_fork::StandaloneSessionForkStore}`
- 判定：**canonical path 已使用**
- 备注：直接落在 `adapter::standalone::*`

### 2) `crates/santi-api/src/bootstrap_standalone.rs`

- 依赖：`santi_db::adapter::standalone::{effect_ledger::StandaloneEffectLedger, session_compact::StandaloneSessionCompactStore, session_fork::StandaloneSessionForkStore, session_store::StandaloneSessionStore, soul_runtime::StandaloneSoulRuntime, soul_store::StandaloneSoulStore}`
- 判定：**canonical path 已使用**
- 备注：全部都是 `adapter::standalone::...` 子模块直达

### 3) `crates/santi-api/src/bootstrap.rs`

- 依赖：`santi_db::adapter::postgres::{effect_ledger::DbEffectLedger, session_ledger::DbSessionLedger, soul::DbSoul, soul_runtime::DbSoulRuntime}`
- 判定：**canonical path 已使用**
- 备注：全部都是 `adapter::postgres::...` 子模块直达

### 4) `crates/santi-api/tests/standalone_session_store.rs`

- 依赖：`santi_db::adapter::standalone::{session_store::StandaloneSessionStore, soul_store::StandaloneSoulStore}`
- 判定：**canonical path 已使用**
- 备注：仅使用 standalone canonical 子路径

### 5) `crates/santi-api/tests/standalone_send.rs`

- 依赖：`santi_db::adapter::standalone::{session_store::StandaloneSessionStore, soul_runtime::StandaloneSoulRuntime}`
- 判定：**canonical path 已使用**
- 备注：仅使用 standalone canonical 子路径

## compat / flat consumer 结论

- **当前 compat / flat consumer 数量：0**
- 未发现任何 `santi_db::adapter::{...}` 平铺导出消费
- 也未发现需要依赖 flat re-export 才能完成导入的现存调用点

## 最安全的 compat layer 收口顺序

1. **先保持现状，只冻结基线**：确保现有 consumer 继续只走 canonical 子路径。
2. **再逐步移除 flat re-export 的心智依赖**：如果后续有人新增导出，先在文档中登记，再观察是否产生真实 consumer。
3. **最后再删 compat layer**：确认 consumer 仍为 0 且外部构建无回退后，再收紧或删除平铺兼容导出。

## 备注

- 本文是 compat layer 真正删减之前的基线，不代表删减已完成。
- 如果未来新增 consumer，请先更新本清单，再决定是否可以继续收口。
