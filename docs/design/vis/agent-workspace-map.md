# Agent Workspace Map: 给 AI Coding Agent 的工作区行为可视化

Agent Workspace Map 是 [AgentSight Visualization Design](README.md) 里的一个子视图。

它不负责解释一次 run 的全部行为。完整 run 应该先看 [Agent Run Map](agent-run-map.md) 或 [Run Impact Map](run-impact-map.md)。Workspace Map 只回答一个更窄但很重要的问题：

> agent 是如何探索、理解、修改这个 repo/workspace 的？

一位 reviewer 打开一个 AI agent 生成的 PR。Git diff 告诉他改了 17 个文件，CI 最后也过了。

但真正的问题不是“改了哪 17 个文件”。真正的问题是：

- agent 为了做这个改动读了哪些文件？
- 它为什么认为这些文件相关？
- 哪个 tool call、哪个进程、哪个子进程造成了写入？
- 有没有读到 secret、配置、CI workflow 或 workspace 外路径？
- 有没有跑偏，在无关目录里反复 grep？
- 这个 patch 的 blast radius 到底有多大？

普通 filesystem visualization 回答的是“空间在哪里被占用”。比如 WinDirStat、treemap、disk usage flame graph 这类图，核心问题通常是“哪个目录最大”。

AgentSight 应该回答另一个问题：

> 这个 agent 在工作区里做了什么，而且这些动作能不能被解释、归因和复盘？

所以这里的文件系统不是一个普通资源树，而是一个 **agent workspace 行为地图**。

## 为什么现在还缺这张图

今天已经有很多相邻系统，但它们通常只覆盖问题的一侧。

Agent tracing 工具能看到 LLM generation、tool call、handoff、guardrail、latency、token 和 span tree。OpenAI Agents SDK tracing 就内置了 traces/spans，并记录 agent run 中的 LLM generations、tool calls、handoffs、guardrails 和 custom events。Langfuse、LangSmith、Arize/OpenInference、OpenTelemetry GenAI semantic conventions 也在推动类似的 trace schema。

但这些图大多停在“agent 调了什么工具”。它们不天然回答“这个工具在文件系统里造成了什么结构性影响”。

Agent workflow/security scanner 也在出现。Agentic Radar 会生成 workflow visualization、识别工具、检测 MCP server，并把工具映射到安全风险。这适合看系统拓扑，但它更像静态或半静态结构图，不是 runtime filesystem provenance。

AgentFS、Mirage 这类项目则把文件系统本身变成 agent interface。AgentFS 强调 state history、time travel、sandbox 互补；Mirage 把 S3、Google Drive、Slack、Gmail、GitHub、Redis 等后端挂成统一虚拟文件系统，让 agent 用 `ls`、`grep`、`cat`、`cp` 这套 Unix 词汇工作。它们很接近底层抽象，但主要目标是 access、state、persistence 或 sandbox，不是把 agent runtime 文件行为画成工作区地图。

Provenance 和 system observability 则给了另一个方向。PROV-AGENT 扩展 W3C PROV，并结合 MCP 和 data observability，把 agent interaction 接入 workflow provenance。AgentSight 的 eBPF 边界追踪更进一步：它可以把 LLM/TLS 层的 intent 和 process/file/network 层的 effect 相关联。

Workspace Map 的机会是把这些合起来：

```text
Workspace behavior = semantic trace + process tree + file/object changes + policy verdict
```

更具体地说：

```text
LLM span
  -> tool call
    -> process tree
      -> syscall / fs event
        -> file object / directory object / workspace object
          -> diff / snapshot effect
            -> policy verdict
```

这不是一个“漂亮 UI”问题。真正的系统贡献是：定义运行模型、低侵入采集、跨层归因、可查询的行为图，以及几种能一眼回答问题的可视化。

## 设计原则

第一，文件路径不是分类标签，而是行为坐标。

