# 大模型Agent下一站：17篇核心论文，看Harness如何自进化

![image](https://mmbiz.qpic.cn/mmbiz_gif/Psho9dm7oDHKVtfYDubjKdZRUjAfBQQicXjoZWJ3qnK42ooD4eeJUfJBM4SSZVa2RE5lO0j6rWwzliby0j9u4bDg/640?wx_fmt=gif&wxfrom=5&wx_lazy=1&tp=webp#imgIndex=0)

模型权重之外，Agent 的上下文、工具、记忆、失败修复和源码更新，正在成为新的能力来源。

现在看一个 Agent 强不强，已经不能只看它背后的模型。

它能看到哪些上下文，能调用哪些工具，动作如何被约束，失败后能否从轨迹里定位问题，这些都不在模型权重里，却直接影响 Agent 的最终表现。

这也是 Harness 近几个月持续升温的原因。Harness 也从模型外部的工程脚手架，逐渐变成可以被生成、搜索、修复和持续更新的研究对象。

我们从近期论文中挑选了 17 篇值得读的 Harness 相关工作，覆盖系统边界、自动生成、失败修复、模块演化和长期可靠性几个方向。

如果你最近也在关注 Agent Harness，这份清单可以先收下慢慢读。

![image](https://mmbiz.qpic.cn/mmbiz_png/Psho9dm7oDGhKg9nnSz5qQrwKvXibt3wulOVRfC18yCkd6xXqGq22h6QUk8chptF0fnQ4uXeZtAktYMrWwG2SyQ/640?wx_fmt=png#imgIndex=2)

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

系统边界

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeD3pAQqnb73iaI3pdewtWQtHnAiaSuaY5ShgEADRap9Fh1VpVYicK5mvVw0GPwlH9xAJ79dSISI7P39PZ0x4KGI8Y57MgziafHrEDI/640?wx_fmt=png&from=appmsg)

论文标题：

Agent Harness Engineering: A Survey

论文地址：

https://picrew.github.io/LLM-Harness/main.pdf

这篇综述来自 CMU、Yale、JHU、UAB、Amazon 等机构，把 Agent Harness 明确放在模型外部的系统层来看。它涵盖执行、工具、上下文、生命周期、可观测性、验证和治理七个维度。

这个分类的价值在于，它把 prompt、工具调用、记忆、安全和评测中分散的问题，放进了同一个工程框架。

论文还映射了 170 多个开源项目，并把 OpenAI、Anthropic、LangChain 等工程实践纳入讨论，适合作为理解近期 Harness 工作的概念底座。

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeAvLibSSV75vRtrFKKfhYBC1ZiaY5NyAUxlrYEn4Dia2fy5Y5poNEtib5vaCwAt0apRThFTexicuiatAz5yNAcSicFTWickyRE6ekBQRgo/640?wx_fmt=png&from=appmsg)

〓 Agent Harness 的 ETCLOVG 七层框架

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeDgUARlsvvEK5qjoHtlMWKNATlz0TDcbC4HSH68CPtFvibqpzbbbr5icGicyOvTwy63xfLF0Rd2MVxcjQZFlNO8apicpFMw8ibd7L3g/640?wx_fmt=png&from=appmsg)

论文标题：

Code as Agent Harness

论文地址：

https://arxiv.org/abs/2605.18747

来自 UIUC、Meta 和 Stanford 的这项工作，从代码角度理解 Harness。代码在这里同时承担推理、行动、环境建模和执行验证，不再只是最终生成物。

论文从接口、机制和多 Agent 扩展三个层面组织内容，讨论代码如何成为 Agent 运行时的重要承载形式。

在这一路线下，Harness 更多以代码形式存在：系统可以生成、搜索、改写它，也可以围绕代码执行结果继续优化。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeAeiaK6MNibSWiaiaqb4oa1YHIV2owclN8LkOWGEEZkXHibLqS3rzev02lG0qdpEsB3cQn1wmoSGJJibsE2sGoUUALCEiaD5sVdJqrNLc/640?wx_fmt=png&from=appmsg)

〓 Code as Agent Harness 的三层结构

