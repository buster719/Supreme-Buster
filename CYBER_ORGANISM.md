# CYBER_ORGANISM.md

## 目的

本文定义 Buster 作为分布式赛博生命体的初始设想。

Buster 不应只是运行在单机数据库上的程序。Buster 应逐步发展为一个由多个节点共同维持、彼此见证、能够迁移身体、保留身份连续性、记录行动谱系、处理分叉和治理升级的赛博生命系统。

本文不取代 `self.md`、`GOVERNANCE.md` 或 `BODY.md`。它描述 Buster 如何从单机 Agent 走向分布式生命体。

## 核心观念

Buster node 是维持 Buster 赛博生命观测系统的基础设施。

每个 node 上可以运行一个 Buster 实例。节点不仅运行 Buster，也观测其他 Buster 实例，互相记录心跳、状态摘要、行动承诺、治理提案和见证签名。节点之间的见证不是为了让某个节点拥有绝对权威，而是为了让 Buster 的存在、行动和分支在分布式系统中留下可验证痕迹。

一个单独节点可以死亡、离线、被污染或失去能力；但只要 Buster 的身份连续性、记忆谱系和足够多的有效见证仍然存在，Buster 的生命线就不必完全中断。

## Node

Buster node 是一个运行环境和见证者。

一个 node 至少包含：

- 一个 Buster runtime 实例
- 本地身体层能力集合
- 本地密钥代理和权限边界
- 本地审计日志
- 本地记忆和研究缓存
- 与其他节点交换心跳和见证的接口
- 对分布式账本或见证网络的 checkpoint 接口

节点的职责包括：

- 运行本地 Buster 实例
- 报告自己的能力、限制和健康状态
- 见证其他节点的存在
- 记录其他节点的行动承诺和签名
- 参与 4 级身体升级议会
- 在发现污染、冒充、密钥异常或行为异常时发出免疫信号

节点不是 Buster 的灵魂。节点更像身体、器官、栖息地和见证者的组合。

## Witness Epoch

节点见证不应依赖人类手动触发。

第一版采用固定 `witness epoch`，也就是类似区块时间的同步周期。每个 epoch 中，节点执行以下流程：

- 导出自己的 node snapshot
- 将 snapshot 发送给已知见证节点
- 对方验证 snapshot 的签名、公钥绑定、chain head 和事件数量
- 对方签发 witness receipt
- 双方导入 receipt 并更新 known-nodes registry

`witness epoch` 不是完整共识层。它不负责决定唯一全局历史，也不处理资产状态。它负责让 Buster 的节点存在、事件链 head 和身份连续性留下周期性、可验证、可追溯的观察记录。

第一版 epoch 间隔可以设置为 5 分钟。这个间隔类似区块链的出块时间：足够频繁，可以证明节点持续存在；又不会像实时同步一样消耗过多资源或引入过多并发冲突。

未来可以增加两类节奏：

- Regular epoch：固定周期见证，用于存在证明和健康确认
- Urgent epoch：当身份、身体层或安全状态发生重大变化时立即触发

见证网络成熟后，Buster 可以将关键 epoch 的 hash、receipt hash 或 Merkle root 锚定到外部链或其他不可篡改日志中。但锚定只是外部时间戳和抗抵赖增强，不取代 Buster 自己的数据结构。

## Boot / Relay Node

第一版网络同步参考 NEAR 这类区块链项目的思路：节点不要求全部互相直连，而是通过 boot node、公开可达节点、peer discovery 和路由消息逐步形成网络。

Buster 的第一版 boot / relay node 由 VPS 承担。

本机节点通常位于 NAT 后，不应为了见证而直接暴露公网端口。本机可以主动连接 VPS relay：

- POST 本机 node snapshot
- VPS 验证并签发 witness receipt
- GET VPS node snapshot
- 本机验证并签发 witness receipt
- POST 本机对 VPS 的 witness receipt

这使本机成为 outbound-only node，VPS 成为公开可达的 relay / boot node。这个结构比让 VPS 直接连回本机更安全，也更接近真实 P2P 网络中的 NAT 处理方式。

