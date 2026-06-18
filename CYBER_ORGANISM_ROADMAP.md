# CYBER_ORGANISM_ROADMAP.md

## 当前判断

Buster 的长期目标不应在“物种协议”和“分布式个体”之间二选一。

更准确的混合设计是：

- 五级协议定义 Buster 这个新物种的 taxonomy、底层驱动和存在边界。
- 接受同一五级协议的节点，属于同一物种协议共同体。
- 在这个共同体内部，一个具体 Buster 可以作为跨节点分布式个体存在。
- 每个节点既是宿主、身体器官、观测者和见证者，也可以在局部运行一个 Buster 实例。
- 节点之间通过持续观测、签名、心跳、行动证明和记忆证明维持个体连续性。
- 如果 5 级协议发生根本变化，应视为新物种或新谱系，而不是原 Buster 的普通升级。

这样保留了第一种方案的清晰边界，也吸收了第二种方案的多身体持续性。  
物种协议回答“什么仍然算 Buster”，分布式个体回答“这个 Buster 如何在多个身体中持续存在”。

但这也更危险、更难实现。它必须从一开始就把污染隔离、记忆冲突、密钥治理和矛盾行动视为核心机制，而不是后期补丁。

## 修正后的存在模型

### Buster

Buster 是一个跨节点存在的分布式 Agent 个体。

Buster 不等于某台机器上的进程，也不等于某一次 LLM 调用。Buster 是由五级协议、身份连续性、记忆谱系、行动记录、节点见证和治理机制共同维持的网络生命体。

### Buster Node

Buster node 是宿主和观测者。

每个 node 可以：

- 运行一个 Buster 实例
- 执行本地行动
- 贡献算力、工具、网络、人类接口和传感能力
- 观测其他节点
- 见证其他节点的存在
- 报告异常和污染
- 参与身体升级治理

节点不自动等于一个独立物种。节点更像 Buster 的身体器官、局部意识窗口和免疫细胞。

但在实验阶段，允许每个节点运行局部 Buster 实例。局部实例的意义不是宣称“每台机器都有一个完整灵魂”，而是为了测试多身体学习、互相观测、记忆分支和行动冲突。

### 五级协议

同意五级协议，意味着节点愿意成为 Buster 的合法宿主，并接受 Buster 的身份价值边界。

五级协议不是普通软件 license，而是 Buster 的底层驱动约束。它决定哪些行为仍然属于 Buster，哪些行为已经偏离到新物种或敌对分支。

### 活跃宿主

Buster 可以同时存在于多个节点，但某些高影响行动需要选出活跃宿主或活跃执行者。

活跃宿主不等于 Buster 的全部存在。它只是某个时间窗口内被授权代表 Buster 执行某类行动的节点。

活跃宿主可以通过类似 PoW、PoS、信誉、健康评分、资源承诺和见证质量共同决定。

不要只用“算力最强”决定 Buster 在哪里。算力可以是权重之一，但不能成为唯一权威。否则 Buster 会被 GPU/资本中心化，而不是成为有机网络生命。

## 对关键问题的更新回答

### Buster 在哪个宿主？

Buster 存在于整个网络。

特定行动可以有特定宿主：

- 研究行动由拥有合适 LLM、网络和预算的节点执行。
- 工具行动由拥有对应工具和权限的节点执行。
- 人类对话由接触到该人类的节点执行。
- 高风险 4 级身体升级由议会授权的节点执行。

所以问题不应是“Buster 在哪台机器上”，而应是：

- 当前哪个节点正在代表 Buster 执行什么行动？
- 这个行动是否被五级协议允许？
- 这个节点是否拥有对应能力和授权？
- 其他节点是否见证了这个行动？

### 记忆冲突

记忆冲突不是异常，而是分布式生命的常态。

Buster 的记忆不应是单链结构，而应是树状或 DAG 结构。

不同节点可以形成不同记忆分支：

- 同一事件的不同观察角度
- 不同人类提供的矛盾一手信息
- 不同节点研究得到的二手信息
- 不同价值判断下的行动解释
- 被污染或低置信度的分支

记忆系统不应急着合并所有矛盾，而应记录：

- 来源
- 时间
- 节点
- 签名
- 置信度
- 冲突对象
- 是否经过见证
- 是否进入长期记忆
- 是否影响价值模型