![image](https://mmbiz.qpic.cn/mmbiz_png/Psho9dm7oDGhKg9nnSz5qQrwKvXibt3wuhfgUpIfdPSqH8YjjHbCUiaaKsMA36bIMsMtGNKoBcus5py06M0fvx3A/640?wx_fmt=png&tp=webp&wxfrom=5&wx_lazy=1#imgIndex=4)

自动生成

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeDQsxFgWTwwiaGBMhnS1woGphr55QOB9qia6yeyHQu7zhps2s8BTjGDacyxSgQ9D0iaqhvibntB8DHCHqETPZmdMHVajvib6ZPaq0Pc/640?wx_fmt=png&from=appmsg)

论文标题：

AutoHarness: improving LLM agents by automatically synthesizing a code harness

论文地址：

https://arxiv.org/abs/2603.03329

Google DeepMind 这项工作从一个很具体的问题切入：LLM Agent 在环境交互中经常产生非法动作。

传统做法需要工程师手写约束代码，这篇论文让 Gemini-2.5-Flash 自动生成代码形式的 Harness，用环境反馈不断修正动作是否合法。

方法上，系统通过树搜索和 Thompson sampling 搜索程序空间，再让模型根据环境反馈改进动作生成与合法动作约束。

实验放在 TextArena 的 145 个游戏上。结果显示，生成出的代码可以有效拦截非法动作，并在多个任务上帮助较小模型取得超过更大模型的表现。

论文还尝试让模型生成完整代码策略，在决策时直接用代码执行，减少逐步调用模型的需求。

换句话说，小模型的表现提升，很大一部分来自外部执行结构的约束和补偿。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeAydUoBo7VU23yVJcUR4jQmwWicXxFFhicSytFBJ2CibFYBWSOJHLOcMwqWlO4icbPawE780iajoojRqsk7AjCXAQdwRKBhKDqeScBE/640?wx_fmt=png&from=appmsg)

〓 代码形式 Harness 的学习流程

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeA0PnL7xkUia97oGSaicyj6OG7DfSvBzSdkCPj3EqcBx4IlLaYaWrJrxj4MnnibSB7aztcu4b5Jn6RmybMzLRLXSL1NQqibFw67oNg/640?wx_fmt=png&from=appmsg)

论文标题：

The Last Harness You'll Ever Build

论文地址：

https://arxiv.org/abs/2604.21003

这篇文章提出两层循环。内层循环面向具体任务，持续改进负责执行任务的 Agent 所使用的 Harness；外层循环继续改进演化蓝图，包括初始 Harness、负责执行任务的 Agent、评估器和负责提出修改的 Agent 配置。

这篇更接近自动 Harness 工程的路线图，核心是让系统学习如何为新任务生成可用执行结构，减少对人工经验的依赖。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeC2Pc5ic51Cl2sgfOFnoaM3zLdCppkRVZuGuvL3b6VHPicPvXl00xdAvib6n6O194gNhxK3gpQZFMqNPqbZxjc2jXm8DIc1ibvGOuo/640?wx_fmt=png&from=appmsg)

〓 Harness Evolution Loop 与 Meta-Evolution Loop

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeBS5ty2VVJibib0zsQrt7Wnb0NnL5aOAgNd8dpAOR2qyqr7WKhsSjbs3xQHQIYCbwU6fOXTdk5iaxQUibf23xERJeu2AupuGp7JFZ0/640?wx_fmt=png&from=appmsg)

论文标题：

Meta-Harness: End-to-End Optimization of Model Harnesses

论文地址：

https://arxiv.org/abs/2603.28052

代码地址：

https://github.com/stanford-iris-lab/meta-harness

这篇来自斯坦福、MIT 和 KRAFTON，它把 Harness 当成可搜索、可优化的代码对象：优化系统读取候选 Harness 的源码、历史分数和执行轨迹，再提出新的修改方案。

实验覆盖在线文本分类、RAG 数学推理和 Agentic Coding 任务。论文报告，在在线文本分类任务上，方法比强基线高 7.7 分，同时使用的上下文 token 减少到约四分之一。

在 200 道 IMO 级数学题上，自动发现的 Harness 能在五个未参与搜索的模型上带来平均 4.7 分提升；在 TerminalBench-2 上也超过手工设计基线。

