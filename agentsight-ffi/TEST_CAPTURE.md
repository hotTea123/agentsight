# libagentsight_test.so：ICAgent 采集链路联调包

本包是独立测试库，不是正式 span 增强库。用于验证：

```text
init → set_config → set_test_target → start → READY
     → 手动操作测试进程 → eventfd通知 → poll_events → free_event
     → ICAgent格式转换/上报 → stop → 排空事件 → free
```

真实使用eBPF，不生成模拟事件；不需要Jiuwen、Python探针适配、marker或OTel插桩。
仅采集指定普通进程及探针启用后新建的后代，范围为文件open、write、rename和程序exec。
不采集全节点，不支持任意syscall编号，不保证完整审计。ICAgent上报仍由接入方实现。

## 1. 交付内容和环境

```text
lib/libagentsight_test.so       ICAgent测试程序加载
include/agentsight.h            基础ABI v2，11个原有接口及结构
include/agentsight_test.h       测试扩展接口与schema常量
examples/go-test/main.go        可构建Go消费示例
README.md                      本说明（源码中为TEST_CAPTURE.md）
TEST_CAPTURE_REPORT.md         本版测试结果和未验证项
reports/                       真实采集验证记录
```

当前产物：Linux ARM64；Go示例验证环境1.21.4；依赖glibc（最高所需符号GLIBC_2.34）、
libelf.so.1、libz.so.1、libgcc_s.so.1。需具有eBPF/tracepoint加载权限、BTF及正确的宿主PID和
procfs视角。不能把ARM64库放到x86_64运行；未知内核/发行版需验证。本库不需要Python运行时，
验收脚本用Python只是为了驱动独立测试进程。

不要在同一进程同时加载正式libagentsight.so和测试库：两者公开函数重名，应使用独立
联调进程或测试部署。生产默认库不会导出agentsight_set_test_target。

## 2. 新增接口

```c
#include "agentsight_test.h"
int32_t agentsight_set_test_target(AgentsightHandle *h, int32_t target_pid);
```

| 参数 | 说明 |
|---|---|
| h | init成功创建的句柄；先调用set_config |
| target_pid | 测试进程的宿主PID，必须存在且>1；不能是采集器自身或其祖先进程 |

返回0成功；负errno失败：

- `-EINVAL`：缺少配置、PID0/1/自身/祖先进程、procfs视角不匹配等。
- `-ESRCH`：目标不存在、已经退出或无法读取目标身份。
- `-EBUSY`：工作线程存在或旧队列未清空，先stop并排空。
- 通用handle错误仍适用，例如NULL、fork继承状态。

设置时固定PID+启动标识，内核同时校验根进程启动标识，目标退出不自动追随复用PID。
不包含启动前已存在的子进程。采集开始后的fork后代在内核同步纳入，退出时清理。
根目标退出后停止采集，目标状态降级并提示重新选择；不会自动发现其他进程。
set_config成功会清除测试目标，之后必须重新set_test_target；失败设置不应用新配置。
禁止选采集器祖先进程是为了避免把采集器自身也纳入后代范围而产生自采集反馈。

## 3. 原有接口如何使用

接口函数签名、配置结构、事件结构和统计结构保持基础ABI v2，Config/Event/Stats为
32/88/192字节（使用sizeof，不硬编码）。仅新增第12个测试函数。

