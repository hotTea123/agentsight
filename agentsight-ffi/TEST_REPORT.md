# libagentsight ABI v2 验证报告

验证日期：2026-11-05～2026-11-06。上游基线：`c1574366122a90c451bca6a84713e28813ab95e2`
（v1.0.35），目标分支 `hotTea123/agentsight:span_enhance`。
原有增强迁移提交为 `70438029`；后续提交实现本文描述的自动发现与适配。

## 本次交付范围

- Rust `cdylib` + 直接链接的 C/libbpf 核心；不启动 collector 子进程。
- 按实际加载的 marker 自动发现唯一业务根进程，不要求 ICAgent 提供进程清单。
- 实际 Python ELF 符号解析，以及严格双 Build ID 兜底；未知构建降级。
- 发现线程先启动、延迟加载、退出/exec/重启后的卸载和重新挂载。
- 事件、eventfd、状态/错误/丢弃统计；上报仍由 ICAgent 原有链路负责。
- 库 ABI 为 2，marker ABI 为 1。Config/Event/Stats 大小分别为 32/88/192 字节。

源代码改动集中在 `agentsight-ffi/` 和 BPF 关联/加载代码。保留原 AgentSight
工作树、暂存区和未跟踪交付源文件；未修改 agent-core、JiuwenSwarm 的业务代码。

## 环境

- 所有 AgentSight C/BPF/Rust 构建在 `claude-agentsight-ebpf-build` 容器中进行。
- Linux ARM64，实际宿主内核 `6.6.0-28.0.0.34.oe2403.aarch64`。
- Rust 1.88.0，CPython 3.11.2；Go 消费示例在宿主 Go 1.21.4/ARM64 验证。
- 库动态依赖 `libelf.so.1`、`libz.so.1`、`libgcc_s.so.1`、glibc。
  ELF 要求的最高 glibc 符号版本为 GLIBC_2.34；这不是完整系统兼容性认证。
- 实际采集使用 README 中已列出的 ARM64 Python/_asyncio 双 Build ID。

## 已完成验证

| 检查 | 结果 |
|---|---|
| release 构建、Rust 单元测试、格式检查 | 通过；12/12 |
| C ABI、动态符号、失败清理、fork 防护、信号/RLIMIT 不变 | 通过 |
| C++ 结构布局和调用 | 通过 |
| Go/cgo 强制重编译、ABI/布局、后台降级和停止 | 通过；Config/Event/Stats 为 32/88/192 |
| ELF 完整符号、strip、Build ID 缺失、版本、偏移边界 | 通过 |
| 上游 BPF C 单元测试 | 通过：3 + 63 + 73 + 53 + 29；SSL JSON 检查通过 |
| 正式 callback，业务先启动 | 16 个 tool span，71 条关联证据 |
| 正式 callback，采集库先启动 | 16 个 tool span，71 条关联证据 |
| 关闭增强 | 16 个 tool span，0 条增强证据 |
| 未知 stripped 构建，增强仍开启 | DEGRADED；16 个 tool span，0 条增强证据 |
| 自动发现/生命周期 | 9 项通过，见下文 |

正式业务测试使用当前本地 agent-core、JiuwenSwarm 的 `LocalFunction` 和正式
observability callback，不自建替代 tool span。每个场景执行两轮，期间 stop/start
采集器；检查 `trace_id + parent_span_id`、文件类型、异常清理、并行隔离及
`task / task_parent / py_context / process` 四条关联路径。工具外行为不能被错挂。
所有通过的最终场景中，队列丢弃、内核 ring 申请失败、JSON 错误及后台错误均为 0。

未知构建对照仅对独立 `_asyncio` 测试副本移除 Build ID，未改系统 Python。
该副本仍能执行业务，但无法通过已验证的偏移配置，因此必须拒绝挂载。

9 项生命周期检查：宿主先启动、延迟加载 marker、同 PID 同二进制 exec、fork
子进程、已观察孤儿不升为根、业务根重启、多根歧义、歧义消除后恢复，以及同名
但不具备 marker 符号的普通动态库不被识别为目标。

迭代中发现过扫描期间丢事件：无关 open/exec/exit 占用 ring，且大节点扫描会延迟
消费。本版在增强模式内核侧提前过滤，并在扫描每批进程间继续非阻塞消费。
增加 `kernel_ring_dropped`，验收明确断言该值为 0；未用早期通过记录替代最终复测。

最终完整测试记录位于构建容器：

- `/tmp/agentsight-span-v2-verified-20261105/verification.json`
- `/tmp/agentsight-span-v2-lifecycle-20261105/verification.json`

交付目录 `reports/` 保留上述 JSON 副本。

## 复测入口

先将当前源代码和 libbpf/bpftool 子模块同步到规定的构建容器目录，再执行：

```sh
make -C bpf process libagentsight_tool_marker.so unit-test
cargo test --release --locked --manifest-path agentsight-ffi/Cargo.toml
cargo build --release --locked --manifest-path agentsight-ffi/Cargo.toml
cargo fmt --check --manifest-path agentsight-ffi/Cargo.toml
```

C/C++ 检查入口为 `tests/api.c`、`tests/header.cpp`；链接 `-lagentsight`，api
另链接 `-ldl`。真实采集宿主为 `tests/capture.c`，传入默认配置即可，不传偏移。
`tests/elf_fixture.c`/`tests/runtime_elf.c` 检查 ELF 元数据；不是实际 Python 实现。

`tests/test_live.py` 需要正式 agent-core/JiuwenSwarm 及其依赖可导入，参数为
`--workdir <新测试目录> --marker <真实marker绝对路径> --capture <C测试宿主>`；
可增加 `--unsupported-asyncio <去除BuildID副本所在目录>`。
`tests/test_discovery.py` 使用相同三个参数；可传 `--decoy-marker <同名libz副本>`。
这些测试应串行运行，避免彼此成为多个独立业务候选。

Go 示例的 ABI/生命周期冒烟检查：

```sh
GO111MODULE=off go build -a -o /tmp/agentsight-host ./examples/go
/tmp/agentsight-host
/tmp/agentsight-host -capture -seconds 1 -procfs-root /nonexistent-agentsight-test/proc
```

最后一条应报告目标 DEGRADED 并正常停止，不代表成功挂载探针。

## 未完成验证与边界

- **不是 ICAgent 插件和 Jaeger 的端到端联调**；正式业务测试使用内存 span exporter。
- 只真实验证 CPython 3.11.2 ARM64；完整符号策略有 ELF/选择规则测试，未覆盖所有
  3.11 构建。3.11.4、3.12+、x86_64 和其他发行版不据此宣称已支持。
- 上游完整 collector 构建未通过验证：离线缺少锁定的 `hyper-util 0.1.21`，在线
  补齐尝试超时。本 `.so` 构建不链接 collector，已独立构建和验证。
- 容器采集端要求宿主 PID namespace 和可访问的宿主 procfs；仅 bind-mount host
  `/proc` 的私有 PID namespace 模式未实现。不能把默认 `/proc` 当作宿主可见性的证明。
- 首次扫描之前已成为孤儿的 fork 子进程无法可靠还原来源；仍以每节点一个业务根
  为前提。重挂存在证据缺口，不能恢复正在执行工具的绑定。
- `Task*`/`PyContext*` 复用、所有 map 失败/读取失败、长期后台任务/持久 MCP、隐私
  策略及高并发性能仍需要后续工程化；计数为 0 不证明不存在任何观测缺口。
- 进程内动态库不能隔离 native 段错误或进程级 OOM。