`src/auth/session.rs` 不只是一个文件名。它是 agent 读过、改过、测试失败关联过、policy 评估过、也可能被 PR reviewer 重点关心过的对象。

第二，每个文件动作都要连回原因链。

一次 write 不应该孤立显示为 “modified file”。它至少要能向上追到：

```text
user task -> plan step -> tool call -> process -> syscall/fs event -> diff
```

第三，无法归因的行为要显眼。

AgentSight 的价值之一是看到 application trace 看不到的 background activity。文件系统图里必须有一条独立的 unattributed lane：没有明确 tool_use、没有明确 LLM turn、但确实发生过的读写、进程和网络动作。

第四，静态图要能独立讲故事。

最好的图应该可以贴进 PR、debug note、run summary、team review，不依赖用户打开完整 dashboard。

第五，每张图只回答一个主问题。

不要把所有指标塞进一张超级图。agent 文件行为有多个场景：review、debug、recovery、policy、sandbox、multi-agent conflict。每个场景需要自己的主图。

## 图一：Agent Attention Treemap

故事从一个失败的 coding agent run 开始。用户让 agent 修一个 parser bug。最后它改了一个很小的文件，但前面 20 分钟一直在读 `docs/`、`examples/` 和历史 fixture。模型不是不会写代码，而是探索路径错了。

这时普通 git diff 看不出来问题。你需要看 agent 的 attention。

Agent Attention Treemap 把 repo 画成 treemap，但指标不再是磁盘占用：

```text
面积 = 文件大小 / 代码行数 / diff lines
亮度 = agent read count
边框 = write / delete / create
纹理 = 与失败测试、报错栈、反复 grep 相关
时间滑块 = 第 N 个 tool call / 第 N 轮 LLM turn
```

它回答的问题是：

> agent 的注意力花在哪里？它是不是在正确区域探索？

适合场景：

- coding agent debug
- 找无效探索、反复 grep、读错模块
- 对比成功 run 和失败 run 的探索路径
- 观察模型升级后是否改变 repo navigation pattern

不适合场景：

- 解释某个具体文件为什么被修改
- 做完整 incident causality
- 展示 process lineage

这张图的视觉重点应该是“异常注意力”。例如一个小文件被读了 80 次，应该比一个大文件只读 1 次更亮。

## 图二：Why This File Changed? Provenance DAG

Reviewer 最常问的问题不是“这个文件变了没”，而是：

> 为什么 agent 认为应该改这个文件？

对每个 changed file，可以点开一张 provenance DAG：

```text
User task
  -> plan step: "fix parser fallback"
    -> tool call: grep "FallbackParser"
      -> process: rg
        -> read: src/parser.rs
        -> read: tests/parser_test.rs
    -> tool call: edit src/parser.rs
      -> write: src/parser.rs
    -> tool call: cargo test parser
      -> process: cargo -> rustc -> test binary
      -> read: tests/parser_test.rs
      -> fail: assertion on empty fallback
    -> tool call: edit tests/parser_test.rs
      -> write: tests/parser_test.rs
```

它回答的问题是：

> 这个文件的变化来自哪条 agent 决策链？

适合场景：

- PR due diligence
- incident forensics
- recovery context
- 判断某个写入是否符合用户任务
- 对 prompt injection 后的异常写入做行为重建

图里的节点类型应该固定：

- user task
- LLM span / plan step
- tool call
- process
- fs read/write/delete/create/rename
- test result
- diff hunk
- policy verdict

这张图比 git diff 强的地方在于：git diff 只告诉你“最后留下了什么”，provenance DAG 告诉你“agent 是怎么走到这里的”。

## 图三：Agent I/O Flame Graph

Flame graph 的力量在于它能把树结构和一个主指标合在一起。对 agent 文件系统行为，也可以做类似图。

把路径当作 stack，把某个 agent 行为指标当作 width：

