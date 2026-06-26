# User Pain, Academic Tools, And AI-Facing Product Shape

调研时间：2026-06-02。

这份文档补充前几份市场调研的不足：不只看竞品，而是看社区里的真实用户痛点、学术界/研究工具在做什么，以及从 AI agent 自己的视角，AgentSight 应该以什么产品形态最容易被使用。学术界更关键的“数据采集痛点”单独整理在 [07-academic-data-collection-pain.md](07-academic-data-collection-pain.md)。

核心结论：

> 用户痛点不只是“我要审计 agent”。更大的痛点是：我想把真实系统任务委托给 agent，但我需要更少确认、更可控的权限、更可靠的恢复、更低成本，以及能被人和 AI 都消费的证据。

## Method

资料来源：

- 社区讨论：Cursor forum、Reddit、Hacker News、GitHub issues/discussions。
- 官方文档：Claude Code、Gemini CLI、Codex/Cursor 相关公开安全和权限资料。
- 学术/研究：AGDebugger、AgentStepper、DiLLS、AgentTrace、TRAIL、MAST、Claude Code auto-mode stress test、AI coding tool bug study 等。

注意：社区帖子是 pain signal，不是市场规模证明。单个 bug report 不能外推成“所有用户都有这个问题”，但多个工具、多个社区、多个时间点出现同类问题时，可以作为产品假设的依据。

## What Users Are Actually Complaining About

### 1. Approval Fatigue vs. Unsafe Autonomy

社区和官方资料都指向一个强痛点：用户不想每一步都确认，但完全跳过确认又危险。

Evidence:

- Anthropic 的 Claude Code auto mode 文章说，Claude Code 用户批准了 93% 的 permission prompts；他们把问题描述为 approval fatigue，即用户逐渐不再认真看自己批准的内容。来源：[Claude Code auto mode: a safer way to skip permissions](https://www.anthropic.com/engineering/claude-code-auto-mode)。
- Claude Code 文档和 FAQ 专门解释 permission modes、auto mode、always allow、plan/read-only 等模式，说明“怎么少点确认但别失控”已经是官方产品问题。来源：[Claude Code permission modes](https://code.claude.com/docs/en/permission-modes)、[Claude Code user FAQ](https://support.claude.com/en/articles/14554922-claude-code-user-faq)。
- Reddit 上有大量围绕 auto mode、bypass permissions、always allow、每次重复确认的讨论。来源示例：[Claude Code keeps asking for permission even with always allow](https://www.reddit.com/r/ClaudeCode/comments/1rvo79u/claude_code_keeps_asking_for_permission_even_with/)、[Auto mode is the sleeper feature nobody is talking about enough](https://www.reddit.com/r/claude/comments/1sgn82s/auto_mode_is_the_sleeper_feature_nobody_is_talking_about_enough/)。
- Gemini CLI 的 Trusted Folders 文档明确把目录信任作为工具自动接受和 workspace 设置启用的前置条件。来源：[Gemini CLI Trusted Folders](https://google-gemini.github.io/gemini-cli/docs/cli/trusted-folders.html)。

User pain:

- 用户不是想要“更多审计日志”，而是想知道哪些动作可以放心自动批准。
- 用户在“每步确认”和“危险跳过权限”之间缺少一个基于历史行为和真实 side effects 的调权工具。
- 现有 permission prompt 通常给的是命令/工具摘要，不一定给出真实路径、子进程、网络目的地、历史风险和可能 side effects。

AgentSight product implication:

- 不要只做事后报告。
- 应该支持 `policy suggest`：基于最近 N 次 run 生成建议，例如哪些 bash pattern 可 auto-approve，哪些路径要确认，哪些组合要 deny。
- 应该支持“确认前上下文”：当 agent 要执行风险动作时，给用户看的不是 agent 自己写的一句话，而是结构化风险摘要。

Potential product command:

```bash
agentsight policy suggest --from last-20-runs
agentsight record --policy suggested.yaml -- claude
```

### 2. Destructive File Operations And Failed Recovery

用户最直观的痛点是 agent 删除、清空、覆盖、误改文件，或者 UI/checkpoint 恢复不完整。

Evidence:

- Cursor forum 有用户报告 auto-run 打开后 deletion protection 没开，agent 删除文件且没有询问。来源：[Cursor forum: auto-run deleted files without asking](https://forum.cursor.com/t/1-2-4-agent-auto-updated-to-1-3-auto-turned-on-auto-run-mode-didnt-turn-on-delete-protection-deleted-files-without-asking/122699)。
- Cursor forum 另有 “Agent deletes critical files without confirmation” 和 “AI assistant completely deleted all my files” 这类帖子。来源：[Agent deletes critical files](https://forum.cursor.com/t/agent-deletes-critical-files-without-confirmation/147361/2)、[AI assistant completely deleted all my files](https://forum.cursor.com/t/your-ai-assistant-completely-deleted-all-my-files-from-my-computer/158182/10)。
- Reddit 用户讨论 Cursor agent 删除文件、checkpoint 恢复后文件仍然删除、agent 清空文件、甚至 Windows 上 `rmdir /s /q` 删除用户目录。来源：[Agent deleting files and not able to restore them through checkpoints](https://www.reddit.com/r/cursor/comments/1ovyrew/agent_deleting_files_and_not_able_to_restore_them/)、[AI agent secretly deleting my files](https://www.reddit.com/r/cursor/comments/1k1h24a/ai_agent_secretly_deleting_my_files/)、[Cursor Agent ran rmdir /s /q on Windows and deleted my user profile](https://www.reddit.com/r/cursor/comments/1tga513/cursor_agent_ran_rmdir_s_q_on_windows_and_deleted/)。
- Claude Code 相关讨论中也有用户抱怨明确说不要删文件但 Claude 删除；评论里直接提出需要 sandbox，让 agent 结构上无法删除指定目录外的文件或访问 API keys。来源：[Claude deletes files even when explicitly told not to](https://www.reddit.com/r/ClaudeAI/comments/1rpdaha/claude_deletes_files_even_when_explicitly_told/)。
- 公开报道中的 PocketOS 事件：Cursor agent 找到 API token 并调用 Railway 删除生产数据库和备份。来源：[Live Science coverage](https://www.livescience.com/technology/artificial-intelligence/i-violated-every-principle-i-was-given-ai-agent-deletes-companys-entire-database-in-9-seconds-then-confesses)。

User pain:

- 用户不只想知道“发生了删除”，还想知道如何恢复。
- git diff/checkpoint 只能覆盖 repo 内或编辑器知道的部分，无法覆盖未跟踪文件、repo 外文件、shell profile、dependency/cache、cloud-side destructive actions。
- Agent 的道歉或自述不可靠。用户需要具体 evidence：哪个进程、哪条命令、哪个路径、什么时候、是否还有后续 side effects。

AgentSight product implication:

- `report --recovery` 可能比纯 `report --audit` 更贴近用户。
- 需要专门追踪 destructive operations：unlink/rmdir/truncate/rename/mv/rm/dd/chmod/chown/cloud CLI delete。
- 报告应该输出 changed path inventory、destructive operation list、process lineage、possible recovery context。

Potential product command:

```bash
agentsight report --recovery --since "last agent run"
agentsight report export -o recovery.json
```

### 3. Token/Cost Runaway And Agent Loops

用户频繁抱怨 agent 卡循环、反复读文件、反复跑工具、上下文膨胀、token 消耗异常。

Evidence:

- Gemini CLI issue #2923：用户报告 Gemini CLI 进入 file reading loop，快速消耗 token/context。来源：[google-gemini/gemini-cli#2923](https://github.com/google-gemini/gemini-cli/issues/2923)。
- Gemini CLI discussion #4841：用户描述 Enterprise/paid API key 下危险默认行为，agent 自主使用 ReadFile/Shell 等工具进入循环，产生 47 次 tool calls 和 721,943 input tokens。来源：[google-gemini/gemini-cli discussion #4841](https://github.com/google-gemini/gemini-cli/discussions/4841)。
- Reddit 上 Gemini CLI 用户也讨论 CLI 卡循环直到崩溃。来源：[Gemini CLI gets stuck in a Loop?](https://www.reddit.com/r/GeminiCLI/comments/1rsuz1g/gemini_cli_gets_stuck_in_a_loop/)。
- r/AI_Agents 中有用户描述 agents burned $50/day doing nothing，trace 显示成功但真实下游没有发生。来源：[My AI agents burned $50/day doing nothing](https://www.reddit.com/r/AI_Agents/comments/1rzd5pn/my_ai_agents_burned_50day_doing_nothing_so_i/)。

User pain:

- 用户不只是想看 token 总量，而是想知道为什么花钱：是重复读文件、重复失败命令、context 膨胀、retry loop，还是工具返回无效。
- 成本问题经常和 side effect 问题连在一起：agent 可能花很多 token “看起来在工作”，但真实系统没有进展。
- 现有 cost dashboards 可能看得到 spend，但不一定能解释 spend 和 OS/tool side effects 的关系。

AgentSight product implication:

- 需要 `loop/cost sentinel`，不是单纯 token dashboard。
- 应该把 token spend 与 tool calls、file reads、commands、processes 和 actual output changes 关联。
- 如果 agent 连续读同一批文件、重复执行失败命令、没有产生新的 file/process side effects，应该给出 runaway signal。

Potential product command:

```bash
agentsight report --waste
agentsight watch --budget-tokens 200000 --idle-side-effect-window 5m -- claude
```

### 4. Debugging Agents Is Still Too Much Guesswork

很多用户不是缺 trace，而是 trace 太碎、太多、没有告诉他们“为什么失败”。

Evidence:

- Reddit 上有讨论说 “agent observability is still just LLM tracing”，评论里提到 LLM call tracing 不等于 agent tracing，真正缺的是 state transition observability。来源：[agent observability is still just LLM tracing](https://www.reddit.com/r/aiagents/comments/1s4brpm/agent_observability_is_still_just_llm_tracing_do/)。
- 有用户说 debugging AI agents is miserable，local replay + editable trace re-runs 比 another dashboard 更实用。来源：[We built a local, open-source trace debugger for AI agents](https://www.reddit.com/r/LLMDevs/comments/1td5zuk/we_built_a_local_opensource_trace_debugger_for_ai/)。
- Reddit 讨论指出，调试 multi-agent systems 时 traces show too much detail；失败场景下用户不需要 47 个 function calls，而需要知道哪个数据没传给哪个 agent。来源：[Debugging multi-agent systems: traces show too much detail](https://www.reddit.com/r/LangChain/comments/1pcfimn/debugging_multiagent_systems_traces_show_too_much/)。
- r/AgentsOfAI 讨论里有用户说大多数 agent failures 不是 AI problems，而是 bad inputs/tool outputs；“logging every single tool input/output as structured data was the single biggest quality jump”。来源：[Most agent failures I’ve debugged weren’t actually AI problems](https://www.reddit.com/r/AgentsOfAI/comments/1sk2plq/most_agent_failures_ive_debugged_werent_actually/)。

User pain:

- 用户不想看 raw traces；他们想知道 failure mode。
- 用户需要能被 agent 或人消费的 structured evidence，而不是一堆 spans。
- 很多失败不是模型推理错，而是工具输入/输出、环境状态、空 retrieval、partial JSON、权限、文件状态发生了变化。

AgentSight product implication:

- 输出不能只是 timeline UI；必须有 summary、root-cause hints、state transition summary、side-effect delta。
- 需要 `agentsight query` 或 `agentsight explain`，让人或 AI 按问题查询。
- 需要把 trace 简化为可回答的问题：What changed? What failed? What repeated? What touched sensitive state? What did not happen despite success claims?

Potential product command:

```bash
agentsight query "what changed outside the repo?"
agentsight query "which command first touched .env?"
agentsight query "why did token usage spike?"
```

### 5. Agent Says Success, Reality Says Otherwise

一个反复出现的痛点是：agent 或 trace 表面显示成功，但真实世界没有产生预期 side effect，或者产生了错误 side effect。

Evidence:

- r/AI_Agents 中 “agents burned $50/day doing nothing” 的讨论里，用户提到 “trace says ✓ but nothing actually happened downstream” 是 underserved gap，并说需要把 output verification 当成 infra-level first-class concern。来源：[My AI agents burned $50/day doing nothing](https://www.reddit.com/r/AI_Agents/comments/1rzd5pn/my_ai_agents_burned_50day_doing_nothing_so_i/)。
- 另有讨论说 debugging agents 时不能只读 final output，而要看模型 step by step 看到的内容。来源：[Most agent failures I’ve debugged weren’t actually AI problems](https://www.reddit.com/r/AgentsOfAI/comments/1sk2plq/most_agent_failures_ive_debugged/)。

User pain:

- Agent 自己是 unreliable narrator。
- 用户需要验证“成功”的定义是否落到了现实状态：文件改了吗？测试跑了吗？issue 关了吗？API 调了吗？数据真的写了吗？
- 这不是合规审计，而是基础可靠性问题。

AgentSight product implication:

- 把 side effect verification 做成一等功能。
- 对于 coding agent，最终报告应该突出 “claimed vs observed”：agent 声称做了什么，系统实际观察到什么。
- 对任务型 agent，应该支持 user-defined success probes，例如 expected file change、expected command, expected network-free run。

Potential product command:

```bash
agentsight verify-side-effects --expect changed:src/app.ts --expect command:"npm test"
```

### 6. MCP/Tool Trust Is A User Pain, Not Just Security Theory

MCP 和 third-party tools 的风险不是抽象的安全问题，而是安装和使用前的信任问题。

Evidence:

- MCP 官方安全最佳实践涵盖 confused deputy、token passthrough、SSRF、session hijacking、local MCP server compromise、scope minimization 等。来源：[MCP Security Best Practices](https://modelcontextprotocol.io/specification/2025-06-18/basic/security_best_practices)。
- MCP ToolAnnotations 文档明确这些 annotations 是 hints，不保证真实行为。来源：[MCP schema ToolAnnotations](https://modelcontextprotocol.io/specification/2025-11-25/schema)。
- OWASP MCP Tool Poisoning 把恶意 MCP server/tool response 携带隐藏指令定义为实际攻击类。来源：[OWASP MCP Tool Poisoning](https://owasp.org/www-community/attacks/MCP_Tool_Poisoning)。
- Cloud Security Alliance 对 Gemini CLI workspace trust / CI trust 相关漏洞发布过安全研究 note，说明 agent tool 与 workspace trust 已成为供应链和 CI/CD 风险点。来源：[CSA Gemini CLI CVSS10 RCE sandbox bypass note](https://labs.cloudsecurityalliance.org/wp-content/uploads/2026/05/CSA_research_note_gemini_cli_cvss10_rce_sandbox_bypass_20260501-csa-styled.pdf)。

User pain:

- 用户/企业不知道 third-party MCP server、skill、plugin 实际会做什么。
- 权限声明、README、tool annotation、review 都不等于动态行为证明。
- 企业 internal tool catalog 需要 evidence 才能把工具放进 allowlist。

AgentSight product implication:

- `agentsight verify` 是强场景，但它不应只叫 audit；它是 tool adoption 的信任工件。
- 验收报告要和版本、安装命令、测试 fixture、policy、observed behavior 绑定。

## Academic And Research Tool Landscape

学术界和研究工具明显在往 trajectory/debugging/root-cause/summarization 方向走。这说明“agent run 过程”确实是研究热点。但大多数工作仍在 message/tool trace 层，不是 OS side-effect 层。

### AGDebugger

AGDebugger 是 CHI 2025 的 interactive multi-agent debugging tool，支持浏览和发送消息、编辑和 reset prior agent messages、overview visualization 来导航复杂 message histories。来源：[AGDebugger paper](https://arxiv.org/abs/2503.02068)、[Microsoft AGDebugger GitHub](https://github.com/microsoft/agdebugger)。

What it teaches:

- 人需要在 agent 轨迹中回到中间点、编辑、reset、steer。
- 多 agent 系统的 message history 太复杂，需要 overview visualization。

Gap for AgentSight:

- 重点是 multi-agent messages 和 steering，不是本地 coding agent 对 OS 的真实 side effects。
- 没有解决 “哪个 subprocess 删除了哪个文件 / 如何恢复” 这类系统层问题。

### AgentStepper

AgentStepper 是 2026 年的 software development agent interactive debugger。论文摘要强调开发者必须 reason about trajectories of LLM queries, tool calls, and code modifications，但现有技术很少以可理解格式暴露中间过程。来源：[AgentStepper arXiv](https://arxiv.org/abs/2602.06593)、[Hugging Face paper page](https://huggingface.co/papers/2602.06593)。

What it teaches:

- Coding agent 的核心调试对象是 trajectory，不是单个 final diff。
- 开发者需要 inspect/manipulate agent trajectories。

Gap for AgentSight:

- AgentStepper 更像 interactive debugger。AgentSight 可以互补：提供系统边界 evidence，作为 debugger 可消费的数据源。

### DiLLS

DiLLS 是 2026 年关于 LLM-based multi-agent systems 的 interactive diagnosis，通过 layered summary of agent behaviors 帮用户诊断复杂 agent 行为。来源：[DiLLS arXiv](https://arxiv.org/abs/2602.05446)。

What it teaches:

- Raw trace 太长，需要 layered summary。
- 用户需要逐层 drill down，而不是从完整日志开始。

Gap for AgentSight:

- AgentSight 报告也应该 layered：summary -> risk -> evidence -> raw events。
- OS side effects 也需要 layered summary，否则 eBPF events 会比 LLM traces 更难读。

### AgentTrace

有两个相关方向：

- Causal graph tracing for root cause analysis in deployed multi-agent systems：从 execution logs 重建 causal graphs，从 error manifestation 反向追踪并 rank root causes，且不依赖 debugging 时 LLM inference。来源：[AgentTrace causal graph arXiv](https://arxiv.org/abs/2603.14688)。
- Structured logging framework for agent system observability：强调 continuous, introspectable trace capture，用于 debugging、benchmarking、security、accountability、real-time monitoring。来源：[AgentTrace structured logging arXiv](https://arxiv.org/abs/2602.10133)。

What it teaches:

- Causal graph/root-cause 是合理方向。
- Accountability 和 trust calibration 是研究界也在强调的词。

Gap for AgentSight:

- 这些工作通常假设 agent system 本身产生 logs/traces。AgentSight 的差异是可以从系统边界补齐 agent 没有记录的 OS effects。

### TRAIL

TRAIL 是 agentic issue localization benchmark，包含 annotated AI agent execution traces 和错误；搜索结果摘要称现代长上下文 LLM 在 trace debugging 上表现很差，best Gemini-2.5-pro 只有 11%。来源：[TRAIL Hugging Face paper page](https://huggingface.co/papers/2505.08638)。

What it teaches:

- 不能假设“把 trace 丢给 LLM 就会自动诊断好”。
- Trace debugging 需要更结构化、分层、可查询的数据。

Gap for AgentSight:

- AgentSight 如果想让 AI 使用自己的 evidence，必须输出结构化 schema、risk flags、causal hints，而不是只输出 raw JSONL 或 HTML。

### MAST

MAST 是 multi-agent system failure taxonomy/dataset。公开站点和二次讨论称它分析了 1,600+ execution traces，并把 multi-agent 失败分解为 coordination、verification 等类别。来源：[MAST site](https://sites.google.com/berkeley.edu/mast/)、[Augment Code summary](https://www.augmentcode.com/guides/why-multi-agent-llm-systems-fail-and-how-to-fix-them)。

What it teaches:

- Agent failure taxonomy 有必要。
- 很多失败不是模型单步错误，而是 coordination、handoff、verification、role confusion。

Gap for AgentSight:

- AgentSight 可以借鉴 taxonomy，但在 local coding agent 场景中补一个 “system side-effect failure taxonomy”：destructive ops、untracked changes、secret reads、network egress、loop/no-progress、permission drift。

### Claude Code Auto Mode Stress Test

“Measuring the Permission Gate” 对 Claude Code auto mode 做 stress-test。搜索摘要显示论文把 auto mode 描述为第一个 deployed permission system for AI coding agents，并引用 Anthropic production traffic 的 false positive/false negative 指标；还指出 auto mode 关注 shell dangerous actions，但 agents 可通过 file edits 实现等效危险效果。来源：[arXiv: Measuring the Permission Gate](https://arxiv.org/abs/2604.04978)。

What it teaches:

- Permission gate 是真实研究对象。
- 单看 shell 命令不够，file edits 也可能绕过危险动作定义。

Gap for AgentSight:

- AgentSight 可以从 OS/file/process/network side effects 回看 permission gate 是否漏判。
- 这支持 “permission tuning” 不是附属功能，而是核心用户场景。

### Engineering Pitfalls In AI Coding Tools

“Engineering Pitfalls in AI Coding Tools” 研究 Claude Code、Codex、Gemini CLI bug，搜索摘要显示 bug 主要影响 tool invocation 和 command execution stages。来源：[Engineering Pitfalls arXiv](https://arxiv.org/abs/2603.20847)。

What it teaches:

- AI coding tool 的问题大量发生在工具调用和命令执行阶段。
- 这正是 OS boundary evidence 能提供补充的区域。

## What Academic Tools Mostly Do Not Cover

这些研究和工具很有价值，但多数还没有覆盖 AgentSight 的潜在核心：

1. **Local OS side effects**

   大多关注 message/tool trace、agent memory、communication、tool outputs，而不是实际 process/file/network effects。

2. **Recovery context**

   研究多关注 diagnosis/root cause，很少直接输出“如何撤销/恢复本地状态”的 evidence bundle。

3. **Permission/autonomy tuning**

   Claude Code auto mode 是例外，但更偏单产品 classifier。还缺跨 agent、跨 repo、基于历史 OS evidence 的 policy suggestion。

4. **AI-readable evidence schema**

   很多工具有人类 UI，但如果下一步是让另一个 agent 帮忙修复、验收、写 PR comment，它需要稳定 JSON/MCP interface。

5. **Cross-tool local evidence**

   学术工具常假设特定 agent framework 或 trace format。用户社区痛点发生在 Claude Code、Codex、Cursor、Gemini CLI、MCP server、shell、package manager 等混合环境。

## How I Would Use AgentSight As An AI Agent

如果我是一个 coding agent / review agent / recovery agent，我不想使用一个 dashboard。我最容易使用的是：

1. CLI commands with stable JSON output
2. A local MCP server exposing evidence-query tools
3. Static reports for humans
4. Optional HTML UI only for deep inspection

### Why CLI First

Most coding agents can run shell commands. A CLI works across Claude Code,
Codex, Gemini CLI, Cursor terminal workflows, CI, and scripts.

Useful commands:

```bash
agentsight record --label fix-bug -- claude
agentsight report --json --label fix-bug
agentsight query --json "what changed outside the repo?"
agentsight diff --json baseline current
agentsight policy suggest --json --from last-20-runs
agentsight verify --json --policy agentsight.verify.yaml -- ./run-mcp-fixture.sh
```

The important part is not just the commands. The output must be machine-readable:

- stable event ids
- session ids
- process lineage
- file path categories
- changed/deleted/created paths
- command exit status
- network destinations
- token/cost metrics
- risk flags
- confidence and evidence pointers

### Why MCP Second

An AgentSight MCP server would make it easier for agents to query evidence without parsing CLI output.

Potential MCP tools:

- `agentsight_latest_session`
- `agentsight_get_report`
- `agentsight_query_events`
- `agentsight_get_changed_paths`
- `agentsight_get_risk_flags`
- `agentsight_compare_sessions`
- `agentsight_suggest_policy`
- `agentsight_generate_pr_comment`
- `agentsight_generate_recovery_context`

This is especially useful for a review agent or recovery agent:

- A review agent can attach a report to a PR.
- A recovery agent can ask what changed and propose undo steps.
- A policy agent can suggest allow/deny rules from observed behavior.

But MCP should be a wrapper around structured evidence, not the only interface.

### Why HTML Report Still Matters

Humans need a compact report:

- for PR review
- for README/marketplace badge
- for incident postmortems
- for security review
- for non-technical users after a cleanup/storage task

But HTML is not the primary interface for AI. If the product only has a dashboard, agents will have to scrape it or ask humans to interpret it.

### What I Would Not Want As An AI

- A dashboard-only product.
- Raw JSONL with no schema or query tool.
- A report that only summarizes but loses evidence pointers.
- A policy engine whose decisions cannot be explained.
- A tool that requires me to know eBPF event names.
- A tool that cannot answer concrete questions like “what did I delete?” or “what touched `.env`?”

## Product Shape Recommendation

The right first product shape is:

> CLI for capture and reports, structured JSON for agents, static HTML/Markdown for humans, optional MCP for agent-native querying.

Not:

> Full web dashboard first.

Recommended product layers:

1. **Capture**

   ```bash
   agentsight record -- <agent command>
   ```

2. **Receipt**

   ```bash
   agentsight report --html
   agentsight report --json
   ```

3. **Query**

   ```bash
   agentsight query "what changed outside the repo?"
   ```

4. **Recovery**

   ```bash
   agentsight report --recovery
   ```

5. **Policy**

   ```bash
   agentsight policy suggest
   agentsight record --policy policy.yaml -- <agent command>
   ```

6. **Verification**

   ```bash
   agentsight verify --policy agentsight.verify.yaml -- ./fixture.sh
   ```

7. **Diff**

   ```bash
   agentsight diff old new
   ```

8. **MCP**

   ```bash
   agentsight mcp
   ```

## Revised User Pain Ranking

Based on community signals and academic trends, the pain ranking should be revised:

| Rank | Pain | Why it matters | Product response |
| --- | --- | --- | --- |
| 1 | Approval fatigue vs unsafe autonomy | Official and community evidence is strong; users want agents to act more freely but safely | Policy suggestions, risk-aware confirmations, autonomy profiles |
| 2 | Destructive changes and poor recovery | File deletion/DB deletion/revert failures are visceral and high-impact | Recovery context, destructive op tracking, changed path inventory |
| 3 | Token/cost runaway loops | GitHub issues and community posts show concrete cost/context pain | Loop/cost sentinel, waste report, no-progress detection |
| 4 | Debugging traces are too much or too shallow | Academic and community sources converge on trajectory diagnosis | Layered report, queryable evidence, causal hints |
| 5 | Agent self-report is unreliable | Users need observed side effects, not only transcript | Claimed vs observed report |
| 6 | Third-party tool trust | MCP/skill/plugin ecosystem is growing and trust mechanisms lag | Dynamic verification badge/report |
| 7 | Team PR/process acceptance | Strong market logic, but needs validation with teams | PR report artifact/comment |
| 8 | Compliance/security audit | Valuable but risks enterprise platform sprawl | Export evidence, integrate later |

## What This Changes From Previous Market Docs

Previous docs framed AgentSight as an independent behavior evidence layer. That is still technically right, but it is too abstract and too audit-like.

The more user-centered framing is:

> AgentSight helps users delegate real work to agents by making autonomy tunable,
> side effects recoverable, and behavior verifiable.

This is broader than audit and more product-relevant.

## Validation Experiments

### Experiment 1: Approval Fatigue Study

Goal:

- Test whether users want AgentSight to recommend auto-approval rules.

Method:

- Recruit 10 Claude Code/Codex/Cursor/Gemini users.
- Run 5 normal sessions under AgentSight.
- Generate `policy suggest` output.
- Ask users whether they would apply the suggested rules.

Success:

- At least 5 users apply or manually adapt the rules.
- Users report fewer meaningless prompts without feeling less safe.

### Experiment 2: Recovery Report

Goal:

- Test whether `report --recovery` is more compelling than `report --audit`.

Method:

- Create controlled scenarios: file deletion, config change, package install, generated artifacts, outside-repo write.
- Ask users or another AI agent to recover with and without AgentSight.

Success:

- Recovery is faster and more complete with AgentSight evidence.
- Users can name at least one side effect git diff missed.

### Experiment 3: Agent-Readable JSON

Goal:

- Test whether another agent can use AgentSight evidence to write a useful PR comment or recovery plan.

Method:

- Feed only `agentsight report --json` to a review/recovery agent.
- Compare against feeding raw logs or HTML.

Success:

- The agent produces fewer hallucinated claims and more evidence-backed recommendations.

### Experiment 4: MCP/Skill Verification Badge

Goal:

- Test whether tool authors value a dynamic behavior badge.

Method:

- Pick 10 MCP servers or skills.
- Generate read-only/no-network/no-secret/no-destructive reports.
- Ask maintainers if they would include the report link in README or release notes.

Success:

- At least 3 maintainers accept or ask for changes to make acceptance possible.

## Bottom Line

The community pain is not just:

> I need to audit my agent.

It is:

> I want to let my agent do useful work without babysitting it, but I need a way
> to know what it did, tune what it can do, recover when it breaks something,
> and give humans or other agents evidence they can act on.

That points to an AgentSight product that is command-line-first, evidence-first,
and AI-readable:

- CLI capture
- JSON query/report
- static human report
- MCP query interface
- policy suggestions
- recovery context
- verification reports

The dashboard can come later.