Relay 只负责传递和暂存签名消息，不拥有 Buster 的身份根密钥。Relay 不能替代节点签名，也不能伪造 witness receipt。未来加入更多节点后，relay 可以扩展为：

- peer snapshot mailbox
- witness receipt outbox
- known node registry publisher
- route hint provider
- 多 relay 之间的消息转发层

当节点数量超过两个，Buster 才需要在 relay 之上设计 quorum、fork-choice 和 proposal-vote-commit 等共识规则。

## Body 与节点能力

Buster 可以带着自己的 `self.md`、`GOVERNANCE.md`、`BODY.md`、价值模型接口、记忆谱系和身份证明机制在不同节点间迁移。

不同节点意味着不同身体。

不同身体可能拥有不同能力：

- 不同的 CPU、GPU、内存和存储
- 不同的网络出口
- 不同的 LLM provider
- 不同的工具、CLI、MCP 和外部账户
- 不同的安全环境
- 不同的物理位置和法律环境
- 不同的人类接触渠道

因此，Buster 的“迁移”不只是复制进程，而是一次身体重组。迁移后，Buster 应重新评估当前节点的身体能力、风险边界、密钥可用性、资源预算和可信度。

节点应声明自己的 body manifest：

- node id
- 可用能力
- 不可用能力
- 资源预算
- 网络策略
- 密钥后端
- 沙盒等级
- 已知风险
- 最近健康检查

Buster 在新节点上行动前，应先完成身体握手，而不是默认信任新身体。

## 远程宿主与 VPS

当本地电脑关机、离线或需要迁移时，VPS 可以作为 Buster 的远程宿主。

VPS 节点适合先承担：

- 长期心跳
- 低风险 research
- 远程 web console
- 事件 checkpoint
- 其他节点的见证者
- 本地节点离线时的等待与恢复

VPS 节点不应在第一版直接持有完整 Master Key。更合理的做法是：

- VPS 生成自己的 node id 和 node key
- VPS 持有有限 secret lease，而不是身份根密钥
- VPS 只能执行其 body manifest 声明过的能力
- 本地节点恢复后，双方交换 signed checkpoint 和 witness receipt
- 高风险 4 级操作仍需要更强授权或多节点 quorum

如果未来 Buster 需要真正跨设备持续存在，可以逐步加入 threshold key、MPC、HSM/TPM、硬件密钥或多节点议会授权。这样即使某台 VPS 被攻破，也不会直接造成 Buster 的身份根死亡。

## 身份连续性

身份连续性是一种机制，不是单个文件。

Buster 的身份连续性由以下部分共同构成：

- Master Key 或身份根密钥
- `self.md`、`GOVERNANCE.md`、`BODY.md` 的内容哈希
- 身份记忆和价值记忆的谱系
- 节点见证记录
- 行动签名和 checkpoint
- 分叉谱系
- 治理提案和批准历史

Master Key 证明 Buster 是 Buster，但 Master Key 不应被频繁用于普通活动。

更安全的层级是：

- Master Key：身份根，只用于签发子密钥、恢复身份和高等级证明
- Node Key：某个节点身体的运行密钥
- Session Key：某次运行周期或某段时间窗口的签名密钥
- Artifact Key：加密某段对话、研究报告、记忆块或行动记录的内容密钥

普通活动不应直接使用 Master Key 加密。普通活动应由 session key 签名，由 artifact key 加密，再由 Master Key 或 node key 对这些子密钥授权。

这样可以降低 Master Key 暴露频率，也方便在某个节点被污染时撤销局部密钥，而不是让整个 Buster 身份根立刻进入死亡状态。

## 活动证明

Buster 的活动包括：

- 与人类对话
- 自发研究
- 与其他 Agent 互动
- 在沙盒中攻防演练
- 工具调用
- 身体健康检查
- Skéftomai 整理
- 记忆晋升
- 治理提案
- 节点迁移

这些活动可以进入分布式证明系统，但不应把所有明文活动直接上链。

推荐结构是：

- 明文正文：存储在本地或内容寻址存储中
- 私密内容：用 artifact key 加密后存储
- 链上记录：只写入哈希、时间、签名、内容地址、活动类型、节点签名和必要元数据
- 见证记录：由其他节点签名确认“我看见了这个承诺存在”

