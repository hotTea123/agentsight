# Agent Run Map: 一次 agent 工作过程的第一屏

用户运行 agent，不是为了看一串 telemetry。他想知道：

> 这次 run 从我的目标开始，到底怎么走到了最后的结果？

Agent Run Map 是 AgentSight 的总入口图。它不专注文件系统，也不专注 LLM trace。它把一次 run 分成几个可读的阶段：目标、计划、工具、系统动作、结果。

## 适合什么场景

- agent run 结束后的第一屏 summary
- 用户想快速判断“它是不是按我的目标在推进”
- team lead 或 reviewer 想理解一次 AI-generated change 的来龙去脉
- 另一个 agent 要接手继续修复时，需要先读懂上一轮做了什么

## 它回答的问题

```text
这次 run 的主要阶段是什么？
每个阶段的语义目的是什么？
每个目的触发了哪些命令、进程、文件、网络动作？
哪些结果成功、失败、重试或没有实际发生？
有没有系统动作无法归因到明确的 agent intent？
```

## 视觉结构

```text
User goal
  "fix the failing API test"

┌─ Phase 1: Understand failure ───────────────────────────────┐
│ Semantic: inspect failing test                              │
│ Tool: Read tests/api.test.ts                                │
│ System: open tests/api.test.ts                              │
│ Result: found assertion mismatch                            │
└──────────────────────────────────────────────────────────────┘

┌─ Phase 2: Reproduce ─────────────────────────────────────────┐
│ Semantic: run focused test                                  │
│ Tool: Bash("npm test tests/api.test.ts")                    │
│ System: npm -> node -> jest                                 │
│ Files: package.json, tests/api.test.ts, src/api/handler.ts  │
│ Network: none                                               │
│ Result: exit 1                                              │
└──────────────────────────────────────────────────────────────┘

┌─ Phase 3: Patch ─────────────────────────────────────────────┐
│ Semantic: update handler behavior                           │
│ Tool: Edit src/api/handler.ts                               │
│ System: write src/api/handler.ts                            │
│ Result: diff +12 -4                                         │
└──────────────────────────────────────────────────────────────┘

┌─ Phase 4: Verify ────────────────────────────────────────────┐
│ Semantic: rerun test                                        │
│ Tool: Bash("npm test tests/api.test.ts")                    │
│ System: npm -> node -> jest                                 │
│ Result: exit 0                                              │
└──────────────────────────────────────────────────────────────┘

Unattributed:
  git status x6
  background process touched ~/.config/tool/config.json
```

## 必须跨层展示

Agent Run Map 的每个 phase 都至少包含：

- `Semantic`: 这一阶段 agent 以为自己在做什么
- `Tool`: agent 选择了什么工具或命令
- `System`: 真实进程、文件、网络、资源动作
- `Result`: 成功、失败、重试、输出、diff、测试结果

如果某一栏缺失，它就退化成普通 trace 或普通系统 dashboard。

## 视觉编码

- 阶段块按时间排序。
- 左侧写语义目的，右侧写系统结果。
- 线条连接 tool intent 和系统 effect。
- 灰色块表示无法归因的活动。
- 红色或高亮只用于“和用户目标明显不一致”的行为，不要把整张图做成安全告警面板。

## 和其他图的关系

Agent Run Map 是第一屏，不是细节页。

用户从这里进入：

- 想看哪一步最贵：打开 [Intent-to-Effect Flame Graph](intent-to-effect-flame-graph.md)
- 想看时间顺序：打开 [Causal Timeline](causal-timeline.md)
- 想看改动范围：打开 [Run Impact Map](run-impact-map.md)
- 想看 repo 探索和 patch：打开 [Agent Workspace Map](agent-workspace-map.md)
- 想看多个 agent：打开 [Multi-Agent Causal Map](multi-agent-causal-map.md)