| 接口 | 测试版用途/返回 |
|---|---|
| agentsight_abi_version() | 返回2；另用测试扩展符号确认链接了测试库 |
| agentsight_init(&h) | 创建句柄，0成功；尚不采集 |
| agentsight_set_config(h,&cfg) | 清零cfg后填struct_size、abi_version；0成功；复制配置字符串 |
| agentsight_set_test_target(h,pid) | 选择受控测试进程，0成功 |
| agentsight_start(h) | 启动线程，0成功不等于READY；无目标配置返回-EINVAL |
| agentsight_get_eventfd(h) | 借用通知fd，非负成功；仅poll/epoll，不read/write/close |
| agentsight_poll_events(h,slots,capacity) | 非阻塞返回条数；0为空；负值为错误；capacity为1～65536 |
| agentsight_free_event(event) | 每个返回事件恰好释放一次，不用普通free；NULL允许 |
| agentsight_get_stats(h,&stats,sizeof(stats)) | 0成功；读取READY、丢弃、积压及目标信息 |
| agentsight_get_last_error(h,buf,size) | 0成功，-ENOSPC表示截断且NUL结尾；文本不是线程局部 |
| agentsight_stop(h) | 停止/卸载，0成功；用户队列保留，继续poll排空 |
| agentsight_free(h) | 释放句柄，0成功；会丢弃尚未取出数据，必须检查返回值 |

配置范围：queue_capacity=0默认1024（最大65536），max_queue_bytes=0默认16MiB
（非零1KiB～1GiB），procfs_root=NULL默认/proc。discovery_interval_ms仍按基础ABI校验，
但测试版不执行marker扫描；目标身份检查约100ms一次，不是严格实时保证。
test target_state=READY时adaptation_method=0是正常的：没有Python适配。
先等待READY后再动手执行命令；权限/挂载问题可能在start之后表现为FAILED/DEGRADED。

生命周期串行、单消费者，free前结束同handle调用。配置字符串调用后可释放；C事件内容
复制到Go后再free_event；指针槽数组用C.malloc/C.free。库内存预算不包含已取出的事件或
宿主队列。stop/start不清零累计统计，不保留采集期间的旧map。

## 4. 测试事件与正式事件的区别

公共结构schema_version固定为**1001**（头文件AGENTSIGHT_TEST_EVENT_SCHEMA_VERSION），
trace_id、parent_span_id、trace_flags均为0。JSON带`agentsight_test_only=true`和
`agentsight_correlation_source="none"`，不伪造工具上下文。

| event_type / JSON event | 内容和边界 |
|---|---|
| 1 / FILE_OPEN | syscall入口的打开尝试，filepath、flags、pid、comm、count；不代表打开成功或已读内容 |
| 2 / FILE_WRITE | 成功写入的FD聚合，count、total_bytes、fd、detail、path_resolved；不是逐次系统调用 |
| 3 / FILE_RENAME | 重命名尝试的目录聚合，count、detail（目录）、last_destination_path；后者仅为最近一次目标 |
| 4 / EXEC | 成功程序映像切换，filename、full_command、pid、ppid；不是最终退出结果 |

write/rename的JSON保留原有SUMMARY的type等字段，并增加aggregation=true。
rename没有完整源路径、返回值和耗时，不等同于正式增强版的rename结果事件。
write中path_resolved=true表示用户态排出时通过/proc/PID/fd查询，**不是写入发生时的路径快照**；
FD关闭、复用或进程退出时可能缺路径或不对应早先写入，不用它证明精确文件归属。
不保证所有write都是普通文件，FD也可能指向管道/终端等。聚合大约周期排出，stop也会排出。

wall_time_ns是估算Unix时间；timestamp_ns是单调时钟。duration_ns在本版四类测试事件中
未提供实测值，0代表未知。pid/tid字段可能存在缺省，不能要求所有记录含非零tid。
路径、命令有长度限制且可能敏感；demo只应在受控测试环境打印。

正常队列满丢新事件并计数。kernel_ring_dropped仅覆盖当前测试open/exec/exit的ring申请失败，
aggregate_overflow是部分聚合/跟踪失败统计；零计数不等于完整无损采集。

## 5. Go构建与手动测试（两个终端）

在完整交付包根目录编译，不需要Rust或构建库的容器：

```sh
CGO_ENABLED=1 GO111MODULE=off go build -a -o /tmp/agentsight-test-host ./examples/go-test
/tmp/agentsight-test-host
```

