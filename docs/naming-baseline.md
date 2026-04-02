# 命名收口基线

> 这是一份后续改名前的**基线文档**。它只冻结当前命名认知、列出不一致点和最小收口建议，不改代码逻辑，也不要求立刻执行重命名。

## 1. 冻结当前建议词汇表

当前先按下面的词汇分工理解命名：

- `Port`：只用于 trait 名
- `Ledger`：用于账本语义
- `Runtime`：用于执行态
- `Store`：不要再和上面三类词混用来表达同一职责

补充口径：`get` / `create` / `acquire` 作为基础动词分开理解；其中 `get_or_create` 在 domain 层收束为 `acquire`，表示高频获取/创建的一体化原子，不要把它理解成 lock 语义。

这意味着后续命名收口应优先让 trait、实现类型、模块路径在词汇上对齐，而不是继续沿用历史混搭。

## 2. 当前一对一命名不一致点

以下是目前最明显的“trait 词汇”和“实现词汇”不一致点：

| trait / port | 当前 local 实现名 | 当前模块文件名 | 不一致说明 |
| --- | --- | --- | --- |
| `SessionLedgerPort` | `LocalSessionStore` | `crates/santi-db/src/adapter/local/session_store.rs` | trait 是 `Ledger`，实现仍用 `Store` |
| `SoulPort` | `LocalSoulStore` | `crates/santi-db/src/adapter/local/soul_store.rs` | trait 是 `Port`，实现仍用 `Store` |
| `SessionLedgerPort` | `DbSessionLedger` | `crates/santi-db/src/adapter/postgres/session_ledger.rs` | 这里与 `Ledger` 词汇一致，但模块文件仍保留旧层级命名习惯 |
| `SoulPort` | `DbSoul` | `crates/santi-db/src/adapter/postgres/soul.rs` | trait 是 `Port`，实现名较短，模块名未显式表达 `Port` |
| `SoulRuntimePort` | `LocalSoulRuntime` / `DbSoulRuntime` | `crates/santi-db/src/adapter/local/soul_runtime.rs` / `crates/santi-db/src/adapter/postgres/soul_runtime.rs` | 这组是目前最接近一致的一组 |

补充：`adapter/mod.rs` 仍保留兼容导出，说明当前模块入口还不是完全单一事实源。

## 3. 最小命名收口清单（仅建议映射）

下面只是建议映射，不是现在就改代码：

| 当前名字 | 建议目标名 |
| --- | --- |
| `LocalSessionStore` | `SessionLedgerPort` 对应实现名，优先收口到 `LocalSessionLedger` 或等价的 `Ledger` 命名 |
| `LocalSoulStore` | `SoulPort` 对应实现名，优先收口到 `LocalSoulPort` 或等价的 `Port` 命名 |
| `local/session_store.rs` | `local/session_ledger.rs` |
| `local/soul_store.rs` | `local/soul.rs` 或 `local/soul_port.rs` |
| `DbSessionLedger` | 维持或与 local 端统一到同一命名风格 |
| `DbSoul` | 维持或补足为更显式的 `DbSoulPort` 风格 |

最小目标不是一次性把所有名字改漂亮，而是先把“同一职责用同一词汇”落实到最关键的 1:1 对应上。

## 4. 现在不该急着改的名字

以下名字当前不建议急改：

- `SoulRuntimePort` / `LocalSoulRuntime` / `DbSoulRuntime`
  - 这一组已经相对稳定，词汇边界也最清楚
- `EffectLedgerPort` / `LocalEffectLedger` / `DbEffectLedger`
  - 这组已经与 `Ledger` 词汇一致
- `adapter/mod.rs`
  - 这里还承担兼容导出，先不要因为命名收口而立刻删改入口
- 任何已经被大量调用的公共符号
  - 先等这份基线后续收口方向确认，再逐步迁移

## 5. 文档定位

这份文档的作用是：

- 作为后续改名前的命名基线
- 冻结当前对 `Port / Ledger / Runtime / Store` 的使用口径
- 记录现状中的一对一不一致点
- 给出最小收口建议，但不推动代码变更

补充边界：`load_turn_context` 和 `list_assembly_items` 不应继续被当作稳定 contract surface；它们是待拆的 composite reads，后续命名应跟随更小的 atoms 与 runtime-side composition，而不是继续固化大而全的动词。

换句话说：它是“先统一认知，再决定改名”的起点，不是最终重命名方案。