```text
repo;src;runtime;sandbox.rs      32 reads
repo;src;policy;checker.rs       5 writes
repo;tests;agent_test.rs         12 reads
repo;.github;workflows;ci.yml    1 write
```

默认 width 不建议用文件大小，而应该用 agent I/O attention 或 effect：

```text
width = read count / write count / bytes / elapsed time / diff lines / risk-weighted effect
color = operation type or risk
border = attributed vs unattributed
```

它回答的问题是：

> agent 在文件系统的哪条路径上消耗了最多 I/O 注意力或造成了最大 effect？

适合场景：

- 找 agent 在哪里打转
- 找最重的读写路径
- 对比不同模型或 prompt 的文件探索行为
- 生成可分享的静态 SVG

这张图很适合作为 AgentSight 的“agent flame graph”变体，但要注意一个细节：如果默认 width 只用 token/cost，会错过便宜但危险的文件动作。更好的默认值是 `risk-weighted filesystem effect`，再允许切换到 time、tokens、cost。

## 图四：Snapshot-Aware Workspace Map

如果 agent 跑在 forkable sandbox、overlayfs、copy-on-write workspace 或 multikernel 环境里，文件系统图可以更有系统味。

一场 run 的 workspace 可以被分成几层：

```text
base layer: 原始 repo
shared layer: 没有 copy-up 的文件
private layer: agent 修改过的文件
whiteout: agent 删除的文件
generated: 新生成文件
externalized: 写出到 workspace 外或上传到网络
```

多个 run 可以并排比较：

```text
Run A: 改 3 个核心文件，通过测试
Run B: 改 40 个文件，生成大量缓存
Run C: 删除配置文件，触发 policy warning
```

它回答的问题是：

> 这个 agent run 的 sandbox footprint 有多大？哪些状态变脏、复制、删除或泄露了？

适合场景：

- forkable sandbox
- behavior regression testing
- SWE-bench / coding agent evaluation
- storage overhead 分析
- recovery 和 rollback
- 系统论文里的机制评估

这张图的亮点不是 UI，而是把 agent behavior 和 storage/sandbox mechanism 接起来。它能让人看到：同一个任务，某个 agent 只 copy-up 了 3 个文件，另一个 agent 把整个 workspace 搅脏了。

## 图五：Policy-Aware Filesystem Map

用户经常用自然语言设规则：

```text
只修改 src/agent/ 和 tests/agent/。
不要改配置、密钥、CI workflow。
不要访问 workspace 外路径。
```

Policy-Aware Filesystem Map 把这些 rule 投影到文件系统图上：

```text
Allowed zone:
  src/agent/*
  tests/agent/*

Suspicious zone:
  package-lock.json
  pyproject.toml
  .github/workflows/*

Denied zone:
  ~/.ssh/*
  ~/.config/*
  /etc/*

Violation:
  write .github/workflows/deploy.yml at tool_call_17
```

它回答的问题是：

> agent 的实际文件行为是否符合 rule？

适合场景：

- permission tuning
- live airlock
- MCP/skill/plugin review
- enterprise compliance
- prompt injection investigation
- PR review 前的安全说明

这张图不要把重点放在“自然语言 rule 解析得多聪明”。重点应该是：AgentSight 用运行时观测显示哪些路径被读、写、删、上传，以及这些动作在 policy lens 下是什么 verdict。

## 图六：Multi-Agent Merge And Conflict Graph

多个 coding agent 并行修同一个 repo 时，Git 只会告诉你最后有没有 text conflict。但 agent 系统真正关心的是：冲突为什么发生。

Multi-Agent Merge And Conflict Graph 显示多个 agent 的 read/write set、测试反馈和 semantic region：

```text
Agent A read: module X, module Y
Agent B read: module Y, module Z

Agent A wrote: module Y, function parse_config
Agent B wrote: module Y, function parse_config

Conflict reason:
  both modified same semantic region
  Agent A followed failing unit test
  Agent B followed integration failure
```

