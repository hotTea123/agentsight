# Market Map And Whitespace

This is a first-pass synthesis while deeper subagent research is still running.
It answers one question:

> What is everyone already doing, and what is still not well covered?

The conclusion is provisional. It is based on public docs and market signals
available on 2026-06-02, plus the current AgentSight capability set.

## Short Answer

The market is already crowded around:

- LLM application tracing
- prompt/response/tool-call observability
- token and cost tracking
- evals and prompt iteration
- LLM gateway/proxy observability
- prompt-injection scanners and agent firewalls
- coding-agent audit logs based on vendor hooks

The plausible whitespace is narrower:

- independent OS-level evidence for local/CLI agents
- intent-to-effect correlation across LLM calls and real process/file/network
  side effects
- post-run behavior receipts backed by system events, not only agent self-logs
- delegation confidence and permission tuning
- recovery context after an agent damages local state
- behavior verification reports for skills/MCP servers/plugins
- behavior diffing across agent/tool/model versions
- incident forensics after an agent caused damage

The strongest product wedge is not "agent observability" in general. That
category is already taken.

It is also too narrow to call the product only an "agent audit" tool. Audit is
one scenario, not the whole user problem.

The stronger wedge is:

> A tamper-resistant behavior receipt for agents that operate on your real
> machine or codebase.

The broader user problem is:

> Users want to delegate real work to agents, but they need confidence, control,
> and recovery when that delegation touches real systems.

## What Everyone Is Already Doing

### 1. LLM Application Observability

Examples:

- LangSmith
- Langfuse
- Arize Phoenix
- Braintrust
- Weights & Biases Weave
- AgentOps
- Datadog / New Relic / Honeycomb GenAI views

Common product shape:

- traces
- spans/runs
- prompts and responses
- tool calls and tool results
- token usage
- latency
- feedback
- evals
- prompt management
- experiments
- dashboards

Evidence:

- Langfuse describes its core as application tracing that captures prompt,
  model response, token usage, latency, and tools/retrieval steps between them.
  It also emphasizes LLM-specific features such as evals, prompt management,
  experiments, and dashboards:
  https://langfuse.com/docs/observability/overview
- LangSmith structures data as projects, traces, runs, and threads. It supports
  integrations and manual instrumentation for LLM providers and agent
  frameworks:
  https://docs.langchain.com/langsmith/observability-concepts
- Phoenix describes traces as capturing model calls, retrieval, tool use, and
  custom logic, with OpenTelemetry/OpenInference instrumentation:
  https://arize.com/docs/phoenix
- OpenTelemetry now has GenAI semantic conventions for inference and tool
  execution spans, including `execute_tool` spans and opt-in tool call
  arguments/results:
  https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/

What this means for AgentSight:

- Generic prompt/response/tool-call tracing is not a whitespace.
- Generic evals are not a whitespace.
- Generic cost/latency dashboards are not a whitespace.
- If AgentSight competes here, it will look like a less mature Langfuse or
  LangSmith.

### 2. LLM Gateway And Proxy Observability

Examples:

- Helicone
- LiteLLM proxy
- Portkey
- OpenRouter-style routing layers
- provider gateways

Common product shape:

- route LLM traffic through a unified endpoint
- log requests/responses
- track latency/cost/status
- add caching, rate limits, failover, model routing

Evidence:

- Helicone's gateway docs describe a unified entry point for provider traffic
  with caching, monitoring, rate limiting, vaults, feedback, and more:
  https://docs.helicone.ai/getting-started/integration-method/gateway
- Helicone's AI Gateway positions itself as a unified API for many LLM
  providers with routing, fallbacks, and observability:
  https://docs.helicone.ai/gateway

What this means for AgentSight:

- Provider gateway observability is not a whitespace.
- Routing/fallback/caching is not AgentSight's lane.
- AgentSight's lane is outside the application/provider path: what the agent did
  to the local system after model/tool decisions.

