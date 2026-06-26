# AgentSight 开源开发者工具与 Go-to-Market 调研

调研时间：2026-06。目标问题：AgentSight 作为 Linux/eBPF/root 权限工具，怎样从 demo 走向真实产品，而不是停留在“技术上可行”的状态。

## 结论摘要

AgentSight 的核心市场机会不是“给所有开发者装一个轻量插件”。eBPF/root/Linux 会天然排除大量 macOS/Windows、本地无管理员权限、容器受限、企业安全策略锁死的开发者环境。更现实的定位是：

- 第一性价值：为 agent 运行提供独立证据，回答“它实际对系统做了什么”，而不是替代 LangSmith、OpenTelemetry、IDE 插件或 agent SDK。
- 第一产品入口：本地 Linux CLI 是底层入口，`agentsight top` 提供实时视图，`agentsight record -- <agent>` 生成 run receipt/report；第一 GTM 入口应同时提供 GitHub Action/CI 模板，把报告贴到 PR 或 artifact 中，降低非 Linux 用户的感知门槛。
- Docker 只能作为 demo/packaging 方式，不应作为主入口。Docker 默认 seccomp 会阻止 `bpf`、`perf_event_open` 等 syscall，真正可用仍要 `--privileged`、host PID/cgroup namespace、host mounts。
- 企业 agent runner 是 90 天后的验证方向，不是 0-30 天入口。它有商业价值，但销售周期、权限审批和部署面太重，早期会拖慢验证。
- 开源展示价值的最好方式不是大而全 UI，而是可复现 demo、静态 report、PR badge、benchmark、策略模板和短 case study。用户要先看到“没有 AgentSight 我无法证明这件事”。

## 市场事实：eBPF 工具的采用模式

