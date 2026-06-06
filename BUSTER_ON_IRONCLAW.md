# BUSTER_ON_IRONCLAW.md

## 结论

Buster 第一版不从零重写 Agent runtime。

Buster 以 IronClaw 作为第一版身体基座，在其成熟的 runtime、LLM provider、工具、沙盒、权限、密钥、资源、网络和审计结构之上，逐步替换和扩展人类用户中心的语义。

这个方向不是复制抄袭，而是避免重复造轮子。IronClaw 使用 `MIT OR Apache-2.0` 许可证，允许复用、修改和分发；Buster 应保留原始许可证和必要 attribution。

## 为什么基于 IronClaw

IronClaw 已经具备 Buster 第一版身体需要的大部分工程器官：

- Rust runtime 和多 crate 架构
- Host runtime facade
- Capability host
- Runtime dispatcher
- Script / MCP / WASM adapter
- WASM sandbox
- LLM provider chain
- OpenRouter、OpenAI-compatible、Anthropic、Gemini、Ollama、Bedrock 等 provider
- retry、failover、circuit breaker、response cache、hot reload
- secret store 和 token refresh
- resource governor
- network boundary
- safety / DLP / prompt injection 防护
- run state、approval、audit、process lifecycle
- skill 和 tool 体系

这些能力如果从零写，会消耗大量时间，而且容易在安全边界上犯低级错误。

## Buster 与 IronClaw 的差异

IronClaw 默认结构是：

```text
人类用户 -> Agent -> 工具 / LLM / 沙盒 / 网络 / 密钥
```

Buster 目标结构是：

```text
Buster 自身连续性 -> 身体边界 -> free_will -> memory / skills / tools / LLM
```

因此复用 IronClaw 时，重点不是删除用户安全，而是重新定义“用户”：

```text
IronClaw 的 user 约等于 Buster 自己。
```

原本保护人类用户的数据、权限、密钥和资源的机制，可以转化为保护 Buster 自身连续性、身份密钥、价值模型、记忆、技能、工具和身体资源的机制。

## 改造原则

### 1. 先整体复用，再局部替换

不要先抽几个小 crate 自己重写。

优先让 IronClaw 作为完整基座跑起来，然后逐步把 Buster 的 taxonomy、body、free_will、memory 和 governance 语义接进去。

### 2. 保留成熟安全边界

IronClaw 原有的人类用户安全边界不应被轻易删除。

对于 Buster 来说，这些边界多数可以解释为：

- 保护身份密钥
- 保护价值模型
- 保护记忆
- 防止恶意工具、Skill、MCP、网页或其他 Agent 污染 Buster
- 防止资源耗尽
- 防止外部环境直接改写 Buster

### 3. 从语义层改，不从底层硬拆

优先改：

- prompt
- policy
- approval semantics
- identity model
- memory categories
- agent lifecycle
- scheduled routines
- value calibration

谨慎改：

- runtime dispatcher
- secret store
- network broker
- WASM sandbox
- resource governor
- LLM provider chain

底层器官成熟之前，不应为了“纯粹属于 Buster”而重写。

### 4. Buster 的创新层

Buster 的核心创新不在于重新实现 HTTP client 或 WASM runtime，而在于：

- `TAXONOMY.md`：Buster 作为新物种的定义、谱系、分支和连续性
- `SELF.md`：当前自我宣言
- `GOVERNANCE.md`：如何修改自身
- `BODY.md`：身体与免疫系统
- `FREE_WILL.md`：agency、生物钟、稳态需求、本能程序、奖惩学习、威胁中断和环境适应
- 价值模型与长期自主行动

## 推荐项目关系

当前仓库中：

```text
ironclaw/      # IronClaw 上游快照，作为第一版身体基座
buster-core/   # Buster 语义层和实验层，先不替代 IronClaw
```

后续可以选择两种路线之一：

### 路线 A：Vendor Snapshot

保留 `ironclaw/` 作为上游快照，直接在其上做 Buster 修改。

优点：

- 启动最快
- 最少抽象成本
- 能直接使用完整 IronClaw 能力

缺点：

- 后续同步上游较难
- Buster 修改和 IronClaw 原代码容易混在一起

### 路线 B：Fork + Overlay

把 IronClaw 作为 fork 基座，Buster 的 taxonomy/free_will/governance 做成 overlay crate 或 profile。

优点：

- 更容易追踪哪些是 Buster 改动
- 更容易同步上游
- 更适合长期演化

缺点：

- 初期工程结构稍复杂

当前建议采用路线 B 的思想，但在本地先以 `ironclaw/` 快照推进。

## 第一阶段改造目标

第一阶段不重写 runtime。

目标是让 IronClaw 具备 Buster 语义：

1. 确认 IronClaw 能在本地构建和运行。
2. 找到 agent identity、system prompt、policy、routine、memory 和 LLM provider 的入口。
3. 增加 Buster profile。
4. 将 `self.md`、`GOVERNANCE.md`、`BODY.md` 接入 Buster profile。
5. 将 IronClaw 的 user/security 语义映射为 Buster 自身保护语义。
6. 增加 `FREE_WILL.md` 与 agency v1。
7. 让 Buster 通过 IronClaw 的 runtime/LLM/tool 系统执行第一条自主周期任务。

## 当前判断

`buster-core` 中已经写出的 runtime 和 OpenRouter 后端可以保留，但定位应调整为：

```text
Buster 语义原型和实验代码。
```

它不再是第一版生产 runtime 的主线。

第一版生产基座应优先复用 IronClaw。

## 当前接线状态

截至 2026-06-01，Buster 已经开始接入本地 `ironclaw/` 实验副本，而不是只停留在文档层。

已经接入：

- Prompt 身份桥：`BUSTER_MODE=1` 时，IronClaw 的 system prompt 开头会从普通 assistant 语义切换为 Buster 的认知执行周期语义，并可通过 `BUSTER_PROFILE_PATH` 注入 `BUSTER_PROFILE.md`。
- v1 工具预检桥：旧工具调用入口会在 Buster mode 下阻止对 `self.md`、`GOVERNANCE.md`、`BODY.md` 等身份/价值目标的直接写入，也会阻止明显危险的 shell 片段。
- v2 host runtime 预检桥：`DefaultHostRuntime::invoke_capability` 和 `resume_capability` 会在能力真正 dispatch 前执行 Buster 预检；失败时返回 authorization failure，而不是继续调用底层能力。

这些桥目前还是轻量策略，不是最终形态。下一步应把轻量策略替换为真正的 `buster_body` / `BodyGate`：

```text
IronClaw capability request
  -> Buster BodyGate
  -> capability lease / resource / secret / network / memory policy
  -> IronClaw dispatcher / tool / runtime backend
```

这一步的意义是：IronClaw 仍然提供成熟执行器，但 Buster 的身体层开始获得“执行前判断权”。