### 3. OpenTelemetry Standardization For GenAI And MCP

Examples:

- OpenTelemetry GenAI semantic conventions
- OpenTelemetry MCP semantic conventions
- OpenInference conventions/instrumentation

Common product shape:

- standard names for model calls, tool calls, token metrics, MCP calls
- app/framework instrumentation emits spans
- observability platforms ingest spans

Evidence:

- OpenTelemetry defines GenAI spans for inference and tool execution:
  https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/
- OpenTelemetry also defines MCP semantic conventions, including MCP tool call
  spans compatible with GenAI execute-tool spans:
  https://opentelemetry.io/docs/specs/semconv/gen-ai/mcp/

What this means for AgentSight:

- The market is moving toward standardized application-layer traces.
- AgentSight should not invent a competing semantic convention unless necessary.
- A useful strategy is to export AgentSight observations into OTel-compatible
  shapes while preserving its independent OS-level evidence.

### 4. Coding-Agent Audit Logs Based On Hooks

Examples:

- Tribunal
- likely future vendor-native logs from Claude Code, Cursor, Codex, Copilot CLI
- community tools that normalize local coding-agent hooks

Common product shape:

- install adapters/hooks for each coding agent
- collect prompts, tool calls, file edits, commands, cost
- apply policy rules
- provide local log plus optional hosted dashboard

Evidence:

- Tribunal explicitly markets "One audit log. Every coding agent" and says it
  records prompts, tool calls, and dollars spent for Claude Code, Cursor,
  Copilot CLI, and Codex CLI:
  https://tribunal.dev/
- Tribunal docs say each adapter normalizes native hooks into a v1 event schema,
  and examples include Claude Code, Cursor, Copilot CLI, and Codex CLI adapters:
  https://tribunal.dev/docs
- Tribunal's policy examples include blocking writes to production terraform and
  enabling shipped packs such as `secrets-readonly`, `no-prod-writes`, and
  `soc2-baseline`:
  https://tribunal.dev/docs

What this means for AgentSight:

- "Unified coding-agent audit log" is not an empty market.
- "Policy engine over agent hook events" is also not empty.
- AgentSight needs to clearly differentiate from hook-based audit tools.

Potential AgentSight differentiation:

- Tribunal depends on what agent hooks expose.
- AgentSight can observe child processes and system effects even when the agent
  does not log them.
- AgentSight can correlate LLM/TLS payloads with process/file/network events.
- AgentSight can reveal background activity and untracked side effects.

This is one of the most important competitive findings. It narrows the product
scope.

### 5. Agent Security, Prompt Injection, And Tool Firewalls

Examples:

- Lakera
- Prompt Security
- Lasso
- Protect AI
- HiddenLayer
- CalypsoAI
- NeMo Guardrails
- MCP scanners/firewalls
- DLP/security gateways
- EDR/SIEM integrations

Common product shape:

- classify prompt injection
- detect sensitive data leakage
- guard model inputs/outputs
- mediate tool calls
- sandbox agent execution
- enforce allow/deny policies

Evidence:

- OWASP lists Prompt Injection as a top LLM application risk and notes that
  impact depends heavily on the system's agency:
  https://genai.owasp.org/llmrisk/llm01-prompt-injection/
- OWASP's Excessive Agency risk discusses LLM systems granted tools/skills or
  extensions that can perform damaging actions due to manipulated outputs:
  https://genai.owasp.org/llmrisk/llm06-sensitive-information-disclosure/
- MCP's official security best practices document identifies attack classes such
  as confused deputy, token passthrough, SSRF, session hijacking, local MCP
  server compromise, and scope minimization:
  https://modelcontextprotocol.io/specification/2025-06-18/basic/security_best_practices
- MCP roots define filesystem boundaries and explicitly require clients to
  validate roots, implement access controls, and monitor root accessibility:
  https://modelcontextprotocol.io/docs/concepts/roots

What this means for AgentSight:

