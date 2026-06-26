# AgentSight 当前竞品与替代方案全景

调研日期：2026-06-02。范围：LLM/agent observability、GenAI OpenTelemetry、传统 APM 的 GenAI 功能、以及 coding-agent 专用 transcript/cost 工具。本文只基于公开文档、公开仓库和公开产品页；AgentSight 自身能力基于本仓库 README 和设计文档。

## 0. 摘要结论

1. 主流市场已经把“LLM 调用可观测性”做成红海：LangSmith、Langfuse、Phoenix、Braintrust、W&B Weave、AgentOps、Datadog、New Relic、Honeycomb、Traceloop/OpenLLMetry 都在解决 prompt/response、token/cost、latency、tool span、RAG/retrieval、eval、prompt experiment 这些问题，集成方式主要是 SDK、auto-instrumentation、OTel、proxy/gateway 或平台内置。[LangSmith](https://docs.langchain.com/langsmith/observability)、[Langfuse](https://langfuse.com/docs/observability/overview)、[Phoenix](https://arize.com/docs/phoenix)、[Braintrust](https://www.braintrust.dev/docs/instrument/trace-llm-calls)、[W&B Weave](https://docs.wandb.ai/weave/)、[AgentOps](https://docs.agentops.ai/v2/quickstart)、[Datadog](https://www.datadoghq.com/product/ai/llm-observability/)、[New Relic](https://docs.newrelic.com/docs/ai-monitoring/intro-to-ai-monitoring/)、[Honeycomb](https://docs.honeycomb.io/send-data/use-cases/llm)、[Traceloop](https://docs.traceloop.com/docs/introduction)

2. 这些工具通常能看到“应用或 agent 自己报告的行为”，但默认看不到独立的 OS-level side effects：真实子进程树、grandchild process、文件 open/read/write/delete/rename、网络连接目标、secret 文件访问、后台进程残留等。这个判断是基于其公开集成方式得出的推断：SDK/OTel/proxy/log-reader 只能收集被插桩、被代理、被上报或被本地日志记录的事件；它们不会自动成为内核级事实源。[Langfuse integrations](https://langfuse.com/integrations)、[Helicone platform overview](https://docs.helicone.ai/getting-started/platform-overview)、[Phoenix OTEL setup](https://www.arize.com/docs/phoenix/tracing/how-to-tracing/setup-tracing/setup-using-phoenix-otel)、[OpenTelemetry GenAI semconv](https://opentelemetry.io/docs/specs/semconv/gen-ai/)

3. OpenTelemetry GenAI semantic conventions 正在成为事实标准入口，但仍标注为 Development，并且主要定义 model spans、agent spans、tool spans、events、metrics、provider metadata、token usage 等 GenAI 语义，不等同于 OS-level forensic tracing。[OpenTelemetry GenAI](https://opentelemetry.io/docs/specs/semconv/gen-ai/)、[OpenTelemetry Agent spans](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-agent-spans/)

4. Coding-agent 侧出现了新一类本地账本工具：Claude Code 官方支持 OTel metrics/logs/traces 和 hooks，ccusage/CodeBurn 读取 Claude/Codex/Cursor/Gemini/OpenCode 等本地 session 数据做 token/cost/report，codlogs 读取 Codex 本地 JSONL session，cursor-history 读取 Cursor 本地聊天历史。这证明“本地 coding agent 使用账本、成本解释、session replay/export”是真需求，但这些工具大多仍依赖 agent 自己写入的 JSONL/SQLite/telemetry，不是独立 OS 证据。[Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage.md)、[Claude Code hooks](https://code.claude.com/docs/en/hooks.md)、[ccusage](https://github.com/ryoppippi/ccusage)、[CodeBurn](https://github.com/getagentseal/codeburn)、[codlogs](https://github.com/tobitege/codlogs)、[cursor-history](https://github.com/S2thend/cursor-history)

5. AgentSight 最可验证的差异不是“又一个 trace UI”，而是“从 agent/application 外部观察系统边界”：eBPF/uprobes 捕获 LLM TLS 流量，process tracer 记录进程和文件事件，并把 LLM 交互与进程/文件/系统活动关联；现有 README 明确强调不需要 SDK 或 proxy，可观察已有二进制和 CLI agent。[AgentSight README](../../../README.md)、[AgentSight product scope](../product-scope-agent-native.md)

6. AgentSight 还没有被市场验证的假设包括：用户是否愿意给本地开发机或 CI runner 授予 eBPF/root 权限；企业是否接受 plaintext LLM payload 捕获；独立 side-effect receipt 是否能成为购买理由；以及 Mac/Windows 支持缺失会不会阻断 coding-agent 场景。[AgentSight README](../../../README.md)、[process tracer extension plan](../PLAN_process_tracer_extension.md)

## 1. 市场分层

| 层级 | 代表产品 | 实际解决的问题 | 典型集成方式 | OS-level side effects 可见性 |
| --- | --- | --- | --- | --- |
| LLM app tracing/eval 平台 | LangSmith、Langfuse、Phoenix、Braintrust、W&B Weave、AgentOps | debug agent/RAG、多步骤调用、成本、质量评估、prompt/version/dataset/experiment | SDK、decorator、auto-instrumentation、OTel、framework integration | 默认不可见；只能看到应用或 agent 上报的 tool span、file path、command string |
| LLM gateway/proxy | Helicone、Braintrust AI proxy、LiteLLM/Portkey 类替代方案 | 低改动记录 API 请求、成本、latency、routing/fallback/caching | base URL 替换、gateway/proxy、provider wrapper | 只能看到经过 gateway 的 LLM 请求；看不到本地工具执行和文件/进程副作用 |
| OTel GenAI 与传统 APM | OpenTelemetry GenAI、Datadog、New Relic、Honeycomb、Traceloop/OpenLLMetry | 把 GenAI spans 放进现有 tracing/APM/infra 后端，统一治理、采样、脱敏、路由 | OTel SDK/Collector、vendor agent、auto-instrumentation、HTTP API | 能和已有 infra/app telemetry 并排分析；但没有自动建立 coding agent 到 OS syscall 的完整事实链 |
| Coding-agent transcript/cost 工具 | Claude Code OTel/hooks、ccusage、CodeBurn、codlogs、cursor-history、Langfuse Cursor/Claude integrations | 本地 coding agent session 查账、成本、导出、回放、token waste 分析 | 官方 OTel、hooks、读取 JSONL/SQLite/local logs、CLI | 可见 agent 记录的 transcript/tool/cost；不能保证看见 agent 未记录或日志被关闭/遗漏的 OS 行为 |
| AgentSight 目标层 | AgentSight | 独立运行收据、side-effect forensic、进程/文件/网络/LLM 关联、PR due diligence | eBPF + TLS uprobe + process tracer + `agentsight exec/record` | 当前最强差异点；仍需把 deletion/rename/network 等覆盖做实并验证低开销 |

## 2. 单项产品分析

### 2.1 LangSmith

LangSmith 解决的问题是 LangChain/LangGraph 以及通用 LLM app 的 tracing、monitoring、evaluation、prompt engineering 和 deployment。文档描述其可以捕获完整 request trace，从输入、最终输出到中间步骤，并支持 production monitoring、automation、feedback、online evaluation。[LangSmith observability](https://docs.langchain.com/langsmith/observability)、[LangSmith tracing quickstart](https://docs.langchain.com/langsmith/observability-quickstart)

集成方式：LangChain/LangGraph 可通过环境变量启用；其他 provider 可用 wrappers 或 `@traceable` 手动 trace；非 LangChain 应用也可通过标准 OpenTelemetry 客户端向 LangSmith 发送 trace。[Tracing quickstart](https://docs.langchain.com/langsmith/observability-quickstart)、[Trace with OpenTelemetry](https://docs.langchain.com/langsmith/collector-proxy)

可见内容：LLM calls、chain/agent steps、inputs/outputs、metadata、latency、token/cost、feedback/eval。不可见内容：除非应用显式记录，LangSmith 看不到实际 `execve` 进程树、child process 的文件访问、network connect、文件删除/重命名等 OS 事实。这是基于其 SDK/OTel 集成方式的推断，不是 LangSmith 文档直接声明的限制。[Trace with OpenTelemetry](https://docs.langchain.com/langsmith/collector-proxy)

对 AgentSight 的含义：不要硬碰 LangChain-native trace/eval/prompt workflow。AgentSight 应把 LangSmith 当成上层消费端或互补对象，提供 LangSmith 无法独立获得的本地 side-effect evidence。

### 2.2 Langfuse

Langfuse 解决 LLM application tracing、prompt management、evaluation、datasets、experiments、cost/token tracking、session grouping 等问题。其 observability 文档把 trace 定义为记录 exact prompt、model response、token usage、latency、tools/retrieval/custom logic 的结构化日志。[Langfuse observability](https://langfuse.com/docs/observability/overview)

集成方式：Python SDK、JS/TS SDK、OpenTelemetry endpoint、provider/framework/gateway integrations。Langfuse 文档明确说其基于 OpenTelemetry，可用 SDK 或直接从任意语言向 OTel endpoint 发送 traces。[Langfuse integrations](https://langfuse.com/integrations)

可见内容：nested observations、model/tool/retrieval/custom spans、inputs/outputs、cost、timing、eval scores、prompt versions。不可见内容：默认没有独立 OS 进程、文件、网络事实；如果 Claude/Cursor integration 记录了 file operations 或 shell commands，那也是从 agent hooks/transcripts/logs 中读取或转写，不等同于内核层实际副作用。[Langfuse Claude Code integration](https://langfuse.com/integrations/other/claude-code)、[Langfuse Cursor integration](https://langfuse.com/integrations/other/cursor)

特别注意：Langfuse 已经覆盖 coding-agent integrations。Claude Code 方案通过 hooks 读取 transcript 并转成 Langfuse traces；Cursor 方案声称捕获 prompts、responses、file reads/edits、shell commands、MCP tool calls、session/performance metrics。[Claude Code integration](https://langfuse.com/integrations/other/claude-code)、[Cursor integration](https://langfuse.com/integrations/other/cursor)

对 AgentSight 的含义：Langfuse 在 OSS/self-host LLM observability 上很强，是红海正面竞品。但它也可成为 AgentSight 的输出后端：AgentSight 可以把独立 OS evidence 转成 OTel/trace 或 Langfuse-compatible records。

### 2.3 Helicone

Helicone 解决“低改动记录 LLM API 请求、成本、latency、errors、routing/fallback”问题。平台文档称 Helicone 有两种路径：AI Gateway pass-through billing，或 bring-your-own API keys 的 observability-only mode；每个 request 会记录 costs、latency、errors。[Helicone platform overview](https://docs.helicone.ai/getting-started/platform-overview)

集成方式：AI Gateway、OpenAI-compatible base URL/proxy、SDK。Gateway 提供统一 OpenAI-compatible API，支持 100+ providers、routing、fallback、provider switching。[Helicone gateway](https://docs.helicone.ai/gateway)

可见内容：经过 gateway/proxy 的 LLM requests/responses、headers/metadata、cost、latency、errors、model/provider。不可见内容：不经过 gateway 的调用、agent tool execution、本地 shell/file/network side effects、非 LLM HTTP 请求。Helicone availability 文档也强调默认是 proxy LLM requests，并提供 OpenLLMetry bypass proxy 的路径。[Helicone availability](https://docs.helicone.ai/references/availability)

对 AgentSight 的含义：不要做“又一个 LLM API gateway/cost dashboard”。AgentSight 可以处理 gateway 看不到的本地二进制和 CLI agent 行为。

### 2.4 Arize Phoenix

Phoenix 解决 AI/LLM app 的 debugging、tracing、evaluation、prompt iteration、datasets/experiments。官方文档称 Phoenix built on OpenTelemetry and powered by OpenInference，trace 捕获 model calls、retrieval、tool use、custom logic；Phoenix 接受 OTLP traces，并提供 OpenAI、Bedrock、Anthropic、LangChain、LlamaIndex、DSPy、Vercel AI SDK 等 auto-instrumentation。[Phoenix overview](https://arize.com/docs/phoenix)

集成方式：`arize-phoenix-otel`/`@arizeai/phoenix-otel`、OpenTelemetry registration、OpenInference instrumentation、OTLP collector endpoint、self-host/cloud。[Phoenix OTEL setup](https://www.arize.com/docs/phoenix/tracing/how-to-tracing/setup-tracing/setup-using-phoenix-otel)

可见内容：OpenInference/OTel spans、model/retrieval/tool/custom logic、eval scores、datasets、prompt replay。不可见内容：没有默认 OS syscall 视角；如果工具执行被框架记录为 span，可见的是框架层 tool event，不是实际 file/process/network side effects。

对 AgentSight 的含义：Phoenix 是 OTel/OpenInference-first 的强竞品。AgentSight 差异应定位为 Phoenix 上游的“独立 evidence producer”，而不是 Phoenix 替代品。

### 2.5 Braintrust

Braintrust 解决 observability + systematic evaluation + prompt/playground/datasets/experiments 的问题。文档称 Braintrust auto-instrumentation 可在 startup 后记录 supported AI providers/frameworks 的 inputs、outputs、model parameters、latency、token usage、costs；evaluation 文档强调把 production traces 拉入 datasets，提高 offline test coverage。[Braintrust trace LLM calls](https://www.braintrust.dev/docs/instrument/trace-llm-calls)、[Braintrust evaluate](https://www.braintrust.dev/docs/evaluate)

集成方式：SDK auto-instrumentation、client wrapper、AI gateway/proxy、manual spans、OpenTelemetry 或 HTTP/API 类路径。[Braintrust wrap providers](https://www.braintrust.dev/docs/instrument/wrap-providers)

可见内容：LLM calls、application logic spans、tool calls、retrieval/business logic（如果 trace）、eval scores、datasets、experiments。不可见内容：默认不能独立发现未记录的 subprocess/file/network side effects。

对 AgentSight 的含义：Braintrust 在 eval-driven engineering 上成熟，AgentSight 不应进入 eval CI 平台正面战场。可把 AgentSight evidence 作为 Braintrust dataset/eval 的输入，验证 agent 声称完成的工作是否实际发生。

### 2.6 W&B Weave

Weave 解决 LLM app observability、debug、evaluation、RAG evaluation、LLM judges/custom scorers，并与 W&B 生态连接。官方文档称 Weave 用 Python/TypeScript library tracing LLM calls，观察 inputs/outputs，评估 response quality。[W&B Weave docs](https://docs.wandb.ai/weave/)

集成方式：Python/TypeScript SDK、`weave.init`、implicit/explicit patching OpenAI/Anthropic 等 libraries、AI SDK OTel backend routing。[Weave integrations](https://weave-docs.wandb.ai/guides/integrations/)、[AI SDK Weave observability](https://ai-sdk.dev/providers/observability/weave)

可见内容：patched library calls、call tree、inputs/outputs、code/metadata、latency/cost、evals。不可见内容：未被 SDK patch/instrument 的 CLI child process、文件系统副作用、网络 side effects。

对 AgentSight 的含义：W&B 用户会把 Weave 当现有 ML/LLM workflow 的默认可观测平台。AgentSight 应定位为“可导出的本地运行证据”，而不是替代 W&B experiment/eval。

### 2.7 AgentOps

AgentOps 解决 AI agents/LLM apps 的 session-level observability、debugging、cost/token/errors、multi-agent interactions、tool usage。官方 quickstart 称两行 Python 即可开始监控 supported LLM and agent framework calls，core concepts 文档称 AgentOps built on OpenTelemetry，并支持 automated instrumentation、decorators、sessions、span hierarchy、agents、LLM events。[AgentOps quickstart](https://docs.agentops.ai/v2/quickstart)、[AgentOps core concepts](https://docs.agentops.ai/v2/concepts/core-concepts)、[AgentOps sessions](https://docs.agentops.ai/v1/concepts/sessions)

集成方式：Python SDK、TypeScript SDK、`agentops.init()`、decorators、framework integrations、OTel foundation。

可见内容：agent sessions、LLM calls、tool usage、multi-agent interactions、cost/token/errors、host environment metadata。不可见内容：默认只能看到 SDK/framework 报告的行为，不能独立确认本地 OS 文件/进程/网络副作用。

对 AgentSight 的含义：AgentOps 名字和目标用户相近，但其核心仍是 app/framework-level agent tracing。AgentSight 应避免“agent session dashboard”泛化竞争。

### 2.8 Traceloop / OpenLLMetry

Traceloop/OpenLLMetry 解决 OTel-native LLM app tracing 和 vendor-neutral instrumentation。Traceloop 文档称可安装 OpenLLMetry SDK 或用 Traceloop Hub smart proxy；OpenLLMetry 基于 OpenTelemetry，可接入任何 OTel backend。[Traceloop intro](https://docs.traceloop.com/docs/introduction)、[OTel collector integration](https://docs.traceloop.com/docs/openllmetry/integrations/otel-collector)

集成方式：OpenLLMetry SDK、auto-instrumentation、OTLP endpoint、OpenTelemetry Collector、smart proxy。GitHub README 描述其包含 LLM providers 和 Vector DBs 的标准 OpenTelemetry instrumentations，并输出标准 OpenTelemetry data。[OpenLLMetry GitHub](https://github.com/traceloop/openllmetry)

可见内容：OpenAI/Anthropic/Cohere 等 LLM calls、vector DB calls、LangChain/Haystack 等 framework spans、prompt/response、token、latency、model metadata。不可见内容：非 instrumented OS side effects。

对 AgentSight 的含义：OpenLLMetry 是“把 LLM app 变成 OTel spans”的成熟开源路径。AgentSight 应尽量兼容 OTel GenAI，而不是重造语义层。

### 2.9 Datadog LLM Observability

Datadog 解决 enterprise teams 把 AI workload 放进 Datadog，与 backend services、infra、real user sessions 关联，并提供 tracing、experiments、evaluations、sensitive data scanning。产品页说明可以通过 Python/Node/Java tracer、OpenTelemetry 或 HTTP API 接入；每个 LLM provider call 是 LLM span。[Datadog LLM Observability](https://www.datadoghq.com/product/ai/llm-observability/)

Datadog 已原生支持 OTel GenAI semantic conventions v1.37+，可以从 OTel Collector 或 Datadog Agent OTLP ingest 进入 LLM Observability，并映射 `gen_ai.request.model`、`gen_ai.usage.input_tokens`、`gen_ai.provider.name`、`gen_ai.operation.name` 等字段。[Datadog OTel GenAI support](https://www.datadoghq.com/blog/llm-otel-semantic-convention/)

可见内容：LLM spans、agent/tool spans、latency、token/cost、prompts/outputs、evals、APM/infra/logs 关联。不可见内容：除非另有 Datadog instrumentation 或 OTel spans，LLM Observability 本身不会独立捕获本地 coding agent 的真实 file/process/network side effects。即使 Datadog 能看到 host/process metrics，也需要明确 trace context 和事件模型才能证明“某次 agent prompt 导致某个文件删除”。

对 AgentSight 的含义：Datadog 是企业现有后端，不是适合作为 UI/trace store 的正面对手。AgentSight 可输出 OTel/Datadog-compatible evidence。

### 2.10 New Relic AI Monitoring

New Relic AI monitoring 解决 APM for AI：通过 APM agents 获取 LLM observability data，提供 performance、cost、quality、prompt/response details、model comparison、feedback correlation。官方文档称要先 instrument AI-powered app，然后 enable AI monitoring。[New Relic AI monitoring](https://docs.newrelic.com/docs/ai-monitoring/intro-to-ai-monitoring/)

集成方式：New Relic APM agents、AI monitoring 配置、OpenTelemetry 路由、OpenLLMetry 到 New Relic OTLP endpoint。New Relic 的 Traceloop 页面明确说明 OpenLLMetry built on OpenTelemetry，支持 OpenAI、HuggingFace、Pinecone、LangChain，并可把 traces 路由到 New Relic OTLP endpoint。[New Relic OpenLLMetry](https://docs.newrelic.com/docs/opentelemetry/get-started/traceloop-llm-observability/traceloop-llm-observability-intro/)

可见内容：supported model calls、vector store data、prompt/completion tokens、requests/responses、user feedback、APM context。不可见内容：同 Datadog，除非另外插桩或系统监控并建立 causal link，否则不能独立证明 coding agent 的 OS side effects。

### 2.11 Honeycomb GenAI / Agent Timeline

Honeycomb 解决用 OpenTelemetry 监控 LLM-powered applications 和 AI agents。文档强调 OpenTelemetry traces 可把 LLM request behavior 与应用其他行为关联，并用于分析 latency、retrieval、validation、output correction 等问题。[Honeycomb LLM use case](https://docs.honeycomb.io/send-data/use-cases/llm)

Agent 文档要求用 OTel GenAI semantic conventions 给 spans 加 `gen_ai.conversation.id`、`gen_ai.agent.name`、`gen_ai.operation.name`、token、model、tool call arguments/result 等属性，从而在 Agent Timeline 中探索 agent sessions。[Honeycomb agents](https://docs.honeycomb.io/send-data/use-cases/agents)

可见内容：OTel spans/events、agent timeline、tool calls、model tokens、prompt/completion events、eval result events。不可见内容：没有自动 OS syscall 追踪。Honeycomb 文档还提到 Claude Code 可以 emit telemetry，但需要 transform processor 把 Claude Code spans/attributes remap 到 GenAI semconv；这说明现有 coding-agent telemetry 已可接入 APM，但仍是 agent 自己发出的 telemetry。[Honeycomb agents](https://docs.honeycomb.io/send-data/use-cases/agents)

### 2.12 OpenTelemetry GenAI semantic conventions

OpenTelemetry GenAI semantic conventions 的重要性在于提供跨 vendor 的统一字段和 span/event/metric vocabulary。当前页面状态为 Development，并定义 events、exceptions、metrics、model spans、agent spans，以及 Anthropic、AWS Bedrock、Azure AI Inference、OpenAI、MCP 等 provider/protocol-specific conventions。[OpenTelemetry GenAI](https://opentelemetry.io/docs/specs/semconv/gen-ai/)

Agent spans 定义 `create_agent`、`invoke_agent`、`invoke_workflow`、`execute_tool` 等语义，并把 tool-capable/self-directed workflows 归入 agent 概念；它描述的是语义 tracing，不是 OS forensic tracing。[OpenTelemetry Agent spans](https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-agent-spans/)

对 AgentSight 的含义：OTel GenAI 是 AgentSight 应兼容的出口，不是差异本身。差异应来自“AgentSight 能产生别人没有的系统边界证据”，然后映射成 OTel spans/events。

## 3. Coding-agent transcript/cost 工具

### 3.1 Claude Code 官方 OTel 与 hooks

Claude Code 官方 monitoring 文档称其可通过 OpenTelemetry 导出 metrics、logs/events，并可选导出 traces；用途是 track usage、costs、tool activity。配置使用 `CLAUDE_CODE_ENABLE_TELEMETRY`、`OTEL_METRICS_EXPORTER`、`OTEL_LOGS_EXPORTER`、`OTEL_TRACES_EXPORTER` 等环境变量。[Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage.md)

Claude Code traces 当前是 beta，span hierarchy 包含 `claude_code.interaction`、`claude_code.llm_request`、`claude_code.tool`、hook spans、subagent spans；tool span 可在 gate 开启时记录 `file_path`、`full_command`、skill/subagent 信息和 tool input/output content。[Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage.md)

Claude Code hooks 是 lifecycle hook，可在 SessionStart、UserPromptSubmit、PreToolUse、PostToolUse、SubagentStart/Stop、Stop、SessionEnd 等事件执行 shell/HTTP/LLM prompt hook，并可在 PreToolUse 阶段阻止某些命令。[Claude Code hooks](https://code.claude.com/docs/en/hooks.md)

可见内容：Claude Code 自身知道并发出的 prompt/model/tool/hook/subagent/cost/token/file_path/command 信息。不可见内容：这仍然不是 OS 独立事实源；例如 Bash 命令内部产生的 grandchild process、库加载、实际文件 open/write/delete、网络连接、环境变量读取，需要额外系统追踪才可验证。Claude 文档还说明它不把 `OTEL_*` 环境变量传给 Bash tool、hooks、MCP servers、language servers；这进一步说明外部进程 telemetry 需要另行配置。[Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage.md)

### 3.2 ccusage

ccusage 是本地数据读取型工具，README 写明它分析 coding agent CLI token usage and costs from local data，并支持 Claude Code、Codex、OpenCode、Amp、Droid、Codebuff、Hermes Agent、Goose、OpenClaw、Kimi、Qwen、GitHub Copilot CLI、Gemini CLI 等 sources；支持 daily/weekly/monthly/session reports、Claude 5-hour billing windows、model breakdown、JSON output、cost tracking、cache token support。[ccusage GitHub](https://github.com/ryoppippi/ccusage)

集成方式：CLI 读取本地 session/usage 数据，不需要 SDK/proxy。可见内容：各工具本地日志里的 token、model、cost、session/project/time。不可见内容：OS-level side effects；它不是 process/file/network tracer。[ccusage GitHub](https://github.com/ryoppippi/ccusage)

### 3.3 CodeBurn

CodeBurn README 称其 tracking token usage、cost、performance across 25 AI coding tools，按 task type、model、tool、project、provider 拆分；“Everything runs locally. No wrapper, no proxy, no API keys. CodeBurn reads session data directly from disk and prices every call using LiteLLM.” 支持 Claude Code、Claude Desktop、Codex、Cursor、cursor-agent、Gemini CLI、Copilot、OpenCode、OpenClaw、Goose、Kiro、Roo/Kilo、Qwen、Warp 等。[CodeBurn GitHub](https://github.com/getagentseal/codeburn)

CodeBurn 的功能包括 dashboard、today/month/report/export、optimize、compare、yield、models；它会分析 waste patterns、unused MCP servers、bloated `CLAUDE.md`、retry/one-shot rate、productive vs reverted/abandoned spend。[CodeBurn GitHub](https://github.com/getagentseal/codeburn)

可见内容：本地 session 数据、token/cost、模型、项目、工具、task category、部分文件级 retry。不可见内容：与 ccusage 类似，不是 OS 级独立事实源。CodeBurn 自己也说明不同 provider 的数据质量不同，例如 Cursor 本地 SQLite 的 Auto model 成本只能按 Sonnet 估算，Cursor 不记录 individual tool calls；Kiro token counts 从 content length 估算。[CodeBurn GitHub](https://github.com/getagentseal/codeburn)

### 3.4 Codex/Codex logs 工具

OpenAI Codex CLI 是本地运行的 coding agent；OpenAI 官方开发者页称 Codex 是“one agent for everywhere you code”，官方 GitHub README 称 Codex CLI runs locally on your computer，并区分 CLI、IDE、desktop app、Codex Web。[OpenAI Codex Developers](https://developers.openai.com/codex)、[OpenAI Codex GitHub](https://github.com/openai/codex)

codlogs 是读取 Codex local sessions 的 read-only 工具，支持扫描 `~/.codex` 或 `%CODEX_HOME%`，查找 sessions，导出 `.jsonl` session 为 Markdown/HTML，包含 token summary、errored tool calls、large-session bounded scanning、session sanitization 等。[codlogs GitHub](https://github.com/tobitege/codlogs)

可见内容：Codex session JSONL 里已有的 transcript、tool calls、token counts、images/tool outputs。不可见内容：Codex 没有写入或日志被裁剪/加密/遗漏的 OS side effects。

### 3.5 Cursor history / Cursor agent tracing

Cursor 官方文档提供 Agent chat history UI，可在 Agent sidepane 的 history icon 查看过往 agent conversations。[Cursor history docs](https://docs.cursor.com/agent/chat/history)

cursor-history 是开源 CLI/library，用于 browse、search、export、backup Cursor AI chat history，能显示 full conversations、file edit diffs、detailed tool calls、thinking blocks、timestamps、search/export/migrate/backup/restore。[cursor-history GitHub](https://github.com/S2thend/cursor-history)

Langfuse Cursor integration 也说明可以 trace Cursor agent interactions，包括 user prompts、agent responses、file operations、shell commands、MCP tool calls、session tracking、performance metrics。[Langfuse Cursor integration](https://langfuse.com/integrations/other/cursor)

可见内容：Cursor 本地历史和 integration 能提供 agent UI/log 层的 conversation/tool/file edit 信息。不可见内容：实际 OS-level 进程树、文件 open/delete/rename、网络连接仍需外部 tracer 验证。

## 4. 它们能看到什么，看不到什么

| 观测对象 | SDK/OTel tracing 平台 | Gateway/proxy | APM GenAI | Coding-agent local log tools | AgentSight |
| --- | --- | --- | --- | --- | --- |
| prompt / response | 能，通常是核心功能 | 能，只限经过 proxy 的 LLM API | 能，取决于 OTel/agent 配置 | 能，取决于 transcript | 能，通过 TLS/LLM traffic capture，受 TLS library/provider 支持限制 |
| token / cost | 能，常见功能 | 能，常见功能 | 能，常见功能 | 能，常见功能或估算 | 能，从 LLM traffic/adapters 推导，需与 provider billing 校准 |
| tool call span | 能，若框架或 agent 上报 | 不能，除非 tool call 作为 LLM API payload 可见 | 能，若按 GenAI semconv 上报 | 能，若 transcript 记录 | 能看到 agent traffic 里的 tool use，并可和 OS effect 对齐 |
| Bash command 字符串 | 能，若 tool span 记录 | 通常不能 | 能，若 agent telemetry 记录 | 能，若 transcript 记录 | 能，通过 process tracer 看到实际 exec，同时可从 LLM/tool payload 看到命令意图 |
| child/grandchild process tree | 通常不能 | 不能 | 需要额外 host/process telemetry 且要做因果关联 | 不能 | 是 AgentSight 差异点，当前 README 已强调 process execution；主 process tracer 也支持 fork/session/pgrp 等聚合事件 |
| file read/write/open/delete/rename | 通常只能看 agent 记录的 file path/tool args | 不能 | 需要额外 file/process instrumentation | 只能看 transcript/log 记录 | 当前可记录文件访问，并通过 process tracer 扩展 flag 覆盖删除/重命名/写入等聚合事件 |
| network destination | LLM provider endpoint 通常可见；其他外连不可见 | 经过 proxy 的 LLM endpoint 可见 | 需要 APM/network telemetry | 通常不可见 | process tracer 支持 bind/listen/connect 聚合事件，但产品化展示仍需继续打磨 |
| secret access / sensitive path access | 除非应用记录 | 不能 | 需要 file access telemetry | 通常不能 | 有潜力通过文件访问证据识别，但需产品化规则和隐私策略 |
| tamper resistance | 低到中，应用可关闭/绕过 SDK | 中，只对经过 proxy 的流量有效 | 中，取决于 agent/collector deployment | 低，依赖本地日志 | 较高，独立于被观测进程，但需要 root/eBPF 权限且仍有平台/协议限制 |

## 5. AgentSight 的可验证差异

### 5.1 已有公开/仓库证据支持的差异

1. 外部边界观测，而非 app/framework 插桩。AgentSight README 明确说无需 SDK 或 proxy，可观察已有二进制和 CLI agents，并关联 LLM traffic 与 process execution、file access、system activity。[AgentSight README](../../../README.md)

2. 面向 local coding agents 的系统行为审计。README 提到 `agentsight exec -- claude`、attach to process、Claude Code monitoring、Node.js AI tools、Docker/container support、web UI、process tree、file operations、subprocess executions、OpenTelemetry GenAI export。[AgentSight README](../../../README.md)

3. Claude Code 这类静态链接/剥离符号二进制的 TLS 捕获工程经验。README 和 Claude Code analysis 说明 Claude Code/Bun/BoringSSL 的特殊处理，包括 binary-path、byte-pattern matching、HTTP Client thread 等。[AgentSight README](../../../README.md)、[Claude Code analysis](../../claude-code-analysis.md)

4. 产品场景不是“trace UI”，而是 run receipt、review/acceptance、incident forensics、PR due diligence、behavioral regression testing、live airlock、cost/resource accounting。内部 product scope 明确把“证明发生了什么、保留证据、关联事件、强制信任边界”作为 AgentSight 价值。[AgentSight product scope](../product-scope-agent-native.md)

5. OS side-effect 覆盖正在进入主 process tracer：扩展 flag 覆盖 delete/rename/mkdir/write/truncate/chdir、bind/listen/connect、signals/fork/session/pgrp、mmap/COW 等聚合事件；市场层面仍需要验证这些事件如何转化成默认 UI、review receipt 和付费价值。[process tracer extension architecture](../PLAN_process_tracer_extension.md)

### 5.2 只是我们认为有价值，但还没有市场验证的差异

1. “独立 OS receipt”是否有付费意愿。ccusage/CodeBurn 证明用户关心 coding-agent cost/session，但不能直接证明用户愿意为 eBPF side-effect forensic 付费。[ccusage](https://github.com/ryoppippi/ccusage)、[CodeBurn](https://github.com/getagentseal/codeburn)

2. eBPF/root 权限接受度。AgentSight 的差异依赖 privileged tracing；这对个人 Linux 开发者可行，对企业 Mac/Windows、受管设备、CI SaaS runner、合规环境可能是阻力。[AgentSight README](../../../README.md)

3. LLM plaintext 捕获的隐私边界。AgentSight 捕获 TLS plaintext 对 debugging 很有价值，但企业可能要求 redaction、local-only、zero retention、allowlist、payload sampling 或只捕获 metadata。[AgentSight README](../../../README.md)

4. “市场是否认为 agent 自身 OTel 不够”。Claude Code 已经官方支持 OTel metrics/logs/traces、file_path/full_command/tool output gates、traceparent 传播；Honeycomb/Datadog/Langfuse 已经能消费这类 telemetry。AgentSight 必须证明 agent 自身 telemetry 之外的 OS evidence 在事故、合规、PR review 中有决定性价值。[Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage.md)、[Honeycomb agents](https://docs.honeycomb.io/send-data/use-cases/agents)、[Langfuse Claude Code](https://langfuse.com/integrations/other/claude-code)

5. 跨平台覆盖。当前最强技术叙事是 Linux eBPF；coding-agent 用户大量在 macOS/Windows。没有可用的 macOS/Windows 方案时，market wedge 可能被限制在 Linux CI、containers、remote dev boxes、self-hosted runners。

## 6. 已经红海，不应硬碰的方向

1. 通用 LLM trace UI：prompt/response、span tree、latency、token/cost、tool call timeline。LangSmith、Langfuse、Phoenix、Braintrust、Weave、AgentOps、Datadog、New Relic、Honeycomb 已覆盖。[LangSmith](https://docs.langchain.com/langsmith/observability)、[Langfuse](https://langfuse.com/docs/observability/overview)、[Phoenix](https://arize.com/docs/phoenix)、[Braintrust](https://www.braintrust.dev/docs/instrument/trace-llm-calls)

2. Prompt management、datasets、experiments、LLM-as-judge evals。Langfuse、Phoenix、Braintrust、Datadog、Weave 都有成熟路径。[Langfuse evals](https://langfuse.com/docs/evaluation/overview)、[Phoenix overview](https://arize.com/docs/phoenix)、[Braintrust evaluate](https://www.braintrust.dev/docs/evaluate)、[Datadog LLM Observability](https://www.datadoghq.com/product/ai/llm-observability/)

3. LLM gateway/proxy/routing/caching/fallback。Helicone、LiteLLM、Portkey、Braintrust proxy 等已经拥挤；AgentSight 不应把核心产品做成 API gateway。[Helicone gateway](https://docs.helicone.ai/gateway)、[Braintrust wrap providers](https://www.braintrust.dev/docs/instrument/wrap-providers)

4. OTel GenAI backend ingestion。Datadog、Honeycomb、New Relic、Langfuse、Phoenix 都在接收或映射 OTel GenAI/OpenInference/OpenLLMetry；AgentSight 应输出到它们，而不是替代它们。[Datadog OTel GenAI](https://www.datadoghq.com/blog/llm-otel-semantic-convention/)、[Honeycomb agents](https://docs.honeycomb.io/send-data/use-cases/agents)、[Phoenix OTEL setup](https://www.arize.com/docs/phoenix/tracing/how-to-tracing/setup-tracing/setup-using-phoenix-otel)

5. 本地 token/cost dashboard。ccusage 和 CodeBurn 已经覆盖多 coding tools、本地读取、model breakdown、export、optimize/yield 等功能；AgentSight 不应以“成本看板”作为主要差异。[ccusage](https://github.com/ryoppippi/ccusage)、[CodeBurn](https://github.com/getagentseal/codeburn)

## 7. 可能成立的空白

1. 独立 run receipt：一次 coding-agent session 后自动生成“实际做了什么”的静态证据包，包括 LLM/tool timeline、process tree、commands、files read/written/deleted、network destinations、untracked side effects、risk flags。这与本仓库 product scope 的 Run Receipt 场景一致。[AgentSight product scope](../product-scope-agent-native.md)

2. PR due diligence evidence：把 agent run 证据附到 PR，回答“这段 diff 是怎么来的，期间读了哪些文件，跑了哪些命令，访问了哪些外部地址，是否动过 untracked/secret/system 文件”。现有 trace/eval 平台能看 agent chain，但无法独立证明 OS side effects。

3. Incident forensics：当 agent session 后出现文件丢失、配置改变、secret 可疑读取、外部请求、repo broken 时，AgentSight 用 independent timeline 帮助定位 causality。这个场景比普通 observability 更接近安全/审计/取证，竞争较少。[AgentSight product scope](../product-scope-agent-native.md)

4. Behavioral regression testing for agents/tools：在 CI 或 release testing 中比较某个 agent/skill/plugin 新旧版本的 side-effect envelope，例如是否新增外网访问、是否开始写系统目录、是否读取 secrets、是否扩大文件访问范围。现有 eval 更关注 output quality，OS behavior regression 仍是空白。

5. OTel bridge for OS side effects：AgentSight 不替代 OTel，而是生成 GenAI spans 之外的 process/file/network events，并和 `gen_ai.conversation.id`、tool call、traceparent 对齐，送入 Datadog/Honeycomb/Langfuse/Phoenix。这样能利用现有后端，同时提供独有事实源。[OpenTelemetry GenAI](https://opentelemetry.io/docs/specs/semconv/gen-ai/)、[Datadog OTel GenAI](https://www.datadoghq.com/blog/llm-otel-semantic-convention/)、[Honeycomb agents](https://docs.honeycomb.io/send-data/use-cases/agents)

6. Local-first, privacy-preserving forensic mode：只在本地生成 report，不上传 prompt/response，只上传 redacted metadata 或 hashes。这个方向可缓解 eBPF/TLS plaintext 的企业顾虑，但需要验证用户是否接受报告的隐私/准确性权衡。

## 8. 结论信心等级

| 结论 | 信心 | 理由 |
| --- | --- | --- |
| 通用 LLM trace/eval/cost 已红海 | 高 | 多个成熟产品公开文档覆盖相同功能和集成路径 |
| 现有主流平台默认看不到独立 OS-level side effects | 高 | SDK/OTel/proxy/local-log reader 的集成方式决定了默认盲区；但 APM 可以通过额外 telemetry 间接补部分信号 |
| OTel GenAI 会成为互操作标准 | 高 | OpenTelemetry 官方 semconv、Datadog/Honeycomb/New Relic/Phoenix/Langfuse/OpenLLMetry 均已围绕 OTel 建设 |
| Coding-agent 本地 session/cost 工具需求真实 | 中高 | ccusage、CodeBurn、codlogs、cursor-history、Claude Code 官方 OTel/hooks 均出现，但付费规模和企业需求仍不清楚 |
| AgentSight 的 OS side-effect evidence 是可验证差异 | 中高 | 本仓库实现和设计文档支持，但需要把 deletion/rename/network/write 等覆盖做实并形成稳定 UX |
| 用户愿意安装 privileged eBPF 作为日常 coding-agent receipt | 中低 | 技术价值明确，但权限、隐私、跨平台、运维门槛尚未市场验证 |
| AgentSight 可直接替代 LangSmith/Langfuse/Phoenix | 低 | 这些平台的 eval/prompt/dataset/workflow 完整度强，AgentSight 更适合作为独立证据层或 OTel 上游 |

## 9. 需要继续验证的问题

1. 最早愿意付费的用户是谁：个人 power user、AI coding heavy team、security team、platform team、CI/release engineering team，还是 agent marketplace/tool reviewer？

2. 对目标用户而言，最小可购买报告是什么：成本报告、run receipt、PR evidence、incident forensic report、policy violation alert，还是 live airlock？

3. eBPF/root 权限如何被接受：只在 Linux CI/self-hosted runners/remote dev boxes 上跑是否足够形成 wedge？macOS/Windows 是否需要单独路线？

4. 是否需要捕获 full prompt/response：很多差异可以只靠 model metadata、tool call、process/file/network evidence 完成；full plaintext capture 可能增加合规阻力。

5. AgentSight 如何和 Claude Code 官方 OTel 共存：直接消费 Claude Code spans，再补 OS side effects，可能比完全依赖 TLS capture 更稳。[Claude Code monitoring](https://code.claude.com/docs/en/monitoring-usage.md)

6. 如何证明“agent 自己的 transcript 不够”：需要公开 case studies，展示 transcript 说 A、OS evidence 显示 B，或 transcript 没有记录但系统被改动。

7. 哪些 OS events 必须首发：exec、file open/read/write/delete/rename、network connect、secret path access、untracked artifact、background process、package manager install，优先级需要通过真实事故/PR review 访谈确认。

8. 如何计量准确性：LLM token/cost 要和 provider billing 或官方 local metrics 对齐；file/network/process 要和 ground-truth tools 对齐；否则“独立证据”会被质疑。

9. 报告如何避免成为噪声：pip/npm/git/test runs 会产生大量文件和网络事件，需要聚合、白名单、风险排序、diff-aware summarization。

10. 输出到哪些后端最重要：HTML/Markdown evidence bundle、SQLite/JSONL、本地 Web UI、OTel Collector、Datadog/Honeycomb/Langfuse/Phoenix adapters，优先级需要由用户 workflow 决定。

## 10. 产品定位建议

AgentSight 应避免把自己描述成“LLM observability platform”或“LangSmith/Langfuse alternative”。更准确的定位是：

> 面向本地和 CI coding agents 的独立系统行为证据层：证明 agent 实际对机器做了什么，并把这些证据导出到现有 LLM observability/APM 后端。

短期最合理 wedge：

1. `agentsight record -- claude|codex|cursor-agent` 生成本地 run receipt。
2. 报告首屏回答：commands、process tree、files touched、network destinations、LLM/token/cost summary、unexpected/risky actions。
3. 支持导出 Markdown/HTML/JSONL/OTel。
4. 和 ccusage/CodeBurn 的成本能力保持互补：可读取或对齐它们的成本数据，但不以成本看板为核心。
5. 用 3 到 5 个真实 case study 验证 transcript/log-only 工具遗漏的 OS side effects。