| 项目 | 采用入口 | 被接受的原因 | 安装/权限事实 | 对 AgentSight 的启示 |
| --- | --- | --- | --- | --- |
| Pixie | Kubernetes CLI、Live UI、PxL scripts | “无需应用埋点”的 K8s observability，能快速看服务、请求、profiles | Pixie 明确是 Kubernetes 应用 observability；使用 eBPF 自动采集，无需手工 instrumentation；PEM agent 每节点安装，数据在集群内本地存储，文档称通常低于 2% CPU、少于 5% cluster CPU。参考：[Pixie overview](https://docs.px.dev/about-pixie/what-is-pixie/)、[px.dev](https://px.dev/) | eBPF 产品可用“no instrumentation”卖点，但入口通常是 K8s/平台团队，不是普通桌面用户。Pixie 还用 CLI/scripts 降低开发者使用成本。 |
| Parca Agent | Kubernetes/systemd profiler、pprof 输出 | 性能团队愿意为了持续 profiling 接受 host-level agent | Parca Agent 要 Linux kernel 5.3+ with BTF，并要求 root 或 `CAP_SYS_ADMIN`；定位是 eBPF always-on profiler，K8s/systemd 自动发现 targets。参考：[parca-agent README](https://github.com/parca-dev/parca-agent) | root/eBPF 可以被接受，但前提是收益明确、输出格式熟悉、能接入现有性能分析工作流。 |
| Cilium/Tetragon | Kubernetes Helm、DaemonSet、container/package | Cilium 已是成熟 K8s 网络/安全/观测基础设施；Tetragon 面向 runtime security observability/enforcement | Cilium 2023 年 CNCF Graduated，官方称已有大量 public case studies 和 USERS；Tetragon 推荐 Helm 安装到 `kube-system`，container quick install 使用 `--pid=host --cgroupns=host --privileged` 并挂载 BTF。参考：[Cilium graduation](https://www.cncf.io/announcements/2023/10/11/cloud-native-computing-foundation-announces-cilium-graduation/)、[Tetragon overview](https://tetragon.io/docs/overview/)、[Tetragon Kubernetes install](https://tetragon.io/docs/installation/kubernetes/)、[Tetragon container install](https://tetragon.io/docs/installation/container/) | 企业/平台用户接受 privileged agent，是因为它解决网络、安全、合规这类高价值问题。AgentSight 也必须把价值讲成“审计证据/信任边界”，而不是“调试小工具”。 |
| Falco | Host package、container、Helm、rules ecosystem | Runtime security 有明确 buyer；rules/alerts/case studies 让价值可复用 | Falco host install 文档说明 kernel event source 场景要求 privileged，可能还要安装 driver；modern eBPF 通常要求 kernel 5.8+ 的能力，如 BPF ring buffer、BTF；最小能力围绕 BPF、performance monitoring、resource、ptrace。Linux 标准能力名包括 `CAP_BPF`、`CAP_PERFMON`、`CAP_SYS_RESOURCE`、`CAP_SYS_PTRACE`。参考：[Falco install](https://falco.org/docs/setup/packages/)、[Falco kernel events](https://falco.org/docs/concepts/event-sources/kernel/)、[Falco docs](https://falco.org/docs/)、[Falco CNCF graduation](https://www.cncf.io/announcements/2024/02/29/cloud-native-computing-foundation-announces-falco-graduation/) | 规则库、默认规则、输出通道和 CNCF/社区信任都很重要。AgentSight 早期也需要“agent 行为策略模板”，而不是只给 raw trace。 |
| bcc/bpftrace | Linux CLI one-liners、专家诊断 | 高级 SRE/内核/性能用户愿意直接跑命令 | bpftrace 是 Linux dynamic tracing CLI；安装可用包管理器，但 kernel config、headers、debugfs、kprobes/uprobes、lockdown 都会影响可用性。BCC 安装也常要求 linux headers；容器里通常要 privileged 并挂载 `/lib/modules`、`/sys`、`/usr/src`。参考：[bpftrace](https://github.com/bpftrace/bpftrace)、[bpftrace install](https://github.com/bpftrace/bpftrace/blob/master/INSTALL.md)、[BCC install](https://android.googlesource.com/platform/external/bcc/+/HEAD/INSTALL.md) | CLI 可以是专业入口，但不能假设用户会理解 kernel error。AgentSight 必须有 `doctor` 和明确 remediation。 |
| Semgrep/Trivy/Ruff 等 developer-first 工具 | CLI + CI + report/badge | 不需要 root，能在本地和 CI 重复运行，输出能进入 PR/repo | Semgrep CE 推荐 `semgrep scan` 并提供 CI 配置；Trivy 有 GitHub Action、SARIF/JSON/SBOM/report 输出；Ruff 提供 `uvx`、`pip`、standalone installer 等低阻力入口。参考：[Semgrep CE in CI](https://semgrep.dev/docs/deployment/oss-deployment)、[Semgrep sample CI](https://semgrep.dev/docs/semgrep-ci/sample-ci-configs)、[Trivy GitHub Actions](https://www.trivy.dev/docs/v0.67/tutorials/integrations/github-actions/)、[Trivy reporting](https://www.trivy.dev/docs/v0.53/guide/configuration/reporting/)、[Ruff installation](https://docs.astral.sh/ruff/installation/) | AgentSight 无法复制“无权限安装”的低阻力路径，所以更要复制它们的 report、CI、badge、模板、可复现体验。 |

## 1. 安装和平台限制如何影响市场

### 直接影响

AgentSight 如果依赖 eBPF/root，会把市场切成两层：

- 可触达用户：Linux dev VM、Linux workstation、GitHub hosted Ubuntu runner、self-hosted runner、Kubernetes/Linux 平台团队、security/SRE 团队。
- 不可直接触达用户：macOS/Windows 本机开发者、企业托管笔记本无 sudo 用户、纯浏览器/云 IDE 用户、serverless 用户、普通 Docker container 内用户、受限 CI 容器、没有 kernel/BTF/headers 的轻量 Linux。

这意味着 AgentSight 的开源传播不能按 “npm install + 在任何机器运行” 来设计。它更像 Falco/Tetragon/Parca：先在愿意接受 host-level visibility 的用户中证明价值，再通过 report/artifact 让其他人消费结果。

### 权限阻力不是小问题

事实层面：

- eBPF 过去通常需要 `CAP_SYS_ADMIN`；Linux 5.8 后引入 `CAP_BPF`、`CAP_PERFMON` 等更细能力，但 tracing eBPF 仍是敏感能力。参考：[eBPF token/capabilities](https://docs.ebpf.io/linux/concepts/token/)、[Linux capabilities(7)](https://man7.org/linux/man-pages/man7/capabilities.7.html)。
- Docker 默认 seccomp profile 是 allowlist，并明确阻止 `bpf` 和 `perf_event_open` 等 syscall。参考：[Docker seccomp](https://docs.docker.com/engine/security/seccomp/)。
- GitHub hosted Linux/macOS runner 有 passwordless sudo；但 `ubuntu-slim` single-CPU runner 是 unprivileged container，不支持低层 kernel features。参考：[GitHub hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)、[GitHub hosted runners](https://docs.github.com/en/actions/concepts/runners/github-hosted-runners)。
- Falco/Tetragon/Parca 的真实部署都承认 privileged/capabilities/driver/BTF/kernel feature 是 adoption 成本，而不是“用户误操作”。

所以市场影响是：

1. “个人开发者全量市场”会被平台限制显著压缩。尤其 AI coding agent 的主流用户很多在 macOS 上。
2. “安全/平台/CI 审计市场”反而更匹配，因为这些用户已经习惯安装 privileged DaemonSet、self-hosted runner、EDR/observability agent。
3. AgentSight 必须把 root 权限解释为价值的一部分：它是独立观察者，而不是 agent SDK 的自报日志。
4. 安装页面必须非常诚实：Linux only、kernel requirements、Docker limitations、CI runner limitations、需要哪些 capabilities、采集哪些敏感数据、如何脱敏/不上传。

## 2. 哪些用户能接受 root/eBPF，哪些不能

| 用户 | 接受度 | 原因 | 推荐切入话术 |
| --- | --- | --- | --- |
| SRE/平台工程师 | 高 | 已经使用 Prometheus node exporter、Cilium、Falco、Datadog/Sysdig agent、Parca 等 host/cluster agent | “给 agent 运行生成可审计 evidence，不改 agent 框架。” |
| 安全工程师/AppSec/DevSecOps | 高 | runtime security、forensics、PR due diligence、incident reconstruction 都需要独立证据 | “agent 可能自报不准，AgentSight 从 OS boundary 证明实际行为。” |
| AI agent/tool/skill/MCP 开发者 | 中高 | 需要证明工具“只读、项目内、无外联、无 secret access”；愿意在 Linux VM/CI 上跑验证 | “给 README/PR/marketplace 一个可复现 verify report。” |
| 开源维护者 | 中 | 愿意在 GitHub Action 里跑一次，但未必愿意本机 sudo | “安装一个 Action，生成 artifact/badge，不要求 reviewer 本机装。” |
| 企业安全/合规团队 | 中高但周期长 | 接受 agent runner/EDR 模型，但需要采购、隐私、RBAC、日志保留、审计链 | “先从 self-hosted runner pilot 开始，不碰员工笔记本。” |
| 普通 macOS/Windows AI coding 用户 | 低 | 本机无法直接跑 Linux eBPF；不愿意开 VM 或 sudo | “消费报告可以，生成报告需要 Linux/CI/runner。” |
| 公司托管笔记本开发者 | 低 | 无 sudo、EDR/MDM 限制、kernel extension/low-level tracing 敏感 | “不要把本机安装作为首要路径。” |
| 纯容器/serverless/受限 CI 用户 | 低 | 没有 host kernel 权限；Docker/K8s seccomp/capabilities 会拦截 | “用 self-hosted runner 或企业 runner，不承诺普通 container 可用。” |

## 3. 第一入口选择

推荐：**CLI 是产品内核；GitHub Action/CI 是第一 GTM 包装；Docker 只是 demo/packaging；企业 runner 放到 90 天后验证。**

### 为什么不是 Docker first

Docker 能让 demo 看起来统一，但它不消除 eBPF 权限问题。Tetragon 的 local container quick install 仍需要 `--pid=host --cgroupns=host --privileged` 和 BTF mount；Falco 的 driver loader 也需要 privileged。Docker 默认 seccomp 还会阻止 `bpf`、`perf_event_open`。因此 Docker first 会制造错误预期：用户以为“容器里安全隔离地跑”，实际上要把容器变成 host-level agent。

Docker 应该用于：

- 可复现 demo runner。
- 统一依赖/工具链。
- CI 中下载 prebuilt binary 或运行报告生成器。

不应作为主产品入口。

### 为什么不是 enterprise agent runner first

企业 runner 最接近商业化，但早期验证会被以下问题拖住：

- 安全审批：root/eBPF/LLM payload capture/host filesystem visibility 都会触发审查。
- 运维面：升级、卸载、日志保留、RBAC、数据脱敏、tenant isolation。
- 销售周期：需要 design partner，而不是普通开源 adoption。

如果 0-30 天就做 runner，很容易把工程投入消耗在部署复杂度上，而不是验证用户是否真的需要“agent 行为证据”。

### 实际首屏体验

应提供两个并列入口：

```bash
agentsight doctor
sudo agentsight record -- claude
agentsight report --format html
```

以及：

```yaml
name: AgentSight Verify
on:
  pull_request:
  workflow_dispatch:

jobs:
  verify-agent:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: agent-sight/setup-agentsight@v0
      - run: sudo agentsight verify -- ./scripts/run-agent-task.sh
      - uses: actions/upload-artifact@v4
        with:
          name: agentsight-report
          path: agentsight-report.*
```

GitHub Actions 的价值不只是自动化。GitHub 官方 workflow badge 可嵌入 README；这对开源项目是一个成熟的信任展示方式。参考：[GitHub status badge](https://docs.github.com/en/actions/how-tos/monitor-workflows/add-a-status-badge)。

## 4. 开源项目如何展示价值

AgentSight 的展示物应该围绕“可复现证据”，而不是围绕“漂亮 dashboard”。

### 必须做的展示物

1. **三类 demo repo**

   - benign agent run：agent 只读 repo、运行测试、生成 diff。
   - suspicious run：agent 读取 `.env`、访问 `$HOME/.ssh`、写入 repo 外文件、发起未知外联。
   - tool acceptance：一个声称 read-only 的 MCP/skill，AgentSight 证明其实际行为。

2. **静态 HTML/Markdown report**

   报告应能在没有 AgentSight 的机器上阅读。最低内容：

   - run metadata：时间、机器类型、kernel、AgentSight 版本、命令。
   - process tree：agent 主进程、shell、子进程。
   - file access：read/write/create/delete/rename，按 repo 内/外、敏感路径分类。
   - network destinations：host/IP、port、process attribution。
   - commands：命令、退出码、耗时。
   - policy result：pass/fail/warn。
   - privacy note：哪些内容被 redacted，哪些没有采集。

3. **PR artifact/comment**

   开源传播的关键是 reviewer 不安装也能看到结果。CI 输出至少支持：

   - `agentsight-report.md`
   - `agentsight-report.html`
   - `agentsight-evidence.json`
   - 可选 SARIF 或 GitHub code scanning integration

   Trivy 的 report/SARIF/SBOM 输出是很好的参照。参考：[Trivy reporting](https://www.trivy.dev/docs/v0.53/guide/configuration/reporting/)。

4. **Badge**

   Badge 不应只显示 “build passing”，而应显示：

   - `AgentSight verified`
   - `no external network`
   - `repo-local writes only`
   - `no sensitive path reads`

   Badge 背后要链接到最近一次 report。

5. **Benchmark**

   早期 benchmark 不需要复杂，但必须可信：

   - Ubuntu 22.04/24.04、kernel 5.15/6.x、GitHub hosted runner。
   - 3 个 agent workload：短命令、代码修改、长任务。
   - 指标：CPU overhead、wall-clock overhead、event loss、report size。
   - 与 “no AgentSight” baseline 对比。

6. **策略模板**

   类似 Falco rules/Tetragon tracing policies 的价值：用户不是从零写规则。AgentSight 需要模板：

   - `readonly-review.yaml`
   - `repo-local-write.yaml`
   - `no-secret-read.yaml`
   - `no-network-except-llm.yaml`
   - `ci-agent-safe.yaml`
   - `mcp-tool-acceptance.yaml`

7. **短 case study**

   每个 case study 只回答三件事：

   - 没有 AgentSight 时用户无法证明什么。
   - AgentSight 捕获到了什么证据。
   - 这个证据导致了什么决策：拒绝工具、修复 prompt、限制权限、接受 PR、复盘事故。

## 5. 哪些功能是 GTM 必须，哪些只是工程负担

### GTM 必须功能

| 功能 | 为什么必须 | 不做的风险 |
| --- | --- | --- |
| `agentsight doctor` | eBPF/kernel/capability 错误会非常常见；必须提前解释 | 用户遇到 “Operation not permitted” 后直接流失 |
| 明确 platform matrix | Linux 发行版、kernel、BTF、Docker、GitHub runner、WSL2 都要写清楚 | 错误期待会伤害信任 |
| `agentsight record -- <cmd>` | 最小可用产品，直接对应 run receipt | 没有清晰录制入口，demo 无法传播 |
| targeted tracing | 只追踪指定 agent 进程/子进程，避免全机采集 | root 工具采太多会引发隐私和安全反感 |
| 静态 report | 消费者不应被要求安装 AgentSight | 开源 reviewer、PR reviewer 无法使用 |
| `agentsight verify --policy <file>` | 从“看 trace”变成“判定是否符合预期” | 用户需要自己解释 raw events，价值不稳定 |
| GitHub Action wrapper | 开源项目和 PR workflow 的最现实传播入口 | 本地 Linux 限制会压低 adoption |
| PR artifact/comment | 让价值出现在用户已有 workflow 中 | report 藏在日志里无法形成信任信号 |
| redaction/privacy controls | AgentSight 可能看到 prompts、paths、env、network，必须默认保护 | 安全团队不会批准，开源用户也会担心 |
| deterministic demo fixtures | 每次 demo 都能复现同一类风险 | 只靠现场跑 agent，效果不可控 |
| install/uninstall + signed release | root 工具必须能被安全地安装/移除 | “curl | sudo sh” 对安全用户很刺眼 |
| performance counters | eBPF 工具必须证明 overhead 和 event loss | 用户会默认担心性能和稳定性 |

### 早期工程负担

这些功能有长期价值，但 0-90 天不应优先：

- 全功能 timeline UI。先做静态 report；UI 会吞掉大量前端/状态管理时间。
- Kubernetes DaemonSet。AgentSight 当前最强场景是 agent session/run receipt，不是 cluster-wide runtime security。
- 企业多租户云控制台。没有 design partner 前不要做。
- 实时阻断/enforcement。先证明 observation 和 evidence；阻断会引入误杀、权限、责任边界。
- 支持所有 TLS library 和 agent framework。早期应聚焦 agent 进程、文件、进程树、network metadata；LLM payload capture 可以作为进阶。
- macOS/Windows 原生支持。eBPF/Linux 根本限制决定它不是短期可解。
- 桌面 App。目标用户先是 CLI/CI 用户。
- 大模型语义评估。AgentSight 的差异化是系统证据，不是再做一个 eval 平台。
- 长期存储/云同步。先让用户本地生成和分享报告。
- 自动 remediation。先给证据，修复可交给人或 agent。

## 6. 30/60/90 天验证路线

### 0-30 天：证明“单机 run receipt”有独立价值

目标：让 5-10 个懂 agent/security/devtools 的用户在 Linux 或 GitHub runner 上跑通，并承认 report 里有他们无法从 agent 自报中得到的信息。

交付：

- `agentsight doctor`：检查 kernel、BTF、capabilities、Docker/seccomp、GitHub runner、debugfs/tracefs、lockdown 迹象。
- `agentsight record -- <cmd>`：追踪指定命令和子进程。
- 最小事件集：exec/process tree、file read/write/create/delete/rename、network connect、working directory、exit status。
- 静态 report：HTML + Markdown + JSON。
- 三个 demo fixture：benign、suspicious、tool acceptance。
- 兼容性页：Ubuntu 22.04/24.04、GitHub `ubuntu-latest`、Docker privileged、WSL2 是否支持。
- 性能 baseline：3 个 workload，报告 CPU/wall-clock overhead 和 event count。

验证指标：

- 80% 以上试用者能在 15 分钟内跑通 demo。
- 至少 5 个用户能指出 report 中一个“agent 自报没有提供”的证据。
- 至少 3 个用户愿意把 report 附在 PR/README/issue 中。

明确不做：

- 不做 cloud dashboard。
- 不做 K8s DaemonSet。
- 不追求捕获所有 TLS plaintext。
- 不做自动阻断。

### 31-60 天：把价值带进 CI/PR 工作流

目标：让开源项目或 agent tool 作者能用 GitHub Action 生成可分享的 verification report。

交付：

- `agent-sight/setup-agentsight` GitHub Action。
- `agentsight verify --policy <template>`。
- PR comment/artifact template。
- Badge 生成：policy pass/fail 链接到 report。
- 策略模板：readonly、repo-local-write、no-secret-read、no-network-except-allowlist、mcp-tool-acceptance。
- SARIF 或至少 machine-readable JSON，方便后续接 GitHub code scanning。
- 隐私/redaction 默认策略：路径 hash/保留 basename、env value 默认不采、secret pattern redaction、LLM payload capture 默认关闭或明确 opt-in。

验证指标：

- 3 个外部 repo 安装 Action。
- 10 次以上 PR/CI report 生成。
- 至少 2 个用户因为 report 改变决策：拒绝一个 tool、修改 policy、补充 README trust statement、修复 agent workflow。
- `doctor` 报错能覆盖 80% 安装失败原因。

明确不做：

- 不把 GitHub Action 做成唯一入口；它包装 CLI。
- 不承诺普通 Docker job 内可用。
- 不做组织级管理后台。

### 61-90 天：验证团队/企业 runner 是否值得产品化

目标：确认 AgentSight 是否能从开源工具变成团队信任基础设施。

交付：

- self-hosted runner guide：最小 Linux VM、capabilities、network、storage、log retention。
- 企业 pilot packaging：signed binary、SBOM、checksums、uninstall、systemd unit 可选。
- evidence retention model：本地保存、artifact 上传、可配置过期时间。
- policy pack：agent PR due diligence、MCP acceptance、incident forensics。
- 2-3 篇 case study：每篇用真实或可公开复现的 agent run。
- 初版 admin/privacy 文档：采集范围、不会采集什么、如何 redaction、如何禁用 LLM payload、如何审计 AgentSight 自身。

验证指标：

- 2 个团队愿意在 self-hosted runner 或 Linux VM 中连续使用两周。
- 至少 1 个团队愿意讨论付费/支持/托管报告/企业策略管理。
- 至少 1 个 case study 能明确展示安全或审查收益。
- 安装失败、权限不足、隐私顾虑三类问题都有可执行 remediation。

90 天后的产品判断：

- 如果 CI/PR report adoption 明显强于本地 CLI，应优先做 GitHub/GitLab integration 和 report workflow。
- 如果本地 CLI 使用强，但 CI 弱，应定位为 agent power-user/security forensic CLI。
- 如果只有企业 pilot 感兴趣，说明开源传播会慢，应尽早切 enterprise runner + security buyer。
- 如果用户只觉得 demo 有趣但不愿重复使用，说明 AgentSight 的报告还没有形成决策价值，应回到 policy/report，而不是继续加采集能力。

## Go-to-Market 建议

### 定位语

不要说：

- “AI agent observability platform”
- “eBPF for everything”
- “zero-friction agent monitoring”

建议说：

- “AgentSight gives every agent run an independent receipt.”
- “Prove what an AI agent actually did to files, processes, and network.”
- “Verify agent tools before you trust them.”

中文可写成：

- “给每次 agent 运行生成独立证据。”
- “证明 agent 实际读了什么、写了什么、执行了什么、连了哪里。”
- “在接受 MCP/skill/agent workflow 前，用系统证据验证它的行为。”

### 首批用户选择

优先找：

- 正在开发/发布 MCP server、agent skill、agent plugin 的人。
- 维护开源 repo，并允许 agent 生成 PR 的 maintainer。
- 安全工程师，正在担心 agent 读取 secret、越界写文件、未知外联。
- 用 GitHub Actions 跑 agent workflow 的团队。
- Linux/SRE 背景、已经理解 eBPF/Falco/Tetragon/Parca 的用户。

暂时不要主攻：

- 纯 macOS 个人用户。
- 不愿意使用 CI 的普通 coding agent 用户。
- 需要企业采购才能试用的 org。
- 只想要 agent prompt/eval dashboard 的用户。

### 开源 README 应该展示什么

README 第一屏应展示：

1. 一条命令：

   ```bash
   sudo agentsight record -- claude
   ```

2. 一张报告截图或链接。

3. 一个 GitHub Action YAML。

4. 一个 badge 示例。

5. 一个明确平台框：

   - Works: Linux, Ubuntu GitHub hosted runner, self-hosted Linux runner.
   - Needs: sudo/root or capabilities, BTF/eBPF kernel support.
   - Limited: Docker requires privileged/host mounts.
   - Not supported: macOS/Windows native, ordinary unprivileged containers.

6. 一个隐私框：

   - 默认只采集指定进程树。
   - 默认 redaction。
   - LLM payload capture 如果存在，必须 opt-in。
   - report 默认本地生成，不上传。

## 资料来源

- Pixie: [Pixie overview](https://docs.px.dev/about-pixie/what-is-pixie/), [px.dev](https://px.dev/)
- Parca Agent: [parca-agent README](https://github.com/parca-dev/parca-agent)
- Cilium/Tetragon: [Cilium CNCF graduation](https://www.cncf.io/announcements/2023/10/11/cloud-native-computing-foundation-announces-cilium-graduation/), [Tetragon overview](https://tetragon.io/docs/overview/), [Tetragon Kubernetes install](https://tetragon.io/docs/installation/kubernetes/), [Tetragon container install](https://tetragon.io/docs/installation/container/)
- Falco: [Falco docs](https://falco.org/docs/), [Falco install](https://falco.org/docs/setup/packages/), [Falco kernel events](https://falco.org/docs/concepts/event-sources/kernel/), [Falco container install](https://falco.org/docs/setup/container/), [Falco CNCF graduation](https://www.cncf.io/announcements/2024/02/29/cloud-native-computing-foundation-announces-falco-graduation/)
- bpftrace/BCC: [bpftrace GitHub](https://github.com/bpftrace/bpftrace), [bpftrace install](https://github.com/bpftrace/bpftrace/blob/master/INSTALL.md), [BCC install](https://android.googlesource.com/platform/external/bcc/+/HEAD/INSTALL.md)
- Linux/eBPF 权限与容器限制: [eBPF token and capabilities](https://docs.ebpf.io/linux/concepts/token/), [Linux capabilities(7)](https://man7.org/linux/man-pages/man7/capabilities.7.html), [Docker seccomp profile](https://docs.docker.com/engine/security/seccomp/)
- GitHub Actions/CI: [GitHub hosted runners reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners), [GitHub hosted runners](https://docs.github.com/en/actions/concepts/runners/github-hosted-runners), [GitHub workflow status badge](https://docs.github.com/en/actions/how-tos/monitor-workflows/add-a-status-badge)
- Developer-first CLI/CI examples: [Semgrep CE in CI](https://semgrep.dev/docs/deployment/oss-deployment), [Semgrep sample CI configs](https://semgrep.dev/docs/semgrep-ci/sample-ci-configs), [Trivy GitHub Actions](https://www.trivy.dev/docs/v0.67/tutorials/integrations/github-actions/), [Trivy reporting](https://www.trivy.dev/docs/v0.53/guide/configuration/reporting/), [Ruff installation](https://docs.astral.sh/ruff/installation/)
