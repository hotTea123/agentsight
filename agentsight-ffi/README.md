# libagentsight：进程内工具 span 证据采集

本模块供 ICAgent 等 Go/C 宿主加载。Rust 提供稳定 C ABI、后台线程、有界队列、
eventfd、错误和统计；直接调用现有 C/libbpf 采集核心。**不启动 AgentSight 或
`bpf/process` 子进程，不包含 Tokio、数据库、LLM 解析和 OTLP 上报。**

业务进程仍加载独立的 `libagentsight_tool_marker.so`，由正式侵入式插桩调用
bind/unbind。采集库不能代替 marker，不能自行生成或猜测工具上下文。

## 交付与构建

- `lib/libagentsight.so`：ICAgent 进程加载的采集库。
- `include/agentsight.h`：cbindgen 0.29.2 从真实 Rust 声明生成；不要手改。
- `lib/libagentsight_tool_marker.so`：供业务进程使用的 ABI v1 marker。
- `examples/go/main.go`：Go/cgo 示例；`tests/capture.c`：C 拉取式消费示例。

当前产物为 Linux ARM64；amd64 要在对应架构的构建环境重新构建和验证，不能复用
ARM64 二进制。头文件不能保证任意 libc、内核或 CPython 构建兼容。

本项目必须在 `claude-agentsight-ebpf-build` 容器中构建，先同步当前工作树至
`/tmp/jiuwenclaw-agentsight-build`。然后在该目录执行：

```sh
make -C bpf libagentsight_tool_marker.so
cargo build --release --locked --manifest-path agentsight-ffi/Cargo.toml
cargo test --release --locked --manifest-path agentsight-ffi/Cargo.toml
```

需要 Rust 1.88+、Clang/BPF 工具链、make、libelf/zlib 开发文件。首次构建需要下载
Cargo.lock 中的依赖。`build.rs` 调用 `make ffi-core`，使用独立 PIC libbpf 归档，
并隐藏 C/libbpf 内部符号；不链接原 collector 或 agentsight-capture-core crate。
运行宿主不需要 Rust/Cargo。具体 ELF 依赖和校验和见交付包报告。

## 调用顺序与返回值

1. `agentsight_abi_version()` 与 `AGENTSIGHT_ABI_VERSION` 比较。
2. `agentsight_init(&handle)`：仅创建句柄，不采集。
3. `agentsight_set_config(handle, &config)`：检查并深拷贝配置。
4. `agentsight_start(handle)`：启动发现线程；Jiuwen 尚未运行也可成功返回。
   再通过 `agentsight_get_stats()` 确认 `target_state == AGENTSIGHT_TARGET_READY`。
5. `agentsight_get_eventfd(handle)`：获取借用 FD，加入宿主 poll/epoll。
6. FD 可读后调用 `agentsight_poll_events(handle, events, capacity)`，直到返回 0。
7. 每个返回的事件调用且仅调用一次 `agentsight_free_event(event)`。
8. `agentsight_stop(handle)`，继续 poll 排空剩余事件，再 `agentsight_free(handle)`。

除版本、FD、poll 数量及 void free_event 外，函数成功返回 0，失败返回负 errno。
poll 返回非负事件数或负 errno。错误不会调用 exit/abort 或向宿主发送停止信号。
无 `suspend/enable/disable`，没有暂停或热更新协议。

同进程可创建多个未启动句柄，但同一库实例只允许一个活跃采集实例（其余 start
返回 `-EBUSY`）。start 在运行状态下幂等。停止后必须排空队列，才能修改配置或
重新启动。重启重建 BPF maps，**不会恢复之前工具的绑定，也不会补采停止期间的行为**。

free 会先 stop 并丢弃尚未取出的队列数据；已取出的事件独立存活，可以在 handle
释放后调用 free_event，但必须在卸载 `.so` 前释放。所有对象都不得用宿主 free/delete
释放。借用 eventfd 不得自行 read/write/close，库在 poll_events 中确认和重新通知。
部分读取后 FD 保持可读；终止/后台失败也会通知，空队列不等于采集器仍健康。

## 线程、安全与错误

