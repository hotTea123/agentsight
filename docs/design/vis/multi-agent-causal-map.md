# Multi-Agent Causal Map

多 agent 系统最难读的地方不是消息多，而是因果关系分散。

Agent A 的一句话可能触发 Agent B 的工具调用；Agent B 的失败可能让 Agent C 改计划；一个 MCP server 或 background worker 可能在两个 agent 之外改变系统状态。

Multi-Agent Causal Map 的目标是回答：

> 多个 agent、tool worker 和系统动作之间，谁影响了谁？

## 适合什么场景

- multi-agent coding
- agent handoff debugging
- MCP server 行为理解
- parallel agent solving and merge conflict
- agent swarm 的资源和结果归因

## 视觉结构

```text
Agent A: planner
  semantic: assign API test fix
    -> handoff to Agent B

Agent B: coder
  semantic: patch handler
    -> tool: Edit src/api/handler.ts
      -> system: write src/api/handler.ts
    -> tool: Bash("npm test")
      -> system: npm -> node -> jest
      -> result: exit 1
        -> feedback to Agent A

Agent C: verifier
  semantic: investigate failing test
    -> tool: Read tests/api.test.ts
      -> system: open tests/api.test.ts
    -> message to Agent B: "handler returns wrong status"

Unattributed worker
  system: background process writes cache
```

## 必须跨 agent 和跨系统层

不能只画 conversation graph：

```text
Agent A -> Agent B -> Agent C
```

也不能只画 process tree：

```text
npm -> node -> jest
```

合格的 Multi-Agent Causal Map 要展示：

```text
Agent A semantic decision
  -> Agent B tool call
    -> process/file/network effect
      -> result
        -> Agent C or Agent A next decision
```

## 冲突视图

多 agent coding 的一个强场景是解释 conflict：

```text
Agent B
  intent: fix API status
  write: src/api/handler.ts
  reason: unit test failed

Agent C
  intent: add auth fallback
  write: src/api/handler.ts
  reason: integration test failed

Conflict:
  same file, same function, different failure source
```

冲突类型可以分成：

- text conflict
- semantic conflict
- test-result conflict
- dependency conflict
- workspace state conflict
- human approval conflict

## 视觉编码

- 每个 agent 一条 swimlane。
- tool/process/file/network effect 作为 agent lane 下的子节点。
- 跨 agent message 用实线。
- system result feedback 用回环线。
- shared resource 冲突用汇聚节点。
- unattributed worker 单独一条 lane。

## 这张图的价值

它不是为了展示“多 agent 很复杂”。它是为了找到：

- 哪个 agent 的判断影响了其他 agent？
- 哪个 tool result 被错误传播？
- 哪个系统动作绕过了 agent message 层？
- 哪个 workspace/resource 冲突导致最终失败？
- 哪个 agent 可以被重跑，哪个不能？

这张图应该和 [Causal Timeline](causal-timeline.md) 联动：timeline 看时间顺序，Multi-Agent Causal Map 看跨 actor 因果。