- "Agent security" is a very large and crowded direction.
- Prompt firewall alone is not whitespace.
- MCP scanner alone is not whitespace.
- Enterprise policy engine alone will pull AgentSight into a heavy security
  platform battle.

Potential AgentSight differentiation:

- evidence-first security: what actually happened, not only what a prompt or
  tool call looked like
- post-run and incident forensics for local agents
- OS-level verification of policy claims such as "workspace-only" or "read-only"

### 6. Traditional OS/Security Observability

Examples:

- Falco
- Tetragon
- Cilium
- bpftrace/bcc tools
- EDR
- DLP
- SIEM
- shell history
- git diff
- CI logs

Common product shape:

- process/syscall/network/file telemetry
- security policies
- container/Kubernetes observability
- endpoint event collection

What this means for AgentSight:

- OS telemetry is not new.
- eBPF itself is not a product moat.
- The product value is not "we can collect process/file events."
- The value is correlating those events with agent intent and making the result
  understandable to agent users.

## What Looks Under-Covered

### 1. Delegation Confidence, Not Just Audit

Most current products answer:

- What did the LLM say?
- Which tool did the app or agent report?
- How many tokens did it spend?
- Did the prompt look suspicious?

The more user-facing question is:

> Can I safely let this agent keep working without watching every step?

This affects autonomy:

- which actions can be auto-approved
- which actions need confirmation
- which actions should be blocked
- whether the user can trust future runs more
- whether a team can adopt agents without every reviewer becoming a detective

AgentSight's evidence can support this, but the product should frame the value
as delegation confidence rather than only audit logs.

### 2. Recovery After Agent Damage

Audit answers "what happened." Recovery answers:

> What do I need to undo or inspect to get back to a good state?

This is under-covered because git diff is incomplete:

- untracked files
- files outside the repo
- package installs
- shell profile changes
- background processes
- network calls
- local caches and generated artifacts

AgentSight can make agent side effects recoverable by producing:

- changed path inventory
- destructive operation list
- process lineage
- likely recovery context
- evidence for a human or another agent to fix the state

### 3. Permission And Confirmation Tuning

Permission systems exist in Claude Code, Codex, Cursor, Gemini CLI, and other
agents. But users still struggle with the tradeoff:

- too many confirmations: the agent becomes annoying
- too few confirmations: the agent becomes risky

AgentSight could help tune this using observed behavior:

- "This agent has only written inside the workspace for the last 20 runs."
- "This MCP server attempted network access despite claiming read-only."
- "This command pattern is safe in this repo but risky outside it."

This is different from audit. It is a feedback loop for autonomy.

### 4. Independent OS-Level Evidence For Local Coding Agents

Most LLM observability tools see the application layer. Hook-based coding-agent
audit tools see what each agent chooses to expose through hooks.

The gap:

- child processes outside the explicit tool-call record
- files read or written by child commands
- network activity from tools and subprocesses
- untracked generated artifacts
- destructive actions that were not faithfully logged
- system effects after a tool call exits

This is where AgentSight may have a real wedge.

The positioning should be:

> Agent hooks tell you what the agent said it was doing. AgentSight records what
> the operating system observed.

### 5. Intent-To-Effect Correlation

Tools can show LLM spans. Security tools can show process/file events. The
uncommon part is connecting:

- model call
- tool decision
- shell command
- child processes
- file/network side effects

This is harder than dashboards, but it is also more defensible.

The key product question:

> Which model/tool decision caused this concrete system effect?

This is not fully solved by Langfuse/LangSmith because they stop at application
spans. It is not fully solved by Falco/Tetragon/EDR because they do not know
agent intent. It is not fully solved by Tribunal-style hook logs because they
depend on vendor hooks and may not see OS reality.

### 6. Behavior Verification Report For Skills/MCP Servers/Plugins

This is different from normal observability.

The user question is:

> This tool claims to be safe. Can I see an independent run report proving what
> it actually touched?

