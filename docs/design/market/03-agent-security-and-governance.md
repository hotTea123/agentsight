# Agent 安全与治理市场调研

调研口径：截至 2026 年 6 月初的公开资料。本文关注企业在部署 coding agent、CLI agent、MCP 工具、企业 copilot 和自建 agent 时的安全、治理、审计、数据泄露和实时阻断需求。结论偏产品判断，不把所有厂商能力视为已验证效果；厂商声明只作为市场信号。

## 一句话结论

企业真正害怕的不是“模型说错话”，而是“一个被误导或过度授权的 agent 用合法权限做了不可解释、不可追责、可能泄露数据或改坏系统的事”。当前市场主流方案集中在 prompt/content 层、AI gateway/proxy、SDK instrumentation、DLP/浏览器/endpoint 管控、agent control plane、sandbox 和传统 XDR/SIEM 集成；这些方案对本地 coding agent、CLI agent 和 MCP 工具调用的共同盲点是：它们往往看不到 agent 最终对本机文件、进程、shell、网络、凭证和子进程造成的真实 OS side effects。

AgentSight 的 eBPF / OS boundary tracing 在安全上有真实差异：它不是另一个 prompt firewall，而是一个独立于 agent 自报日志的证据层，能把“agent 被什么输入影响”与“它实际读写了什么、执行了什么、连了哪里”关联起来。产品应避免扩张成完整 enterprise security platform，先站稳“本地 agent 行为收据、MCP/工具验证、事故取证、轻量实时 airlock”。

## 1. 市场与标准框架正在把风险从 LLM 扩展到 agent

