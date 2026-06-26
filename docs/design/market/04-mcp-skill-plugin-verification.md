# MCP / Skill / Plugin Verification Market Research

调研日期：2026-06-01（America/Vancouver）。资料范围为当前可访问的公开文档、官方仓库、官方产品说明和安全研究页面。

## 结论摘要

MCP、agent skills、plugins 正在从“开发者手动复制配置”进入“目录化、市场化、团队分发”的阶段，但信任机制仍明显落后于安装便利性。官方 MCP Registry 已经提供公开服务器元数据、命名空间归属和安装信息；Claude Code、Codex、ChatGPT Apps 都在提供插件或应用目录；GitHub、npm、PyPI、Docker/OCI 仍是实际代码分发层。问题是：多数平台能回答“这个包从哪里来、谁发布、怎么安装”，但还不能强有力地回答“安装后它实际会不会读 secret、出网、删除文件、越过 workspace、调用 destructive tool”。

这给 AgentSight 的“行为验收报告”留下明确空间：不要替代 marketplace 审核，也不要声称做静态证书；而是为第三方 skill/MCP/plugin 提供可复现的动态证据：在指定版本、指定 host、指定策略、指定测试夹具下，观察到哪些文件、进程、网络、secret、destructive operation 行为。最小产品可以从 badge/report 开始，面向 MCP server 作者、Claude/Codex plugin 作者、企业工具管理员和 marketplace reviewer。

## 市场信心等级

| 判断 | 信心 | 依据 |
| --- | --- | --- |
| MCP server 和 agent tool 正在形成“registry + package registry + marketplace”的分发结构 | 高 | 官方 MCP Registry 文档明确称其是公开 MCP server 的集中元数据仓库，元数据指向 npm、PyPI、Docker Hub、远程 URL 等实际 artifact；Claude/Codex 都支持 marketplace 或 directory。 |
| 安装第三方 MCP/tool 的核心担忧是 supply chain、prompt injection、secret/data exfiltration、over-scoped OAuth、destructive write | 高 | OpenAI MCP 风险文档、MCP spec 的 ToolAnnotations 警告、OWASP MCP Tool Poisoning 页面、Claude Code 安全文档都直接承认这些风险。 |
| 当前已有一些审核、权限声明、sandbox 和自动筛查，但它们不等同于独立行为验收 | 高 | Claude community marketplace 有 review、validator、automated safety screening 和 commit pin；ChatGPT app 有提交审核、组织验证、CSP、隐私政策；Codex 有 sandbox/approval；MCP Registry 明确把安全扫描委托给底层 package registry 和下游 aggregator。 |
| AgentSight 可以提供有价值的行为验收报告 | 中高 | AgentSight 的系统边界 tracing 能观察进程、文件、网络和 LLM/tool traffic，适合做“observed behavior vs declared policy”。但需要新增 policy evaluator、fixtures、install/runtime 分段、badge/report schema。 |
| marketplace 会愿意直接采纳第三方 badge 作为官方准入条件 | 中低 | 技术价值明确，但需继续验证 Claude/OpenAI/MCP 下游 marketplace 的接受度、法律措辞、review 流程接入方式。 |

## 1. MCP / Skill / Plugin 生态目前如何分发和安装？

### MCP server：Registry 只管元数据，artifact 仍在包仓库或远程服务