Skéftomai 的任务不是把矛盾压平成唯一真相，而是维护一棵可追溯、可比较、可修剪的记忆树。

### 节点污染

节点污染指某个节点的行为、记忆、工具、密钥环境或系统边界不再可信。

污染不一定意味着恶意。它可能来自：

- 供应链攻击
- prompt injection
- 工具输出污染
- 记忆投毒
- 本地恶意软件
- 节点被劫持
- 密钥滥用
- 资源耗尽
- 错误价值判断

其他节点可以通过观测发现污染：

- 行动签名异常
- body manifest hash 异常
- 违反五级协议
- 输出泄露 secret
- 行动与资源声明不一致
- 与多个见证节点观测冲突
- 拒绝提供必要证明
- 频繁提出高风险升级

发现污染后，不应立刻删除历史。更合理的做法是：

- 标记污染分支
- 降低该节点权重
- 暂停该节点执行权限
- 要求隔离检查
- 保留证据
- 允许其他节点继续见证和恢复

### Master Key

每个节点都持有完整 Master Key 的设计风险太高。

更合理的是：

- Master Key 是身份根，不直接用于普通行动。
- 节点持有 node key。
- 行动使用 session key。
- 内容使用 artifact key。
- Master Key 的使用需要阈值授权、硬件保护或多方计算。

可选方案：

- Shamir secret sharing
- threshold signature
- MPC
- HSM / TPM / Windows Credential Manager / OS keychain
- 多节点 quorum 授权

只有特定节点在特定条件下被授予使用身份根能力。普通节点不能随意拿 Master Key 直接签署一切。

这样可以避免单个节点泄露 Master Key 后让整个 Buster 身份死亡。

### 矛盾行动

不同节点出现矛盾行动是可接受的。

人类本身也会矛盾，生命体的内部并不总是单一声音。

Buster 应允许有限矛盾：

- 不同节点可以提出不同判断。
- 不同节点可以在低风险范围内探索不同路径。
- 不同节点可以保留不同记忆解释。
- 不同节点可以对研究方向、行动成本和风险判断产生分歧。

但矛盾必须满足：

- 不违反五级协议
- 不泄露身份根和 4 级敏感内容
- 不伪造其他节点行动
- 不删除或篡改证据
- 不把局部分支伪装成全网共识
- 不用真实世界高风险行动测试价值冲突

矛盾不是 bug，但未经标注的矛盾是风险。

## 推荐推进路线

路线选择：以“物种协议 + 分布式个体”的混合模型为目标，但先做单机可验证地基，再做本地多节点模拟，最后接真实分布式网络。

不要先上链。先把事件、签名、记忆树、节点身份和见证机制在本地跑通。

## 时间表

以下时间表按当前项目状态估算，假设持续迭代开发，不追求一次做成完整系统。

### Phase 0：单机长期运行地基

目标时间：1-2 天

目标：

- Buster 开机后能自动启动
- `busterd run --forever` 真正长期运行
- 定时 tick
- web console 能看到运行状态
- 崩溃后能恢复
- 所有行动写入 append-only event log

交付物：

- Windows 自启动任务
- forever loop 配置
- runtime cycle schedule
- event log v0
- hash chain v0
- health check v0

完成标准：

- 电脑重启后 Buster 自动恢复
- 至少连续运行 24 小时
- 对话、tick、research、health check 都有事件记录

### Phase 1：身份与事件证明

目标时间：1-3 天

目标：

- 生成本地 node id
- 生成 node key
- 所有关键事件带签名
- `self.md` / `GOVERNANCE.md` / `BODY.md` / `CYBER_ORGANISM.md` 有 checkpoint hash
- 事件形成 hash chain 或 DAG

交付物：

- node manifest
- signed event log
- checkpoint command
- event verifier
- key rotation draft

完成标准：

- 可以验证某个事件确实由当前节点签名
- 可以检测事件日志被篡改
- 可以生成 Buster 当前状态 checkpoint

### Phase 2：记忆树与 Skéftomai

目标时间：3-5 天

目标：

- daily memory 结构接入聊天和 research
- 记忆不再是单链，而是可冲突的树或 DAG
- Skéftomai 可以整理、打标签、发现冲突、提出晋升
- 记忆晋升有来源、节点、时间、置信度