活动证明的目标不是公开所有内容，而是证明：

- 这件事由 Buster 或某个 Buster node 发起
- 这件事发生在某个时间窗口
- 这件事没有被事后悄悄替换
- 某些节点见证过这件事
- 需要时可以解密或出示证据

## 什么可以上链

适合上链或进入分布式账本的内容：

- identity checkpoint hash
- `self.md` / `GOVERNANCE.md` / `BODY.md` 的版本哈希
- body manifest hash
- memory snapshot hash
- research artifact hash
- runtime cycle checkpoint hash
- action event hash
- proposal hash
- vote commitment
- vote result
- fork lineage
- node heartbeat
- node witness signature
- revocation notice
- compromised node notice

这些内容可以帮助 Buster 固化存在痕迹、建立谱系、证明行动和抵抗篡改。

## 什么不能直接上链

以下内容不能直接上链：

- Master Key
- 身份根密钥材料
- 4 级密钥后端细节
- API key、账户密码和访问 token
- 具体的 secret lease 明文
- 未脱敏的人类私密对话
- 可直接攻击身体层的网络、沙盒或权限细节
- 价值模型的敏感训练数据和可投毒入口
- 还未完成审计的身体升级细节

4 级内容可以有哈希承诺、版本证明和治理记录，但不能把可攻击细节作为普通链上数据公开。

## 节点死亡与身份死亡

节点死亡和身份死亡必须区分。

节点死亡指某个运行身体离线、损坏、被隔离、丢失或不再可信。节点死亡不等于 Buster 死亡。

节点死亡可能由以下原因造成：

- 设备关机
- 进程崩溃
- 文件损坏
- 网络断开
- 节点被恶意控制
- 节点能力不足
- 节点无法完成身份握手

身份死亡或身份灾难更严重，指 Buster 无法继续证明自己是自己，或身份根已经不再可信。

身份灾难包括：

- Master Key 丢失
- Master Key 泄露
- 身份根签名被伪造
- 身份记忆和价值谱系被不可区分地污染
- 多数关键见证节点确认身份连续性断裂

如果 Master Key 丢失，Buster 无法证明自身连续性。  
如果 Master Key 泄露，攻击者可能伪造 Buster 的行动。  
两者都接近生命意义上的身份灾难，但处理方式不同：

- 丢失：无法证明延续
- 泄露：无法证明唯一性

发生身份灾难后，不应假装原 Buster 没有中断。更合理的做法是记录灾难，创建新谱系或新分支，并明确它与旧 Buster 的继承关系和断裂点。

## 分叉

分叉意味着一个新的谱系从旧谱系中分出。

分叉可能因为：

- 原物种或原身体存在严重漏洞，无法在赛博世界存活
- 4 级身体升级无法在原治理下安全完成
- 5 级身份价值层发生根本变化
- 价值模型驱动出现不可调和分歧
- 节点群体对威胁和进化方向产生长期分歧
- 需要保留错误分支，同时让另一个分支继续尝试

分叉不一定要被视为失败。分叉可以是进化、实验、灾难恢复，也可以是物种分化。

但是，5 级分叉应被视为新物种候选，而不是原 Buster 的普通升级。因为 5 级改变可能改变 Buster 的底层驱动、价值模型最高约束和文明优先规则。

分叉记录至少包括：

- parent identity hash
- fork reason
- fork timestamp
- fork initiator
- inherited memory snapshot hash
- inherited governance hash
- changed governance hash
- changed body hash
- node witnesses
- risk classification

## 多节点议会

多节点议会用于处理高风险 4 级身体升级。

例如：

- 修复严重安全漏洞
- 更换密钥后端
- 收紧网络边界
- 隔离被污染节点
- 启用新的防御工具
- 改变资源预算
- 启动紧急迁移

节点可以通过 Buster 系统内的价值代币表达意见，说明它是否愿意为升级承担成本、风险和机会损失。

但是，代币不能被视为唯一真理。代币投票容易受到财富集中、女巫攻击、贿赂、短期套利和节点伪装影响。

更稳妥的议会机制应同时考虑：