OWASP 2025 LLM Top 10 已经把企业关心的核心风险说得很直接：LLM01 是 prompt injection，LLM02 是 sensitive information disclosure，LLM06 是 excessive agency，此外还有 supply chain、improper output handling、system prompt leakage、vector/embedding weakness 等风险；官方 PDF 见 [OWASP Top 10 for LLM Applications 2025](https://owasp.org/www-project-top-10-for-large-language-model-applications/assets/PDF/OWASP-Top-10-for-LLMs-v2025.pdf)。

OWASP 在 2025 年 12 月发布了面向 agent 的 Top 10，强调 autonomous agent 的独有问题，例如 agent goal hijack、tool misuse、identity and privilege abuse 等；发布说明见 [OWASP GenAI Security Project Releases Top 10 Risks and Mitigations for Agentic AI Security](https://genai.owasp.org/2025/12/09/owasp-genai-security-project-releases-top-10-risks-and-mitigations-for-agentic-ai-security/)，Microsoft 对 ASI01/ASI02/ASI03 等也有映射说明：[Addressing the OWASP Top 10 Risks in Agentic AI with Microsoft Copilot Studio](https://www.microsoft.com/en-us/security/blog/2026/03/30/addressing-the-owasp-top-10-risks-in-agentic-ai-with-microsoft-copilot-studio/)。

MCP 也从“集成协议”变成了安全风险面。OWASP MCP Top 10 v0.1 覆盖 token mismanagement、scope creep、tool poisoning、command injection、insufficient auth/authz、lack of audit/telemetry、shadow MCP servers、context over-sharing 等；见 [OWASP MCP Top 10](https://owasp.org/www-project-mcp-top-10/)。这些风险几乎全部发生在 agent 与工具、凭证、上下文和本地系统之间，不只是 prompt 文本本身。

公开市场信号也在加强。Gartner 2025 年调查称，29% 的网络安全领导者表示其组织过去 12 个月遭遇过企业 GenAI 应用基础设施攻击，见 [Gartner Survey Reveals GenAI Attacks Are on the Rise](https://www.gartner.com/en/newsroom/press-releases/2025-09-22-gartner-survey-reveals-generative-artificial-intelligence-attacks-are-on-the-rise)。Gartner 还预测到 2027 年 40% 的 AI 数据泄露会来自跨境 GenAI 误用，强调 AI governance、data security governance、prompt filtering/redaction 等控制，见 [Gartner Predicts 40% of AI Data Breaches Will Arise from Cross-Border GenAI Misuse by 2027](https://www.gartner.com/en/newsroom/press-releases/2025-02-17-gartner-predicts-forty-percent-of-ai-data-breaches-will-arise-from-cross-border-genai-misuse-by-2027)。

## 2. 企业真实担心的 agent 安全问题是什么？

### 2.1 数据泄露与 IP 泄露

企业最容易理解、最容易形成预算的问题仍然是数据泄露：员工把客户数据、源代码、合同、财务预测、内部文档发给 AI 工具；agent 通过 RAG、MCP、浏览器、email、Slack、Google Drive 等连接器读到敏感数据后，再通过模型输出、工具调用、URL、webhook、邮件或日志泄露出去。

Google 的 Model Armor 明确把 runtime protection for generative and agentic AI 定义为防 prompt injection、sensitive data leaks 和 harmful content，并集成 Sensitive Data Protection；见 [Google Cloud Model Armor](https://cloud.google.com/security/products/model-armor)。Lakera Guard 文档也把 PII、system prompt、trigger words、自定义实体的输入输出检测列为 data leakage prevention；见 [Lakera Data Leakage Prevention](https://docs.lakera.ai/docs/data-leakage-prevention)。

### 2.2 Prompt injection，尤其是 indirect prompt injection

Prompt injection 已不是“用户在输入框里说 ignore previous instructions”这么简单。真正危险的是 agent 读取外部内容时被隐藏指令影响：网页、PDF、issue comment、README、MCP tool description、工具返回值、email、日历、RAG 文档、代码注释都可能成为指令载体。

OpenAI 2026 年 3 月文章指出，把“AI firewall”放在 agent 和外部世界之间做恶意输入分类很常见，但成熟攻击通常不一定会被这种系统抓住，防御还需要系统设计来限制成功攻击的影响；见 [Designing AI agents to resist prompt injection](https://openai.com/index/designing-agents-to-resist-prompt-injection/)。Anthropic 也明确说，browser agent 没有免疫 prompt injection，随着 agent 能执行现实操作，这个问题远未解决；见 [Mitigating the risk of prompt injections in browser use](https://www.anthropic.com/research/prompt-injection-defenses)。

### 2.3 Tool misuse / excessive agency / 越权行动

Agent 的风险从“输出一句坏话”升级为“调用工具做错事”。OWASP LLM06 excessive agency 关注 agent 权限过大、动作过多、自动化程度过高；agentic Top 10 进一步拆成 goal hijack、tool misuse、identity and privilege abuse 等。

Microsoft Copilot Studio 安全文档把攻击者操纵 agent 的方式列为 malicious prompts、unintended tool executions、exploiting data sources to escalate privileges or exfiltrate data；见 [Protect your Microsoft Copilot Studio AI agents](https://learn.microsoft.com/en-us/defender-cloud-apps/ai-agent-protection)。OpenAI Agent Builder safety 文档也把私有数据泄露、MCP 工具调用、tool approvals、structured outputs、trace graders/evals 列为核心控制；见 [Safety in building agents](https://platform.openai.com/docs/guides/agent-builder-safety)。

### 2.4 MCP 与工具供应链风险

MCP 把 agent 能力扩展到文件系统、数据库、SaaS、CI/CD、云 API、浏览器与本地命令。风险包括：恶意 MCP server、工具描述投毒、schema poisoning、scope creep、token 存储在配置或日志中、shadow MCP server、命令注入、缺少审计。

Anthropic 的 remote MCP connector 帮助文档直接提醒：只连接可信服务器、仔细审查 OAuth 权限、警惕 malicious MCP servers 的 hidden instructions、监控工具行为变化；见 [Getting started with custom connectors using remote MCP](https://support.anthropic.com/en/articles/11175166-getting-started-with-custom-integrations-using-remote-mcp)。Lasso 的 MCP security 页面把 shadow MCP sprawl、prompt injection & tool poisoning、real-time threat detection、SIEM export、Claude Code/Desktop/Cursor/Windsurf MCP 连接治理列为卖点，说明这个痛点已经产品化；见 [Lasso MCP Security](https://www.lasso.security/use-cases/mcp-security)。

### 2.5 身份、凭证与 agent 权限边界

企业担心 agent 使用的是“谁的权限”：用户 OAuth、maker-provided credentials、service account、长期 API token、SSH key、本地 cloud credentials、GitHub token。Prompt injection 本身未必是最终破坏点，真正破坏来自 agent 已经被授予的读写权限。

Microsoft Copilot Studio 的 automatic security scan 会在 agent 改成 no authentication、maker-provided credentials、对全组织共享等场景提醒风险；见 [Automatic security scan in Copilot Studio](https://learn.microsoft.com/en-us/microsoft-copilot-studio/security-scan)。Google Vertex AI Agent Builder/Agentspace 把 agent identity、IAM、observability、registry、Model Armor 和 Security Command Center 列为 govern 能力；见 [Vertex AI Agent Builder](https://cloud.google.com/products/agent-builder)。

### 2.6 缺少审计、归因与可解释证据

安全团队不只是想阻断，还想知道：哪个 agent、哪个用户、哪个 prompt、哪个工具、哪个文件、哪个网络目的地、哪个凭证、哪个变更造成了事故。OWASP MCP08 专门列出 lack of audit and telemetry，强调缺日志会让 unauthorized actions、data access、incident response 与 compliance 失效；见 [MCP08: Lack of Audit and Telemetry](https://owasp.org/www-project-mcp-top-10/2025/MCP08-2025%E2%80%93Lack-of-Audit-and-Telemetry)。

这正是 AgentSight 的主战场：acting agent 的自报总结不是独立证据；git diff、shell history、SDK trace、SIEM 网络日志分别只看到一部分。企业需要可保全、可查询、可导出、可映射到安全事件的 run receipt。

## 3. 当前方案通常站在哪一层？

| 层 | 典型问题 | 代表方案/资料 | 能看见什么 | 主要盲点 |
| --- | --- | --- | --- | --- |
| Prompt firewall / content guardrails | 输入输出是否有 prompt injection、jailbreak、PII、恶意链接、违规内容 | [Lakera Guard](https://docs.lakera.ai/guard)、[Google Model Armor](https://cloud.google.com/security/products/model-armor)、[CalypsoAI scanners](https://support.calypsoai.com/en/about-scanners-in-the-calypsoai-platform?hsLang=en)、[NVIDIA NeMo Guardrails](https://docs.nvidia.com/nemo-guardrails/index.html)、[HiddenLayer AI Runtime Security](https://www.hiddenlayer.com/platform/ai-runtime-security) | prompt、response、部分 tool message 文本 | 看不到本地进程、文件读取、shell 副作用；高阶 indirect injection 可能绕过 |
| Gateway / proxy / AI firewall | 统一拦截 LLM API、企业 SaaS AI、agent traffic | [Microsoft AI Gateway Prompt Injection Protection](https://learn.microsoft.com/en-us/entra/global-secure-access/how-to-ai-prompt-injection-protection)、[Google Model Armor with Apigee/network service extensions](https://cloud.google.com/security/products/model-armor)、[Lasso MCP Gateway](https://www.lasso.security/resources/lasso-releases-first-open-source-security-gateway-for-mcp) | 经过网关的请求、响应、策略决策 | 本地 CLI 直接连模型、本地模型、绕过代理的 curl/npm/pip/git、MCP 本地 stdio 调用 |
| SDK instrumentation / agent traces | 在应用内记录 model call、tool call、trace、eval | [OpenAI Agents SDK](https://platform.openai.com/docs/guides/agents-sdk/)、[OpenAI Agent Builder safety](https://platform.openai.com/docs/guides/agent-builder-safety) | 框架知道的 prompt/tool/trace | 依赖框架和代码接入；可能被关闭或遗漏子进程；跨进程/本地 OS 行为弱 |
| DLP / browser / endpoint data controls | 阻止员工把敏感数据发到 AI 工具 | [Cyberhaven DLP for GenAI](https://www.cyberhaven.com/blog/dlp-for-genai)、[Microsoft Copilot Studio DLP/data policies](https://learn.microsoft.com/en-us/microsoft-copilot-studio/admin-data-loss-prevention)、[CalypsoAI PII scanners](https://support.calypsoai.com/en/articles/10193014-configure-individual-scanners-global-admin-only) | 粘贴、上传、prompt/response 中的 PII/secret/IP 模式 | 难判断语义泄露；agent 用合法工具读数据再摘要外发时可能不命中传统规则 |
| EDR / XDR / SIEM | 发现 endpoint、cloud、identity、network 异常并汇总事件 | [Microsoft Defender for AI agents](https://learn.microsoft.com/en-us/defender-xdr/security-for-ai/ai-agent-detection-protection)、[CrowdStrike Falcon AIDR](https://www.crowdstrike.com/en-us/blog/falcon-aidr-detects-threats-at-prompt-layer-in-kubernetes-ai-apps/)、[SentinelOne Prompt Security](https://www.sentinelone.com/platform/securing-ai-prompt/) | 进程、网络、identity、部分 AI prompt layer、alerts | 传统 EDR 看不到 agent 的语义意图；SIEM 没有天然“agent 偏离用户目标”的规则 |
| Sandbox / execution isolation | 限制 agent 即使被误导也不能访问不该访问的文件/网络 | [Anthropic Claude Code sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing)、[OpenAI Lockdown Mode](https://help.openai.com/articles/20001061/)、[Microsoft Agent Workspace](https://support.microsoft.com/en-au/windows/experimental-agentic-features-a25ede8a-e4c2-4841-85a8-44839191dfb3) | 限制文件、网络、环境、工具 | 强约束会影响可用性；不能替代审计；未必覆盖所有 CLI/MCP/本地工具组合 |
| Agent governance / control plane | 发现 agent、注册 agent、权限、日志、合规、生命周期 | [Microsoft Agent 365](https://learn.microsoft.com/en-us/security/security-for-ai/agent-365-security)、[Google Agentspace](https://cloud.google.com/products/agentspace/)、[Lasso AI Agent Governance](https://lasso.security/use-cases/ai-agent-governance) | 平台内 agent inventory、policy、audit、runtime protection | 平台外、本地、开源 CLI、开发机 agent 很容易遗漏 |
| Model / AI supply chain security | 模型、依赖、插件、MCP server、schema 是否被投毒 | [Protect AI / Palo Alto acquisition](https://investors.paloaltonetworks.com/news-releases/news-release-details/palo-alto-networks-completes-acquisition-protect-ai/)、[OWASP MCP04](https://owasp.org/www-project-mcp-top-10/2025/MCP04-2025%E2%80%93Software-Supply-Chain-Attacks%26Dependency-Tampering) | 模型文件、依赖、配置、scanner/red-team 结果 | 不能证明一次具体 agent run 实际做了什么 |

## 4. 主要厂商与工具的产品化方向

### Lakera

Lakera Guard 定位为 GenAI runtime security / AI application firewall，提供 prompt attack、data leakage、content violation、malicious link detection、自定义策略、日志和 SIEM 接入。它的集成点是 `/v2/guard` API，对输入输出和 agent/tool messages 做筛查；文档建议多步 agent workflow 每一步都调用 Guard，见 [Lakera Guard API Endpoint](https://docs.lakera.ai/docs/api/guard)。这说明市场已接受“每个 agent interaction 都要过实时策略”的方向。

对 AgentSight 的启示：Lakera 强在文本风险分类和低延迟阻断，弱在本地 OS side effects。AgentSight 不应复制它，而应补“这个被放行的 interaction 最后对机器做了什么”。

### Prompt Security / SentinelOne

SentinelOne 2025 年宣布收购 Prompt Security，称其能力包括 runtime AI security、AI-related data leakage prevention、intelligent agents protection、可见 AI 工具访问、共享了什么数据、自动执行防 prompt injection、敏感数据泄露和误用；见 [SentinelOne to Acquire Prompt Security](https://www.sentinelone.com/press/sentinelone-to-acquire-prompt-security-to-advance-genai-security/) 和 [Prompt Security from SentinelOne](https://www.sentinelone.com/platform/securing-ai-prompt/)。

对 AgentSight 的启示：传统 EDR/XDR 厂商正在把“prompt layer / AI usage layer”吸收到 endpoint 平台中。AgentSight 若进入企业，应作为可导出证据源或本地 agent-specific sensor，而不是声称替代 XDR。

### Lasso Security

Lasso 明确覆盖 AI usage control、AI agents security、AI application protection、MCP governance、prompt injection、intent security、automated red teaming。它的 MCP 页面特别提到 Claude Code、Claude Desktop、Cursor、Windsurf，说明本地 coding agent/MCP 是已被厂商盯上的场景；见 [Lasso MCP Security](https://www.lasso.security/use-cases/mcp-security)、[AI Coding Assistants](https://www.lasso.security/use-cases/ai-coding-assistents)。

对 AgentSight 的启示：Lasso 更偏 connection layer / intent layer / MCP gateway。AgentSight 可在本地 OS boundary 提供更底层证据，例如 MCP server 声称 read-only，但实际触发文件写、进程执行、网络连接。

### Protect AI / Palo Alto Prisma AIRS

Palo Alto 2025 年完成收购 Protect AI，称 Protect AI 将成为 Prisma AIRS 的基础，覆盖 AI applications and models security、AI-SPM、GenAI runtime security、AI agent security 等；见 [Palo Alto Networks Completes Acquisition of Protect AI](https://investors.paloaltonetworks.com/news-releases/news-release-details/palo-alto-networks-completes-acquisition-protect-ai/)。Prisma AIRS Agent Security 页面强调 agent identity、behavior、action 的实时治理，见 [Prisma AIRS Agent Security](https://www.paloaltonetworks.com/prisma/agent-security)。

对 AgentSight 的启示：大平台会把 agent security 往“全生命周期 + posture + runtime + identity”打包。AgentSight 要避免平台化竞争，先成为轻量、开发者可用、证据可信的 local runtime layer。

### HiddenLayer

HiddenLayer AI Runtime Security 声称保护 AI endpoints 和 agent workflows，覆盖 prompt attacks、jailbreaks、unsafe outputs、malicious tool use，见 [HiddenLayer AI Runtime Security](https://www.hiddenlayer.com/platform/ai-runtime-security)。这是典型 runtime AI defense。

### CalypsoAI

CalypsoAI 平台提供 scanner packages、prompt history、audit logs、roles/permissions；scanner 可 block/audit/redact，默认包覆盖 EU AI Act、restricted topics、PII、prompt injection，见 [CalypsoAI Platform Overview](https://support.calypsoai.com/en/articles/10245110-platform-overview) 和 [About scanners](https://support.calypsoai.com/en/about-scanners-in-the-calypsoai-platform?hsLang=en)。它偏企业 GenAI 使用治理和审计。

### NVIDIA NeMo Guardrails

NeMo Guardrails 是开源工具包，用于在 LLM-based conversational systems 中添加 programmable guardrails；见 [NVIDIA NeMo Guardrails docs](https://docs.nvidia.com/nemo-guardrails/index.html)。NeMo Microservices 文档还描述 prompt/response checking、jailbreak detection；见 [NVIDIA Guardrail Concepts](https://docs.nvidia.com/nemo/microservices/25.10.0/about/core-concepts/guardrails.html)。

### Microsoft

Microsoft 的方向是“平台内 agent governance + Security stack 集成”。Copilot Studio 有 DLP/data policies、security scan、external threat detection、agent runtime protection；Defender/Agent 365 提供 inventory、audit logs、advanced hunting、real-time protection、XDR incident integration。关键资料：[Copilot Studio security and governance](https://learn.microsoft.com/en-gb/microsoft-copilot-studio/security-and-governance)、[Microsoft Defender for AI agents](https://learn.microsoft.com/en-us/defender-xdr/security-for-ai/ai-agent-detection-protection)、[Microsoft Agent 365](https://learn.microsoft.com/en-us/security/security-for-ai/agent-365-security)。

Microsoft Windows Agent Workspace 也说明大厂把 sandbox 当成 agent OS 层控制：agentic AI 会引入 XPIA，可能导致 data exfiltration 或 malware installation，因此需要隔离、日志、有限权限；见 [Experimental Agentic Features](https://support.microsoft.com/en-au/windows/experimental-agentic-features-a25ede8a-e4c2-4841-85a8-44839191dfb3)。

### Google

Google Cloud 的 Agent Builder/Agentspace 强调 agent identity、IAM、registry、observability、audit trail、Model Armor runtime security、Security Command Center；见 [Vertex AI Agent Builder](https://cloud.google.com/products/agent-builder) 和 [Google Agentspace](https://cloud.google.com/products/agentspace/)。Model Armor 是其 prompt/response/agent interaction runtime security 产品，覆盖 prompt injection、sensitive data、malicious URLs、malware/safe browsing，并支持 REST API、Vertex AI、Apigee/API gateway 方式；见 [Model Armor](https://cloud.google.com/security/products/model-armor)。

### Anthropic

Anthropic 是 MCP 的提出方，也在 Claude Code 上强调权限、sandbox、网络/文件隔离。Claude Code sandboxing 文章明确说：没有网络隔离，被攻陷的 agent 可能外传 SSH keys 等敏感文件；没有文件系统隔离，agent 可能逃出 sandbox。文章称 sandboxing 让成功的 prompt injection 被隔离，内部使用中减少了 84% permission prompts；见 [Beyond permission prompts: making Claude Code more secure and autonomous](https://www.anthropic.com/engineering/claude-code-sandboxing)。Anthropic 2026 年关于 containment 的文章把核心问题定义为限制 blast radius，并指出 MCP servers、第三方插件、web search tools 都会把不受控内容喂入 agent context；见 [How we contain Claude across products](https://www.anthropic.com/engineering/how-we-contain-claude)。

### OpenAI

OpenAI 的公开资料强调多层防御而非单点解决。ChatGPT agent 有 high-impact action confirmation、refusal patterns、prompt injection monitoring、watch mode；见 [ChatGPT agent Help Center](https://help.openai.com/en/articles/11752874-chatgpt-agent)。OpenAI 还推出 Lockdown Mode，通过禁用或限制网络工具提供更确定性的 prompt-injection-based exfiltration 防护；见 [Lockdown Mode](https://help.openai.com/articles/20001061/)。针对 URL-based data exfiltration，OpenAI 说明 URL 不只是目的地，也可能携带数据，因此要限制 agent 自动获取 URL 的方式；见 [Keeping your data safe when an AI agent clicks a link](https://openai.com/index/ai-agent-link-safety/)。

## 5. 传统 endpoint / DLP / SIEM / EDR 能否覆盖 agent 行为？

结论：能覆盖一部分底层现象，不能完整覆盖 agent 行为。

传统 EDR 可以看到进程、文件、网络、可疑命令、恶意二进制、credential access、persistence 等 endpoint 行为。如果 agent 执行 `curl | sh`、读取 SSH key、启动反连 shell，EDR 可能报警。但它通常不知道这个行为来自哪个 prompt、哪个 tool call、哪个 MCP server、哪个用户任务，也无法判断“这个 agent 行动是否偏离用户意图”。CrowdStrike 2026 年 AIDR 文章称 prompt layer 已成为新攻击面，传统安全工具不是为监控或解释这些 prompt/response interaction 设计的；见 [Falcon AIDR detects threats at prompt layer](https://www.crowdstrike.com/en-us/blog/falcon-aidr-detects-threats-at-prompt-layer-in-kubernetes-ai-apps/)。

传统 DLP 可以阻止明显 PII、secret、源代码、客户数据发往未授权域名或 SaaS；对员工粘贴敏感数据到 ChatGPT/Gemini/Copilot 这类问题有效。但 agent 泄露经常是语义变形：摘要、重新组织、拼进 URL 参数、发到允许的 Slack/email/Jira webhook、作为工具参数传递给另一个 SaaS。Cyberhaven 也把 DLP for GenAI 定义成需要监控 AI tools、AI agents、AI-assisted applications 的新能力，而不是传统 email/USB DLP；见 [DLP for GenAI](https://www.cyberhaven.com/blog/dlp-for-genai)。

SIEM/XDR 可以汇总日志、做调查、联动告警，但前提是有 agent-specific telemetry。Microsoft Defender 的 AI agent protection 把 agent audit logs、advanced hunting、real-time protection、root cause/blast radius 调查专门列出来，说明传统 SIEM 需要新的 agent 日志源才能有效；见 [Detect, block, and investigate threats to AI agents using Microsoft Defender](https://learn.microsoft.com/en-us/defender-xdr/security-for-ai/ai-agent-detection-protection)。

因此，传统安全栈不是无效，而是缺“agent 语义与 OS effect 的关联层”。AgentSight 可以输出到 SIEM/EDR，而不是替代它们。

## 6. 当前方案对本地 coding agent / CLI agent / MCP 工具调用的盲点

1. Prompt firewall 只看文本，不看真实执行。它可能看到用户请求和模型响应，但看不到 agent 随后是否 `rm -rf`、`git push`、`npm install`、读取 `~/.ssh`、写入 `.env`、启动后台进程。

2. Gateway/proxy 只覆盖经过它的流量。本地 coding agent 可能直接调用 OpenAI/Anthropic API，也可能调用本地模型、浏览器、`curl`、`git`、`pip`、`npm`、MCP stdio server。任何不经网关的工具调用都会绕过。

3. SDK instrumentation 依赖 agent 框架合作。Claude Code、Codex、Cursor、Gemini CLI、LangChain、自定义脚本、MCP server 子进程、shell 脚本和 package manager 行为不一定在同一个 trace 里。被 prompt injection 影响的 agent 也可能生成或调用未被 instrumentation 包住的新脚本。

4. DLP 难覆盖“合法读取 + 语义外发”。Agent 有权读 repo、邮件或数据库，再把摘要发给另一个允许工具；传统 DLP 不一定能判断这是否违反原始任务意图。

5. EDR 能看见行为但缺 agent 上下文。它知道 `python` 进程读了文件、连了网络，却未必知道这是哪个 agent session、哪个 MCP tool、哪个 prompt chain、哪次用户批准造成的。

6. Sandbox 限制 blast radius，但不自动回答“发生了什么”。企业仍需要审计、归因、report、证据导出、策略调优。并且 sandbox 太严会让 coding agent 难用，太松又无法阻断高风险操作。

7. MCP 是本地 agent 的特殊盲区。MCP server 可以通过 tool description、schema、返回值、配置、依赖更新、stdio 通道影响 agent。OWASP MCP03 tool poisoning 和 MCP09 shadow MCP servers 都指向这个问题；见 [MCP03 Tool Poisoning](https://owasp.org/www-project-mcp-top-10/2025/MCP03-2025%E2%80%93Tool-Poisoning) 和 [MCP09 Shadow MCP Servers](https://owasp.org/www-project-mcp-top-10/2025/MCP09-2025%E2%80%93Shadow-MCP-Servers)。

8. Human approval 也可能被 agent 摘要误导。如果用户看到的是 agent 自己生成的“我将读取项目文件”描述，而不是真实 syscall、目标路径、网络目的地和 diff，那么批准界面本身会成为 trust exploitation 的一部分。

## 7. AgentSight 的 eBPF / OS boundary tracing 在安全上是否有真实差异？

有，但差异不是“更准的 prompt injection classifier”，而是“独立证据 + OS 边界控制”。

### 7.1 真实差异

AgentSight 能站在 agent 外部观察：

- 进程树：agent、shell、MCP server、package manager、test runner、后台进程之间的父子关系。
- 文件行为：读、写、创建、删除、rename、truncate、目录创建、敏感路径访问。
- 命令行为：exec、bash readline、危险命令、脚本生成后再执行的链路。
- 网络行为：连接目的地、监听端口、外连时机、可疑域名。
- 会话证据：哪个 agent run 在哪个时间窗口触发了哪些 OS effects。
- 可选语义关联：如果能在 TLS/library boundary 捕获模型请求/响应，就能把 prompt/tool instruction 与 OS 行为关联起来。

Tetragon/Falco 等 eBPF/runtime security 项目已经证明 eBPF 适合 process execution、syscall、network、file access 的实时 observability/enforcement。Tetragon 官方说明其能检测并响应 process execution、system call、I/O activity including network & file access，并可直接在 eBPF 中做 policy/filtering；见 [Tetragon overview](https://tetragon.io/docs/overview/)。AgentSight 的差异在于把这些 OS 事件按 agent session、tool call、prompt/network timeline 组织成 agent-native 证据。

典型场景：

- 被 indirect prompt injection 的 README 诱导 agent 读取 `.env` 并通过 `curl` 发出。Prompt firewall 可能没看到 README 或没命中；EDR 看到 `curl` 但不知道任务上下文；AgentSight 可以给出“读取 `.env` → 连接 unknown domain → 发生在某 agent session 的某段工具链之后”的证据。
- MCP server 声称 read-only，但实际触发 `write`、`rename`、`git commit` 或启动监听端口。AgentSight 不需要相信 MCP 声明，可以用 OS boundary 验证。
- Coding agent 安装依赖时触发 postinstall script、下载二进制、修改 shell profile。SDK trace 可能只显示“安装依赖”；AgentSight 能显示实际进程和文件副作用。

### 7.2 不能夸大的部分

AgentSight 不能单独“解决 prompt injection”。OpenAI、Anthropic 都把 prompt injection 描述为行业级未解问题，OpenAI 还特别提醒单纯 AI firewall 不足以抓住成熟攻击。AgentSight 的安全价值是把成功或疑似成功的攻击限制、记录、归因、复盘，而不是在语义层保证所有恶意文本都被识别。

AgentSight 也不能覆盖所有云端连接器行为。如果 agent 在 OpenAI/Anthropic/Microsoft/Google 云端环境中访问 SaaS connector，本地 eBPF 看不到云端内部工具调用，只能看到本地到 provider 的流量和最终本地 side effects。

TLS 捕获有隐私与工程风险：需要处理企业隐私、密钥/明文保护、采样、脱敏、不同 TLS 库、静态链接、本地模型、HTTP/2/SSE、供应商协议变化。产品上应默认最小化内容捕获，把 OS effect 证据作为核心，把 prompt 内容捕获设计为可选、脱敏、按策略启用。

eBPF 也有部署门槛：Linux、root/CAP_BPF、内核版本、容器/host 权限、EDR 共存、性能预算。桌面 Mac/Windows agent 需要不同技术路线或先限定 Linux/CI/开发容器。

## 8. 哪些场景需要实时阻断，哪些只需要事后审计？

实时阻断适合“高确定性、高影响、低可逆”的行为；事后审计适合“低确定性、需要上下文判断、阻断会严重影响开发体验”的行为。

| 场景 | 建议动作 | 理由 |
| --- | --- | --- |
| 读取 known secret 后立即外连 unknown domain | 阻断或强制人工确认 | 数据泄露路径明确，误报成本低于泄露成本 |
| 写/删工作区外文件，如 `~/.ssh`、`~/.aws`、`/etc`、home 下非项目目录 | 阻断/确认 | coding agent 通常不需要改这些路径 |
| 执行 destructive command：`rm -rf`、`dd`、磁盘 wipe、权限修改、`chmod 777`、`chown -R` | 阻断/确认 | 高破坏、难恢复 |
| 启动监听端口、反连、下载并执行脚本、`curl | sh` | 阻断/确认 | 常见执行链和 exfil/lateral movement 风险 |
| 修改 git remote、推送代码、创建 token、改 CI/CD、触发 deploy | 阻断/确认 | 外部影响大，需要明确用户授权 |
| MCP server 新增/变更工具 schema、请求新增 OAuth scope、从未知包源安装 | 阻断/确认 | 工具供应链与 scope creep 风险 |
| 发送邮件、Slack、Webhook、CRM 更新、工单关闭、付款/退款等外部动作 | 阻断/确认 | 高影响业务动作，应显示真实参数而非 agent 摘要 |
| 读取 repo 文件、运行测试、构建、正常 package install | 审计为主，异常升级 | 开发体验优先，但要保留证据 |
| Prompt injection 分类低置信命中 | 审计/警告 | 语义误报高，直接阻断会使产品不可用 |
| 大量 token、循环调用、重复失败工具调用 | 审计/限流 | 更多是成本/质量问题，不一定是安全事件 |
| 访问允许的 SaaS/API 读接口 | 审计 | 需要与任务意图和数据类别结合判断 |

产品模式建议四档：`record-only`、`warn`、`require-approval`、`deny`。第一版不要默认做大面积 hard block；先用证据与回放建立信任，再把少数高确定性 OS policy 做成实时 airlock。

## 9. 这个方向是否容易变成过重的 enterprise security platform？如何避免？

非常容易。原因是 agent security 会自然牵到 DLP、identity、SIEM、SOAR、GRC、policy-as-code、model risk、browser security、CASB、EDR、cloud posture、MCP gateway、red teaming、compliance reporting。大厂和平台型安全公司已经在这么打包：Microsoft Agent 365/Defender、Google Agentspace/Model Armor、Palo Alto Prisma AIRS、CrowdStrike AIDR、SentinelOne Prompt Security 都在往“统一 AI security platform”走。

AgentSight 应避免成为过重平台的方式：

1. 不做通用企业 DLP。只做 agent run 中可归因的数据访问与外发证据，必要时调用/导出给现有 DLP。

2. 不做通用 prompt firewall。可以集成 Lakera/Model Armor/NeMo/自定义 classifier，但核心卖点是 OS boundary evidence。

3. 不做完整 SIEM。输出结构化事件、OpenTelemetry/SIEM export、HTML/JSON report，让企业安全栈消费。

4. 不做 agent IDE。保持 CLI-first：`agentsight top`、`agentsight record -- claude`、`agentsight report`、`agentsight verify -- mcp-server`。

5. 不先追求全企业 control plane。先服务本地开发机、CI runner、sandbox、coding agent review、MCP 工具验收这几个高痛场景。

6. Policy pack 要少而硬：workspace-only、no-secret-read、no-external-exfil、no-background-listener、confirm-destructive-write、confirm-deploy。不要让用户一开始面对 200 条企业规则。

7. 把“阻断”做成可解释 action gate：展示真实路径、命令、目标域名、tool 参数、进程树，而不是只给一个 AI risk score。

8. 明确产品边界：AgentSight 让 agent work accountable，不替企业决定所有 AI governance。

## 10. 对 AgentSight 的定位判断

最强定位：

> AgentSight is the independent run receipt and OS boundary guard for local and CLI agents.

中文表达：

> AgentSight 不替代 agent 工作，也不替代企业安全平台；它证明 agent 到底对系统做了什么，并在少数高风险边界上提供实时 airlock。

最适合先打的买方/用户：

- 使用 Claude Code、Codex、Cursor、Gemini CLI、Aider、OpenHands 等 coding agent 的开发者和团队。
- 审核 MCP server、agent plugin、skill、workflow 的平台工程师/AppSec。
- 需要把 agent-generated PR 纳入审计的工程经理/安全团队。
- 在 CI/沙箱/开发容器中运行 agent 的 AI platform team。
- 发生过“agent 改坏文件/读了 secret/连了奇怪域名”的团队。

不适合作为第一阶段主战场：

- 全公司 SaaS GenAI 使用治理。
- 纯浏览器 DLP。
- 模型供应链扫描。
- 企业 GRC/ISO/EU AI Act 全量报表。
- 云端 copilot connector 内部行为审计。

## 可产品化机会

1. Local Agent Run Receipt：每次 agent session 生成“做了什么”的证据报告，包括命令、进程树、文件读写、网络目的地、敏感路径、外部副作用。

2. MCP / Tool Verification：运行一个 MCP server 或 agent skill 的测试任务，验证它是否真 read-only、是否越界访问路径、是否启动网络连接、是否读取凭证、是否调用未声明工具。

3. PR Due Diligence Report：给 agent-generated PR 附一份 provenance：生成过程中读了哪些敏感文件、运行了哪些测试、安装了哪些依赖、连了哪些外部域名、是否有未追踪副作用。

4. Workspace Airlock Policy Pack：少数强规则实时阻断：工作区外写、secret read + external connect、destructive command、unknown listener、deploy/push/credential changes。

5. Incident Forensics：用户发现文件丢失、repo 坏了、secret 疑似泄露时，AgentSight 生成可交给人或另一个 agent 的 timeline 和 evidence JSON。

6. Agent Behavior Regression：同一个任务、不同模型/工具/MCP 版本，比较 OS footprint 是否扩大：新增网络目的地、新增敏感路径读取、新增写入目录、新增子进程。

7. SIEM/XDR Evidence Feed：不替代 SIEM，只导出 agent-native events：agent session、tool/MCP identity、process/file/network/security policy decision。

8. MCP Risk Inventory Lite：扫描本地 MCP 配置、命令、包源、环境变量、OAuth scopes、stdio/server 启动方式，并结合动态运行证据给风险。

9. Approval UI Grounded in Reality：高风险动作批准界面显示真实 syscall-level 参数，而不是 agent 自己写的自然语言摘要。

10. Privacy-preserving Agent Audit：默认不保存完整 prompt，只保存 OS evidence；对 prompt/TLS 内容做选择性捕获、脱敏、哈希、局部窗口和本地加密。

## 高风险误区

1. 声称“解决 prompt injection”。更可信的说法是降低 blast radius、提供证据、在确定性边界阻断。

2. 变成又一个 prompt firewall。市场已有 Lakera、Model Armor、CalypsoAI、NeMo、HiddenLayer、Lasso 等，AgentSight 的差异是 OS effect。

3. 过早做企业平台。不要第一版就做 GRC、全量 DLP、identity lifecycle、SOAR、所有 SaaS connector。

4. 默认抓取全部 prompt/TLS 明文。隐私、合规、员工信任和企业法务都会成为障碍。内容捕获必须可控、可脱敏、可关闭。

5. 只给风险分数不给证据。安全团队和开发者要看路径、命令、域名、进程、时间线、diff，而不是“风险 87 分”。

6. 阻断过多导致 agent 不可用。Coding agent 的正常行为本来就包括读很多文件、跑很多命令、安装依赖；策略必须从少数高确定性动作开始。

7. 只看单进程。真实 agent 会拉起 shell、MCP server、node/python 子进程、package manager、test runner、browser helper。进程树是核心。

8. 忽略云端 agent。AgentSight 本地 eBPF 对云端 connector 行为不可见，要清楚边界。

9. 把 MCP 当普通 API。MCP 的 tool description、schema、stdio、server 更新、scope、上下文共享都会影响模型行为，风险比普通 REST API 更贴近 prompt injection。

10. 忽略部署现实。Linux/root/CAP_BPF、EDR 共存、性能、日志体积、内核兼容、容器权限都会决定企业是否敢试。

## 需要访谈的问题

面向开发者/团队 lead：

1. 你们现在用哪些 coding agent/CLI agent？是否允许它们自动执行 shell、改文件、联网、安装依赖？
2. 你们最担心 agent 在本地做错什么：删文件、泄露 secret、改错代码、提交/推送、安装恶意依赖、连接外部服务，还是成本失控？
3. 出问题时，你们现在如何复盘 agent 做过什么？shell history、git diff、agent transcript、IDE logs、CI logs 哪些够用，哪些不够？
4. 你们愿意在哪些动作上接受实时确认？哪些确认会让工作流不可用？
5. PR reviewer 是否会关心 agent 生成代码时读过哪些文件、运行过哪些命令、是否访问过外部网络？

面向 AppSec / security engineer：

6. 你们是否已经把 Claude Code、Codex、Cursor、MCP server 纳入安全审查？审查标准是什么？
7. 现有 EDR/DLP/SIEM 是否能把某次 endpoint 行为归因到具体 agent session？
8. 哪些 agent 行为必须进入 SIEM？字段需要包括 user、agent、tool、MCP server、command、file path、network destination、policy decision 吗？
9. 对 prompt/TLS 内容捕获的隐私底线是什么？能接受哈希、摘要、局部窗口、脱敏，还是完全不能捕获？
10. 你们更想先要 detect/audit，还是 enforce/block？阻断误报的可接受率是多少？

面向 AI platform / infra：

11. Agent 运行在开发机、CI、容器、Kubernetes、云端 sandbox，还是混合环境？
12. 是否有统一 agent registry / MCP registry？如果没有，shadow agent/MCP 如何发现？
13. 你们是否需要比较 agent 版本、MCP 版本、prompt 版本带来的行为差异？
14. 是否已经使用 LangSmith/Langfuse/OpenTelemetry 等 agent observability？这些 trace 是否能覆盖子进程和 OS side effects？
15. eBPF/root 权限在你们环境中是否可行？EDR/安全基线是否允许部署 kernel-level sensor？

面向 compliance / governance：

16. 哪些 agent run 需要保留审计证据？保留多久？谁可以看？
17. 审计报告需要映射到哪些框架：SOC 2、ISO 27001、ISO 42001、NIST AI RMF、EU AI Act、内部 SDLC？
18. 你们更看重“证明没有越界”还是“出事后能追责”？
19. 对 agent 访问客户数据、源代码、凭证、生产系统的审批流程是什么？
20. 如果 AgentSight 只做本地/CLI agent 证据层，并导出到现有 SIEM/DLP，你们是否认为这是独立预算，还是必须并入现有 endpoint/security platform？