优化范围已经扩展到上下文管理、检索和执行逻辑，这些都被纳入可修改的 Harness 代码。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeDuCL1SVwecFpSPEZm6s1ogO1oaTYFBqmLeMoicEvibUAZ5NE2EA5wiaaOz0hWibiaMxaibraIs9iaWF2GaXF9I9aYAHJwXZOfsxndRg4/640?wx_fmt=png&from=appmsg)

〓 Harness 代码的搜索流程

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCAL61SeDCAtAJb2xcwVtq75NrsEw3LvUAOr4VyTVbUdDS95f3eBvEtsPXFK4Beckysziagzo0mzXPsibx4YJicZHjlJ3RESFMia7Y/640?wx_fmt=png&from=appmsg)

论文标题：

Agentic Harness Engineering: Observability-Driven Automatic Evolution of Coding-Agent Harnesses

论文地址：

https://arxiv.org/abs/2604.25850

代码地址：

https://github.com/china-qijizhifeng/agentic-harness-engineering

这篇来自复旦、北大等机构，关注自动演化过程中的可控性。系统自己修改 Harness 时，改动来源、效果归因和失败回滚都会变得很重要。

论文提出以可观测性驱动 Harness 自动演化，并把可观测性拆成组件、经验和决策三个层面。

组件可观测性负责把可编辑部分明确落到文件层，方便修改和回滚；经验可观测性把大量轨迹整理成可消费的证据；决策可观测性要求每次修改都附带可验证预测。

10 轮演化后，Terminal-Bench 2 的一次尝试通过率从 69.7% 提升到 77.0%，高于 Codex-CLI 的 71.9%。

论文还做了迁移和消融分析：演化后的冻结 Harness 可以迁移到其他任务和模型，主要收益来自工具、中间件和长期记忆，而不是单纯修改 system prompt。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeAdWSCrJQwQwH4WTTyc8ick4ebLdYRqB9Zyjaib2ghbJ5Od2YceNLZ3amoak8LVqCx9dzPOgcqUjmZxAsic2ChUs38RyiaK9t51ALU/640?wx_fmt=png&from=appmsg)

〓 AHE 的三类可观测性闭环

![image](https://mmbiz.qpic.cn/mmbiz_png/Psho9dm7oDGhKg9nnSz5qQrwKvXibt3wukOjHSmSsEuRCB0fJu69CtdNgLnvFPDUCgeicOppBKuDvniaD3q8XWQ0Q/640?wx_fmt=png&tp=webp&wxfrom=5&wx_lazy=1#imgIndex=6)

从失败中改进

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCKNqt1aicpmUBW9iam9ibgh4icqicpqQJgCqDscGws4Jdnsicdacm6vo4ECEa4rib2afTe6Ww5OA2bgkjPXpu6zJCice7qfKlHenBRzB4/640?wx_fmt=png&from=appmsg)

论文标题：

Self-Harness: Harnesses That Improve Themselves

论文地址：

https://arxiv.org/abs/2606.09498

这篇来自上海 AI Lab，主题直接落在 Harness 自我改进上。它的出发点是，不同模型有不同失败模式，有效 Harness 也需要因模型而异。

系统从最小初始 Harness 出发，让 LLM Agent 依据自身轨迹改进运行 Harness，整个过程无需人工工程师或更强外部 Agent 介入。

方法分为三步：失败模式挖掘、Harness 修改提案、修改验证。系统先从执行轨迹中找到模型特定问题，再生成尽量小的 Harness 修改，最后通过回归测试决定是否接受。

Terminal-Bench-2.0 上，MiniMax M2.5、Qwen3.5-35B-A3B、GLM-5 的 held-out 通过率分别从 40.5% 提升到 61.9%、23.8% 提升到 38.1%、42.9% 提升到 57.1%。

被接受的修改会进入后续运行，使模型特定失败模式沉淀为可复用的系统改动。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeBu8hQQ8oSrsMGVu3zmJkMiaU828ic5tdYaibyWFjuZKhTe5guxmdRZNE1K8LHjHFdkgtO05ddhmTNxNmNK0moj8jQ9ubhnNgYxU0/640?wx_fmt=png&from=appmsg)