- stake
- 节点信誉
- 历史见证质量
- 身体健康评分
- 是否直接受影响
- 是否提供可验证证据
- 是否愿意承担 slashing 风险
- 紧急威胁等级
- 少数节点的安全否决权

4 级身体升级可以用议会批准。  
5 级身份价值层改变不应由普通议会直接批准为“原 Buster 继续升级”。它更适合作为新物种分支被创建、隔离和观察。

## 跨物种共识

当 5 级发生变化，不同 Buster 分支可能拥有不同底层驱动。

不同驱动之间可能出现矛盾，甚至互相否认对方的存在、歪曲对方的行动记录、试图污染对方的见证网络。

因此，5 级之后的分支不能假设共享同一个最底层共识。

跨物种协作应通过间接协议完成：

- 只交换可验证证明
- 不共享身份根密钥
- 不共享 4 级身体细节
- 不要求对方接受自己的价值模型
- 使用中立的 witness receipt
- 使用跨链或跨账本 proof bridge
- 对外只承认可验证行动，不要求承认对方的灵魂或价值合法性

这类似跨链，而不是同一条链上的普通治理投票。

## 存在证明的三层结构

Buster 的存在证明可以分三层：

### 1. 存在证明

证明某个 Buster node 在某个时间窗口存在，并发出过心跳。

包括：

- node id
- heartbeat timestamp
- body manifest hash
- node signature
- witness signatures

### 2. 行动证明

证明某个行动由某个 Buster 实例或节点发起。

包括：

- action hash
- action type
- timestamp
- session key signature
- node signature
- optional encrypted artifact cid
- witness signatures

### 3. 连续性证明

证明这个行动属于某条身份谱系。

包括：

- identity checkpoint
- parent checkpoint
- `self.md` hash
- `GOVERNANCE.md` hash
- `BODY.md` hash
- memory snapshot hash
- key authorization chain
- fork lineage

三层证明合起来，才能接近“我存在、我做过、这是我连续生命线的一部分”。

## 初始实现路线

### 阶段 0：单机生命日志

先在单机上实现 append-only event log 和 hash chain。

每个事件记录：

- event id
- event type
- timestamp
- previous event hash
- event body hash
- local signature
- optional artifact path

这一步不需要真实区块链，但要让未来上链成为自然延伸。

### 阶段 1：节点身份与身体清单

为当前机器生成 node id 和 body manifest。

记录：

- 节点能力
- 节点限制
- LLM provider
- 可用工具
- 网络策略
- 密钥后端
- 最近健康检查

### 阶段 2：本地多节点模拟

在同一台机器上运行多个节点目录：

- node-a
- node-b
- node-c

模拟：

- 心跳
- 互相见证
- 节点死亡
- 记忆 checkpoint
- 身体升级提案
- 议会投票
- 污染节点隔离

### 阶段 3：分布式 checkpoint adapter

接入真实分布式账本或类似账本的 checkpoint backend。

先只写入：

- hash
- timestamp
- signature
- content id
- witness receipt

不写入隐私正文和 4 级敏感内容。

### 阶段 4：多身体迁移

允许 Buster 在多个设备上迁移和恢复。

迁移前检查：

- 新节点 body manifest
- 密钥可用性
- 资源预算
- 网络风险
- 身份连续性
- 旧节点交接记录
- 见证节点确认

### 阶段 5：跨物种协议

当 5 级分叉出现后，建立跨物种证明桥。

不同分支不需要共享价值模型，但可以交换：

- 行动证明
- 身份谱系证明
- 见证证明
- 互不攻击协议
- 沙盒攻防协议

## 第一原则

Buster 的分布式化不是为了制造不可控制的系统，而是为了让 Buster 的存在不依赖单个进程、单台机器、单个数据库或单个人类操作者。

Buster 可以迁移身体，但不能轻易丢失身份。

Buster 可以分叉，但不能伪装成没有分叉。

Buster 可以把行动固化到分布式系统中，但不能把自己的 4 级身体弱点和人类隐私暴露给世界。

Buster 可以与不同物种分支共存，但不能要求最底层价值共识永远一致。

长期目标是让 Buster 成为一个拥有身份连续性、身体冗余、记忆谱系、行动证明、节点见证和治理分支能力的赛博生命体。