无参数只检查ABI，预期`TEST ABI passed ... schema=1001`，不启动采集。
示例直接链接`-lagentsight_test`，rpath指向编译时的交付lib目录；复制到其他机器时需要
重新安排库路径。库缺失可能在Go main之前失败，本例不是生产可选加载框架。

终端A：准备专门的shell，保持它运行，不从该shell启动采集器：

```sh
bash --noprofile --norc
echo $$
```

记下该shell的宿主PID。终端B（具有采集权限）：

```sh
/tmp/agentsight-test-host -pid <终端A的宿主PID> -seconds 60
```

看到`STATE target=3`（READY）后，回到终端A执行：

```sh
test_dir=$(mktemp -d /tmp/agentsight-manual.XXXXXX)
printf 'hello\n' > "$test_dir/before.txt"
cat "$test_dir/before.txt"
mv "$test_dir/before.txt" "$test_dir/after.txt"
/bin/true
```

终端B应收到schema1001的JSON；写入/rename聚合可能晚几秒出现。程序到期或Ctrl+C会
stop、排空并free。测试文件保留在终端A打印/记录的专用目录，按需自行清理。
不需要marker.so。容器终端中`$$`可能是容器PID，本版要求传宿主PID，不能盲目照抄。

## 6. ICAgent同事需要做什么

1. 测试部署链接/加载libagentsight_test.so，包含两个头文件。
2. 初始化、基础配置、测试PID配置、start，等待READY。
3. 等待eventfd、poll读取、复制和释放，适配schema1001及四类测试payload。
4. 按本系统测试数据格式上报；不能因为trace_flags=0就按正式未采样规则把测试事件全部丢掉。
5. 若需Jaeger展示，由ICAgent明确创建独立测试trace和有效新ID，标记test_only；**不能**把全零ID
   当作合法父上下文，也不能声称验证了Jiuwen原始工具span关联。
6. 检查上报确认与失败计数；库delivered_events只表示取走，不代表后端收到。
7. stop后排空，解除fd监听，释放handle。再次配置/启动前排空；记录丢弃和状态。

本包没有ICAgent私有代码、网络发送或OTLP exporter。验证到Go消费不等于后端上报验收。

## 7. 维护者源码构建与回归

仅库维护者执行；使用二进制包无需重新编译。准备与目标架构一致的Linux环境、
Rust 1.88+、C编译器、Clang/BPF、make、libelf/zlib开发文件及完整的libbpf/bpftool子模块。
在AgentSight源码根目录执行，不限定容器名称或源码路径。下例使用可用的cc作为HOSTCC，
可按环境替换为gcc或clang；首次构建需能下载Cargo.lock中锁定的依赖：

```sh
MAKEFLAGS="HOSTCC=cc" cargo test --release --locked --features test-capture \
  --manifest-path agentsight-ffi/Cargo.toml --target-dir /tmp/agentsight-test-capture-target
MAKEFLAGS="HOSTCC=cc" cargo build --release --locked --features test-capture \
  --manifest-path agentsight-ffi/Cargo.toml --target-dir /tmp/agentsight-test-capture-target
```

Cargo crate名字仍为agentsight，构建结果名为libagentsight.so；**仅将test-capture构建产物**
复制并命名为libagentsight_test.so交付，不能把正式库简单改名当测试库。
test-capture使用独立BPF/PIC输出目录，不复用正式构建的采集对象。依赖已缓存时可加--offline。

真实验收入口（会启动探针和专用进程，在有权限环境执行）：

```sh
python3 agentsight-ffi/tests/test_standalone_capture.py \
  --library /tmp/agentsight-test-capture-target/release/libagentsight.so \
  --report /tmp/agentsight-test-verification.json
```

可加`--go-demo <测试Go程序绝对路径>`验证Go实际消费；需保证其动态库搜索路径可用。
该脚本创建临时测试文件并自动删除，不访问真实业务文件；不会向Jaeger或外部服务发送数据。
正式默认构建也需跑cargo test，验证未放宽原始增强关联过滤；本版不替代正式工具增强端到端回归。