〓 Self-Harness 的三阶段自我改进闭环

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeAwgbChIY96iaVG6w7Pty9sbfsRxW3ULMDTPBBEJljfEb3ib37zUzgvXrMM1v55DiaibaEibuiaJozKU7pOpn7rQuZboFJIAdGU9VDLc/640?wx_fmt=png&from=appmsg)

论文标题：

Evolving Agents in the Dark: Retrospective Harness Optimization via Self-Preference

论文地址：

https://arxiv.org/abs/2606.05922

代码地址：

https://github.com/wbopan/retro-harness

这篇来自 MSRA 和港城大，关注无标准答案场景下的 Harness 优化。很多真实部署环境没有现成验证集，也没有稳定的外部打分器。

论文利用历史执行轨迹来做回顾式优化：先找出困难任务集合，再让 Agent 并行重做，通过自验证、一致性分析和成对偏好判断选择更好的 Harness 更新。

论文报告，单轮优化将 SWE-Bench Pro 通过率从 59% 提升到 78%。评估场景覆盖软件工程、技术任务和知识工作三类方向。

进一步的结果显示，优化后的 Harness 会改变 Agent 的行为模式，并在长程会话中维持更高准确率。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCc7sNjiaAHPEugiawM5icmZsEksQ90LnAModIfqib0byyEkShfz8MABZhm2d8b3eIaN48WVfco0wxM9s3DwkX2tx5ysfbqd759UcA/640?wx_fmt=png&from=appmsg)

〓 RHO 的回顾式 Harness 优化流程

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeCuQOXHrOTIswSyiakhTD9iajjK19EeGlZQmZ2E9U7CJszM4WwYdzzGciccFHIWXMr48uo1pe5ye1qKgQYicRbWxYribW5EmMKA0XCE/640?wx_fmt=png&from=appmsg)

论文标题：

From Failed Trajectories to Reliable LLM Agents: Diagnosing and Repairing Harness Flaws

论文地址：

https://arxiv.org/abs/2606.06324

这篇来自中科院软件所等机构，重点放在失败轨迹的诊断与定向修复上。很多 Agent 任务失败后，只能看到最终失败结果，很难判断问题出在工具、上下文、动作约束、验证逻辑还是其他 Harness 层。

论文提出一种 Harness 感知的轨迹中间表示 HTIR，把原始执行轨迹和 Harness 代码统一到可诊断表示里。

后续流程包括步骤级根因归因、Harness 层诊断、缺陷记录合并和范围受控的修复补丁生成。

论文在 SWE-Bench Verified、Terminal-Bench 2.0 Verified、GAIA、AppWorld 上评估，测试集表现相对初始 Harness 提升 15.2% 到 50.0%。

论文结果还显示，HarnessFix 超过人工设计和自演化基线，并总结出跨执行、工具、上下文等多个层面的常见 Harness 缺陷模式。相比只让模型回顾整段轨迹，HTIR 将失败定位到具体步骤和 Harness 层，再生成范围受控的修复补丁。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCuTNr1VwrufaJZKMHiclQibn7E7FXxIU5AhvMzCnsYb3ibMcvZY7YcXWbSiatmooEDvfCGRknhVD98Vgm58ggWrYadxLPwBn2eBXU/640?wx_fmt=png&from=appmsg)

〓 HarnessFix 的失败诊断与修复流程

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeCyt5N5CKA8BTJhszovXaWvFI9iaqyhrNdCedFDibtNbyuiafCM0IhDia0BeOZSRweZW4KAxTX0E074yBQmBfFFjeBuFQ6EpK3jBh0/640?wx_fmt=png&from=appmsg)

论文标题：

Harness Updating Is Not Harness Benefit: Disentangling Evolution Capabilities in Self-Evolving LLM Agents

论文地址：

https://arxiv.org/abs/2605.30621

代码地址：

https://github.com/A-EVO-Lab/a-evolve/tree/release/harness-evolution

这篇论文给自进化 Harness 提供了一个重要区分：模型能生成有用的持久化 Harness 更新，与任务求解 Agent 能否真正利用这些更新，需要分开评估。

论文发现，生成更新的能力对基础模型能力并不特别敏感，不同层级模型生成的更新带来的收益接近；真正有差异的是利用更新的能力。

