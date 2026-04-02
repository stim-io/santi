# SoulRuntimePort candidate trait reshaping sequence baseline

> 这是一份**顺序基线**，只用于在现有 tightening plan / capability matrix / ledger / do-not-split zones 之上约束讨论顺序。**不改代码，不拍板最终 trait 形状。**

## 0. 先定前提

在进入任何 reshaping 讨论前，先默认以下现状不动：

- `SoulRuntimePort` 仍按当前主路径被整体消费。
- `send.rs` 仍是最重的综合路径，不能被“先拆再说”的直觉带偏。
- local 当前 unsupported 的能力，不能被当成已稳定主合同。
- 任何拆分讨论都必须先排除错误方向，再讨论可做方案。

## 1. 最安全的阶段顺序

### Phase 1 — 先冻结事实基线

目标：先把“现在到底有哪些方法、哪些路径、哪些支持面”固定住。

允许做什么：

- 复核 capability matrix 的 core / conditional / optional 分类。
- 复核 ledger 里的调用路径归属。
- 复核 do-not-split zones 里的耦合簇。
- 只做文档级对齐，不做实现修改。

不允许做什么：

- 不许先画新的 trait 层级图。
- 不许把任何 future capability 直接提升为主合同。
- 不许以局部命名差异作为拆分依据。

### Phase 2 — 先排除错误 reshaping 方向

目标：先明确哪些方向不能走，避免把错误边界固化成新接口。

允许做什么：

- 逐条排除“仅仅因为 capability matrix 有一行就拆”。
- 逐条排除“仅仅因为方法名不同就拆”。
- 逐条排除“仅仅因为返回类型不同就拆”。
- 逐条排除“为了整齐先拆一堆 subtrait”。

不允许做什么：

- 不许因为单一路径独占就立刻外拆。
- 不许把 unsupported 方法伪装成稳定可移交能力。
- 不许把局部讨论写成最终 contract。

### Phase 3 — 再固定兼容观察期的稳定面

目标：在任何 reshaping 之前，先冻结兼容观察期内不能变的调用面。

允许做什么：

- 仅标注“必须保持不变”的方法/簇。
- 仅说明哪些调用方仍依赖同一条主路径。
- 仅定义观察期内的稳定边界。

不允许做什么：

- 不许改这些方法的名字、入参、返回语义或调用责任。
- 不许把观察期内的稳定面拆散到多个新接口。
- 不许把查询/定位/执行态混成新的不稳定抽象。

### Phase 4 — 只做保守候选分组，不做拍板

目标：在稳定面不动的前提下，才讨论候选分组。

允许做什么：

- 只保守列出候选 subtrait 方向。
- 只讨论哪些簇可能在未来被独立命名。
- 只记录“可观察”的边界，不下最终结论。

不允许做什么：

- 不许宣布最终 trait 形状。
- 不许把候选分组当成已确定设计。
- 不许提前移动实现归属。

## 2. 兼容观察期内必须保持不变的方法 / 簇

以下内容在兼容观察期内应保持不变：

### 必须保持不变的方法

- `get_or_create_soul_session`
- `get_soul_session`
- `load_turn_context`
- `start_turn`
- `append_message_ref`
- `complete_turn`
- `fail_turn`
- `get_soul_session_by_session_id`
- `list_assembly_items`

### 必须保持不变的主路径耦合簇

- **Turn lifecycle 簇**
  - `start_turn`
  - `complete_turn`
  - `fail_turn`
- **Session -> soul session 入口簇**
  - `get_or_create_soul_session`
  - `append_message_ref`
  - `list_assembly_items`
- **共享定位簇**
  - `get_soul_session_by_session_id`

### 观察期内特别要保守对待的方法

- `load_turn_context`
- `fail_turn`
- `list_assembly_items`

这些方法都已经进入主路径，但语义上仍有较强的共享面 / 装配面 / 失败收口面特征，不能先被当成独立稳定岛。

## 3. 不可做清单

先排除错误 reshaping 方向，再进入可做方案。以下事项在 baseline 阶段都不可以做：

- 不要先做大而全的 runtime 架构重写。
- 不要把所有 future 能力继续塞进主 trait。
- 不要为了“看起来整齐”先拆一堆 subtrait。
- 不要在没有收敛调用面的情况下改行为。
- 不要把 local 当前未支持的能力伪装成已稳定支持。
- 不要仅凭 capability matrix 的一行就拆。
- 不要仅凭方法命名不同就拆。
- 不要仅凭返回类型不同就拆。
- 不要仅凭“以后可能独立发展”就拆。
- 不要把局部独占调用误判成可立即移交的独立 contract。

## 4. 仅供后续讨论的可做方向

在上面的禁止项都排除之后，后续才可以讨论：

- 是否需要把最小生命周期面单独观察。
- 是否需要把查询 / 定位 / 执行态继续分层讨论。
- 是否需要在足够稳定后再考虑 subtrait。

但这一步仍然只是讨论入口，不是最终方案。