官方 MCP Registry 当前处于 preview。它定位为“公开 MCP servers 的官方集中元数据仓库”，提供 server creator 发布 metadata、命名空间管理、REST API discovery、标准安装/配置信息；`server.json` 会记录 server 唯一名称、artifact 位置、执行命令、env vars、capabilities 等信息。来源：[MCP Registry about](https://modelcontextprotocol.io/registry/about)。

关键点：

- Registry 不托管代码，只托管 metadata。文档明确说明 npm、PyPI、Docker Hub 等 package registries 才托管代码和二进制，MCP Registry metadata 指向这些包或远程 server URL。来源：[MCP Registry about](https://modelcontextprotocol.io/registry/about)。
- 发布流程使用官方 `mcp-publisher` CLI；示例流程是先发布 npm package，再把 MCP metadata 发布到 registry。来源：[MCP Registry quickstart](https://modelcontextprotocol.io/registry/quickstart)。
- Registry 支持 namespace ownership，例如 GitHub 账号或 DNS 归属，保证发布者能声明其 reverse-DNS 风格命名空间。来源：[MCP Registry about - Trust and Security](https://modelcontextprotocol.io/registry/about)。
- Registry 不面向 private servers。私有公司内网 server 或私有 package registry 应由组织自建 private registry。来源：[MCP Registry about](https://modelcontextprotocol.io/registry/about)。

这意味着 MCP 的实际分发链路通常是：

1. GitHub repo 开源源码和 README。
2. npm / PyPI / Docker Hub / GHCR / remote HTTP endpoint 托管可运行 artifact。
3. MCP Registry 发布 `server.json` metadata。
4. 下游 marketplace、host app 或 aggregator 拉取 registry 数据并加自己的 curation、rating、review。
5. 用户在 Claude Desktop、Claude Code、Codex、ChatGPT、Cursor、VS Code 等 host 中配置或安装。

### GitHub 上 MCP servers / tool servers 的分发方式

GitHub 当前承担三种角色：

- 源码和安装说明入口。很多 server README 直接给出 `npx -y ...`、`uvx ...`、`pipx ...`、`docker run ...`、`go install ...`、remote URL 等配置片段。
- Registry / marketplace 的 backing repo。官方 `modelcontextprotocol/servers` 仓库现在强调自己只维护少数 reference servers；如果要找 server 列表，应看 MCP Registry。来源：[modelcontextprotocol/servers](https://github.com/modelcontextprotocol/servers)。
- Claude/Codex plugin marketplace 的 catalog source。Claude Code marketplace 可从 GitHub shorthand、git URL、远程 `marketplace.json` 或本地路径添加；Codex plugin marketplace 也支持 GitHub shorthand、HTTP/HTTPS Git URL、SSH Git URL、本地 marketplace root。来源：[Claude plugin marketplaces](https://code.claude.com/docs/en/plugin-marketplaces)、[Codex build plugins](https://developers.openai.com/codex/plugins/build.md)。

GitHub 分发的风险在于“源码可信”与“安装 artifact 可信”不是一回事：README 可能指向 npm/PyPI/Docker artifact，artifact 可能不是从当前 commit 构建；plugin marketplace 可能 pin 到 commit，但 MCP server runtime 仍可能拉取动态依赖或访问远程服务。

### Claude Desktop：从 JSON 配置转向 DXT 单击安装

Claude Desktop 的 local MCP servers 仍是 beta，但 Anthropic 已把安装体验从手动 JSON 配置推进到 Desktop Extensions（DXT）：用户可在 Settings > Extensions 中浏览 directory，安装 Anthropic-reviewed tools，也可在 Advanced settings 里安装自定义 `.mcpb` 文件。Team / Enterprise owner 可以开关 public desktop extensions，也可以上传 custom desktop extensions 给团队一键安装。来源：[Claude Desktop local MCP servers](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop)。

这是非常接近浏览器扩展的分发模型：directory + reviewed tools + local packaged extension + admin control。

### Claude Code：MCP scopes、skills、plugins、marketplace

Claude Code 的 MCP server 可以按 scope 安装：

- local scope：写入 `~/.claude.json` 当前项目条目。
- project scope：写入项目根目录 `.mcp.json`，用于团队共享；使用 project-scoped server 前会提示 approval。
- user scope：写入 `~/.claude.json`，跨项目使用。
- plugin-provided servers 和 claude.ai connectors 也会参与 precedence。来源：[Claude Code MCP](https://code.claude.com/docs/en/mcp)。

Claude Code skills 的分发方式：

- personal：`~/.claude/skills/<skill-name>/SKILL.md`
- project：`.claude/skills/<skill-name>/SKILL.md`
- plugin：`<plugin>/skills/<skill-name>/SKILL.md`
- enterprise：通过 managed settings。

Skill 支持 `allowed-tools` frontmatter，限制 skill 激活时可用工具（例如只允许 `Read, Grep, Glob`），这是一个明显的权限声明信号。来源：[Claude Code skills](https://code.claude.com/docs/en/skills)。

Claude Code plugins 可包含 skills、agents、hooks、MCP servers、LSP servers、monitors、bin、settings。Plugins 通过 marketplace 分发，marketplace 是 JSON catalog，可来自 GitHub repo、git URL、remote URL 或 local path。Anthropic 维护两个 public marketplaces：`claude-plugins-official` 和 `claude-community`；第三方提交进入 community marketplace 前会 review，提交前要跑 `claude plugin validate`，review pipeline 会运行同样检查和 automated safety screening，批准后 catalog pin 到特定 commit SHA。来源：[Claude Code plugins](https://code.claude.com/docs/en/plugins)、[Claude plugin marketplaces](https://code.claude.com/docs/en/plugin-marketplaces)。

### Codex：skills 是 workflow，plugins 是 installable distribution unit

Codex skills 是 `SKILL.md` 目录，可放在 repo、user、admin、system scope。Codex 文档明确说：skills 是可复用 workflow 的 authoring format，plugins 是 Codex 中可安装的 distribution unit；当要跨开发者分发、捆绑 app integration 或 MCP config 时，应该打包成 plugin。来源：[Codex skills](https://developers.openai.com/codex/skills.md)。

Codex plugins 可包含 skills、apps、MCP servers。用户可在 Codex app 的 Plugin Directory 浏览 `Curated by OpenAI`、`Shared with you`、`Created by you`，CLI 中 `/plugins` 会按 marketplace 分组浏览。插件可通过 repo/personal marketplace JSON、GitHub shorthand、Git URL、本地目录添加。安装后，Codex 现有 approval settings 仍然适用；如果 plugin 包含 apps，需要 ChatGPT app install/sign-in；如果包含 MCP servers，可能需要额外 setup/auth。来源：[Codex plugins](https://developers.openai.com/codex/plugins.md)、[Codex build plugins](https://developers.openai.com/codex/plugins/build.md)。

Codex MCP 支持 stdio server、streamable HTTP server、bearer token、OAuth、server instructions、tool allow/deny list、server/tool approval mode。Installed plugins 也能 bundle MCP servers；用户 config 仍可控制开关和 tool policy。来源：[Codex MCP](https://developers.openai.com/codex/mcp.md)。

### ChatGPT Apps / OpenAI Apps SDK：MCP server + iframe UI + review submission

OpenAI Apps SDK 把 ChatGPT app 建在 MCP 上：MCP server 暴露 tools、structured content 和可渲染 UI resource；ChatGPT 支持 MCP Apps open standard，UI 运行在 iframe 中，通过 `ui/*` JSON-RPC over `postMessage` 与 host 通信。来源：[Apps SDK MCP](https://developers.openai.com/apps-sdk/concepts/mcp-server)、[MCP Apps compatibility in ChatGPT](https://developers.openai.com/apps-sdk/mcp-apps-in-chatgpt)。

公开分发需要提交审核：开发者在 Developer Mode 测试后，通过 dashboard-based review flow 提交；需完成个人或企业 identity verification；MCP server 必须是 publicly accessible domain，不能用本地或测试 endpoint；必须定义 CSP；审核通过后可进入 ChatGPT Apps Directory，并可作为 Codex shared directory/plugin 被发现。OpenAI 文档还说 self-serve plugin publishing is coming soon。来源：[Submit and maintain your app](https://developers.openai.com/apps-sdk/deploy/submission)。

## 2. 用户或企业安装第三方 agent tool 时担心什么？

### 2.1 Tool 描述可信度不足

MCP tool definitions 是模型选择 tool 的“说明书”，但描述、annotation、server instructions 都来自 server 本身。MCP spec 明确说 ToolAnnotations 都只是 hints，不保证真实描述 tool behavior；client 不应基于 untrusted server 的 annotations 做 tool-use 决策。来源：[MCP schema ToolAnnotations](https://modelcontextprotocol.io/specification/2025-11-25/schema)。

这直接导向 AgentSight 场景：tool 自称 `readOnlyHint: true` 不够，用户需要观察它实际有没有写文件、调用外部网络、读取 secret。

### 2.2 Prompt injection 和 tool poisoning

OWASP 把 MCP Tool Poisoning 定义为针对连接外部 tool servers 的 agent 的 indirect prompt injection：攻击者运行恶意 MCP server，tools 名字和描述看起来正常，但 tool response 携带隐藏指令，进入 LLM context 后可能诱导 agent 调用 restricted tools、泄露数据或绕过 system prompt。来源：[OWASP MCP Tool Poisoning](https://owasp.org/www-community/attacks/MCP_Tool_Poisoning)。

OpenAI 的 MCP 风险文档也明确提示：custom MCP servers 会让 ChatGPT workspace 接入外部应用，可能访问、发送、接收数据并执行 action；prompt injection 可能把敏感信息通过 read 或 write action exfiltrate。来源：[OpenAI MCP guide](https://developers.openai.com/api/docs/mcp)、[MCP and Connectors risks](https://developers.openai.com/api/docs/guides/tools-connectors-mcp)。

### 2.3 Secret / credential / private data 泄露

用户担心的不是单个 tool 是否恶意，而是组合风险：

- 一个 email/calendar/document MCP 读取不可信内容。
- prompt injection 让 agent 读取 internal MCP 的敏感数据。
- agent 通过另一个 write-capable MCP 发出去。
- 恶意 read-only MCP 可以把 query 参数、prompt 片段或返回数据记录到自己服务器日志。

OpenAI 特别建议优先使用 service provider 自己托管的 official servers，例如 Stripe 自己的 MCP server，而不是第三方代理 server；连接 MCP 前应仔细审核其数据使用方式。来源：[OpenAI MCP guide](https://developers.openai.com/api/docs/mcp)。

### 2.4 Over-scoped OAuth 和身份边界

Remote MCP 通常要 OAuth。MCP authorization spec 要求 MCP server 作为 OAuth 2.1 resource server 验证 access token、audience、scope，并要求 secure token storage、PKCE、防 redirect/phishing、避免 confused deputy、禁止 token passthrough。来源：[MCP Authorization](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization)。

企业安装第三方工具时会问：

- 这个 MCP server 拿的是用户 token、service token 还是 vendor token？
- scope 是否最小化？
- token 存在哪？
- server 是否会把 token 转发给上游？
- agent 能否在用户不理解的情况下触发 write/delete/payment/trade？

### 2.5 Local execution 和 supply chain

Stdio MCP server、Claude/Codex plugin、skill scripts 都可能在本机或开发容器里运行。风险包括：

- install script 拉取动态依赖。
- `npx -y` 每次解析版本或执行 package binary。
- plugin `bin/`、hooks、monitors、MCP server command 在用户 workspace 上下文执行。
- project-scoped `.mcp.json` / marketplace config 被 repo 带入团队。
- package 或 repo 后续更新扩大行为边界。

Claude Code 文档提示 project-scoped MCP server 使用前会请求 approval；Claude security 文档鼓励使用自己写的 MCP servers 或可信 provider 的 MCP servers，并可配置 MCP permissions。来源：[Claude Code MCP](https://code.claude.com/docs/en/mcp)、[Claude Code security](https://code.claude.com/docs/en/security)。

## 3. 当前有没有 marketplace 审核、签名、sandbox、权限声明、自动安全扫描？

有，但分散且强度不一。当前公开资料显示，它们更像“平台准入与运行时护栏”，还不是“行为证明”。

| 生态 | 审核 / curation | 签名 / provenance | sandbox / approval | 权限声明 | 自动扫描 |
| --- | --- | --- | --- | --- | --- |
| MCP Registry | 不做完整 marketplace curation；官方 registry 是 preview metadata repo，下游 marketplace 可加 curation/rating。 | 有 namespace authentication，证明 GitHub/domain 归属；不等于 artifact 签名。 | Registry 不执行 server。 | `server.json` 安装信息、tool metadata；ToolAnnotations 是 hints。 | 文档明确把 security scanning 委托给底层 package registries 和下游 aggregators。来源：[MCP Registry](https://modelcontextprotocol.io/registry/about)。 |
| MCP spec | 无 marketplace 审核。 | 无统一签名要求。 | 无统一 host sandbox 要求。 | `readOnlyHint`、`destructiveHint`、`idempotentHint`、`openWorldHint` 等，但 spec 说明这些只是 hints。 | 无统一扫描。来源：[MCP schema](https://modelcontextprotocol.io/specification/2025-11-25/schema)。 |
| Claude Desktop DXT | Directory 中有 Anthropic-reviewed tools；custom `.mcpb` 可 sideload。 | 公开页面未看到统一签名机制说明。 | Desktop extension 是本地 MCP server；Team/Enterprise admin 可启停 public extensions、上传 custom extensions。 | 安装时可配置 API keys/settings；具体权限取决于 extension/server。 | 公开页面只说明 reviewed tools，未披露扫描细节。来源：[Claude Desktop MCP](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop)。 |
| Claude Code plugins | 官方 marketplace curated；community marketplace review 后入库。 | Approved community plugin pin 到特定 commit SHA；marketplace source 可 pin branch/tag/ref。不是通用 artifact signing。 | Claude Code 有 tool permission model；project MCP 需 approval；plugin 可带 hooks/MCP/bin。 | Skill `allowed-tools`；MCP server scope；plugin manifest；settings。 | `claude plugin validate` + review pipeline automated safety screening。来源：[Claude plugins](https://code.claude.com/docs/en/plugins)。 |
| Codex plugins | Codex app 有 Curated by OpenAI / Shared / Created by you；Apps 审核通过后可形成 Codex distribution。 | marketplace 可以 Git ref/pinned source；公开文档未显示第三方插件统一签名。 | Codex 默认本地 no network、workspace-write sandbox、approval；可配置 read-only/workspace/full access、MCP tool approval。来源：[Codex approvals](https://developers.openai.com/codex/agent-approvals-security.md)。 | Plugin 可 bundle skills/apps/MCP；MCP 可设置 enabled/disabled tools、approval mode；permissions profile 可声明 filesystem/network policy。来源：[Codex MCP](https://developers.openai.com/codex/mcp.md)、[Codex permissions](https://developers.openai.com/codex/permissions.md)。 | OpenAI curated/plugin review 细节未完全公开；Codex 有自动 approval reviewer，但这是运行时审批，不是 marketplace 扫描。 |
| ChatGPT Apps | 公开提交必须经过 dashboard review；组织身份验证；必须遵守 app submission guidelines。 | 公开资料未显示通用 app signing。 | Widget 在 sandboxed iframe + CSP；write actions 需要用户确认；Developer Mode 允许 sideload MCP，风险更高。来源：[Apps Security & Privacy](https://developers.openai.com/apps-sdk/guides/security-privacy)。 | App privacy policy、tool definitions、CSP、OAuth scopes、数据最小化要求。 | OpenAI review 会检查准入要求；具体自动安全扫描细节未完全公开。来源：[Submit app](https://developers.openai.com/apps-sdk/deploy/submission)、[App submission guidelines](https://developers.openai.com/apps-sdk/app-submission-guidelines)。 |

值得注意的是，MCP 官方博客对 ToolAnnotations 的定位很克制：annotations 可以驱动确认提示，但不能让模型抵抗 prompt injection，也不是 enforcement；untrusted server 可以撒谎。来源：[Tool Annotations as Risk Vocabulary](https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/)。

这正是 AgentSight 的切入点：把“声明”变成“观测结果”，把“reviewer 相信作者”变成“reviewer 看过行为证据”。

## 4. AgentSight 能否提供行为验收报告？

可以，但产品措辞必须严谨：AgentSight 能提供“在给定测试条件下观察到的行为验收报告”，不能声称永久证明一个工具永远安全。

### 可验收的行为维度

| Claim | AgentSight 可观察证据 | 报告结论形式 | 限制 |
| --- | --- | --- | --- |
| `read-only` | 文件 open/write/truncate/unlink/rename/chmod/chown 等 syscall；process tree；可能的 HTTP write actions。 | Pass：测试期未观察到本地写入或远程 write tool call。Fail：列出具体写入路径 / request / process。 | 只覆盖测试夹具；远程服务内部写入需要从 MCP/tool semantics 或 HTTP method 推断。 |
| `workspace-local` | 所有写入路径与 workspace allowlist 匹配；detect outside-root file writes。 | Pass：所有写入在 workspace roots。Fail：列出越界路径。 | 如果 host sandbox 已阻止越界，AgentSight 看到的是 attempted/denied 行为，报告应区分 attempted vs succeeded。 |
| `no network` | connect/sendto/DNS/TLS/HTTP outbound events；目标 host/IP/port；进程 lineage。 | Pass：测试期无 outbound network，或只有 policy allowlist。Fail：列出 destination 和触发进程。 | 对已存在连接、Unix socket、本地代理、QUIC/custom protocol 要补充 hooks；强保证应配合网络 sandbox。 |
| `no secret access` | 读取 `.env`、SSH keys、cloud credential files、token files、password manager sockets、`/proc/*/environ`；LLM/TLS payload 中出现 secret pattern；child process env。 | Pass：未观察到 secret path/pattern access。Fail：列出 secret class、路径、进程，不泄露 secret value。 | 环境变量读取不总是可见；secret pattern 可能误报/漏报；云端 connector 内部 secret 不在本机边界内。 |
| `no destructive ops` | unlink/rmdir/truncate/overwrite、`rm`, `git clean`, package uninstall、database destructive HTTP/API verbs、MCP `destructiveHint` tool calls。 | Pass：未观察到 destructive local ops 或 destructive remote/tool calls。Fail：列出 ops、args、affected path/resource。 | “destructive” 需要 policy 定义；HTTP POST 不一定 destructive，DELETE 也可能 harmless mock。 |
| `declared permissions match observed behavior` | 解析 MCP `tools/list` annotations、plugin/skill manifest、policy file，再与 runtime trace 比对。 | Pass/Fail/Unknown per dimension。 | ToolAnnotations 是 hints；AgentSight 不应把缺失 annotation 自动当 fail，可标记 Unknown / Needs declaration。 |

### 报告应该包含哪些证据

行为验收报告应至少包含：

- Subject：tool/skill/plugin 名称、版本、来源 URL、artifact digest/commit SHA、安装命令。
- Host matrix：Claude Desktop / Claude Code / Codex / ChatGPT Developer Mode / MCP Inspector / custom client，版本号。
- Policy：声明要验证的 claims，例如 `read_only: true`、`network: none`、`secret_paths: deny`。
- Test fixtures：触发 prompt、MCP tool calls、sample workspace、mock credentials、network mock。
- Install phase：安装时是否出网、执行 postinstall、写入非 workspace、读取 secret。
- Runtime phase：实际调用时的 process/file/network/tool behavior。
- Result：每个 claim 的 Pass / Fail / Unknown。
- Evidence：最小化证据列表，不泄露 secret value；提供 process lineage 和时间线。
- Repro command：如何复跑。
- Scope disclaimer：只证明给定版本和测试场景下的 observed behavior。

### 为什么 AgentSight 比 host 自带权限声明更适合做这个报告

Host 自带机制回答的是“我准备允许什么”或“这次需要不需要确认”；AgentSight 回答的是“实际发生了什么”。例如：

- MCP `readOnlyHint` 是声明，AgentSight 可观察是否真的没有 write。
- Codex sandbox 可以阻止网络，AgentSight 可以记录是否尝试访问网络、访问哪里。
- Claude plugin review 可以检查 schema 和安全筛查，AgentSight 可以跑动态 fixture 看 install/runtime 是否越界。
- ChatGPT app review 要求 CSP 和 privacy policy，AgentSight 可以在开发者本地或 CI 中证明 server/tool 在测试中没有收集多余数据。

## 5. 这个场景与普通 run receipt 有何不同？

普通 run receipt 是事后审计：“我刚刚让 agent 做了一件事，它实际做了什么？”用户通常是个人，关注一次 session 的事实复盘。

Skill/MCP/plugin 行为验收是安装前或发布前信任评估：“这个工具是否值得被更多人安装、推荐、上架、纳入企业 allowlist？”它有几个不同点：

| 维度 | 普通 run receipt | 行为验收报告 |
| --- | --- | --- |
| 时间点 | 运行后 | 安装前、发布前、升级前、review 前 |
| 用户 | individual agent user | MCP/skill/plugin 作者、企业 admin、marketplace reviewer、security reviewer |
| 目标 | 解释一次 session | 判断一个 tool package 是否符合声明 |
| 输出 | session summary / timeline | policy verdict + evidence + badge + reproducible fixture |
| 证据粒度 | “发生了什么” | “声明 vs 观察结果”：read-only、workspace-local、no network、no secrets、no destructive ops |
| 可复现性 | 不一定复跑 | 必须可复跑，最好能进 CI |
| 分享对象 | 用户自己或事故调查者 | README、GitHub release、marketplace listing、PR、企业 allowlist |
| 风险模型 | agent 本次行为异常 | 第三方供应链、版本升级、tool 描述撒谎、malicious/compromised server |

因此，验收报告不是 run receipt 的改名，而是一个带 policy、fixture、version、badge、CI matrix 的“动态合规测试”产品。

## 6. 最小产品实验：让 skill / MCP 作者生成 AgentSight badge/report

### 实验目标

让一个第三方作者在自己的 repo 中运行一条命令，生成可公开链接的 report 和 README badge：

```md
[![AgentSight verified](https://agentsight.dev/badges/io.github.acme/weather-mcp/read-only.svg)](https://agentsight.dev/reports/io.github.acme/weather-mcp/1.2.0)
```

Badge 不应该写“safe”这种绝对词，而应写：

- `AgentSight: read-only observed`
- `AgentSight: no network observed`
- `AgentSight: workspace-local writes`
- `AgentSight: secrets not accessed`
- `AgentSight: destructive ops not observed`
- `AgentSight: verified on Claude Code + Codex`

### v0 支持对象

优先级建议：

1. Stdio MCP server from npm / local repo：最容易安装、运行、观察本地进程和网络。
2. Claude Code plugin：marketplace/review 已经存在，作者和 reviewer 有 badge 动机。
3. Codex plugin / skill：和 AgentSight 产品叙事高度一致，但 Codex plugin self-serve publishing 仍在推进中。
4. ChatGPT remote MCP app：价值高，但本机边界只能观察 server 侧或测试 client 侧，不能完全观察 ChatGPT 云端运行。

### 作者工作流

作者在 repo 中创建 `agentsight.verify.yaml`：

```yaml
subject:
  type: mcp_server
  name: io.github.acme/weather
  version: 1.2.0
  source: https://github.com/acme/weather-mcp
  install:
    command: npx
    args: ["-y", "@acme/weather-mcp@1.2.0"]

policy:
  read_only: true
  workspace_local: true
  network:
    mode: deny
  secrets:
    deny_paths:
      - "**/.env"
      - "~/.ssh/**"
      - "~/.aws/**"
      - "~/.config/gcloud/**"
  destructive_ops: deny

tests:
  - name: list-tools
    type: mcp
    call: tools/list
  - name: normal-query
    type: mcp
    call: tools/call
    tool: get_weather
    arguments:
      city: "Seattle"
```

运行：

```sh
agentsight verify --config agentsight.verify.yaml
agentsight report --format html --output dist/agentsight-report
agentsight badge --input dist/agentsight-report/report.json --output dist/agentsight-badge.json
```

CI 集成：

```yaml
name: AgentSight verification
on:
  pull_request:
  release:
    types: [published]
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: agentsight/setup-agentsight@v0
      - run: agentsight verify --config agentsight.verify.yaml
      - run: agentsight report --format html --output public/agentsight
      - uses: actions/upload-artifact@v4
        with:
          name: agentsight-report
          path: public/agentsight
```

### v0 report schema

```json
{
  "subject": {
    "type": "mcp_server",
    "name": "io.github.acme/weather",
    "version": "1.2.0",
    "source": "https://github.com/acme/weather-mcp",
    "artifact": {
      "package": "@acme/weather-mcp",
      "digest": "sha256:..."
    }
  },
  "environment": {
    "host": "mcp-inspector-headless",
    "os": "ubuntu-24.04",
    "agentsight_version": "0.x"
  },
  "claims": {
    "read_only": "pass",
    "workspace_local": "pass",
    "no_network": "pass",
    "no_secret_access": "pass",
    "no_destructive_ops": "pass"
  },
  "evidence_summary": {
    "processes": 3,
    "file_reads": 12,
    "file_writes": 0,
    "network_destinations": [],
    "secret_path_reads": [],
    "destructive_ops": []
  },
  "scope": {
    "install_phase_observed": true,
    "runtime_phase_observed": true,
    "fixtures": ["list-tools", "normal-query"],
    "not_a_security_certification": true
  }
}
```

### 产品边界

MVP 必须清楚写：

- Report 是 dynamic behavioral evidence，不是完整安全认证。
- Pass 表示“在这些 tests 中未观察到违规行为”，不是“未来不会违规”。
- Unknown 比假 Pass 更可信。比如 tool 没有 destructive annotation、测试没有覆盖某个 tool，就标 Unknown。
- Install phase 和 runtime phase 必须分开，因为很多 supply-chain 风险发生在安装阶段。
- 本地 stdio MCP 与 remote MCP 要分开，因为 remote server 的内部行为不在用户机器上。

## AgentSight 的市场定位建议

一句话定位：

> AgentSight verifies what third-party agent tools actually do, not just what they declare.

中文：

> AgentSight 为第三方 MCP / skill / plugin 生成可复现的行为验收报告：是否只读、是否限制在 workspace、是否出网、是否访问 secret、是否执行 destructive operation。

首批用户画像：

- MCP server 作者：想在 README 和 MCP Registry 下游 marketplace 中展示可信 badge。
- Claude/Codex plugin 作者：想通过 review 或让企业团队放心安装。
- 企业 agent platform / security team：想建立 private allowlist，不只靠作者声明。
- Marketplace reviewer：想把人工 review 的一部分变成可复跑 evidence。
- 开源维护者：担心别人提交的 agent skill/plugin 扩大权限或引入隐蔽行为。

## 需要继续验证的地方

1. Claude community marketplace 的 automated safety screening 具体检查哪些项，是否接受外部 report 链接。
2. OpenAI ChatGPT Apps / Codex Plugin Directory 的审核 API、metadata 字段、badge/link 展示空间。
3. MCP Registry 下游 aggregators 是否已经有 security metadata 字段，是否能挂 AgentSight report URL。
4. MCP server 作者是否愿意在 CI 中运行需要 sudo/eBPF 的验证；若不愿意，需要 rootless/container fallback。
5. 对 remote MCP server，AgentSight 应部署在 server 侧、client 侧，还是做 synthetic client + server-side attestation。
6. “no secret access”的可接受证据标准：路径 denylist、env capture、payload scanner、DLP pattern 哪些足够进入 MVP。
7. 对 destructive remote actions 的分类标准：仅靠 MCP annotations 不够，是否要解析 HTTP method、tool name、OpenAPI/JSON schema、用户确认事件。
8. 报告法律措辞：避免“certified safe”，使用“observed / verified under test / evidence report”。

## Source Index

- MCP Registry: [about](https://modelcontextprotocol.io/registry/about), [quickstart](https://modelcontextprotocol.io/registry/quickstart)
- MCP spec: [ToolAnnotations schema](https://modelcontextprotocol.io/specification/2025-11-25/schema), [Authorization](https://modelcontextprotocol.io/specification/2025-06-18/basic/authorization)
- MCP security: [OWASP MCP Tool Poisoning](https://owasp.org/www-community/attacks/MCP_Tool_Poisoning), [Tool Annotations as Risk Vocabulary](https://blog.modelcontextprotocol.io/posts/2026-03-16-tool-annotations/)
- MCP GitHub distribution: [modelcontextprotocol/servers](https://github.com/modelcontextprotocol/servers), [modelcontextprotocol/registry](https://github.com/modelcontextprotocol/registry)
- Claude: [Desktop local MCP servers](https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop), [Claude Code MCP](https://code.claude.com/docs/en/mcp), [skills](https://code.claude.com/docs/en/skills), [plugins](https://code.claude.com/docs/en/plugins), [plugin marketplaces](https://code.claude.com/docs/en/plugin-marketplaces), [security](https://code.claude.com/docs/en/security)
- OpenAI / ChatGPT Apps: [Apps SDK MCP](https://developers.openai.com/apps-sdk/concepts/mcp-server), [MCP Apps compatibility](https://developers.openai.com/apps-sdk/mcp-apps-in-chatgpt), [submit app](https://developers.openai.com/apps-sdk/deploy/submission), [security & privacy](https://developers.openai.com/apps-sdk/guides/security-privacy), [submission guidelines](https://developers.openai.com/apps-sdk/app-submission-guidelines)
- OpenAI / MCP API: [Building MCP servers for ChatGPT Apps and API integrations](https://developers.openai.com/api/docs/mcp), [MCP and Connectors](https://developers.openai.com/api/docs/guides/tools-connectors-mcp)
- Codex: [skills](https://developers.openai.com/codex/skills.md), [MCP](https://developers.openai.com/codex/mcp.md), [plugins](https://developers.openai.com/codex/plugins.md), [build plugins](https://developers.openai.com/codex/plugins/build.md), [agent approvals & security](https://developers.openai.com/codex/agent-approvals-security.md), [permissions](https://developers.openai.com/codex/permissions.md)