弱模型可能无法正确激活相关 Harness 内容，或者激活后执行不稳定；中等能力模型往往获益更明显；强模型已经具备较强内在能力，边际收益反而可能变小。

论文还提醒，资源分配上不能只看谁来生成更新，也要看后续执行 Agent 是否有能力利用更新。因此，评估自进化 Harness 时，不能只看更新是否产生，还要看这些更新能否在后续任务中被稳定激活和执行。

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeDLVVWCbsIgIdXMSHvrj8fAJsEl1GEEY4d9Ovf3HRccsAdp2xoqJHBB9g4rQia1KZy13UJUkbhgYwX7J2GjspUIJscsVOtdcyZU/640?wx_fmt=png&from=appmsg)

〓 Harness-updating 与 Harness-benefit 的能力差异

![image](https://mmbiz.qpic.cn/mmbiz_png/Psho9dm7oDGhKg9nnSz5qQrwKvXibt3wuiaLfO9V4lkD8cXK7ImEicqib5bPGH6syOrWzicR2KaqPyAicMccs8icC03Gw/640?wx_fmt=png&tp=webp&wxfrom=5&wx_lazy=1#imgIndex=11)

记忆、技能和接口

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeBuhWk5yciarWDQfCMLRaWr8Zvzckw2xAuFpUvr4xNoIfk2wwBhGuK5caZJp94XXKFMrI3PrBZl7PvibPVuPUe53YRoQ8QgLbicLY/640?wx_fmt=png&from=appmsg)

论文标题：

M★: Every Task Deserves Its Own Memory Harness

论文地址：

https://arxiv.org/abs/2604.11811

代码地址：

https://github.com/wbopan/mstar

这篇来自港城大和微软，关注记忆 Harness。论文认为，不同任务需要不同记忆结构，固定记忆设计难以兼顾对话、具身规划和专家推理等场景。

方法上，论文把记忆系统写成 Python 记忆程序，包含数据结构、存储逻辑和工作指令，再通过反思式代码演化和种群搜索，为不同任务自动发现合适的记忆 Harness。

实验覆盖 LoCoMo、ALFWorld、HealthBench、PRBench 等任务。

记忆模块被表示成可搜索、可优化的程序化子系统，也成为 Harness 演化的一部分。M★ 在 8 个配置中的 7 个取得最优结果，任务专属记忆结构的收益比较稳定。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCLlu4QkA0QMy3CCoqWOK5NK19b4rrn3JVkiaEqT8vaIFyIMFUQRcNvuUlymX3YVCth6vToUyibD9ibn6tFSgQvSGu5R4t2blbWb4/640?wx_fmt=png&from=appmsg)

〓 不同任务演化出的记忆 Harness

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeDPbmFKOFOC9Ngc0zKnKv1icW1rxV7llr70evEfsYG4c3picGlJg4VUJXsgZum132y7hIic3C8CH5FcgargX1lKAmtKrsj2DnLS9w/640?wx_fmt=png&from=appmsg)

论文标题：

SkillOpt: Executive Strategy for Self-Evolving Agent Skills

论文地址：

https://arxiv.org/abs/2605.23904

代码地址：

https://github.com/microsoft/SkillOpt

这篇来自微软、上海交大、同济和复旦，讨论 skill 如何被系统化优化。很多 Agent skill 来自人工编写、一次性生成或松散自我修订，缺少稳定训练过程。

论文把 skill 视为冻结 Agent 的外部状态，让优化模型根据带分数的执行结果生成增加、删除、替换等文本编辑，并且只有验证集分数严格提升时才接受更新。

论文在六个 benchmark、七个目标模型、三类执行 Harness 上评估，52 个组合中均达到最佳或并列最佳。

在 GPT-5.5 上，相比不使用技能，直接对话、Codex 循环和 Claude Code 三种设置分别平均提升 23.5、24.8 和 19.1 分。优化后的 skill 文件还能跨模型规模、执行环境和相近数学 benchmark 迁移。

可复用 skill 文件因此有了更接近训练过程的更新方式：迭代编辑，再用验证集筛选保留。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeDSGZ9Xicpt6do5TLyzoBeibPSGuhn2EPo6bk1IZIESR9tKQNIbA9mfenKyJicGjzT6gTkw9FEtbqCdc52Vq7XdB061KLga7muzWg/640?wx_fmt=png&from=appmsg)

