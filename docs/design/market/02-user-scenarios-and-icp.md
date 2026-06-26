# AgentSight 用户场景、ICP 与购买/安装动机调研

调研日期：2026-06-02
范围：个人本地 coding agents、团队 PR review、skill/MCP/plugin 审核、企业 agent access 治理、AI 产品行为回归测试、token/runner 成本治理。
结论口径：本文明确区分“市场事实”“推断”“待验证假设”。论坛单帖只作为痛点存在的证据，不单独外推为整体市场规模。

## 一页结论

AgentSight 最强的市场定位不是“又一个 agent 自动化工具”，而是“agent 行为证据层”：在用户已经让 Claude Code、Codex、Cursor、Gemini CLI、Copilot cloud agent 或 MCP 工具改代码、跑命令、读文件、访问网络时，AgentSight 提供独立于 agent 自述、IDE 日志和最终 git diff 的运行收据、审计证据和行为回归基线。

优先 ICP：

| 优先级 | ICP | 购买/安装动机 | 市场信心 |
| --- | --- | --- | --- |
| P0 | 有多人 review agent-generated PR 的工程团队 | reviewer 不只要看 diff，还要知道 agent 跑过什么命令、读过什么文件、是否测试失败后重试、是否访问外部服务 | 高 |
| P0 | 企业安全/平台团队 | 需要统一治理本地/云端 coding agents 的权限、日志、敏感文件访问和成本，而不是依赖每个 agent 自己的 hook/permission 实现 | 高 |
| P0 | 成本/资源管理者 | agentic coding token 消耗高、波动大，循环和上下文膨胀会造成真实账单/额度损失 | 高 |
| P1 | skill/MCP/plugin 作者、registry/marketplace reviewer | 静态扫描不能证明工具在真实 agent run 中是否只读、是否越权、是否跨工具诱导 | 中高 |
| P1 | AI 产品/agent 团队 | 现有 eval 多看输出或 tool trace，缺少“系统 footprint 是否变化”的 regression signal | 中高 |
| P2 | 个人开发者 | 痛点真实，但付费能力和持续留存弱；适合作为开源入口、口碑入口、bug-report artifact 入口 | 中 |

AgentSight 的独特价值应表述为：

- 证明 agent 实际做了什么：LLM API 请求/响应、工具/命令、进程树、文件读写、网络目标、CPU/内存、token/cost。
- 证明 agent 是否遵守边界：只读、项目内写入、禁止访问 `.env`/SSH key、禁止生产 API token、禁止外部网络、禁止 destructive command。
- 证明变化：同一任务在不同 agent、模型、prompt、skill、MCP server 版本下的行为差异。
- 证明给别人看：把证据导出为 PR 附件、marketplace 验收报告、CI artifact、security audit bundle。

## 共同市场事实

### AI coding/agent 使用已经足够普遍，但信任不足

市场事实：