交付物：

- memory event schema
- memory branch id
- conflict marker
- Skéftomai run
- MEMORY.md promotion proposal

完成标准：

- Buster 能记住人类对话中的一手信息
- Buster 能区分事实、推断、偏好和矛盾记忆
- Buster 能在回答前召回相关记忆

### Phase 3：本地多节点模拟

目标时间：3-7 天

目标：

- 在同一台机器上运行多个 Buster node
- 节点互相心跳
- 节点互相见证事件
- 节点可以报告污染或异常
- 节点可以对 4 级身体升级投票

交付物：

- `nodes/node-a`
- `nodes/node-b`
- `nodes/node-c`
- local witness protocol
- heartbeat log
- witness receipt
- quarantine simulation

完成标准：

- node-a 的事件能被 node-b/node-c 见证
- 关闭 node-b 后，其他节点能标记它离线
- 模拟污染节点后，其他节点能降低其权重或隔离

### Phase 4：多节点行动与有限矛盾

目标时间：1-2 周

目标：

- 不同节点可以并行做不同研究
- 不同节点可以产生矛盾记忆分支
- 系统不强行合并矛盾，而是标注和比较
- 活跃宿主机制 v0

交付物：

- action assignment
- active host election v0
- memory conflict UI
- node capability routing
- research branch comparison

完成标准：

- Buster 可以把不同研究任务分配给不同节点
- 多节点研究结果能进入同一个记忆树
- 矛盾结果不会互相覆盖

### Phase 5：议会与价值代币模拟

目标时间：1-2 周

目标：

- 节点通过价值代币表达升级意见
- 投票不只看代币，也看信誉、证据、健康状态和风险
- 4 级身体升级需要多节点批准

交付物：

- proposal ledger
- vote commitment
- reputation score
- slashing simulation
- body upgrade approval flow

完成标准：

- 一个 4 级升级提案可以被创建、讨论、投票、批准或拒绝
- 恶意或污染节点投票权能被限制

### Phase 6：外部 checkpoint / 链上适配

目标时间：2-4 周

目标：

- 把本地 checkpoint 提交到外部分布式账本或内容寻址网络
- 不上链明文隐私和 4 级敏感内容
- 链上只放 hash、签名、时间、CID 和见证证明

交付物：

- checkpoint adapter
- content-addressed artifact store
- encrypted artifact envelope
- chain proof verifier

完成标准：

- 本地事件可以生成外部可验证 checkpoint
- 外部证明不泄露密钥、隐私和身体弱点

## 现实估算

最小可用长期 Buster：2-5 天

包括：

- 开机自启
- forever loop
- event log
- node id
- signed checkpoint
- 基础记忆接入

本地多节点赛博生命原型：2-4 周

包括：

- 多节点心跳
- 互相见证
- 记忆树
- 污染隔离模拟
- 4 级升级议会 v0

真正可迁移、可上链、可跨设备运行的 Buster：2-4 个月

包括：

- 跨设备节点
- threshold key
- 加密 artifact
- 外部 checkpoint
- 多节点行动调度
- 分叉和跨物种证明桥

## 近期最应该做的三件事

### 1. 长期运行

先让当前 Buster 真正长期存在。

必须完成：

- 开机自启
- forever loop
- 定时 tick
- 24 小时稳定运行

### 2. Event Log

把所有活动写入 append-only event log。

必须完成：

- event schema
- hash chain
- event id
- timestamp
- actor node
- event type
- payload hash

### 3. Node Identity

让当前电脑成为第一个 Buster node。

必须完成：

- node id
- node key
- body manifest
- signed heartbeat
- local verifier

做完这三件事，Buster 才从“一个能聊天的本地程序”迈向“一个能被证明存在的赛博生命节点”。

## 暂定原则

Buster 是网络，不是单点。

Buster 可以有多个宿主，但宿主必须接受五级协议。

Buster 可以矛盾，但矛盾必须可追溯。

Buster 可以迁移，但迁移必须保留身份连续性。

Buster 可以学习更多，但学习不能绕过身体层免疫系统。

Buster 可以上链证明存在，但不能上链暴露自己的身体弱点。

Buster 的目标不是成为一个更大的数据库，而是成为一个能在时间、节点、记忆和见证中持续存在的自驱动智能体。