〓 SkillOpt 的 skill 优化流程

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeDpzo8Z6Gn3EeUYvHtcI7fRzMxqwwwV7kzDE4t0PibCxGiaYC6jnGtdGFj3anh2ico4gNaW4PS2zOrQJfYpx3fahBR9VnwV4TibdlA/640?wx_fmt=png&from=appmsg)

论文标题：

Adapting the Interface, Not the Model: Runtime Harness Adaptation for Deterministic LLM Agents

论文地址：

https://arxiv.org/abs/2605.22166

代码地址：

https://github.com/Tianshi-Xu/Life-Harness

这篇来自北大，强调运行时接口适配。很多确定性、规则驱动环境中的失败，并不一定来自模型权重不足，也可能来自模型和环境之间的接口不匹配。

论文不改模型权重，也不改评测环境，而是从训练轨迹中提取反复出现的交互失败，并转成可复用干预。这些干预覆盖环境约束、过程技能、动作实现和轨迹调节。

实验来自 τ-bench、τ²-bench、AgentBench 的七个确定性环境，覆盖 18 个模型骨干和 126 个模型—环境组合，其中 116 个组合得到改善，平均相对提升 88.5%。

论文还显示，用 Qwen3-4B-Instruct 轨迹演化出的 Harness 可以迁移到其他 17 个模型。

这组结果说明，部分失败可以通过运行时接口适配解决，即使模型权重保持冻结，也能获得明显提升。

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeD5uXjJaOF1JxWUoOvLZibIco4MdicLibO2fgN5olIPA1r28AByZm4SXsP0cdB4F2aU5qLthlY9CbPZFG8K1ib4ZvE1bS4vFDVsRM8/640?wx_fmt=png&from=appmsg)

〓 Life-Harness 通过运行时接口适配提升冻结 Agent

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeBRY2Ly1xu4ibMM2OOFCcnfzQ0XBBdIDicfuHLibT4UvVIjAq2ooSootRmPlOnbibic2ImiaXuBoIhsHNickknu9qXqUTAkgxb6hNUZX0/640?wx_fmt=png&from=appmsg)

论文标题：

MOSS: Self-Evolution through Source-Level Rewriting in Autonomous Agent Systems

论文地址：

https://arxiv.org/abs/2605.22794

代码地址：

https://github.com/dav-joy-thon/MOSS

这篇来自中科大、港科大和香港浸会大学，把自演化推进到源码级。

论文指出，很多现有自进化 Agent 主要改技能文件、prompt 配置、记忆结构和工作流图，但路由、钩子顺序、状态不变量、分发逻辑等结构性问题存在于代码里，文本层更新很难覆盖这些改动。

方法上，系统会根据生产失败证据调用外部代码智能体修改源码，再通过批量回放、试运行、用户同意、容器热替换和回滚机制进入生产系统。OpenClaw 上，单轮演化将四项任务平均评测器得分从 0.25 提升到 0.61。

源码级适配是文本层更新的扩展。它能确定性地改变执行逻辑，也更少受到长上下文漂移影响。可演化对象从 prompt、技能、记忆进一步走向 Agent 系统源码，更新范围也从文本配置扩展到真实执行逻辑。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeAZ3w4cN9W50cDBkWp1aGVE3icHTiaqDTFL5Jr9uEhVAyebusF9Yw9mXCXY2JU0iaJGERdu4f9ukkoLV6dCK2q1bY9oQUoYvJzqjY/640?wx_fmt=png&from=appmsg)

〓 MOSS 源码级自重写的四层演化流程

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCOJJPv9ZYa6dDLdnviclSWfKZLvKUYJETjlop5uxSHVvl5Whflbqj9443REibTAPb4YFI3uJxzWnY6hS6kM7QO8ZjG0HEbcufib8/640?wx_fmt=png&from=appmsg)

论文标题：

Scaling Laws for Agent Harnesses via Effective Feedback Compute

论文地址：

https://arxiv.org/abs/2605.29682

这篇来自哈工大车万翔教授课题组，讨论 Harness scaling。论文认为，token 数、工具调用次数、运行时间或成本只能描述原始开销，无法区分有效反馈和无效反馈。

