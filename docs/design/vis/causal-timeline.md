# Causal Timeline

普通 timeline 很容易变成事件流水账：

```text
10:01 LLM call
10:02 process exec
10:03 file write
10:04 network request
```

这不够。AgentSight 的 timeline 必须显示跨层因果：

> 哪个语义决策导致了哪个系统动作，哪个系统结果又改变了下一轮 agent 决策？

## 适合什么场景

- debug 一次 agent 失败
- 找重试、循环、等待、卡住和无效动作
- 看 tool result 如何影响下一轮 LLM turn
- 检查 agent 声称成功但系统没有产生对应结果的情况

## 视觉结构

```text
Time ───────────────────────────────────────────────────────────>

User goal     fix failing API test

LLM intent    inspect failure ───── run test ───── patch ─ verify
Tool call     Read test file        Bash npm test  Edit    Bash npm test
Process       open                  npm -> node           npm -> node
Files         tests/api.test.ts     read src/tests write  read src/tests
Network                            registry.npmjs.org
Result                             exit 1          diff   exit 0

Causal links:
  exit 1 -> next LLM turn "patch handler"
  diff -> next tool call "rerun focused test"
```

## 必须有反馈环

Agent 行为不是一次请求。它是一个 feedback loop：

```text
intent -> action -> observation -> revised intent -> action
```

所以 timeline 里最有价值的线不是从 LLM 到 tool 的线，而是从 system result 回到下一轮 LLM 的线：

```text
test failed
  -> model decides to inspect src/api/handler.ts

network timeout
  -> model retries with npm cache clean

file not found
  -> model runs rg to locate renamed module
```

如果没有这种 feedback arrow，timeline 就只是日志播放器。

## 视觉编码

- 横轴是时间。
- 行不是 telemetry source，而是因果层次：intent、tool、process、effect、result。
- 跨行线条表示 attribution。
- 回环线条表示 feedback。
- 灰色 lane 表示无法归因到明确 LLM/tool 的系统活动。
- 用户 approval/rejection 可以作为显眼节点，表示人类改变了 agent trajectory。

## 查询入口

Timeline 应该支持这些问题：

- 哪个 failed command 导致下一轮 prompt 变长？
- 哪个 tool result 被 agent 忽略了？
- 哪个系统动作发生在任何 tool intent 之前？
- 哪个 retry 没有改变输入却重复消耗 token？
- 哪个 LLM turn 声称完成，但后面没有对应 file/network/test effect？

这张图适合细查过程，不适合做第一屏。第一屏应该是 [Agent Run Map](agent-run-map.md)，timeline 是点击某个 phase 后展开的细节。
