# Intent-to-Effect Flame Graph

普通 flame graph 回答“CPU 时间花在哪里”。AgentSight 的火焰图应该回答：

> 哪些 agent intent 造成了最大的系统 effect？

这里的关键词是 **Intent-to-Effect**。不能只画 token/cost，也不能只画 process tree。每个宽块都应该能看见上层语义和下层系统动作之间的关系。

## 适合什么场景

- 快速找一次 run 里最重、最贵、最有影响的阶段
- 发现 agent 在哪里反复尝试、绕路或空转
- 比较两个 agent/model/prompt 的行为差异
- 生成静态 SVG 贴进 run summary 或 PR comment

## 数据结构

```text
Session
  -> LLM turn / semantic phase
    -> tool intent
      -> process tree
        -> file/network/resource/test effect
```

例子：

```text
run;fix api test;run tests;npm;node;jest;read tests/api.test.ts
run;fix api test;run tests;npm;node;jest;exit 1
run;fix api test;patch handler;edit;write src/api/handler.ts
run;fix api test;verify;npm;node;jest;exit 0
run;unattributed;background git status
```

## 视觉编码

默认 width 不应该只是 token 或 cost。更好的默认值是 `effect weight`：

```text
effect weight =
  duration
  + process count
  + file writes
  + network destinations
  + retries
  + failed exits
  + generated diff size
```

可以提供切换：

- `width = time`
- `width = token/cost`
- `width = process count`
- `width = file/network effect`
- `width = no-progress retries`

颜色用于表达 effect 类型：

- 蓝色：LLM/model activity
- 绿色：successful tool/system action
- 黄色：retry or no-progress loop
- 红色：failed result or surprising effect
- 灰色虚线：unattributed system activity

## 它和普通 flame graph 的区别

普通 process flame graph：

```text
npm -> node -> jest
```

AgentSight flame graph：

```text
semantic intent: verify API fix
  -> tool intent: Bash("npm test tests/api.test.ts")
    -> process: npm -> node -> jest
      -> file reads: tests/api.test.ts, src/api/handler.ts
      -> result: exit 0
```

如果没有 semantic intent，图就只是系统 profiler。

如果没有 process/file/network/result，图就只是 LLM trace。

## 一眼抓人的形态

```text
┌────────────────────────────────────────────────────────────┐
│ fix failing API test                                       │
├──────────────┬──────────────────────────┬──────────────────┤
│ understand   │ run tests                │ patch + verify   │
│ Read files   │ Bash npm test            │ Edit + npm test  │
│              ├──────────────┬───────────┤                  │
│              │ npm -> node  │ exit 1    │ write + exit 0   │
└──────────────┴──────────────┴───────────┴──────────────────┘
                 width = effect weight
```

这张图最适合作为 AgentSight 的招牌图。它把 “agent 怎么想” 和 “机器上发生了什么” 放在同一棵树里。