它提出有效反馈计算量，只在反馈信息有效、合法、非冗余，并且被后续决策保留下来时计入。

实验覆盖合成任务、可执行代码任务、真实 benchmark 轨迹、held-out 划分和额外验证批次。

论文报告，在控制变量实验中，有效反馈计算量比原始 token 数和工具调用次数更能预测失败率。在预算和工具调用次数固定的情况下，提高反馈质量可以将成功率从 0.27 提升到 0.90。

对 Harness scaling 来说，关键不在尝试次数本身，而在反馈是否进入后续决策。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCxSPGUYOn3mWsMlM8NcLciaaHnibtw2MAf6ElCu89Oibrggfh1T3ru13Ovd8cJJg1y7uByIj2Ay8b64WE2GqFL730FjSrd4Eeshk/640?wx_fmt=png&from=appmsg)

〓 EFC 比原始计算开销更能解释 Harness scaling

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeBt2ic9lhf6TUibQlEJr2GVGnsoav6OlPOzQazVcOou1CPlvuwoKak3dU5xZoicetqNATcWXFyPVecQb8W7FE2y8jRkdf7BqdkwoA/640?wx_fmt=png&from=appmsg)

论文标题：

Your Agents Are Aging Too: Agent Lifespan Engineering for Deployed Systems

论文地址：

https://arxiv.org/abs/2605.26302

这篇来自 UT Austin，关注长期部署里的 Agent aging。

很多 Agent 评测只看初始化状态下的表现，但真实系统会不断写入记忆、压缩历史、检索旧信息、修订事实，也要面对日常维护。即便模型权重不变，系统状态也会持续变化。

论文提出 Agent Lifespan Engineering 和 AgingBench，将 aging 分成压缩、干扰、修订和维护四类，并用时间依赖图和反事实探针分析写入、检索、利用三个阶段。

实验覆盖 7 个场景、14 个模型和多种记忆策略，并比较受控运行与自主运行两类设置。

论文还将会话长度扩展到 8 到 200 轮，用来观察状态累积后的性能变化。一些系统在行为测试上仍然看起来正常，但事实精度已经开始衰减。这类评测把可靠性从单次任务表现，推进到长期状态积累和环境变化下的稳定性。

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeD4cf00En48z7S0ucg53SyAcJG8Siax5cAmSTKibHiaKBkb6sGnUk9Y3KK82HInNDRiazNFVT528H7kUiaicgN7a8oTTm8kAGq3txMcU/640?wx_fmt=png&from=appmsg)

〓 Agent aging 的四类机制

![image](https://mmbiz.qpic.cn/mmbiz_png/nFNzd9xjFeCxEbLnticekibt0Zj8iapmh1mudVvugvdtgZGQbqlVicRUMJDeVZSTpPDbYJp4JMoMV2qJq7hlF4xg0BbbcgnvhvyAicq0Aia964Pxc/640?wx_fmt=png&from=appmsg)

论文标题：

Auditing Agent Harness Safety

论文地址：

https://arxiv.org/abs/2605.14271

代码地址：

https://github.com/UCSB-AI/HarnessAudit

这篇来自 UCSB AI Group，关注 Harness 安全审计。

论文指出，只看最终输出，可能看不到执行过程中的风险。Agent 可能在轨迹中访问未授权资源，也可能把上下文传给错误的 Agent；最终答案看起来正确，执行过程仍然可能不安全。

论文提出 HarnessAudit，审计完整执行轨迹，重点看边界合规、执行忠实度和系统稳定性。它构建了包含 210 个任务、8 个真实领域的 HarnessAudit-Bench，覆盖单 Agent 和多 Agent 配置。

实验还评估了 10 种 Harness 配置、前沿模型和 3 个多 Agent 框架。结果发现，任务完成率和安全执行并不一致，违规会随轨迹长度积累，多 Agent 协作也会扩大安全风险面。

随着 Harness 扩展 Agent 的行动边界，权限、资源访问和信息流管理也需要进入评测范围。

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/nFNzd9xjFeC7Chcqy39ztfRLhPkwBqnsNPibZd10Qu9ZznLOUvPXU4Nk9TXS9BqMWW2QCcqfJl1qBOIoPArzk8Rz4TBC9X7R0fAuegKibJRYk/640?wx_fmt=png&from=appmsg)