- init/start/stop/config 由库同步；推荐宿主串行管理生命周期，单消费者读取事件。
- free 不得与任何使用同一 handle 的调用并发；free_event 不得与使用该事件并发。
- 入口检查 NULL/长度/ABI，但不能验证任意伪造指针、悬空指针或 double-free。
- fork 后继承的句柄拒绝调用；库曾初始化后，子进程必须 exec 才可重新使用该库。
- 不修改宿主 SIGINT/SIGTERM 处理、不重定向 stdout/stderr、不自动 sudo，不提高
  RLIMIT_MEMLOCK。缺权限或缺运行库应由宿主处理，降级为增强缺失。
- `agentsight_get_last_error(handle, buffer, capacity)` 复制到宿主缓冲区，不依赖
  thread-local，适合跨 OS 线程迁移的 cgo 调用。返回码是判定本次操作结果的依据；
  last_error 是可被其他线程或后台诊断覆盖的最近信息。
- Rust unwind panic 在 FFI/工作线程边界捕获；不能隔离 C 段错误、内存破坏、abort
  或进程级内存耗尽。库崩溃仍可能终止 ICAgent，线程并非进程故障隔离。
- Go 不应把 Go 字符串/切片或未固定的 Go 指针留给后台线程。配置用 C.CString
  传入后可立即释放；poll 输出指针数组使用 C.malloc；事件内容复制到 Go 后释放。

## ABI v2 配置与自动发现

本版本修改了配置/统计结构，**必须使用配套 v2 头文件重新编译宿主**，不能与旧
v1 二进制混用；业务 marker ABI 仍是 v1。清零 `AgentsightConfig`，填写
`struct_size=sizeof(...)`、`abi_version=AGENTSIGHT_ABI_VERSION` 即可使用默认配置。
不再由 ICAgent 提供 PID、Python 路径、marker 路径或四个偏移。

- `procfs_root`：默认 `/proc`；必须是宿主 PID 视图且采集进程在宿主 PID namespace。
- `discovery_interval_ms`：默认 1000，允许 100～60000；扫描不是零延迟发现。
- `queue_capacity`、`max_queue_bytes`：控制用户态有界队列。

库扫描进程实际映射的 `libagentsight_tool_marker.so` 并检查三个约定符号，选择唯一
独立业务根进程。fork 后继承 marker 的已观察子进程不成为新根；多个独立候选则
明确降级，不按名字/时间猜选。文件通过 `map_files` 固定到实际映射；无该权限时
回退 `/proc/PID/root`，仍必须核对设备号和 inode，无法确认就不挂载。

Python 适配读取实际加载的 `_asyncio`、Python 可执行文件或 `libpython` ELF，
不运行目标 Python，不加载目标代码进入 ICAgent：

1. CPython 3.11 探针模型且四个函数符号齐全：libbpf 按符号解析文件偏移。
2. 符号缺失：仅命中经过验证的 **Python + _asyncio 双 Build ID** 才使用内置偏移。
3. 其他版本、未知剥离构建或架构不匹配：`DEGRADED`，不猜偏移，不阻止 Jiuwen。

目前真实采集验证为 CPython 3.11.2 ARM64；完整符号路径有 ELF/规则测试，但没有
覆盖所有 3.11 构建，不能承诺任意 Python 升级自动兼容。新增小版本探针模型或
未知剥离构建仍需库内适配与验证，ICAgent 接口无需再次提供偏移。

当前已验证的双 Build ID：Python `d190c32fa97c85641233f1a813582deecb960b8c`，
_asyncio `1007e17e3911f3b90839a8823bc3228925b817d0`。四个偏移为
24496、19600、2528016、2546048，**仅属于该配置，不能按版本号直接复用**。

`target_state` 为 IDLE / WAITING / ATTACHING / READY / DEGRADED。
RUNNING 只表示发现线程工作，不代表已经增强。库持续扫描、重试，进程退出或
同 PID exec 会通过内核 sticky 标记失效旧关联、卸载探针并重新发现；成功挂载增加
`target_generation`。重挂会有证据缺口，不恢复工具绑定，已入队证据保留原 ID。
扫描每批进程之间继续非阻塞读取采集缓冲区；增强模式在内核侧过滤无关 open/exec/exit，
业务中未绑定工具的 open 仅用于 FD 跟踪，不进入证据队列。

容器仅挂载 host `/proc` 但不共享宿主 PID namespace 尚不支持。私有 PID namespace
下的 `/proc` 自检不能证明看到了整台宿主机，部署端必须满足上述条件。首次扫描前
已成为孤儿的 fork 子进程无法仅靠映射还原来源；本期仍按一个节点一个业务根进程
的部署约定使用。marker 是协作标识，不是安全认证或租户隔离边界。