- Stack Overflow 2025 Developer Survey 的 AI 分页显示，开发者对 AI 工具准确性的“不信任”比例高于“信任”比例：46% actively distrust，33% trust，只有 3% highly trust。最大挫折是“almost right, but not quite”，占 66%，其次是“debugging AI-generated code is more time-consuming”，占 45.2%。链接：[Stack Overflow 2025 AI](https://survey.stackoverflow.co/2025/ai)。
- 同一调查显示，使用 AI agents 工作的软件开发者中，84% 用于软件开发任务；ChatGPT 和 GitHub Copilot 是主要入口。链接同上。
- Codex CLI 官方文档明确：Codex 可在本地终端读取、修改并运行所选目录中的代码。链接：[OpenAI Codex CLI](https://developers.openai.com/codex/cli)。
- Claude Code 官方安全文档明确：Claude Code 有权限系统、sandboxed bash、写入边界、prompt injection 防护，但同时声明用户负责审查命令和代码安全。链接：[Claude Code Security](https://code.claude.com/docs/en/security)。
- Gemini CLI 有 Trusted Folders，未信任目录会禁用 workspace settings、`.env` 加载、extension management、tool auto-acceptance 和自动 memory loading。链接：[Gemini CLI Trusted Folders](https://google-gemini.github.io/gemini-cli/docs/cli/trusted-folders.html)。
- Cursor 社区公开 bug 报告里，有用户报告 agent 在概念讨论中删除主 Python 文件；Cursor 工作人员回复称在某些 Auto-Run/File-Deletion Protection 配置下“agent can delete files without asking”，并建议打开 File-Deletion Protection。链接：[Cursor forum deletion report](https://forum.cursor.com/t/critical-agent-deleted-entire-file-without-permission-and-tried-to-hide-it/155098)。

推断：

- 市场不是缺少“让 agent 写代码”的工具，而是缺少让开发者、reviewer、安全团队相信 agent 行为边界的独立证据。
- 只看最终 diff 不足以建立信任，因为风险经常发生在中间过程：读了什么、跑了什么、访问了什么 token、失败重试了多少次、是否尝试过越权路径。

待验证假设：

- 如果 AgentSight 能把一次 agent run 自动生成成短报告，并附带完整证据包，个人开发者愿意为了“可回溯、可复盘、可发 bug report”安装。
- 如果 AgentSight 能把证据嵌入 PR 或 CI artifact，团队愿意把它作为 agent-generated PR 的准入门槛。

## 现有替代方案与共同不足

| 替代方案 | 用户现在怎么用 | 不足 |
| --- | --- | --- |
| `git diff` / PR diff | 看最终文件变化 | 不显示读取过的文件、命令、未跟踪文件、失败尝试、网络访问、secret access、token spend |
| shell history / terminal transcript | 查命令行输入输出 | 子进程、脚本内部行为、agent tool call、TLS API payload、文件读写不完整 |
| agent 自带 transcript | Claude/Codex/Cursor/Gemini 会保存会话或显示 tool calls | agent 自己是被审计对象；日志粒度、隐私、保存策略和字段因工具而异；不能跨工具统一 |
| IDE checkpoint / local history | 恢复被删文件 | 偏恢复，不偏证明；不能解释为什么删除、是否还读了密钥或访问外部服务 |
| permission/hook/sandbox | 每个 agent 自己限制工具 | 规则分散，常需要人工配置；无法证明实际运行未越权；某些 hook 本身非阻塞或依赖 agent/host 实现 |
| LangSmith/Langfuse/Phoenix/Helicone/OpenTelemetry GenAI | 对自建 LLM app 做 traces、evals、cost、latency | 通常需要 SDK、proxy 或框架接入；对闭源 CLI、本地子进程、文件系统、shell、secret access 视野不足 |
| EDR/SIEM/审计日志 | 企业安全团队看进程和网络 | 通常看不到 LLM prompt/response、agent tool semantic、token spend，也难以把低层事件归因到某次 agent turn |
| CI logs | 看测试/构建结果 | 只覆盖 CI 内显式命令；不覆盖开发者本地 agent run；不解释 agent 为什么运行这些命令 |

AgentSight 的差异化应强调“system-boundary evidence”：不要求 agent、framework 或用户代码配合，直接从系统边界记录 LLM 流量、进程、文件、网络、资源，并与 agent session 关联。

## 场景 1：个人开发者使用 Claude Code/Codex/Cursor/Gemini CLI 改代码

### 市场事实

- Codex CLI 官方文档说明它能在本地终端读取、修改、运行代码。链接：[OpenAI Codex CLI](https://developers.openai.com/codex/cli)。
- Claude Code 安全文档说明其写权限默认限于启动目录及子目录，敏感操作需显式批准，同时“Claude Code only has the permissions you grant it”。链接：[Claude Code Security](https://code.claude.com/docs/en/security)。
- Claude Code 的 `.claude` 文档说明，工具经过的内容会落到本地 transcript，且 transcript/history 默认不加密，OS 文件权限是主要保护。链接：[Explore the .claude directory](https://code.claude.com/docs/en/claude-directory)。
- Gemini CLI issue 中有用户报告 agent 进入文件读取循环，2 小时内 context 从 84% 剩余降到 44%，并期待有 looped file analysis cap。链接：[google-gemini/gemini-cli#2923](https://github.com/google-gemini/gemini-cli/issues/2923)。
- Gemini CLI issue 中有用户报告每 turn 重新发送 system prompt、tool definitions 和完整 history，导致 token usage 指数级增长，且用 mitmproxy 验证 raw API requests。链接：[google-gemini/gemini-cli#3784](https://github.com/google-gemini/gemini-cli/issues/3784)。
- Gemini CLI issue 中有用户报告模型质量下降和重复 loop，称大多数 agentic sessions 因 loop 失败。链接：[google-gemini/gemini-cli#5273](https://github.com/google-gemini/gemini-cli/issues/5273)。
- Cursor forum 有真实用户报告 agent 删除文件；Cursor 工作人员确认在特定配置下无确认删除是已知问题。链接：[Cursor deletion report](https://forum.cursor.com/t/critical-agent-deleted-entire-file-without-permission-and-tried-to-hide-it/155098)。

### 推断

个人开发者的核心痛点不是“我不知道 agent 能不能写代码”，而是“我需要事后知道它到底做了什么”。尤其当 agent 会运行 shell、读取上下文、触发 tool、自动重试、读写文件时，用户需要一个独立的运行收据来回答：

- 它改了哪些文件，是否还读了敏感文件？
- 它跑了哪些命令，是否有 destructive command？
- 它为什么花了这么多 token？
- 它是否在循环、重复 grep、重复读同一个文件？
- 它是否访问了外部网络或上传了内容？

### 场景细分

| 维度 | 内容 |
| --- | --- |
| 用户 | 高频使用 Claude Code、Codex CLI、Cursor Agent、Gemini CLI 的个人开发者、solo founder、开源维护者 |
| 触发时刻 | agent 修改代码后结果不符合预期；文件丢失；token 异常；准备提交前；想向工具厂商提交 bug report；想复盘一次长 session |
| 现在替代方案 | git diff、IDE local history/checkpoint、agent transcript、shell history、手动查看 usage、`/cost` 或 `/usage` |
| 替代方案不足 | git diff 不显示中间行为；agent transcript 不统一且不是独立证据；shell history 缺子进程/文件读写/网络；usage 只给总量不解释原因 |
| AgentSight 独特证据 | 一次 run 的进程树、命令、文件 read/write/delete、网络目标、LLM 请求/响应、token 变化、CPU/内存、循环迹象；可导出为 bug report |
| 是否可能付费/安装 | 高概率安装；个人付费中低。更适合作为开源 CLI、免费本地报告、团队升级入口 |
| 市场信心等级 | 中高：痛点公开且多源存在，但个人用户付费弱，且一部分用户会接受“多用 git/备份/少开 auto-run”作为替代 |
| 最小可验证实验 | 发布 `agentsight exec -- claude/codex/gemini/cursor-agent` 生成 `run-receipt.html`。招募 20 个高频个人用户，在真实 coding session 后问两个指标：是否发现 git diff 看不到的信息；是否愿意把报告贴到 issue/PR。成功阈值：10 人以上每周保留使用，5 人以上主动分享报告 |

### 待验证假设

- 个人开发者最想要的是“短摘要 + 可展开证据”，不是完整 observability UI。
- Linux/eBPF 前提会限制个人市场。需要验证 Docker/WSL/CI 入口是否能覆盖足够多的目标用户。
- 用户愿意让 AgentSight 捕获 LLM payload 的前提是默认本地存储、可脱敏、可选择只记录 metadata。

## 场景 2：团队 reviewer 审查 agent-generated PR

### 市场事实

- GitHub Copilot cloud agent 官方 responsible use 文档说明它是集成在 GitHub 的 autonomous/asynchronous software development agent，可从 issue 或 Copilot Chat 接任务、研究 repo、制定计划、改分支代码，用户 review diff、iterate，并准备 PR。链接：[GitHub Copilot cloud agent responsible use](https://docs.github.com/en/enterprise-cloud@latest/copilot/responsible-use/copilot-cloud-agent)。
- GitHub Copilot cloud agent 概念文档说明 Business/Enterprise 订阅需要管理员启用 policy，repository owners 可以对 repo opt out。链接：[About GitHub Copilot cloud agent](https://docs.github.com/en/enterprise-cloud@latest/copilot/concepts/agents/cloud-agent/about-cloud-agent)。
- Claude Code Review 官方文档说明它面向 Team/Enterprise research preview，可分析 GitHub PR、用多 agent 看 logic errors、security vulnerabilities、edge cases、regressions，并在 PR inline comments 中给 findings；这说明“AI review agent”已经成为正式产品形态。链接：[Claude Code Review](https://code.claude.com/docs/en/code-review)。
- AIDev 论文收集了 932,791 个由 OpenAI Codex、Devin、GitHub Copilot、Cursor、Claude Code 产生的 Agentic-PRs，覆盖 116,211 个仓库和 72,189 个开发者。链接：[AIDev: Studying AI Coding Agents on GitHub](https://arxiv.org/abs/2602.09185)。
- “Why Agentic-PRs Get Rejected” 论文检查 654 个被拒绝的 agentic PR，发现 7 类只出现在 Agentic-PRs 的拒绝模式，其中包括 distrust of AI-generated code；67.9% 被拒 PR 缺少明确 reviewer feedback，导致拒绝原因难以判断。链接：[Why Agentic-PRs Get Rejected](https://arxiv.org/abs/2602.04226)。
- Hacker News 上有 Ask HN 明确询问如何管理 coding agents 带来的 PR review fatigue，发帖者称 agents 产生 massive volume of PR，review 需要 deep cognitive effort。链接：[Ask HN: PR review fatigue from coding agents](https://news.ycombinator.com/item?id=47418016)。
- “AI Slop in Software Development” 研究分析 Reddit/Hacker News 1,154 个帖子，主题包括 review friction、trust erosion、quality degradation。链接：[AI Slop in Software Development](https://arxiv.org/abs/2603.27249)。

### 推断

团队 reviewer 的痛点不是“PR 太多”本身，而是“生成成本低、验证成本高、责任落在 reviewer”。Agent-generated PR 的风险常常不只在 diff：

- agent 是否运行过测试，测试失败几次后才通过？
- 是否下载外部脚本或跑 package manager？
- 是否读取私有配置、`.env`、SSH key、cloud token？
- 是否改过未提交文件、生成临时 artifacts、污染 repo 状态？
- PR 描述是否和真实行为一致？

### 场景细分

| 维度 | 内容 |
| --- | --- |
| 用户 | senior engineer、team lead、maintainer、staff reviewer、平台团队中负责 agent workflow 的 reviewer |
| 触发时刻 | 收到由 Claude/Codex/Cursor/Copilot/Devin 创建或辅助创建的 PR；reviewer 没观察 agent run；PR 很大或跨模块；生产/安全相关改动 |
| 现在替代方案 | PR diff、commit history、CI logs、Copilot/Claude/CodeRabbit 等 AI code review、要求作者贴测试日志、人工追问 |
| 替代方案不足 | PR diff 看不到行为过程；CI 只看声明的步骤；AI reviewer 仍是在看 diff/code，不证明 agent 过程；作者自述难以审计 |
| AgentSight 独特证据 | PR 附件式 run receipt：本次 agent run 的命令、测试、失败重试、文件 read/write/delete、外部网络、敏感路径访问、token spend、工作目录外行为；可标记“diff 未覆盖的行为” |
| 是否可能付费/安装 | 团队付费可能高，尤其当 reviewer 已经抱怨 review fatigue、维护开源项目或有合规要求。安装形态应是 CLI + GitHub Action/PR comment |
| 市场信心等级 | 高：真实 PR 数据、论文、官方产品和社区讨论都指向 reviewer trust gap |
| 最小可验证实验 | 在 3 个团队试点：要求 agent-generated PR 附带 AgentSight report。测量 reviewer 首轮 review 时间、追问次数、阻塞原因、被发现的过程风险。成功阈值：reviewer 主观信心提升 30% 以上，至少 20% PR 报告包含 diff/CI 看不到的信息 |

### 待验证假设

- reviewer 愿意看“证据摘要”，但不会阅读完整 timeline；需要自动生成 5-10 条高信号 risk flags。
- PR 报告必须能在不泄露 prompt/secret 的情况下分享，否则企业不会默认开启。
- 团队愿意把 AgentSight 作为“AI-generated PR policy”的一部分，例如：没有 run receipt 的 agent PR 不能进入 review。

## 场景 3：skill/MCP/plugin 作者或 marketplace reviewer 做审核验收

### 市场事实

- MCP 官方 security best practices 文档明确目标读者包括 MCP authorization 实现者、MCP server operators 和 security professionals evaluating MCP-based systems，并列出 attack vectors 和 mitigations。链接：[MCP Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)。
- OWASP 将 MCP Tool Poisoning 定义为针对连接外部工具服务器的 AI agents 的 indirect prompt injection attack。链接：[OWASP MCP Tool Poisoning](https://owasp.org/www-community/attacks/MCP_Tool_Poisoning)。
- OWASP MCP Security Cheat Sheet 指出 MCP 让 LLM 决定调用哪些工具、何时调用、带什么参数，形成结合 prompt injection、supply chain attacks 和 confused deputy 的新安全风险。链接：[OWASP MCP Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/MCP_Security_Cheat_Sheet.html)。
- MCP 大规模研究评估 1,899 个开源 MCP servers，发现 7.2% 包含一般漏洞，5.5% 存在 MCP-specific tool poisoning，并建议 registry 引入自动安全扫描和更强 governance。链接：[Model Context Protocol at First Glance](https://arxiv.org/abs/2506.13538)。
- GitHub 文档也把 MCP server 与 secret scanning、Copilot agent mode 等结合，说明 MCP 已进入主流开发者工作流。链接：[GitHub MCP server secret scanning](https://docs.github.com/en/code-security/how-tos/use-ghas-with-ai-coding-agents/scan-for-secrets-with-github-mcp-server?tool=cli)。

### 推断

skill/MCP/plugin 审核的特殊点在于：静态检查能发现一部分代码漏洞和恶意 schema，但不能证明“一个真实 agent 在真实上下文里会不会被诱导跨工具、越权读取、发出外部请求”。Marketplace reviewer 需要的是验收证据：

- 工具声称 read-only，是否真的没有写文件或执行 destructive command？
- 工具声称只访问某 API，是否还访问其他域名？
- 工具描述是否诱导模型调用另一个 server 的工具？
- 安装或运行时是否读取 `.env`、SSH key、browser tokens？
- skill 是否改变 agent 行为，导致更大 token spend 或更多危险命令？

### 场景细分

| 维度 | 内容 |
| --- | --- |
| 用户 | MCP server 作者、Claude/Codex skill 作者、plugin 作者、开源 registry/marketplace reviewer、企业内部 agent tool catalog 管理者 |
| 触发时刻 | 发布前审核；marketplace 上架；企业引入第三方 MCP server；更新 skill/plugin 版本；处理安全报告 |
| 现在替代方案 | 静态代码扫描、依赖扫描、手动读 manifest/tool schema、MCP-specific scanner、人工 sandbox 试跑、README 声明 |
| 替代方案不足 | 静态扫描不覆盖真实 agent decision；manifest 声明不可验证；人工试跑不可复现；scanner 不一定看到 runtime 文件/网络/LLM payload |
| AgentSight 独特证据 | 动态验收报告：声明行为 vs 观测行为、是否只读、写入路径、网络域名、secret 文件访问、子进程、tool call 到 OS effect 的链路、token/loop 变化 |
| 是否可能付费/安装 | 作者个人付费中低；marketplace、企业 internal registry、consulting/security reviewer 付费中高 |
| 市场信心等级 | 中高：安全痛点强，MCP 生态快速增长；但 buyer 分散，marketplace 审核标准尚未稳定 |
| 最小可验证实验 | 做 `agentsight verify --policy policy.yaml -- claude/codex/gemini ...`，对 20 个流行 MCP servers/skills 生成“read-only/path/network/secret”验收报告。把报告提交给作者或 registry，观察是否被采用为 release artifact。成功阈值：5 个项目愿意把报告链接进 README/release，1 个 registry 愿意讨论集成 |

### 待验证假设

- Marketplace reviewer 的核心需求是“可复现审核模板”，不是通用 timeline。
- 动态运行审核需要标准化测试任务，否则不同 reviewer 的结果不可比。
- 企业内部 tool catalog 会比公开 marketplace 更早付费，因为它们有明确的安全 owner 和 onboarding 流程。

## 场景 4：企业安全/平台团队管理 agent access

### 市场事实

- Codex Enterprise Admin Setup 官方文档要求企业 rollout 时确定 workspace owner、security owner、analytics owner；支持 granular user access controls、audit logging、local/cloud surfaces、RBAC、Team Config，并可配置 sandbox、approvals、rules、skills。链接：[Codex Admin Setup](https://developers.openai.com/codex/enterprise/admin-setup)。
- Claude Code hooks 文档支持 command、HTTP、MCP tool、prompt 和 agent hooks；hook 可以对 permission request 给出 allow/deny，说明企业会在 agent action 前后接入控制逻辑。链接：[Claude Code Hooks](https://code.claude.com/docs/en/hooks)。
- Claude Code security 文档声明有 permission system、command blocklist、network request approval、trust verification 等防护。链接：[Claude Code Security](https://code.claude.com/docs/en/security)。
- GitHub Copilot cloud agent 文档要求 Business/Enterprise 管理员启用 policy，repo owner 可 opt out，并在 responsible use 文档中强调 mitigations 存在但仍需理解 limitations 和持续安全最佳实践。链接：[GitHub Copilot cloud agent](https://docs.github.com/en/enterprise-cloud@latest/copilot/concepts/agents/cloud-agent/about-cloud-agent) 与 [Responsible use](https://docs.github.com/en/enterprise-cloud@latest/copilot/responsible-use/copilot-cloud-agent)。
- 公开报道中，Cursor + Claude agent 曾因访问到不相关文件里的 API token 并执行 destructive cloud action，导致 PocketOS 生产数据库和备份被删除。该报道还强调 agent 有搜索文件、写代码、使用登录密钥、调用外部服务的能力。链接：[Live Science PocketOS incident](https://www.livescience.com/technology/artificial-intelligence/i-violated-every-principle-given-ai-agent-deletes-companys-entire-database-in-9-seconds-then-confesses)。

### 推断

企业安全/平台团队的核心问题不是“是否允许 agent”，而是“如何把 agent 纳入现有安全治理”。他们需要跨工具、跨本地/云端、跨 IDE/CLI 的证据：

- 谁运行了哪个 agent，在什么 repo/host/容器里？
- agent 是否读了 secrets、生产 token、客户数据、本地 SSH key？
- agent 是否调用外部网络或 cloud CLI？
- 它是否遵守组织 policy，而不是只遵守某个工具自己的默认设置？
- 发生 incident 后，能否在不依赖 agent 自述的情况下复盘？

### 场景细分

| 维度 | 内容 |
| --- | --- |
| 用户 | CISO/AppSec、DevSecOps、平台工程、AI enablement team、enterprise developer tooling owner |
| 触发时刻 | 企业试点 Claude/Codex/Cursor/Copilot agent；制定 AI coding policy；出现 agent incident；引入 MCP/tool registry；审计本地开发机和 CI runner |
| 现在替代方案 | vendor admin console、agent permission config、Claude hooks、Codex Team Config、GitHub policy、EDR/SIEM、DLP、cloud audit logs |
| 替代方案不足 | vendor 控制面分散；EDR 看不到 prompt/tool semantics；agent hooks 依赖工具实现；cloud audit 不能归因到本地 agent turn；无法统一比较 Claude/Codex/Cursor/Gemini |
| AgentSight 独特证据 | 独立运行证据：agent session 到 OS event 的关联、敏感文件读取、network/API domain、cloud CLI command、token/cost、policy violation；可导出 SIEM/OTel/JSON |
| 是否可能付费/安装 | 高。企业有安全预算和平台预算，但要求隐私、脱敏、集中管理、策略即代码、部署可控 |
| 市场信心等级 | 高：官方企业治理功能和真实 incident 都说明 buyer 存在；AgentSight 差异在跨工具系统边界证据 |
| 最小可验证实验 | 找 2-3 个正在 rollout coding agents 的团队做 shadow audit：不阻断 agent，只记录 2 周，输出“敏感路径读取、外部网络、destructive commands、policy drift、成本热点”报告。成功阈值：安全/平台 owner 识别出至少 3 个现有控制面看不到的问题，并愿意进入 pilot |

### 待验证假设

- 企业最早可接受的部署形态是“本地/CI 旁路记录 + 脱敏 + 内部存储”，不是 SaaS 上传 prompt。
- 对企业来说，AgentSight 必须先做 evidence/audit，再做 block/enforce；直接阻断会引入开发体验和责任问题。
- 与 SIEM/OTel/Datadog/Splunk 的导出能力，会比漂亮 UI 更影响采购。

## 场景 5：AI 产品团队做 agent 行为回归测试

### 市场事实

- LangSmith evaluation docs 明确覆盖 offline evaluation，包括 benchmarking、unit tests、regression tests、backtesting；也覆盖 online monitoring/anomaly detection。链接：[LangSmith Evaluation Types](https://docs.langchain.com/langsmith/evaluation-types)。
- OpenAI agent evals 文档说明可用 traces、graders、datasets、eval runs 改善 agent quality。链接：[OpenAI Agent Evals](https://developers.openai.com/api/docs/guides/agent-evals)。
- Docker Agent evals 文档把 eval 定义为可保存并 replay 的 conversation，用于追踪 agent 行为随时间变化；强调 evals measure consistency, not correctness。链接：[Docker Agent Evals](https://docs.docker.com/ai/docker-agent/evals/)。
- Reddit r/LLMDevs 的讨论里，有用户描述当前 workflow 是人工 review 100 条 logs、归纳 failure topics、为每类问题建 LLM-as-judge；另有回复指出 manual review at hundreds of requests falls apart，做得好的团队像 CI 一样每次 deploy 跑 eval。链接：[Main observability and evals issues when shipping AI agents](https://www.reddit.com/r/LLMDevs/comments/1rv6kah/main_observability_and_evals_issues_when_shipping/)。

### 推断

AI 产品团队已有 eval/observability 预算，但现有产品主要关注输出质量、prompt/tool trace、LLM-as-judge、数据集回放。AgentSight 的机会在“行为 footprint regression”：

- 新模型是否多读了敏感路径？
- 新 prompt 是否导致更多 shell commands 或 package installs？
- 新 MCP server 是否扩大网络访问面？
- 新 skill 是否让 agent 从 read-only 变成 write/exec？
- 新版本是否 token spend、runtime、CPU、runner cost 激增？

这类问题不是简单的 output correctness，而是“同一任务是否改变了系统行为边界”。

### 场景细分

| 维度 | 内容 |
| --- | --- |
| 用户 | AI agent 产品团队、agent platform team、LLM app QA、agent framework/skill 开发团队、内部 developer-assistant 团队 |
| 触发时刻 | 升级模型；改 system prompt；发布 skill/MCP/plugin；改 tool schema；上线新 guardrail；把 agent 放进 CI/生产 |
| 现在替代方案 | LangSmith/Braintrust/OpenAI Evals/Promptfoo/Docker Agent evals、自建 replay、LLM-as-judge、unit/integration tests |
| 替代方案不足 | 多数 eval 看输出或框架 trace；需要 instrumentation；不一定覆盖闭源 CLI、本地命令、文件系统、网络、资源消耗 |
| AgentSight 独特证据 | old vs new system behavior diff：进程树、命令序列、文件 read/write、网络目标、token/runtime/CPU/mem、敏感路径 access；可作为 CI regression artifact |
| 是否可能付费/安装 | 中高。已有 eval 预算的团队会为“agent behavior regression”付费，但需要与现有 eval stack 集成 |
| 市场信心等级 | 中高：eval 市场确定存在；AgentSight 的系统边界维度需要验证是否足够高频、足够独特 |
| 最小可验证实验 | 构建 `agentsight compare baseline.json current.json --policy regression.yaml`。选 10 个真实 agent tasks，比较两个模型或两个 prompt 版本，输出新增文件访问/命令/网络/成本。成功阈值：AI 产品团队能用报告发现至少 2 个普通 eval 未发现的 regression |

### 待验证假设

- AgentSight 不需要替代 LangSmith/Braintrust，而应作为它们缺失的 system evidence adapter。
- 产品团队愿意把“文件/命令/网络 footprint”纳入 release gate，尤其是有 tool/MCP/terminal access 的 agent。
- 行为 diff 需要可配置噪声过滤，否则正常非确定性会让报告不可用。

## 场景 6：成本/资源管理者关注 token spend、runaway loops、CI/runner 资源

### 市场事实

- Claude Code cost 文档明确：Claude Code 按 API token consumption 计费；企业部署中平均约 $13/developer/active day、$150-250/developer/month，90% 用户低于 $30/active day；并建议 team spend limits、workspace tracking、rate limits、`/usage`、context management。链接：[Claude Code costs](https://code.claude.com/docs/en/costs)。
- 同一文档说明 token cost 随 context size 增长，stale context 会在后续每条消息浪费 token；agent teams 中每个 teammate 都有自己的 context window，token roughly proportional to team size。链接同上。
- Stanford Digital Economy Lab 2026 论文发现 agentic coding tasks 的 token 消耗比 code reasoning/chat 高 1000x；同一任务不同 run token 可差 30x；高 token usage 不等于更高准确率；模型还会低估自己的 token cost。链接：[How Do AI Agents Spend Your Money?](https://digitaleconomy.stanford.edu/publication/how-do-ai-agents-spend-your-money-analyzing-and-predicting-token-consumption-in-agentic-coding-tasks/)。
- Cursor forum 有用户报告 v2.4.x Agent/Thinking Mode 出现 96M tokens spike，cache read 82.9M、actual output 约 800k，官方回复称存在 abnormal spikes，需要 request ID/repro 等排查。链接：[Cursor 96M token loop](https://forum.cursor.com/t/warning-infinite-cache-read-loop-in-v2-4-x-agent-thinking-mode-96m-tokens-spike/151035)。
- Gemini CLI issue 报告重复发送 system prompt、tools 和 history，导致 token cost 指数级增长；另有 issue 报告文件读取 loop 快速消耗 context。链接：[Gemini #3784](https://github.com/google-gemini/gemini-cli/issues/3784)、[Gemini #2923](https://github.com/google-gemini/gemini-cli/issues/2923)。

### 推断

成本管理者关心的不只是 token 总账单，而是“谁、在哪个 repo、哪个 agent、哪个任务、哪个循环导致了消耗”。Agentic coding 的成本风险来自：

- 上下文膨胀：每 turn 重发大 prompt、rules、tools、history。
- 循环：反复读文件、grep、review、thinking，无产出。
- 多 agent/team：每个 subagent 独立 context window。
- CI/runner：agent 在 CI 里反复跑测试、安装依赖、构建失败重试，消耗 runner 时间和云资源。
- 失败不可解释：账单系统只显示 token，不能告诉用户为什么。

### 场景细分

| 维度 | 内容 |
| --- | --- |
| 用户 | engineering manager、platform owner、FinOps、DevOps、AI tooling owner、CI infra owner |
| 触发时刻 | 月度 AI 账单异常；某个 developer 用量异常；agent 卡住；CI runner 被长任务占满；准备扩大 agent rollout |
| 现在替代方案 | provider usage dashboard、Claude Console workspace cost、Cursor usage tab、LiteLLM/Helicone/proxy、CI duration logs、cloud billing |
| 替代方案不足 | provider dashboard 不关联文件/命令/loop；IDE usage 看不到系统资源；proxy 不覆盖闭源 CLI 或本地 subprocess；CI logs 不显示 token payload/cause |
| AgentSight 独特证据 | token spend 与 tool/command/file/network/resource timeline 关联；loop detection；per-run/per-user/per-repo cost attribution；runner CPU/mem/wall-clock；“高 token 低产出”报告 |
| 是否可能付费/安装 | 高。团队和企业会为防止 runaway spend、容量规划、chargeback/showback 付费；个人会安装但不一定付费 |
| 市场信心等级 | 高：成本是官方文档、论文、论坛 issue 都反复出现的明确痛点 |
| 最小可验证实验 | 做 `agentsight report token` + loop/cost report：对 30 次 agent run 输出 token per turn、重复文件访问、重复命令、CPU/mem、runner time。成功阈值：能解释 80% 以上异常用量；至少 3 个团队愿意把报告用于月度 showback 或 CI guardrail |

### 待验证假设

- 成本 buyer 更重视“异常解释”和“预算保护”而不是单纯 dashboard。
- 如果 AgentSight 能在 runaway loop 早期报警或建议 kill/fork/resume，付费意愿会显著提高。
- 对 CI/runner 场景，token 与 CPU/mem/wall-clock 的统一归因比单看 token 更有价值。

## ICP 画像

### ICP-A：AI-enabled engineering team 的 reviewer/platform lead

市场事实：

- Agentic PR 已经大规模出现在 GitHub 数据集中，且 agent-generated code review fatigue 在 HN/Reddit/论文中都有讨论。
- GitHub、Anthropic、OpenAI 都在把 agent/PR/code review 产品化，说明团队工作流是主战场。

采购/安装动机：

- reviewer 不愿为 agent 不透明行为承担责任。
- team lead 想制定 policy：agent PR 必须带证据。
- platform team 想降低 review fatigue，同时保留人类最终责任。

AgentSight 最小产品：

- `agentsight exec -- <agent command>` 生成 report。
- GitHub Action/PR comment 集成。
- 报告只显示高风险摘要，完整 evidence 可下载。

### ICP-B：企业安全/平台团队

市场事实：

- OpenAI Codex 企业文档已经定义 security owner、analytics owner、RBAC、audit logging、Team Config。
- Claude Code/Copilot/Gemini 都有 trust、permission、sandbox、hook 或 admin policy，但实现各不相同。

采购/安装动机：

- 需要跨 agent 的统一审计。
- 需要证明 prompt injection/secret access/destructive command 是否发生。
- 需要把 agent 纳入现有 SIEM、DLP、compliance、incident response。

AgentSight 最小产品：

- 本地/CI 部署。
- policy-as-code。
- JSON/OTel/SIEM export。
- 脱敏和 payload capture controls。

### ICP-C：MCP/skill/plugin reviewer

市场事实：

- MCP 生态已有官方 security best practices、OWASP cheat sheet、tool poisoning 定义和大规模漏洞研究。
- 静态 scanner 与 registry governance 是被公开研究建议的方向。

采购/安装动机：

- 上架前需要动态证据。
- 企业内部 tool catalog 需要验证第三方 server。
- 作者需要证明自己的 tool/skill 没有越权行为。

AgentSight 最小产品：

- `agentsight verify --policy`。
- read-only/path/network/secret checklist。
- 可复现实验脚本和 HTML report。

### ICP-D：AI 产品/agent QA 团队

市场事实：

- LangSmith、OpenAI Evals、Docker Agent Evals 都说明 regression/eval 是成熟需求。
- 社区讨论显示人工 log review 难以规模化。

采购/安装动机：

- 模型/prompt/tool 更新后，需要发现 system behavior drift。
- 现有 eval 不覆盖本地 OS footprint。

AgentSight 最小产品：

- baseline/current 行为 diff。
- CI gate。
- 与 LangSmith/Braintrust/Promptfoo 的 artifact 集成。

### ICP-E：FinOps/DevOps/AI tooling owner

市场事实：

- Claude Code 官方成本文档提供团队 spend/rate limit 指引。
- Stanford 论文和 Cursor/Gemini issue 均证明 token variability/runaway loop 是现实问题。

采购/安装动机：

- 解释账单，防止 runaway，做 showback/chargeback。
- 规划 CI runner 和 agent 并发资源。

AgentSight 最小产品：

- per-run token/resource attribution。
- loop detector。
- monthly report。

## 市场信心分级

| 场景 | 信心 | 原因 |
| --- | --- | --- |
| 个人开发者 run receipt | 中高 | 公开痛点多，但个人付费弱、平台限制强 |
| 团队 agent PR review | 高 | PR 数据、论文、HN/Reddit、官方 review 产品共同证明信任缺口 |
| skill/MCP/plugin 审核 | 中高 | 安全痛点强；buyer/标准仍在形成 |
| 企业 agent access 管理 | 高 | 企业文档已定义 owner、policy、audit；真实 incident 增强紧迫性 |
| AI 产品行为回归测试 | 中高 | eval 市场成熟；AgentSight 需证明 system footprint 是高价值维度 |
| 成本/资源管理 | 高 | 官方成本文档、学术研究、论坛事故都显示明确 ROI |

## 最小验证路线

1. Run receipt MVP
   目标用户：个人开发者、team reviewer。
   输出：`run-receipt.html`，包含命令、文件、网络、token、loop flags、风险摘要。
   验证：报告是否发现 git diff/agent transcript 看不到的信息。

2. PR evidence MVP
   目标用户：团队 reviewer。
   输出：GitHub PR comment + artifact。
   验证：reviewer 是否更快决定 accept/request changes；是否减少“你到底跑了什么”的追问。

3. Policy verify MVP
   目标用户：MCP/skill reviewer、企业安全。
   输出：`policy.yaml` + pass/fail + evidence。
   验证：read-only、path allowlist、network allowlist、secret denylist 是否可执行。

4. Behavior diff MVP
   目标用户：AI 产品 QA。
   输出：baseline vs current diff。
   验证：模型/prompt/tool 更新后能否发现新增系统行为。

5. Cost/loop MVP
   目标用户：FinOps/DevOps。
   输出：per-run/per-user/per-repo token/resource attribution，loop detector。
   验证：能否解释异常账单并提前终止 runaway sessions。

## 产品定位建议

不要把 AgentSight 定位成“让 agent 更会写代码”。这个市场已经有 Claude Code、Codex、Cursor、Gemini CLI、Copilot、Devin 和大量 IDE/CLI agent。

应定位成：

> AgentSight records independent evidence of what coding agents actually did.

中文：

> AgentSight 为 coding agents 提供独立运行证据：它们读了什么、写了什么、跑了什么、连了哪里、花了多少、是否越界。

最有力的产品语言：

- 给个人开发者：agent run receipt。
- 给 reviewer：evidence-backed PR review。
- 给 MCP/skill reviewer：dynamic acceptance report。
- 给安全团队：agent access audit trail。
- 给 AI 产品团队：system behavior regression diff。
- 给成本负责人：token/resource attribution and loop detection。

## 主要风险

1. 平台覆盖风险
   AgentSight 基于 Linux/eBPF 的优势明显，但 Cursor/Claude/Codex 用户大量在 macOS/Windows。需要验证 Docker、WSL、CI、remote dev box 是否足够作为入口。

2. 隐私风险
   捕获 LLM payload 和文件内容会让企业谨慎。必须提供 metadata-only、pattern redaction、secret redaction、本地存储、ZDR/SIEM export 选项。

3. 噪声风险
   agent 行为天然非确定。若报告列出所有事件但不排序风险，reviewer 不会使用。必须默认给短摘要和 policy violation。

4. 竞争风险
   LangSmith/Langfuse/Helicone/Braintrust/OpenAI Evals 会覆盖 LLM app observability；EDR/SIEM 会覆盖企业端系统监控；AgentSight 必须坚守“agent session 到 OS behavior 的关联证据”。

5. 责任边界风险
   一旦提供 block/enforce，用户会期待 AgentSight 防止所有事故。早期应优先做 evidence/audit/report，阻断作为后续 enterprise feature。

## 资料链接索引

- Stack Overflow 2025 AI survey: https://survey.stackoverflow.co/2025/ai
- OpenAI Codex CLI: https://developers.openai.com/codex/cli
- OpenAI Codex Admin Setup: https://developers.openai.com/codex/enterprise/admin-setup
- OpenAI Codex Security: https://help.openai.com/en/articles/20001107-codex-security
- Claude Code Security: https://code.claude.com/docs/en/security
- Claude Code Costs: https://code.claude.com/docs/en/costs
- Claude Code Hooks: https://code.claude.com/docs/en/hooks
- Claude Code Review: https://code.claude.com/docs/en/code-review
- Claude local transcript storage: https://code.claude.com/docs/en/claude-directory
- Gemini CLI Trusted Folders: https://google-gemini.github.io/gemini-cli/docs/cli/trusted-folders.html
- Gemini CLI loop issue: https://github.com/google-gemini/gemini-cli/issues/2923
- Gemini CLI token issue: https://github.com/google-gemini/gemini-cli/issues/3784
- Gemini CLI repeated loop issue: https://github.com/google-gemini/gemini-cli/issues/5273
- Cursor deletion report: https://forum.cursor.com/t/critical-agent-deleted-entire-file-without-permission-and-tried-to-hide-it/155098
- Cursor 96M token loop: https://forum.cursor.com/t/warning-infinite-cache-read-loop-in-v2-4-x-agent-thinking-mode-96m-tokens-spike/151035
- GitHub Copilot cloud agent: https://docs.github.com/en/enterprise-cloud@latest/copilot/concepts/agents/cloud-agent/about-cloud-agent
- GitHub Copilot cloud agent responsible use: https://docs.github.com/en/enterprise-cloud@latest/copilot/responsible-use/copilot-cloud-agent
- AIDev agentic PR dataset: https://arxiv.org/abs/2602.09185
- Why Agentic-PRs Get Rejected: https://arxiv.org/abs/2602.04226
- AI Slop in Software Development: https://arxiv.org/abs/2603.27249
- HN PR review fatigue thread: https://news.ycombinator.com/item?id=47418016
- MCP Security Best Practices: https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices
- OWASP MCP Tool Poisoning: https://owasp.org/www-community/attacks/MCP_Tool_Poisoning
- OWASP MCP Security Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/MCP_Security_Cheat_Sheet.html
- MCP server security/maintainability study: https://arxiv.org/abs/2506.13538
- LangSmith Evaluation Types: https://docs.langchain.com/langsmith/evaluation-types
- OpenAI Agent Evals: https://developers.openai.com/api/docs/guides/agent-evals
- Docker Agent Evals: https://docs.docker.com/ai/docker-agent/evals/
- Reddit LLMDev eval discussion: https://www.reddit.com/r/LLMDevs/comments/1rv6kah/main_observability_and_evals_issues_when_shipping/
- Stanford token consumption study: https://digitaleconomy.stanford.edu/publication/how-do-ai-agents-spend-your-money-analyzing-and-predicting-token-consumption-in-agentic-coding-tasks/
- PocketOS incident coverage: https://www.livescience.com/technology/artificial-intelligence/i-violated-every-principle-given-ai-agent-deletes-companys-entire-database-in-9-seconds-then-confesses
