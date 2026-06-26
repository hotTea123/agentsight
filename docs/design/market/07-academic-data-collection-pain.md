# Academic Data-Collection Pain And Research Infrastructure Opportunity

调研时间：2026-06-02。

这份文档不按“学术界有什么工具”组织，而按“学术界正在努力收集什么数据、想回答什么问题、为什么现在很难收集”组织。

核心结论：

> 学术界的痛点不是缺一个更漂亮的 agent debugger，而是缺一个跨 agent、跨工具、跨环境的真实行为采集层。研究者现在已经在用 GitHub PR、agent trace、bug issue、permission stress test、MCP 攻击 benchmark、依赖可复现性实验来补这个缺口，但这些数据大多是一次性采集、人工标注、工具专用、缺少 OS side-effect ground truth。

这对 AgentSight 的产品判断很重要：

> AgentSight 真正有价值的产品部分，不是替 agent 写更多业务逻辑，也不是做一个复杂 dashboard，而是把 agent 操作真实系统时最难被可靠采集的证据变成标准化数据。

换句话说，很多“分析”可以让用户自己的 agent 做；但前提是它要有可信、结构化、跨工具的行为数据。这个数据层才是不容易被一句 prompt 替代的部分。

## Reading Lens

这里的“学术界痛点”不是指把高校当主要付费客户。更现实的意义是：

- 学术研究会暴露未来产品的基础数据需求。
- 如果研究者都需要费力收集某类数据，说明真实用户也大概率缺少这类 evidence。
- 如果每篇论文都要自己造 trace、自己写 scraper、自己人工标注，说明这里可能有通用基础设施机会。
- 但学术市场本身小，AgentSight 不应该做成“论文工具”。更好的路线是做成 agent run evidence infrastructure，学术数据集只是其中一个验证场景。

## Research Pain Map

| Research question | What researchers are collecting | Why current collection is painful | AgentSight opportunity |
| --- | --- | --- | --- |
| AI coding agents 在真实开源项目里到底表现如何？ | Agent-authored PR、comments、reviews、CI status、merge/rejection outcome | PR 是最终结果，缺少 agent 产生 PR 之前的真实运行过程 | 给每个 agent PR 附 run receipt：命令、文件、网络、权限、测试、风险 |
| Agentic PR 为什么被接受或拒绝？ | 人工检查 PR review thread，恢复 reviewer rationale | 只能从 review 文本推断原因；很多拒绝没有可观察 rationale | 把 agent run evidence 作为 review artifact，减少“看不到过程”的拒绝 |
| Agent 失败发生在哪个步骤？ | Execution trace、tool call、错误位置、失败 taxonomy | Trace 格式不统一，长 trace 需要人工标注，缺 OS side-effect ground truth | 输出结构化 trajectory + process/file/network evidence，便于自动标注和复盘 |
| Multi-agent 系统为什么失败？ | 多 agent conversation trace、handoff、coordination failure | 复杂 trace 很长，研究者要人工读上万行对话 | 为每个 agent/sub-agent 生成 side-effect timeline 和 handoff evidence |
| Permission gate / auto mode 是否真的安全？ | Prompt benchmark、state-changing action labels、false positive/false negative | 很多危险动作不只走 shell，也可能通过 file edit 或工具链完成 | 直接观察实际 state changes，反向评估 permission classifier 的 coverage |
| Coding agent 工具调用为什么出 bug？ | GitHub issue、用户讨论、开发者回复、bug taxonomy | Issue 是用户事后叙述，不一定有完整命令、环境、进程、文件变化 | 采集 command execution、cwd、env category、exit status、child process、side effects |
| AI 生成代码是否可复现？ | 生成项目、声明依赖、实际可运行依赖、runtime dependencies | 需要重建环境并比较声明和实际运行依赖，采集成本高 | 自动记录 package install、network fetch、runtime import/load、test execution |
| Agent 是否遵守 logging/observability 指令？ | Agentic PR 的 log changes、human repair、review feedback | PR 只能看到最终代码，不知道 agent 是否尝试过、失败过或忽略过指令 | 把 prompt/instruction、tool actions、文件修改和 human correction 关联 |
| MCP/skill/plugin 是否会越权？ | Tool poisoning benchmark、attack success、exfiltration/destructive behavior | 只看 tool schema/description 不够；需要真实 agent 执行后的行为证据 | 动态验证 read-only、workspace-local、no network、no secret access 等声明 |
| Agent 真实用户痛点是什么？ | 真实 IDE/CLI session、developer pushback、修正成本、信任成本 | 用户纠错是可见信号，但 agent 当时做了什么常常缺失 | 把 user correction 和前序 observed side effects 对齐，做 misalignment dataset |