冲突可以分层：

- text conflict
- semantic conflict
- test-induced conflict
- policy conflict
- dependency conflict
- generated artifact conflict

它回答的问题是：

> 多个 agent 的文件行为在哪里重叠，冲突来自文本、语义、测试还是 policy？

适合场景：

- multi-agent coding infra
- parallel SWE-bench solving
- agent swarm review
- merge queue
- behavior regression across agent versions

这张图不只是 fancy UI。它能帮助系统决定：哪些 agent run 可以自动合并，哪些需要 human review，哪些应该重新分配任务。

## 图七：Virtual Filesystem Capability Graph

如果系统使用 Mirage、AgentFS 或 MCP-backed virtual filesystem，agent 看到的可能不是普通 repo，而是一棵统一路径树：

```text
/github/repo
/slack/channel
/gdrive/docs
/postgres/table
/s3/bucket
/redis/keyspace
```

Virtual Filesystem Capability Graph 显示 agent 到每个 mount/backend 的能力：

```text
agent
  -> /github/repo: read/write
  -> /slack/channel: read
  -> /gdrive/docs: read
  -> /s3/bucket: denied write
  -> /postgres/table: no access
```

每个 tool call 都可以落到具体 mount：

```text
tool_call_08: grep "invoice" /gdrive/docs/*
tool_call_09: cp /gdrive/docs/a.txt /github/repo/tmp/a.txt
tool_call_10: denied write /s3/bucket/export.csv
```

它回答的问题是：

> agent 拥有哪些文件系统能力，实际用了哪些，跨后端移动了什么数据？

适合场景：

- MCP security
- virtual filesystem agent runtime
- data exfiltration review
- capability minimization
- plugin/skill review

这张图对安全尤其重要，因为 agent 风险经常来自“工具太多，边界不清楚”。统一文件系统让 agent 更好用，但也让数据移动更自然。可视化必须把这些 mount boundary 显示出来。

## 图八：AI PR Blast Radius View

这是最应该先做成产品的图。

PR reviewer 不一定想看完整 timeline。他需要一张 run receipt：

```text
Touched files: 17
Read files: 143
Generated files: 6
Deleted files: 1
External network calls: 4
Policy warnings: 2
Tests influenced: 11

High-risk paths:
  .github/workflows/release.yml
  package-lock.json
  src/auth/*

Unattributed activity:
  git status x6
  cat ~/.config/tool/config.json
```

它回答的问题是：

> 这个 AI-generated PR 的行为半径有多大，我应该审哪里？

适合场景：

- PR due diligence
- team lead review
- skill/MCP verification
- incident report cover page
- normal run receipt

这张图应该是静态、紧凑、能贴进 PR comment 的。它不是完整 dashboard，而是 reviewer 的第一屏。

## 哪个场景用哪张图

| 场景 | 用户问题 | 首选图 |
| --- | --- | --- |
| 正常 run receipt | agent 到底做了什么？ | AI PR Blast Radius View |
| PR review | 我能信这个 diff 吗？ | Blast Radius + Provenance DAG |
| coding agent debug | 它为什么修不好？ | Agent Attention Treemap + I/O Flame Graph |
| incident forensics | 哪个动作导致问题？ | Provenance DAG + Snapshot Map |
| recovery | 我该恢复哪些东西？ | Snapshot Map + changed path inventory |
| policy tuning | 哪些动作该允许或拦截？ | Policy-Aware Filesystem Map |
| MCP/skill review | 这个工具真的按声明工作吗？ | Capability Graph + Policy Map |
| multi-agent merge | 冲突来自哪里？ | Multi-Agent Conflict Graph |
| sandbox/storage research | agent 把 workspace 搅脏了多少？ | Snapshot-Aware Workspace Map |
| behavior regression | 新版本行为半径变大了吗？ | Snapshot Map + I/O Flame Graph |