默认队列 1024 条/16 MiB。队列满时立即丢弃新证据并计数，不等待宿主腾出空间。
max_queue_bytes 约束排队的序列化数据和事件元数据，不是宿主 RSS 的上限；BPF maps、
ring buffer、解析临时内存和已交给宿主的事件不在此预算内。

## 事件及已有 span 的增强

仅向外返回具有有效且 sampled 的 `trace_id + parent_span_id` 的文件 open/write/
rename 和进程 exec 事件。ID 在事件产生时已由 BPF 快照，不在读取时重新查上下文。
JSON 保留 `agentsight_task_id`、`agentsight_bound_task_id`、
`agentsight_correlation_source`、路径、聚合计数、字节数及已采到的 syscall 结果。
不保证每个事件都有成功/失败结果：没有结果的操作不能被解释为已成功。

接入方需创建一个**新的证据子 span**，复用 trace ID，以 parent_span_id 作为父 span，
分配新 span ID。不要重写、重新结束或重新上报已经结束的原 tool span。
对齐原增强转换逻辑时：只读 open 对应 `agentsight.file.open`（检查 flags 的
访问模式位），write/rename/exec 分别映射为 `agentsight.file.write`、
`agentsight.file.rename`、`agentsight.process.exec`。其余 open 可按产品策略过滤；
不要重复创建 LLM span。rename 的负 syscall_return 表示失败；聚合 write 不是
一条原始 syscall。协议并未为未观测的结果或耗时补造证据。

timestamp_ns 是内核 CLOCK_MONOTONIC；wall_time_ns 使用本机双时钟锚点估算 Unix
纳秒，可供宿主构造时间。duration_ns=0 表示未知。保留原始时间便于校准，不承诺
任意部署无 clock-skew。span 归属仅依赖 ID，绝不按时间窗口/路径/名称猜测。

## 统计与限制

get_stats 还返回目标状态/PID/启动代次、适配方式、扫描错误、挂载尝试/失败和卸载数。
`kernel_ring_dropped` 统计增强 open/rename/exec/exit 探针的 ring-buffer 申请失败；
`supported_counters` 的 `AGENTSIGHT_STATS_KERNEL_RING` 位表明提供该项。
并返回生命周期状态、记录过滤/解析失败、入队/出队、队列满丢弃、序列化
丢弃、后台错误/诊断、排队量和已有聚合 map 溢出统计。聚合溢出是既有计数器的
尽力统计；**尚不覆盖全部 Task/context map 满、读取失败或关联失效**，
不能把全部已提供计数为零理解成“没有任何证据缺口”。

继续继承当前穿刺的 CPython 版本、Task/PyContext/PID 复用、固定 map 容量/链深、
后台任务、远程 MCP 等限制。本次没有增加通用生产化兼容性、脱敏或租户鉴权机制。
路径和命令行可能敏感，接入方应在上报前实施过滤/脱敏。

## 验证与接入状态

`src/tests.rs` 覆盖 API、队列、通知、ID/sampling 和所有权；`tests/api.c` 检查
动态加载、符号、失败清理、fork 防护及宿主信号/资源限制；`tests/header.cpp`
检查 C++ 布局与调用。`tests/test_live.py` 在独立测试目录使用正式 LocalFunction/
callback 和真实 BPF 检查 ID、四种关联来源、异常、重启及关闭增强对照，不访问 Jaeger。

`tests/test_discovery.py` 验证延迟加载、同 PID exec、fork/已观察孤儿排除、进程重启
和多根歧义。未知 stripped ELF 对照使用修改 Build ID 的独立测试副本，不修改系统 Python。

Go 示例在交付目录中可执行 `GO111MODULE=off go run ./examples/go`，无参数只验证
ABI/生命周期；真实采集使用 `-capture -seconds 30`，可选 `-procfs-root /proc`。
它不是 ICAgent 插件实现。**ICAgent 内部接线、实际 Jaeger 上报、x86_64、cgroup
多容器隔离和生产性能必须由后续集成测试验证**。实际通过项以随包测试报告为准。

替换头文件或 `.so` 后，应在示例/宿主包目录中强制重新编译（例如 `go build -a`）；
Go 的构建缓存可能未感知包目录外的 C 头文件变化。出现
`stats buffer too small` 时先检查配套头文件和缓存，不应忽略返回码继续读结构体。
