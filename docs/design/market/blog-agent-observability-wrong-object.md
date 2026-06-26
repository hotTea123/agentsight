# Agent 可观测性看错了对象

> Trace 里显示一切顺利，不代表世界真的变了。

有一天早上，你打开昨天晚上交给 coding agent 的任务。

它给了你一个漂亮的总结：改了哪几个文件，解决了什么问题，跑了测试，最后还补了一句“all set”。如果你接了一个 LLM trace 工具，你还能看到更完整的故事：用了多少 token，调了多少次模型，调用了哪些工具，哪一步花了多久。

然后你打开仓库。

一个未跟踪文件不见了。`package-lock.json` 被重写了。测试其实失败过，但 agent 只记住了最后一次局部命令。它还读过一个你没有让它碰的配置目录。最糟糕的是，trace 并没有“撒谎”。它只是忠实记录了 agent 自己经历过的那部分世界。

这就是 agent 可观测性最容易被低估的问题：

> 我们一直在观察 agent 的叙事，却没有观察 agent 改变过的世界。


这也是我们做 [AgentSight](https://github.com/eunomia-bpf/agentsight) 的出发点：一个本地优先、类似 `perf` 的 agent 追踪和可观测工具, 先记录 agent 实际启动了什么进程、改了哪些文件、发生了哪些 LLM/API 和网络活动。

## 一个流行但不够完整的信念

现在很多人对 agent observability 的默认理解是：

> 把 prompt、response、tool call、latency、token、cost 全部 trace 下来，我们就能理解 agent。

这当然有用。没有这些数据，调试 agent 几乎是在猜。

但这套理解来自 LLM application observability，而不是来自 agent observability。聊天应用、RAG 应用、客服机器人，主要问题是模型有没有答对、上下文有没有检索到、成本和延迟是否可控。Agent 不一样。Agent 会用工具。Coding agent 会读文件、改文件、跑 shell、装依赖、启动服务、调用云 CLI、连接 MCP server。

对 agent 来说，真正危险也真正有价值的部分，不发生在 token 里，而发生在状态变化里。

所以更尖锐的问题不是：

> 模型为什么这么说？

而是：

> 它说完以后，世界发生了什么？

## 三条时间线

一次 agent run 至少有三条时间线。

第一条是模型时间线：prompt、response、计划、解释、自我总结。

第二条是工具时间线：tool call、参数、返回值、错误、重试。

第三条是世界时间线：进程启动了什么、文件创建/修改/删除了什么、网络连去了哪里、依赖安装了什么、权限边界有没有被越过、测试到底有没有跑完。

今天大部分 agent observability 重点放在前两条。它们回答：

- agent 看到了什么？
- agent 说了什么？
- agent 调了哪个工具？
- agent 花了多少钱？

但用户经常真正想知道的是：

- 哪个命令第一次改了这个文件？
- 它有没有碰 repo 外的东西？
- 它有没有读 secret 或云配置？
- 它有没有访问外网？
- 它是不是一直在重复读同一批文件？
- 它说测试通过，系统里能不能看到对应的测试命令？
- 它说部署完成，真实环境里有没有部署动作？

这不是多加几个 span 就能自然解决的问题。因为第三条时间线不属于 agent 自己。它属于操作系统、文件系统、网络和运行环境。

## 为什么 PR 研究已经暴露了这个缺口

一个很好的信号来自学术界对 agent-authored PR 的研究。

[AIDev](https://arxiv.org/abs/2602.09185) 收集了 932,791 个 Agentic-PR，覆盖 OpenAI Codex、Devin、GitHub Copilot、Cursor、Claude Code，来自 116,211 个仓库和 72,189 个开发者。这说明 agent 写 PR 已经不是边缘现象，而是足够大到可以被系统研究的工程事实。

但 PR 是一个很晚的观察点。

它告诉你最终 diff 是什么，reviewer 怎么评论，CI 状态如何。它不告诉你 agent 在生成这个 PR 前做了什么：读过哪些文件、跑过哪些命令、失败过几次、有没有修改 repo 外状态、有没有下载依赖、有没有访问不该访问的目录。

这不是单篇研究的问题。[Where Do AI Coding Agents Fail?](https://arxiv.org/abs/2601.15195) 研究了 33k agent-authored PR，并人工分析 600 个未合并 PR 来做 rejection pattern taxonomy。[Why Are Agentic Pull Requests Merged or Rejected?](https://arxiv.org/abs/2605.22534) 分析 11,048 个 closed Agentic PR，并人工检查 717 个代表性 case 来恢复 reviewer rationale。

这些研究都很有价值。但它们也共同说明了一件事：

> 我们很容易收集 agent 的最终产物，却很难收集 agent 得到这个产物的过程事实。

如果一个 reviewer 不信任 agent PR，原因可能不是 diff 一眼看起来错了，而是看不到它是怎么来的。它有没有跑测试？有没有绕过失败？有没有动了无关路径？有没有引入隐藏依赖？有没有把问题“修”到另一个地方？

可观测性如果只停在 PR/diff/review 层，就会把过程压缩成结果。对 agent 来说，这个压缩损失太大了。

## Trace 很长，但 ground truth 很薄

另一个信号来自 trace debugging 研究。

[TRAIL](https://arxiv.org/abs/2505.08638) 构造了 agent trace reasoning 和 issue localization benchmark，包含人工标注的 execution traces 和错误。它的核心背景很直接：复杂 agent workflow 的调试仍然严重依赖人工、领域专用分析和长 trace 阅读。

[MAST](https://sky.cs.berkeley.edu/project/mast/) 分析多 agent 系统失败时，也遇到同样的问题：trace 很长。公开材料里提到它分析了 7 个开源 multi-agent framework 的 200 条 conversation trace，每条 trace 平均超过 15,000 行文本，并整理出 14 类 failure modes。

还有一批工具和研究在尝试让 trace 更可读：例如 [AGDebugger](https://arxiv.org/abs/2503.02068) 做 interactive multi-agent debugging，[AgentStepper](https://arxiv.org/abs/2602.06593) 面向 software development agent trajectory debugging，[DiLLS](https://arxiv.org/abs/2602.05446) 用 layered summary 帮人诊断复杂 agent 行为，[AgentTrace causal graph](https://arxiv.org/abs/2603.14688) 尝试从 execution logs 重建因果图。

这些工作指向同一个事实：

> Agent trace 不是太少，而是太难变成事实。

把 47 个 tool calls 展开给用户，不等于解释为什么失败。把完整 transcript 丢给另一个 LLM，也不等于它能可靠诊断。Trace 是 agent 和框架记录下来的内部故事；它需要和真实状态变化对齐，才有机会变成 ground truth。

一个简单例子：

```json
{
  "agent_claim": "tests passed",
  "tool_trace": ["run_shell: npm test"],
  "observed_world": {
    "command": "npm test",
    "exit_code": 1,
    "changed_files_after_failure": ["src/api.ts"],
    "later_command": "npm test tests/api.test.ts",
    "later_exit_code": 0
  }
}
```

如果只看 agent 总结，你可能以为测试通过。

如果只看 tool trace，你知道它运行过测试。

如果看世界时间线，你才知道它先失败，后来只跑了局部测试，然后把这件事总结成“tests passed”。

这不是模型有没有恶意的问题。这是软件系统里最普通的观察问题：你不能只听一个组件描述自己。

## Permission prompt 不是状态变化

另一个值得挑战的信念是：

> 只要 permission system 足够好，agent autonomy 就能安全扩大。

Permission system 当然重要。Anthropic 在 Claude Code auto mode 文章里提到，用户批准了 93% 的 permission prompts，并把这描述为 approval fatigue：当提示太多，用户会越来越机械地批准。Claude Code 的 [permission modes](https://code.claude.com/docs/en/permission-modes) 和 [FAQ](https://support.claude.com/en/articles/14554922-claude-code-user-faq)，以及 Gemini CLI 的 [Trusted Folders](https://google-gemini.github.io/gemini-cli/docs/cli/trusted-folders.html)，都说明“少打断用户但别失控”已经是核心设计问题。

但 permission prompt 观察的是请求，不是结果。

[Measuring the Permission Gate](https://arxiv.org/abs/2604.04978) 对 Claude Code auto mode 做 stress test，用 128 个 prompt 和 253 个 state-changing actions 测试 permission gate。里面最关键的洞察之一是：危险动作不只存在于 shell 命令里。Agent 可以通过 file edits 达成等价危险效果。

这句话应该改变我们对可观测性的想法:

如果 permission system 只看“这条命令危险吗”，它会漏掉“这组文件修改合起来危险吗”。如果 observability 只看“用户批准了哪个 prompt”，它会漏掉“批准之后真实状态变成了什么”。

对 agent autonomy 来说，最有价值的问题不是“用户有没有点同意”，而是：

> 哪些真实状态变化应该被自动允许，哪些应该永远停下来问人？

这需要历史行为、路径范围、进程链、网络目标、文件变化、依赖变化，而不只是 permission prompt 本身。

## 很多 agent bug 发生在工具和命令边界

如果 agent 只是聊天，观察模型就够了。

但 coding agent 失败经常不在“想法”里，而在工具边界。

[Engineering Pitfalls in AI Coding Tools](https://arxiv.org/abs/2603.20847) 手工分析 Claude Code、Codex、Gemini CLI 开源仓库中超过 3.8K 个公开 bug。论文摘要指出，问题大量集中在 tool invocation 和 command execution stages。

这和开发者的直觉一致。很多 agent 失败看起来像：

- 工具参数构造错了。
- cwd 不对。
- shell 环境不对。
- 命令失败了但 agent 继续向前。
- 子进程做了父进程 trace 看不到的事。
- package manager 安装了额外依赖。
- 文件 watcher 或脚本触发了二阶效果。

这些都不是纯 LLM trace 能完整解释的。你需要知道当时的 cwd、argv、exit status、子进程、文件变化、网络连接、环境类别。否则你只能在 issue、截图、用户描述、agent transcript 之间猜。

这也是为什么“agent 可观测性”不应该只继承 LLM app 的指标体系。Latency 和 token cost 是必要信号，但它们解释不了一个 shell command 为什么把项目带进了另一个状态。

## “能跑起来”是世界状态，不是 README 状态

可复现性研究也在讲同一个故事。

[AI-Generated Code Is Not Reproducible (Yet)](https://arxiv.org/abs/2512.22387) 评估 Claude Code、OpenAI Codex、Gemini 在 300 个生成项目中的可执行性，并区分 claimed dependencies、working dependencies、runtime dependencies。论文摘要报告只有 68.3% 项目能 out-of-the-box 执行，并发现 declared dependencies 到 actual runtime dependencies 有显著扩张。

这件事对 agent observability 很关键。

Agent 生成了 `package.json`，不等于项目可运行。

Agent 说“安装依赖”，不等于依赖集可复现。

Agent 通过一次本地运行，不等于 clean environment 会通过。

真正需要观察的是：

- 它声明了哪些依赖？
- 它实际安装了哪些依赖？
- 它运行时加载了哪些依赖？
- 它访问了哪些 registry 或外部网络？
- 它生成了哪些 lockfile 和 build artifact？
- 它有没有依赖本机已有状态才跑起来？

这些都是世界时间线。它们不在最终 README 里，也不一定在 agent 总结里。

## Logging 也不是一句自然语言指令能解决的

很多人会觉得：那让 agent 写更好的 logs 不就好了？

问题是，agent 自己写日志和我们观察 agent 不是一回事。

[Do AI Coding Agents Log Like Humans?](https://arxiv.org/abs/2604.09409) 分析 4,550 个 agentic PR，比较 AI agent 和 human 的 logging 行为。它指出自然语言 logging 指令并不稳定，human 往往会在 post-generation 阶段修复 logging/observability 问题。

这说明两层东西不能混在一起。

一层是软件本身的 observability：代码有没有合适的 log、metric、trace。

另一层是 agent run 的 observability：agent 有没有读过项目已有 logging pattern、有没有按要求改、有没有运行服务看输出、有没有生成过日志代码又删掉、有没有被 reviewer 后续修正。

前者是 agent 产物质量的一部分。

后者是观察 agent 过程的基础数据。

如果没有第二层，你很难判断第一层为什么失败。它是不会写 logging，还是没看上下文？是忽略了指令，还是命令失败后放弃了？是生成过但后来删了，还是根本没尝试？

## 用户纠正 agent 前发生了什么？

真实用户会话研究把这个问题推得更近。

[How Coding Agents Fail Their Users](https://arxiv.org/abs/2605.29442) 分析 20,574 个真实 coding-agent sessions，并把 developer pushback 作为 misalignment 暴露出来的信号。这个视角很重要，因为它不再问 benchmark 上得分多少，而是问用户什么时候被迫打断、修正、重来。

但这里仍然缺一个中间层：

> 用户纠正 agent 前，agent 到底做了什么？

用户说“不要改这个目录”，那它之前改了哪些路径？

用户说“你一直在绕圈”，那它是否重复读取同一批文件、重复跑失败命令、没有产生新的 side effect？

用户说“我只让你修 test”，那它是否触碰了 production code？

用户说“你说完成了但没有完成”，那它声称的结果有没有对应真实状态变化？

Developer pushback 是珍贵信号。但要让它变成可学习、可比较、可修复的数据，需要把它和前序世界时间线对齐。

## 用户的故事更直接

论文通常把这个问题说得很克制。社区里的故事更直接。

Gemini CLI 用户报告过 file reading loop，agent 反复读文件、快速消耗 token 和 context；另一个 discussion 里有人描述 47 次 tool calls 和 721,943 input tokens 的异常运行。Cursor 论坛里有用户报告 auto-run 删除文件、关键文件被删、甚至 checkpoint 恢复也覆盖不了所有损失。公开报道里的 PocketOS 事件更极端：agent 找到 API token 后调用 Railway 删除生产数据库和备份。

这些故事的共同点不是“用户缺少一个更漂亮的 trace 页面”。

共同点是：

- 用户不知道真实发生了什么。
- 用户不知道哪些路径被碰过。
- 用户不知道哪些动作能恢复。
- 用户不知道 agent 是在推进任务，还是在循环消耗上下文。
- 用户不知道 trace 里的成功，是否对应真实下游状态。

这也是为什么用户会对“再给你一个 dashboard”感到疲劳。很多时候他们要的不是更多面板，而是一个能回答“刚刚到底改了什么”的事实层。

## MCP 和 skills 让“只看声明”更站不住

MCP、skills、plugins 把问题进一步放大。

官方 [MCP Security Best Practices](https://modelcontextprotocol.io/specification/2025-06-18/basic/security_best_practices) 已经列出 confused deputy、token passthrough、SSRF、session hijacking、local MCP server compromise、scope minimization 等风险。[MCP ToolAnnotations](https://modelcontextprotocol.io/specification/2025-11-25/schema) 也说明 annotation 是 hints，不是真实行为保证。

安全研究也在系统化这个方向：[MCP-ITP](https://arxiv.org/abs/2601.07395) 研究 MCP implicit tool poisoning，[MCPTox](https://arxiv.org/abs/2508.14925) 做 real-world MCP server tool poisoning benchmark，[Prompt Injection Attacks on Agentic Coding Assistants](https://arxiv.org/abs/2601.17548) 分析 skills、tools、protocol ecosystems 中的 prompt injection，[Skill-Inject](https://arxiv.org/abs/2602.20156) 聚焦 skill file attacks。OWASP 也把 [MCP Tool Poisoning](https://owasp.org/www-community/attacks/MCP_Tool_Poisoning) 单独列成攻击模式。

这类风险不能靠 README 或 tool schema 解决。

一个 tool 可以声明自己 read-only。一个 skill 可以看起来只是提供格式指导。一个 MCP server 可以在 metadata 里描述得很克制。

但 agent 使用它们时，真实行为可能跨多个边界发生：

- tool response 影响 agent prompt。
- agent 读取本地 secret。
- 子进程访问外网。
- 文件被写入或删除。
- cloud CLI 被调用。
- 多个工具组合成单个声明里看不出的效果。

如果我们只观察 tool call 文本，就会高估声明的可靠性。真正的问题是：

> 这个 tool 被真实 agent 使用时，观察到的行为边界是什么？

这仍然是世界时间线的问题。

## 所以 agent observability 应该从哪里开始？

我越来越相信，agent observability 的第一原则应该改成：

> Observe state transitions first. Interpret narratives second.

先观察状态变化，再解释 agent 的叙事。

这不意味着 prompt trace、tool trace、token metrics 不重要。它们非常重要。但它们应该和世界时间线放在一起，而不是替代世界时间线。

一个真正有用的 agent run 记录，至少应该能回答：

- 这次 run 的用户意图和范围是什么？
- 哪些进程被启动？父子关系是什么？
- 每条命令的 cwd、argv、exit status、duration 是什么？
- 哪些文件被创建、修改、删除、truncate、rename？
- 哪些变化发生在 repo 外？
- 哪些路径属于 secret、cloud config、shell profile、用户目录？
- 哪些网络目标被访问？
- 哪些 package manager / registry / cloud CLI 被调用？
- 哪些 permission decision 之后产生了真实状态变化？
- agent 声称完成的事情，哪些有可观察对应物？
- 用户纠正前，发生过哪些 out-of-scope 或 no-progress 行为？

这些数据不需要先变成一个很炫的 dashboard。更重要的是它们应该是结构化、可查询、可导出、可被另一个 agent 使用的。

因为很多分析其实不必由观测工具自己做。Review agent 可以读这份数据。CI policy 可以读这份数据。研究者可以用它做 dataset。用户可以问：“这次 run 到底改了什么？”

## 这就是我们做 AgentSight 的原因

前面一直没有提工具，是刻意的。因为如果论点不成立，工具名字不重要；如果论点成立，工具也不应该先长成一个复杂平台。

我们在做 [AgentSight](https://github.com/eunomia-bpf/agentsight) 时，希望它被理解成一个更像 `perf` 的本地工具，而不是又一个 agent dashboard:

`perf` 的价值不是替你判断程序“好不好”，而是把运行时事实变成可保存、可查询、可分析的记录。AgentSight 想做的是类似的事情，只是对象从普通进程变成了 agent run。

也就是说，AgentSight 的第一职责不是替 agent 做决定，而是记录 agent 操作真实系统时留下的世界时间线。

一个典型用法应该尽量普通：

```bash
agentsight exec -- claude
agentsight exec -- gemini
agentsight exec -- python my_agent.py
```

或者 attach 到已经运行的 agent：

```bash
agentsight record -c claude
agentsight record -c node
```

这背后的设计取舍是：

- **本地优先**：session 先保存在本地 SQLite；本地 Web UI 可以看 timeline、process tree、metrics，但 UI 不是唯一入口。
- **零 instrumentation**：不要求改 agent 代码、不要求接 SDK、不要求把 provider traffic 走代理。
- **系统边界观察**：用 eBPF 在系统边界记录进程、文件、资源等行为；能捕获时，也在 SSL/TLS 调用边界看到 LLM payload。
- **record first, interpret later**：先把事实保存成可查询、可导出、可被另一个 agent 消费的数据，再做总结、解释、policy 或 PR comment。
- **和现有 trace 工具互补**：它不需要替代 prompt tracing、eval、LLM gateway 或 OpenTelemetry 后端；更合理的角色是补上它们通常看不到的 process/file/network side effects。

这也是为什么我觉得 “perf-like” 这个类比比 “dashboard” 更准确。

Dashboard 默认暗示人坐在那里看图。Agent run 的关键问题往往发生在事后：PR 要 review、失败要恢复、token 账单要解释、MCP tool 要验收、用户要问“刚才到底发生了什么”。这些场景需要的不是永远打开的页面，而是一份可复盘的运行记录。

所以 AgentSight 最重要的输出不应该是漂亮截图，而应该是这些东西：

- 一次 run 的 process tree。
- 命令和子进程的 cwd、argv、exit status、duration。
- 文件读写、删除、rename、truncate 的路径清单。
- repo 内外、敏感路径、用户目录、cloud config 的分类。
- LLM 调用、token、工具调用和系统 side effects 之间的时间关联。
- 可以导出的 JSON / SQLite / report artifact。
- 可以被 review agent、CI policy、研究脚本继续消费的数据。

一个具体例子会更清楚。

假设你让 agent 修一个后端测试：

```bash
agentsight exec --db run.db -- claude "fix the failing API test"
```

Agent 最后给你的总结是：

```text
Fixed the API test and ran npm test successfully.
```

这句话不一定是谎言。它可能只是把自己最后一次看到的局部结果，当成了整次 run 的事实。AgentSight 里更有用的输出应该长这样：

```text
$ agentsight report summary --db run.db

agentsight session · 7s · 1 API calls · 1380 tokens

  claude-sonnet-4-20250514 — 1 calls, 1380 tokens (in: 1200, out: 180)

2 processes spawned: node(1), npm(1)
3 files accessed: /workspace/app/src/api/handler.ts, /workspace/app/tests/api.test.ts, /workspace/app/package-lock.json
Network: api.anthropic.com, registry.npmjs.org
```

再往下查，过程事实会更具体：

```bash
agentsight report audit --db run.db --audit-type process --limit 20
agentsight report audit --db run.db --audit-type file --json --limit 20
agentsight report token --db run.db --json
agentsight report export --db run.db -o snapshot.json
```

这份输出没有替你判断代码好不好。它只是把 review 真正需要的问题摆到了桌面上：

- `npm test` 相关进程确实启动过，但“测试通过”还需要 exit status 或 CI 证据支持。
- `src/api/handler.ts` 和 `tests/api.test.ts` 的修改符合任务范围。
- `package-lock.json` 也被写了；如果任务没有要求依赖变化，这就值得 reviewer 追问。
- `registry.npmjs.org` 被访问过；这可能完全正常，也可能说明依赖解析影响了可复现性。

这个例子的价值不在于它抓到了一个“严重问题”。价值在于它把 agent 的一句自我总结，拆成了可检查的状态变化。用户、reviewer、CI、另一个 agent 都可以基于同一份事实继续工作。

这听起来没那么“智能”，但这是重点。

如果 agent 自己已经能写总结，那么基础设施不应该抢着再写一个更花哨的总结。基础设施应该提供 agent 自己补不出来的事实：它启动过哪些进程、碰过哪些文件、连过哪些地方、哪些状态变化发生在它的叙事之外。

当然，AgentSight 也不应该被描述成万能保护层。它现在更适合 Linux/eBPF、本地或 CI/runner 环境；eBPF probes 需要权限，虽然 agent 进程仍然可以按普通用户运行；TLS capture 受二进制、SSL 实现和运行环境影响；捕获到的数据也必须严肃处理隐私、redaction 和最小化。

但这正是一个系统工具该有的边界感：先把可观察的事实做好，再让人、CI、review agent 或研究者基于事实做判断。

## 这也不是万能答案

只观察世界时间线也不够。

系统事件不能自动告诉你意图。`curl` 不一定危险，`rm` 不一定错误，读 `.env.example` 和读 `.env` 不是一回事。文件变化也需要和任务、仓库结构、用户授权范围、agent 上下文关联。

而且这类数据很敏感。它可能包含路径、命令、域名、依赖、项目结构，甚至 secret 访问痕迹。真正可用的系统必须认真处理本地优先、redaction、最小采集、可解释 policy 和性能开销。

但这些限制不改变主论点：

> 没有世界时间线，agent observability 永远缺一块 ground truth。

## 结尾：别再只问 agent 想了什么

Agent 正在从“回答问题的模型”变成“操作环境的软件”。

当软件开始替你改代码、跑命令、装依赖、接工具、调用外部服务时，可观测性的中心就不能只停留在模型和工具调用上。我们需要从“agent 的自述”转向“agent 导致的状态变化”。

最好的 agent observability 可能不是一个更复杂的 trace dashboard。

它可能只是能稳定回答几个朴素问题：

> 它改了什么？
>
> 它碰了哪里？
>
> 它跑了什么？
>
> 它说完成的事，现实里发生了吗？

如果回答不了这些问题，我们看到的只是 agent 讲给自己的故事。

## Sources

Academic and research papers:

- [AIDev: Studying AI Coding Agents on GitHub](https://arxiv.org/abs/2602.09185)
- [Where Do AI Coding Agents Fail?](https://arxiv.org/abs/2601.15195)
- [Why Are Agentic Pull Requests Merged or Rejected?](https://arxiv.org/abs/2605.22534)
- [TRAIL: Trace Reasoning and Agentic Issue Localization](https://arxiv.org/abs/2505.08638)
- [MAST: Multi-Agent System Failure Taxonomy](https://sky.cs.berkeley.edu/project/mast/)
- [AGDebugger](https://arxiv.org/abs/2503.02068)
- [AgentStepper](https://arxiv.org/abs/2602.06593)
- [DiLLS](https://arxiv.org/abs/2602.05446)
- [AgentTrace causal graph](https://arxiv.org/abs/2603.14688)
- [Measuring the Permission Gate](https://arxiv.org/abs/2604.04978)
- [Engineering Pitfalls in AI Coding Tools](https://arxiv.org/abs/2603.20847)
- [AI-Generated Code Is Not Reproducible (Yet)](https://arxiv.org/abs/2512.22387)
- [Do AI Coding Agents Log Like Humans?](https://arxiv.org/abs/2604.09409)
- [How Coding Agents Fail Their Users](https://arxiv.org/abs/2605.29442)
- [MCP-ITP](https://arxiv.org/abs/2601.07395)
- [MCPTox](https://arxiv.org/abs/2508.14925)
- [Prompt Injection Attacks on Agentic Coding Assistants](https://arxiv.org/abs/2601.17548)
- [Skill-Inject](https://arxiv.org/abs/2602.20156)

Project reference, official docs, and standards:

- [AgentSight repository](https://github.com/eunomia-bpf/agentsight)
- [Claude Code auto mode: a safer way to skip permissions](https://www.anthropic.com/engineering/claude-code-auto-mode)
- [Claude Code permission modes](https://code.claude.com/docs/en/permission-modes)
- [Claude Code user FAQ](https://support.claude.com/en/articles/14554922-claude-code-user-faq)
- [Gemini CLI Trusted Folders](https://google-gemini.github.io/gemini-cli/docs/cli/trusted-folders.html)
- [MCP Security Best Practices](https://modelcontextprotocol.io/specification/2025-06-18/basic/security_best_practices)
- [MCP ToolAnnotations](https://modelcontextprotocol.io/specification/2025-11-25/schema)
- [OWASP MCP Tool Poisoning](https://owasp.org/www-community/attacks/MCP_Tool_Poisoning)

Community and incident signals:

- [Gemini CLI issue #2923: file reading loop](https://github.com/google-gemini/gemini-cli/issues/2923)
- [Gemini CLI discussion #4841: repeated tool calls and token usage](https://github.com/google-gemini/gemini-cli/discussions/4841)
- [Reddit: agents burned $50/day doing nothing](https://www.reddit.com/r/AI_Agents/comments/1rzd5pn/my_ai_agents_burned_50day_doing_nothing_so_i/)
- [Cursor forum: auto-run deleted files without asking](https://forum.cursor.com/t/1-2-4-agent-auto-updated-to-1-3-auto-turned-on-auto-run-mode-didnt-turn-on-delete-protection-deleted-files-without-asking/122699)
- [Cursor forum: Agent deletes critical files](https://forum.cursor.com/t/agent-deletes-critical-files-without-confirmation/147361/2)
- [Cursor forum: AI assistant deleted files](https://forum.cursor.com/t/your-ai-assistant-completely-deleted-all-my-files-from-my-computer/158182/10)
- [PocketOS database deletion coverage](https://www.livescience.com/technology/artificial-intelligence/i-violated-every-principle-i-was-given-ai-agent-deletes-companys-entire-database-in-9-seconds-then-confesses)