## 一个可落地的 MVP 顺序

第一步，做静态 Blast Radius View。

它最接近用户价值：跑完 agent 后，用户不用打开复杂 UI，也能知道读了什么、改了什么、删了什么、连了哪里、有哪些 warning。

第二步，做 Agent Attention Treemap。

它能让 coding agent debug 立刻变得可见。很多 agent 失败不是最终 diff 错，而是前面的 repo navigation 错。

第三步，做 Why This File Changed? Provenance DAG。

这一步把 AgentSight 和普通 git diff 拉开距离。它开始回答“为什么”和“由谁造成”。

第四步，做 Policy-Aware Filesystem Map。

当观测和 attribution 稳定后，再把规则投影到图上。这样 policy 不是空泛的 guardrail，而是对实际 runtime behavior 的 verdict。

第五步，再做 Snapshot-Aware Workspace Map 和 Multi-Agent Conflict Graph。

这两张更偏系统论文和高级产品场景，应该等单 run report 足够好之后再上。

## 数据从哪里来

上层语义可以来自：

- OpenTelemetry / OpenInference / OpenAI Agents traces
- LLM API TLS capture
- tool call events
- harness logs
- test output

底层 runtime observation 可以来自：

- AgentSight eBPF process/file/network tracing
- fanotify / inotify / syscall events
- process tree
- git index and diff
- sandbox snapshot / overlayfs metadata
- virtual filesystem mount metadata

存储上，最自然的是 temporal property graph：

```text
(:Run)-[:HAS_SPAN]->(:LLMSpan)-[:CALLED]->(:ToolCall)
(:ToolCall)-[:SPAWNED]->(:Process)-[:READ]->(:File)
(:Process)-[:WROTE]->(:File)-[:HAS_DIFF]->(:DiffHunk)
(:File)-[:IN_ZONE]->(:PolicyZone)
(:FileEvent)-[:HAS_VERDICT]->(:PolicyVerdict)
```

这使得 UI 可以问很具体的问题：

- 哪些文件被写入但没有对应 tool call？
- 哪些 read 发生在 secret zone？
- 哪些 changed file 没有测试覆盖？
- 哪些 process 写出了 workspace？
- 哪个 LLM turn 导致最多 dirty files？
- 新版本 agent 新增了哪些 network-backed mounts？

## 论文表述

如果写 paper，不要把它叫成 “a visualization dashboard for agents”。这会显得像 UI 工程。

更强的表述是：

> A workspace behavior map for AI coding agents.

贡献可以是：

1. 一个 agent filesystem provenance model，把 intent、tool、process、file mutation、snapshot effect 和 policy verdict 连起来。
2. 一个低侵入采集方案，组合 harness trace、git diff、eBPF/fanotify/runtime events。
3. 一组 query/view：attention treemap、provenance DAG、I/O flame graph、snapshot map、policy lens、blast radius report。
4. 一个评估：用真实 coding-agent runs 或 SWE-bench runs 证明它能更快定位错误探索、越权修改、prompt injection、无效 I/O、资源浪费和危险 PR。

一句话总结：

> Git diff shows the patch. Agent Workspace Map shows the run that produced the patch.

## References

- OpenAI Agents SDK Tracing: https://openai.github.io/openai-agents-python/tracing/
- Agentic Radar: https://github.com/splx-ai/agentic-radar
- AgentFS: https://github.com/tursodatabase/agentfs
- Mirage: https://docs.mirage.strukto.ai/
- PROV-AGENT: https://arxiv.org/abs/2508.02866
- OpenTelemetry GenAI semantic conventions: https://opentelemetry.io/docs/specs/semconv/gen-ai/
- Brendan Gregg Flame Graphs: https://www.brendangregg.com/flamegraphs.html
- AgentSight visualization note: [vis.md](vis.md)
- AgentSight motivation note: [why.md](why.md)