Potential report claims:

- read-only run
- workspace-local run
- no network egress
- no secret path access
- no destructive operations
- only expected subprocesses
- only expected MCP methods

This could become:

```bash
agentsight verify -- ./run-skill-demo.sh
agentsight report --badge
```

The output could be attached to:

- README
- marketplace page
- PR
- internal approval ticket
- security review

This is likely stronger than a generic dashboard because the user has a clear
decision: trust or do not trust this tool.

### 7. Behavior Diffing

Most eval tools compare outputs. They do not compare system footprint.

Potential user question:

> The new model/prompt/tool produces the same answer, but does it now touch more
> files, spawn more commands, access secrets, or make new network calls?

This could matter for:

- skill authors
- MCP server authors
- coding-agent teams
- internal AI platform teams
- security review

Potential product:

```bash
agentsight record --label old -- ./task.sh
agentsight record --label new -- ./task.sh
agentsight diff old new
```

This is less validated than run receipts and verification reports, but it is a
real product idea because it catches behavior regressions output evals miss.

### 8. Incident Forensics For Agent-Caused Damage

Current tools are often built for monitoring or prevention. A distinct
after-the-fact scenario is:

> My agent broke something. Tell me exactly what happened and what changed.

The value is reconstructability:

- suspicious time window
- destructive commands
- file modifications
- process lineage
- network activity
- tool call or LLM message that preceded the action

This is adjacent to security, but the first buyer may be developers, not CISOs.

## What Is Probably Not Whitespace

Avoid treating these as core differentiation:

- generic LLM traces
- prompt/response history
- token cost dashboards
- evals
- prompt management
- LLM gateway/proxy routing
- prompt-injection classification alone
- YAML policy engine over agent hook events
- generic eBPF process/file dashboards
- generic MCP security checklist

These can be integrations or supporting features, but not the main product
claim.

## Narrow Product Thesis

AgentSight should not say:

> We are an agent observability platform.

That is too broad and already crowded.

AgentSight also should not say only:

> We are an agent audit tool.

That is too narrow. It makes the product sound useful only after something bad
happens, while several stronger user jobs happen before or during delegation:
deciding what to auto-approve, deciding whether to trust a tool, deciding
whether a PR process is acceptable, and deciding what can be safely reverted.

AgentSight should test:

> We generate independent behavior receipts for local agents, backed by
> OS-level evidence.

Then expand only if users pull it:

1. run receipt
2. skill/MCP verification report
3. incident forensics
4. behavior diffing
5. live airlock
6. autonomy/permission tuning

## Questions Still Needing Market Validation

1. Do individual developers care enough to install a root/eBPF tool for a run
   receipt, or is curiosity not enough?
2. Are skill/MCP/plugin authors willing to publish a behavior report as a trust
   artifact?
3. Do reviewers of agent-generated PRs actually want process evidence, or is
   git diff plus CI enough?
4. Do enterprise teams prefer endpoint/EDR/SIEM integration instead of a
   separate AgentSight product?
5. Is Linux/eBPF too limiting for local coding-agent users, many of whom are on
   macOS?
6. Would users accept AgentSight as a verification tool if it cannot yet support
   macOS/Windows with equivalent depth?
7. Is "independent evidence" a buying reason, or only a nice-to-have?

## Initial Confidence

High confidence:

- Generic LLM observability is crowded.
- Prompt/response/tool-call tracing is not enough for AgentSight to stand out.
- Hook-based coding-agent audit is already being attempted by products such as
  Tribunal.
- OS-level evidence is the clearest technical difference AgentSight can claim.

Medium confidence:

- Skill/MCP verification reports could be a strong wedge.
- Incident forensics could be a strong developer/security use case.
- Behavior diffing could matter for agent builders.

Low confidence:

- Individual developers will pay for this directly.
- Enterprises will accept an eBPF/root tool as a separate product without SIEM
  integration.
- Live blocking should be built early.