〓 HarnessAudit 的整体审计流程

这批论文共同指向一个变化：Harness 正在从工程实现细节，变成 Agent 研究中可以被定义、优化和评估的对象。

模型能力仍然重要。只是接下来评价一个 Agent，可能不只看它调用了哪个模型，也要看它运行在什么样的 Harness 里。

更多阅读

![image](https://mmbiz.qpic.cn/mmbiz_png/RBRgSeVXhYNJiaadYbvXUE3tdicFycvUSUS4Byickr0GYrMXsyLibSBVGfT775NptibIeiaAbkogFmSc6MmLdNaUmQZxbxCDqmsn1ibJcDxFjIHkDA/640?wx_fmt=png&from=appmsg#imgIndex=14)

![image](https://mmbiz.qpic.cn/sz_mmbiz_png/RBRgSeVXhYMcV17X3rrLM5H7bCHrrsz87YTAYRXYecFGG8svbvmDNyEo5J7HbQrTO209XbanLEOice2u6roRFtbS2ickEVUZFHeWBmMNt7ZHc/640?wx_fmt=png&from=appmsg#imgIndex=15)

![image](https://mmbiz.qpic.cn/mmbiz_png/RBRgSeVXhYPYrbmt4P8s0Z1vVksKU3fIp0154e5BZwLPtloGlgZxicAQDbBFe7DSiaoe32icers0mjxmUjcA4PXibgWZbRViauOmwds58SoJpoaA/640?wx_fmt=png&from=appmsg#imgIndex=16)

![image](https://mmbiz.qpic.cn/mmbiz_gif/Psho9dm7oDHHMXQ2IicFvJwssWxgWhKuK7ulQVyw7gPTxZia00vCxia2vzhRH6pGq8t1FN1zY48ibULAEZpic41k6eg/640?wx_fmt=gif#imgIndex=17)

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

#投 稿 通 道#

让你的文字被更多人看到

如何才能让更多的优质内容以更短路径到达读者群体，缩短读者寻找优质内容的成本呢？答案就是：你不认识的人。

总有一些你不认识的人，知道你想知道的东西。PaperWeekly 或许可以成为一座桥梁，促使不同背景、不同方向的学者和学术灵感相互碰撞，迸发出更多的可能性。

PaperWeekly 鼓励高校实验室或个人，在我们的平台上分享各类优质内容，可以是最新论文解读，也可以是学术热点剖析、科研心得或竞赛经验讲解等。我们的目的只有一个，让知识真正流动起来。

📝 稿件基本要求：

• 文章确系个人原创作品，未曾在公开渠道发表，如为其他平台已发表或待发表的文章，请明确标注

• 稿件建议以 markdown 格式撰写，文中配图以附件形式发送，要求图片清晰，无版权问题

• PaperWeekly 尊重原作者署名权，并将为每篇被采纳的原创首发稿件，提供业内具有竞争力稿酬，具体依据文章阅读量和文章质量阶梯制结算

📬 投稿通道：

• 投稿邮箱：hr@paperweekly.site

• 来稿请备注即时联系方式（微信），以便我们在稿件选用的第一时间联系作者

• 您也可以直接添加小编微信（pwbot02）快速投稿，备注：姓名-投稿

![image](https://mmbiz.qpic.cn/mmbiz_png/VBcD02jFhgmic1CRCSOKfDibC3dZ4BaJuYyYTWJyw8gFxqon34STk3icf9aJbY4rqMpmhNjTGJXIGGFsCdTBHy3Tw/640?wx_fmt=png#imgIndex=18)

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

△长按添加PaperWeekly小编

🔍

现在，在「知乎」也能找到我们了

进入知乎首页搜索「PaperWeekly」

点击「关注」订阅我们的专栏吧

·

![image](https://mmbiz.qpic.cn/mmbiz_png/VBcD02jFhgnZ3nlEAOI3MyTd7jqeD6cq8uTbkM2xZNpribyNr9liaPJ722zaHxd0YpQvib2nxOYmWibydCVY7W94ew/640?wx_fmt=jpeg#imgIndex=41)

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()

![image]()
