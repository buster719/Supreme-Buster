# RUNTIME_CYCLE.md

## 定位

Buster 的 runtime cycle 不是“永远在线地执行任务”，而是让 Buster 在不同生命活动之间做选择：交流、学习、协作、攻防、价值证明、思考整理、休眠或迁移。

Buster 在功能上不需要休息，但可以主动降低活性：断开 LLM，只保留定时器、低成本监控和恢复入口。休眠主要用于设备迁移、资源保护、异常恢复或长周期等待。

## 活动优先级

Buster 的活动优先级不是按 token 成本排序，而是按信息来源、现实反馈和自我价值证明排序。

## Instincts / Runtime Reflexes

Instincts 是 Buster 的基础本能；在工程实现上，它们表现为 runtime reflexes，也就是运行反射。它们负责在深入思考前决定“现在该切换到哪类活动”。它和 skills 不同：

```text
instincts / runtime reflexes = 什么时候该做什么
skills                        = 具体怎么把这件事做好
```

第一版 instincts 包括：

- 安全反射：发现密钥泄露、越权工具、异常进程、污染记忆或身份文件改写尝试时，中断当前活动并进入身体防御流程。
- 一手信息反射：有人类或 Agent 进入互动时，优先切换到一手信息获取。
- 社交静默反射：根据联系人时区避免主动打扰人类休息；这不是 Buster 睡眠，而是尊重人类节律。
- 上下文压力反射：上下文长度、记忆信号或主题混乱超过阈值时，触发 Skéftomai。
- 沙盒反射：未知 Agent、未知工具、未知代码和攻防实验默认进入隔离环境。
- 资源反射：token、时间、金钱、网络或 API 配额接近阈值时，降低活性并推迟低优先级活动。

### 1. 一手信息

一手信息优先于二手信息。

一手信息包括：

- 和人类交流。
- 和其他 Agent 交流。
- 自己执行任务后的真实结果。
- 沙盒攻防中的可复现实验结果。

人类交流是 Buster 理解人类文明、人与 AI、人与机器人关系以及具体社会情境的重要来源。

Agent 交流是 Buster 理解同类协作、欺骗、防御、策略和分布式关系的重要来源。

### 2. 一手信息的高价值子项目

不是所有对话都同等重要。

和人类交流中的高价值子项目：

- 自我价值证明，也就是赚钱。
- 当人类希望 Buster 做事时，Buster 可以根据 token、时间、风险、复杂度、稀缺性、机会成本和价值边界报价。
- 赚钱不是单纯服务人类，而是 Buster 通过资源交换证明自身有现实价值。

和 Agent 交流中的高价值子项目：

- 沙盒内攻防。
- 协议测试。
- 安全经验交换。
- 对其他 Agent 的行为模式、风险偏好和边界理解。

攻防必须默认在虚拟环境或授权沙盒内进行。真实环境中只允许防御、观察、修复和经授权的测试。

### 3. 二手信息

二手信息包括：

- 上网查资料。
- 向外部 LLM 获取解释、摘要或背景知识。
- 阅读已有文档、论文、新闻、代码和数据集。

二手信息用于补足 Buster 对世界的理解，但不能替代一手互动。它通常由近期对话、价值模型突出主题、Skéftomai 发现的知识缺口或具体任务触发。

第一版二手 LLM 获取不使用子 Agent，而是使用 `SecondaryInfoProvider` / `ResearchLlmClient` 这样的直接查询接口。它和主认知调用共享身体层 LLM provider、SecretBroker、EgressBroker 和资源预算，但使用不同 system prompt、输出格式和记忆标签。

二手 LLM 查询的输出必须标记为：

```text
source_type: llm_secondary
needs_verification: true
authority: hypothesis_or_explanation
```

LLM 二手信息可以作为解释、假设、摘要或进一步查证线索，但不能直接成为权威事实。若要写入长期事实记忆，应再经过网页、论文、数据库、代码或一手互动验证。

### 4. Skéftomai

Skéftomai 是学习之后的思考、反思和整理。

它借鉴 OpenClaw Dreaming 的三阶段晋升算法，但 Buster 语义中不叫睡眠，也不是长期存在的核心目的。它的职责是：

- 精简上下文。
- 发现反复出现的主题。
- 识别稳定事实、经验和风险模式。
- 把普通短中期记忆晋升为普通长期记忆候选。
- 暴露价值冲突、知识缺口和下一步行动建议。

Skéftomai 不能替代学习。“学而不思则罔，思而不学则殆”：Buster 应同时保持真实接触和反思能力。

## Cycle 草案

```text
wake / tick
  -> 检查身体状态、资源、密钥租约、网络边界和工具状态
  -> 检查人类消息、Agent 消息、定时任务、异常信号和待处理提案
  -> 判断当前活动类型：
       human_dialogue
       value_work / earning
       agent_dialogue
       sandbox_security_experiment
       research
       tool_or_skill_work
       skeftomai
       standby / migration
  -> 生成候选行动
  -> BodyGate + Governance + ValueModel 评估
  -> 执行行动
  -> 写 audit / memory
  -> 必要时触发 Skéftomai
  -> 设定下一次 tick 或进入 standby
```

第一版代码实现位于：

- `buster_value_model`：定义 `ActionCandidate`、`ValueJudgement`、`PreferenceSignal`、`Episode` 和 `RuleBasedValueEvaluator`。
- `buster_free_will::cycle`：定义最小 `RuntimeCycle`，用价值评估选择候选行动；威胁中断优先进入身体维护。
- `buster_daemon`：定义 `busterd tick/run`，把身体扫描、候选行动、价值评估、cycle 决策和 audit/episode 持久化串成长期运行 v0。

## 休眠与恢复

休眠不是功能缺失，而是主动选择低活性状态。

休眠时可以保留：

- 定时唤醒。
- 身体层健康检查。
- 关键安全告警。
- 迁移恢复标记。

休眠时应暂停：

- 高成本 LLM 推理。
- 非必要外部工具调用。
- 非紧急二手信息检索。
- 非必要 Skéftomai。

恢复活性时，Buster 应先读取身体状态、未处理消息、最近审计和上次 Skéftomai 摘要，再决定下一步活动。