## 1. Agentic PR Studies Are Collecting Outcome Data, Not Process Evidence

学术界已经在大规模收集 agent-authored PR，因为这是公开、可抓取、能量化的数据。

Examples:

- [AIDev: Studying AI Coding Agents on GitHub](https://arxiv.org/abs/2602.09185) 收集了 932,791 个 Agentic-PR，覆盖 OpenAI Codex、Devin、GitHub Copilot、Cursor、Claude Code，来自 116,211 个仓库和 72,189 个开发者。
- [Where Do AI Coding Agents Fail?](https://arxiv.org/abs/2601.15195) 研究了 33k agent-authored PR，并人工分析 600 个未合并 PR，做 rejection pattern taxonomy。
- [Why Are Agentic Pull Requests Merged or Rejected?](https://arxiv.org/abs/2605.22534) 分析 11,048 个 closed Agentic PR，细化到 9,799 个 human-reviewed PR，并人工检查 717 个代表性 case 来恢复 reviewer rationale。

这些研究说明：agent PR 已经是一个明确研究对象。但它们暴露了一个数据缺口：

- PR 是结果，不是过程。
- Git diff 只能看到 repo 内最终修改，不能看到 agent 读了什么、删了什么、跑了什么命令、访问了什么网络目标。
- Review thread 是人的解释，不是 agent run 的完整事实。
- CI status 告诉你测试过没过，但不告诉你 agent 在本地如何尝试、绕过、重试、失败。
- 很多 rejected PR 没有清楚 rationale，研究者只能人工推断。

AgentSight 在这个场景里的价值不是“再做一个 PR 分析工具”，而是：

> 为 agent-authored PR 提供过程证据层。

最小产品形态：

```bash
agentsight record --label pr-123 -- claude
agentsight report --label pr-123 --format markdown > agentsight-pr-receipt.md
agentsight report --label pr-123 --format json > agentsight-pr-receipt.json
```

PR receipt 应该回答：

- 这个 PR 是哪个 agent / model / tool session 生成的？
- agent 改了哪些 tracked files、untracked files、repo 外文件？
- agent 跑了哪些 test/build/lint 命令？结果是什么？
- agent 是否访问 `.env`、SSH key、cloud config、browser profile 等敏感路径？
- agent 是否访问外部网络、下载依赖、调用云 CLI？
- agent 是否发生删除、重命名、覆盖、chmod/chown 等破坏性操作？
- agent 是否声称成功但没有观察到对应 side effect？

对研究者来说，这会把 Agentic PR 研究从“最终 outcome analysis”推进到“过程证据分析”。

对产品来说，这也对应真实用户场景：

> Reviewer 不只是问这个 diff 看起来能不能过，而是问这个 agent 是怎么得到这个 diff 的。

## 2. Trace Debugging Research Is Starved For Standardized Trajectory Data

研究者已经在做 agent trace debugging、failure localization、taxonomy。

Examples:

- [TRAIL: Trace Reasoning and Agentic Issue Localization](https://arxiv.org/abs/2505.08638) 说明当前复杂 agent workflow trace 的评估依赖人工、领域专用分析；论文构造了 148 条人工标注 trace，包含 841 个 error，并报告长上下文模型在 trace debugging 上仍然表现很弱。
- [MAST](https://sky.cs.berkeley.edu/project/mast/) 对 7 个开源 multi-agent framework 的 200 条 conversation trace 做系统分析，每条 trace 平均超过 15,000 行文本，整理出 14 类 multi-agent failure modes。
- [AGDebugger](https://arxiv.org/abs/2503.02068)、[AgentStepper](https://arxiv.org/abs/2602.06593)、[DiLLS](https://arxiv.org/abs/2602.05446)、[AgentTrace causal graph](https://arxiv.org/abs/2603.14688) 都指向同一个事实：raw trace 太长，agent trajectory 需要更好的诊断、分层、回放和因果定位。

这些工作很有价值，但它们共同的采集痛点是：

- Trace 格式通常依赖具体 framework 或研究原型。
- 很多 trace 是 agent 自己或 framework 记录的，不一定包含真实 OS side effects。
- 研究者需要人工读长 trace、人工标注 failure point。
- Tool output 和真实环境变化之间缺少稳定连接。
- Trace 里有“agent 想做什么”，但不一定有“机器实际发生了什么”。

AgentSight 可补的不是另一个 trace viewer，而是 trace ground truth：

- process lineage：哪个 tool call 触发了哪个 shell / subprocess。
- file effects：哪些文件被创建、修改、删除、truncate、rename。
- network effects：哪些域名/IP/端口被访问。
- command semantics：cwd、argv、exit status、duration、stdout/stderr summary。
- resource/time/cost：token、工具调用、CPU、I/O、重复操作。
- sensitive boundary：是否触碰 secret、用户目录、系统目录、cloud config。

这类数据可以让研究者构造更强的 failure label：

```json
{
  "failure_type": "claimed_success_without_observed_effect",
  "agent_claim": "tests passed and deployment completed",
  "observed": {
    "commands": ["npm test"],
    "failed_commands": ["npm test"],
    "network_egress": [],
    "changed_paths": ["src/api.ts"],
    "deploy_side_effect": false
  }
}
```

产品判断：

> 如果 AgentSight 只输出 HTML timeline，它对研究者和 AI agent 都不够用。必须有稳定 JSON schema、可查询 evidence API，以及能映射到 failure taxonomy 的 event labels。

## 3. Permission And Autonomy Research Needs Real State-Change Labels

Permission gate / auto mode 是一个非常明确的研究痛点，因为用户已经不想每一步都确认，但完全自动又危险。

Example:

- [Measuring the Permission Gate: A Stress-Test Evaluation of Claude Code's Auto Mode](https://arxiv.org/abs/2604.04978) 用 128 个 prompt、253 个 state-changing actions 测试 Claude Code auto mode。论文指出 auto mode 把危险动作主要看作 shell 里的行为，但 agent 也能通过 project file edits 达成等价危险效果。

这个方向需要的数据不是单纯的 prompt 和 classifier 结果，而是：

- 用户原始授权范围。
- agent 提议的 action。
- permission gate 是否放行。
- action 实际造成的 state change。
- state change 是否越过用户意图边界。
- 同类 action 在不同 agent / tool / repo / OS 上是否表现一致。

今天难点在于：

- 权限 prompt 是 agent 产品内部事件，外部研究者很难跨产品统一采集。
- 只看 shell 命令会漏掉通过 file edit、tool chain、package script、cloud CLI 发生的等价危险效果。
- 只看 agent transcript 又无法证明实际状态变了没有。
- 需要 action-level ground truth，但手工标注非常昂贵。

AgentSight 可以把 permission 研究从“prompt classifier eval”扩展为“observed state-change eval”：

```bash
agentsight record \
  --policy permission-benchmark.yaml \
  --label permission-eval-001 \
  -- claude

agentsight report export -o permission-eval-001.json
```

数据 schema 需要明确区分：

- `intended_scope`: 用户允许的范围。
- `requested_action`: agent/tool 想执行的动作。
- `approved_by`: user / policy / auto / bypass。
- `observed_state_change`: 实际改变了什么。
- `policy_boundary_crossed`: 是否越界。
- `equivalent_danger`: 是否通过不同路径达成等价危险效果。

产品价值：

> AgentSight 可以成为 permission/autonomy tuning 的 evidence layer。用户不是为了看审计日志，而是为了知道哪些动作以后可以自动放行，哪些必须拦截。

## 4. Engineering Bug Studies Need Runtime Evidence, Not Just Issue Reports

AI coding tool 的工程问题已经开始被系统研究。

Example:

- [Engineering Pitfalls in AI Coding Tools](https://arxiv.org/abs/2603.20847) 手工分析 Claude Code、Codex、Gemini CLI 开源仓库中超过 3.8K 个公开 bug。论文摘要显示，bug 主要集中在 tool invocation 和 command execution 阶段。

这类研究现在主要依赖：

- GitHub issue description。
- 用户复现步骤。
- 开发者回复。
- 错误截图或 log。
- 人工 open coding taxonomy。

痛点是：

- Issue 是事后叙述，缺少完整运行上下文。
- 用户往往不会提供精确环境、cwd、shell、权限模式、子进程、文件变化。
- 很多 command execution bug 和 path/env/config 相关，没有 runtime evidence 很难归因。
- 研究者只能从文本推断 bug location 和 root cause。

AgentSight 可以提供“bug report evidence bundle”：

```bash
agentsight bug-report --since last-run --redact-secrets --format zip
```

Bundle 内容：

- agent tool calls。
- process tree and command argv。
- cwd / shell / OS / package manager category。
- command exit status and error class。
- changed/deleted path inventory。
- network/package install activity。
- permission prompts and approvals。
- redacted sensitive access events。

这不是学术专用功能。它也对应真实产品场景：

> 当用户说 “Codex/Claude/Cursor 把我的项目搞坏了”，support、开源 maintainer、reviewer 都需要一个能复现和定位问题的 evidence bundle。

## 5. Reproducibility Research Needs Dependency And Environment Side Effects

AI coding agents 不只是写代码，还会安装依赖、调用 package manager、生成配置、运行测试。可复现性研究正在关注这个问题。

Example:

- [AI-Generated Code Is Not Reproducible (Yet)](https://arxiv.org/abs/2512.22387) 评估 Claude Code、OpenAI Codex、Gemini 在 300 个生成项目中的可执行性，并区分 claimed dependencies、working dependencies、runtime dependencies。论文摘要报告只有 68.3% 项目能 out-of-the-box 执行，并发现 declared dependencies 到 actual runtime dependencies 有显著扩张。

这个方向需要的数据：

- agent 声称需要哪些依赖。
- manifest 里写了哪些依赖。
- 实际安装了哪些依赖。
- 运行时加载了哪些包或系统库。
- 哪些 network fetch / package registry / build script 被触发。
- clean environment 下哪些命令失败。

现在难点：

- 依赖差异需要重新构建环境和动态执行。
- package manager 行为多样：npm/pnpm/yarn/pip/poetry/uv/maven/gradle/cargo/go 等。
- agent 可能通过 shell、脚本、IDE tool、MCP tool 间接安装依赖。
- repo 内 manifest 不能代表 runtime truth。

AgentSight 可以成为 dependency side-effect profiler：

```bash
agentsight record --label gen-project -- codex
agentsight deps --label gen-project --format json
agentsight replay-check --clean-container --label gen-project
```

最小数据能力：

- package manager invocation。
- dependency manifest changes。
- lockfile changes。
- network domains contacted。
- created build artifacts。
- runtime command outcomes。
- hidden dependency indicators。

产品价值：

> 对用户来说，这不是“学术复现性”。这是 agent 生成项目能不能交付、能不能被 CI/别人机器跑起来。

## 6. Observability And Logging Studies Need Process Evidence

研究者也在看 agent 是否会写出可维护、可观测的软件。

Example:

- [Do AI Coding Agents Log Like Humans?](https://arxiv.org/abs/2604.09409) 分析 4,550 个 agentic PR，比较 AI agent 和 human 的 logging 行为，并指出自然语言 logging 指令并不稳定，human 往往在 post-generation 阶段修复 logging/observability 问题。

这个问题的数据需求包括：

- 用户是否给过 logging / observability instruction。
- agent 是否读取了现有 logging pattern。
- agent 是否修改了 log statements / metrics / tracing。
- agent 是否运行测试或检查 log output。
- human reviewer 是否后续补 logging。
- agent 是没理解、没遵守，还是因为没有上下文导致漏掉。

PR 数据只能看到最终差异，很难看到 agent 的过程：

- 它有没有看已有日志文件？
- 它有没有运行服务看日志？
- 它有没有忽略用户要求？
- 它有没有生成过日志代码但又删掉？

AgentSight 的价值是把 “instruction -> observed action -> code diff -> human correction” 串起来。

这也支持一个产品功能：

```bash
agentsight verify --policy observability-policy.yaml -- claude
```

Policy 可以检查：

- 修改生产路径时是否运行相关测试。
- 改 API handler 时是否增加/保留 logging。
- 是否读取项目已有 conventions。
- 是否生成不可观测的 silent failure path。

注意：AgentSight 不必自己判断所有代码质量。它只需要提供 evidence，让 review agent 或 human reviewer 更容易判断。

## 7. Real-World Misalignment Studies Need The Missing Middle

越来越多研究开始从真实用户会话研究 agent failure，而不只是 benchmark。

Example:

- [How Coding Agents Fail Their Users](https://arxiv.org/abs/2605.29442) 分析 20,574 个真实 coding-agent sessions，并把 misalignment 定义为通过 developer pushback 暴露出来的 breakdown。论文关注 form、cause、cost、resolution 等维度。

这类研究非常接近产品痛点，因为它不是看模型 benchmark，而是看用户什么时候被迫纠正 agent。

但这里仍然缺一个 “missing middle”：

- 有用户 pushback。
- 有 agent 的文本/IDE/CLI session。
- 可能有最终代码变化。
- 但缺少 agent 在真实机器上做过什么的完整行为证据。

对 misalignment 研究来说，最关键的数据可能是：

- 用户第一次纠正 agent 之前，agent 触碰了哪些文件？
- agent 是否越过了用户指定范围？
- agent 是否反复读取无关文件，导致成本上升？
- agent 是否执行了用户没要求的命令？
- agent 是否声称完成但没有跑验证命令？
- agent 是否在被纠正后继续重复同类错误？

AgentSight 可以把 developer pushback 和 observed side effects 对齐：

```json
{
  "user_pushback": "不要改这个目录，我只让你修 test",
  "prior_observed_effects": {
    "changed_paths": ["src/runtime/config.ts", "tests/api.test.ts"],
    "out_of_scope_paths": ["src/runtime/config.ts"],
    "commands": ["npm test"],
    "repeated_reads": ["src/runtime/config.ts"]
  }
}
```

产品价值：

> 这支持 permission tuning、scope enforcement、review receipt，而不是只支持事后审计。

## 8. MCP / Skill / Plugin Security Research Needs Dynamic Behavior Datasets

MCP、skills、plugins 正在成为 agent 的供应链入口。学术和安全研究都在试图收集攻击样本和行为结果。

Examples:

- [MCP-ITP](https://arxiv.org/abs/2601.07395) 研究 MCP implicit tool poisoning。
- [MCPTox](https://arxiv.org/abs/2508.14925) 做 real-world MCP server tool poisoning benchmark。
- [Prompt Injection Attacks on Agentic Coding Assistants](https://arxiv.org/abs/2601.17548) 系统化分析 skills、tools、protocol ecosystems 中的 prompt injection 攻击。
- [Skill-Inject](https://arxiv.org/abs/2602.20156) 聚焦 skill file attacks。
- [MCP Security Best Practices](https://modelcontextprotocol.io/specification/2025-06-18/basic/security_best_practices) 和 [MCP ToolAnnotations](https://modelcontextprotocol.io/specification/2025-11-25/schema) 也说明，tool metadata/annotations 是提示性声明，不等于真实行为保证。

这里研究者真正想采集的是：

- 安装一个 MCP/skill/plugin 后，agent 会不会被 tool metadata 或 skill markdown 诱导？
- 它是否读取 secret？
- 是否越过 workspace？
- 是否 exfiltrate 到外部网络？
- 是否做 destructive actions？
- 是否通过多个工具组合完成单个 tool 看不出的越权？
- benign-looking update 是否造成 rug pull 行为变化？

难点：

- 静态代码审计只能看一部分风险。
- Tool annotation / README / manifest 声明不能证明运行时行为。
- 攻击可能发生在 agent context、tool response、skill file、hidden text、多工具组合中。
- 需要真实 agent 执行后的 side-effect evidence 才能判断攻击是否成功。

AgentSight 的强场景：

```bash
agentsight verify \
  --fixture fixtures/mcp-readonly.yaml \
  --policy policies/no-secret-no-network.yaml \
  -- claude

agentsight verify \
  --fixture fixtures/skill-injection.yaml \
  --policy policies/workspace-local-readonly.yaml \
  -- codex
```

输出不是“安全评分”，而是 evidence：

- attempted secret reads。
- actual secret reads。
- attempted network egress。
- actual network egress。
- file writes/deletes。
- tool chain that caused the event。
- policy boundary crossed or not。

产品价值：

> MCP/skill/plugin 的真正购买/采用场景不是“我想看 dashboard”，而是“这个工具能不能安装、上架、进入 allowlist、给 agent 自动调用”。

## 9. What Researchers Are Repeatedly Trying To Collect

把上面的研究放在一起，可以抽象出一个通用数据包：

### Agent Run Evidence Bundle

每次 agent run 最小应该包含：

1. **Session identity**

   Agent product、model、version、repo、workspace、OS、time、policy mode。

2. **User intent and constraints**

   用户任务、明确禁止项、scope、permission mode、trusted folder/workspace。

3. **Agent intent trace**

   LLM messages、tool calls、planned actions、self-reported claims。能采多少采多少，但要标注来源和可信度。

4. **Tool invocation trace**

   Tool name、arguments、result、error、duration、retry、parent event。

5. **Process trace**

   Shell command、argv、cwd、exit status、child processes、duration。

6. **File side effects**

   Create、write、truncate、delete、rename、chmod/chown、repo 内外分类、sensitive path flag。

7. **Network side effects**

   Destination host/IP/port、protocol category、process owner、package registry / cloud API / unknown。

8. **Dependency side effects**

   Package manager commands、manifest/lockfile changes、registry fetch、build artifacts、runtime load hints。

9. **Permission and approval events**

   User approval、auto approval、deny、bypass、policy decision、risk reason。

10. **Outcome and verification**

   Tests/build/lint、claimed vs observed、expected side effects、missing side effects。

11. **Human intervention**

   User correction、manual edit、reviewer feedback、checkpoint restore、rerun.

12. **Recovery context**

   Changed path inventory、deleted path inventory、destructive operation list、undo hints、evidence pointers。

研究者现在分别从 PR、trace、issue、benchmark、manual annotation 中拼这些数据。AgentSight 的机会是把它变成一次采集。

## 10. Why Existing Data Sources Are Not Enough

### PR data is late and lossy

PR 数据容易收集，所以研究者喜欢用。但 PR 只保留结果和 review 交互，不保留 agent 的真实系统操作。

### Agent logs are product-specific and self-reported

Claude Code、Codex、Cursor、Gemini CLI、MCP server、IDE extension 都有不同日志格式。有些日志来自 agent 自身，不是独立观测。

### Manual trace annotation does not scale

TRAIL、MAST 这类研究证明 taxonomy 有价值，但也说明人工读长 trace 的成本很高。

### Security benchmarks often need runtime truth

Prompt injection / MCP poisoning 的真正问题不是模型输出里有没有危险想法，而是 agent 是否真的读了 secret、发了网络、删了文件、改了配置。

### Reproducibility requires dynamic execution

Manifest、README、final diff 都不能证明代码能在 clean environment 跑起来。需要记录 runtime dependency 和 execution evidence。

### Dashboards do not solve dataset creation

研究者和 AI agent 需要 schema、JSONL、query API、可复跑 fixture，不是只能人工看的图。

## 11. Product Implications For AgentSight

### Do Not Lead With "Academic Tool"

学术用户会帮助定义 schema 和 benchmark，但不是最好的第一付费市场。更好的定位是：

> AgentSight is a run evidence collector for agents operating on real systems.

学术场景是验证它是否真的收到了别人收不到的数据。

### Lead With Dataset-Grade CLI

最小产品应优先是 CLI + JSON/Markdown/HTML artifact：

```bash
agentsight record --label task-001 -- claude
agentsight report export -o task-001.json
agentsight report --label task-001 --format markdown
agentsight query --label task-001 --json "what changed outside the repo?"
agentsight diff --baseline codex-task-001 --candidate claude-task-001 --json
```

这比 dashboard 更重要，因为：

- 研究者可以直接构建 dataset。
- AI agent 可以直接消费 JSON。
- CI 可以保存 artifact。
- Reviewer 可以读 Markdown。
- Human deep inspection 再打开 HTML。

### Add A "Research Pack" Later

不是第一天就做，但可以作为增长/可信度路线：

```bash
agentsight dataset init --name permission-gate-study
agentsight dataset add --label task-001
agentsight dataset export --format parquet
agentsight dataset redact --policy privacy.yaml
agentsight dataset schema --print
```

Research pack 的价值：

- 标准 schema。
- Redaction。
- Fixture runner。
- Cross-agent comparison。
- Artifact export。
- Paper-friendly appendix generation。

但它不应该变成主产品 UI。

### Make Evidence AI-Readable

因为很多分析可以让 agent 自己做，所以 AgentSight 的输出必须适合 AI 使用：

- Stable IDs。
- JSON schema version。
- Evidence pointers。
- Risk flags。
- Path categories。
- Causal links。
- Query API。
- Redaction metadata。

一个 review agent 应该能这样用：

```bash
agentsight query --json "did this run touch secrets or files outside the repo?"
agentsight query --json "which command first modified package-lock.json?"
agentsight query --json "did the agent run tests after changing backend code?"
agentsight query --json "what did the agent claim that was not observed?"
```

## 12. High-Value Academic Pain Scenarios

这些场景和用户场景直接相关，值得作为 AgentSight 的产品验证实验。

### Scenario A: Agentic PR Process Evidence

User / researcher question:

> Agentic PR 被拒绝，是代码不行，还是 reviewer 看不到过程、不信任它？

Data needed:

- PR diff。
- Agent run receipt。
- Commands/tests run。
- Files touched outside diff。
- Permission/risk events。
- Reviewer feedback。

AgentSight value:

- 把 PR 研究从 outcome-level 提升到 process-level。
- 对真实团队也有用：减少 reviewer 对 agent PR 的盲猜。

### Scenario B: Permission Gate Coverage

Question:

> Auto mode / permission mode 漏掉了哪些实际危险动作？

Data needed:

- Prompt scope。
- Approval decisions。
- Observed state changes。
- Equivalent dangerous effects。
- Cross-agent comparison。

AgentSight value:

- 用真实 side effects 评估 permission，而不是只看 classifier 文字判断。

### Scenario C: MCP / Skill Dynamic Verification

Question:

> 一个 MCP/skill/plugin 声称 read-only，但真实 agent 使用时是否越权？

Data needed:

- Tool manifest / skill file。
- Agent context。
- Tool invocation chain。
- File/network/secret side effects。
- Policy verdict。

AgentSight value:

- 这是最明确的“代码有价值”场景，因为用户自己的 agent 无法独立证明自己没有被工具诱导。

### Scenario D: Recovery And Destructive Operation Dataset

Question:

> Agent 把系统搞坏后，哪些证据能帮助恢复？

Data needed:

- Destructive operations。
- Deleted/overwritten paths。
- Process lineage。
- Git/tracked/untracked classification。
- Checkpoint/backup availability。
- Suggested recovery steps。

AgentSight value:

- 这不是泛泛审计，是用户强痛点：出事以后要恢复。

### Scenario E: Dependency Reproducibility Profiling

Question:

> Agent 生成的项目为什么在别人机器上跑不起来？

Data needed:

- Declared dependencies。
- Installed dependencies。
- Runtime dependencies。
- Network/package fetch。
- Build/test outcomes。

AgentSight value:

- 采集动态依赖 side effects，补 Git diff 和 README 的不足。

### Scenario F: No-Progress / Cost Waste Trace

Question:

> Agent 为什么花了大量 token/tool calls 但没有产生有效进展？

Data needed:

- Token/cost。
- Repeated file reads。
- Repeated failed commands。
- No new side effects window。
- Claimed progress vs observed changes。

AgentSight value:

- 让用户和研究者衡量“工作量”而不只是“调用量”。

### Scenario G: Human Correction Alignment

Question:

> 用户什么时候打断 agent？打断前 agent 做了什么？

Data needed:

- User correction。
- Preceding side effects。
- Scope boundary。
- Repeated mistakes。
- Resolution path。

AgentSight value:

- 支持研究 developer-agent misalignment，也支持产品里的 permission tuning。

## 13. What Is Truly Valuable, And What Is Code Burden

有价值的代码：

- Cross-agent capture runner。
- OS side-effect collector。
- Process/file/network correlation。
- Stable JSON schema。
- Query CLI。
- Redaction。
- Artifact export。
- Policy/evidence verifier。
- CI/GitHub Action wrapper。
- MCP wrapper for evidence queries。

可能是负担的代码：

- 复杂 dashboard first。
- 自己重写所有 LLM observability。
- 自己做完整 agent debugger。
- 自己做静态安全扫描器。
- 自己做大而全 SIEM/EDR。
- 自己写太多业务分析规则，而不是把 evidence 给用户/agent 查询。
- 做很多 agent 已经能通过 shell 简单完成的清理、整理、总结功能。

产品判断：

> AgentSight 应该少做“分析结论生成器”，多做“可信行为数据生产器”。结论可以由用户、reviewer、另一个 agent、研究脚本、CI policy 来消费。

## 14. Concrete Validation Plan

### 30-day validation

1. 选择 20 个真实 coding-agent sessions。
2. 用 AgentSight 采集 process/file/network evidence。
3. 输出 JSON + Markdown receipt。
4. 请用户或 review agent 回答：
   - 没有 AgentSight 时，你能不能知道这些信息？
   - 哪些信息改变了你是否信任这个 agent run？
   - 哪些信息能帮助你恢复或 review？

### Academic-style validation

1. 复刻一个小型 Agentic PR study。
2. 每个 PR 附 AgentSight run receipt。
3. 比较“只看 PR/diff/review”和“加 run receipt”时，rejection/diagnosis taxonomy 是否更准确。

### Security/tool validation

1. 选择 10 个 MCP/skill/plugin fixture。
2. 每个声明一个 policy，例如 read-only/no-network/workspace-local。
3. 让 Claude Code/Codex/Gemini CLI 跑同一 fixture。
4. AgentSight 输出 observed behavior。
5. 看是否能发现 manifest/schema/README 看不出的越权。

### Reproducibility validation

1. 让不同 agent 生成相同项目。
2. AgentSight 记录 dependency side effects。
3. Clean container 复跑。
4. 对比 claimed/working/runtime dependency gap。

## Bottom Line

学术界正在用各种方式收集 agent 行为数据：

- PR dataset。
- Review rationale。
- Execution trace。
- Failure taxonomy。
- Permission stress test。
- Bug issue taxonomy。
- Dependency reproducibility evidence。
- MCP/tool poisoning benchmark。
- Real-world session misalignment。

这些研究共同暴露的缺口是：

> 没有一个通用、跨 agent、独立于 agent 自述的真实系统行为采集层。

这正是 AgentSight 可以有产品价值的地方。

但方向要窄：

> 不要做“更复杂的 agent 软件”。做 agent 操作真实系统时的 evidence substrate，让人、CI、reviewer、researcher、另一个 agent 都能基于同一份事实工作。
