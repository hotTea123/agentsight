# ICAgent 接入测试库：编译与接口调用指南

本文可独立交给 ICAgent 开发同事。只需本文和配套的三个文件：
`libagentsight_test.so`、`agentsight.h`、`agentsight_test.h`。
不要求阅读其他增强文档，也不要求获取源码仓库或使用包内其他示例。

## 1. 这轮要完成什么

本轮只验证 **Go 程序能编译、动态库能加载、接口能按顺序调用并取到测试数据**。
不要求接入 Jiuwen、工具 span、链路关联或上报系统，不需要 marker、Python 或 Rust 环境。
库在调用方进程内运行，start 创建内部工作线程，不另外启动 AgentSight 服务进程。

分两步验收，不能把第一步成功当作第二步成功：

| 步骤 | 验证内容 | 成功标志 |
|---|---|---|
| A：编译和基础接口 | 版本、初始化、配置、通知 FD、统计、错误文本、空队列读取、NULL 事件释放、未启动时停止、句柄释放 | BASIC PASS + FREE rc=0，退出码0；不加载 eBPF |
| B：完整采集接口 | 选定普通测试进程、start、等待READY、事件通知、取数、真实事件释放、stop、排空、free | READY、预期测试操作对应的JSON、CAPTURE PASS + FREE rc=0，退出码0 |

B使用真实eBPF，不生成模拟数据。当前固定支持文件open/write/rename和程序exec四类行为，
不是任意syscall编号配置接口；本轮不增加采集类别。目标只是一个专门用于测试的普通进程，
以及探针启用后它新建的后代，不采集全节点，也不要求业务进程做任何插桩。

## 2. 文件准备与环境要求

### 2.1 最小目录

新建一个工作目录，按下面摆放三个交付文件。将第4节的完整代码保存为根目录的main.go：

~~~text
agentsight-smoke/
├── main.go
├── include/
│   ├── agentsight.h
│   └── agentsight_test.h
└── lib/
    └── libagentsight_test.so
~~~

两个头文件必须同时提供，因为agentsight_test.h包含agentsight.h。不要手动改结构体、
函数声明或文件名。链接名是 **-lagentsight_test**，不是-lagentsight。
不要在同一进程混载导出同名函数的其他AgentSight库。

### 2.2 A步骤的要求

- Linux ARM64（aarch64）。这份.so不能在x86_64、Windows或macOS直接使用。
- Go与C工具链都面向Linux ARM64，启用cgo；本文示例已在Go 1.21.4验证。
- 可用的C编译器和标准C开发头文件。只编译Go调用程序，不需要编译采集库。
- 运行依赖：libelf.so.1、libz.so.1、libgcc_s.so.1和glibc。
  本产物最高需要GLIBC_2.34符号，不能用musl环境直接替代；满足该版本不等于已验证全部发行版。
- 进程可以加载共享库、创建eventfd。A步骤不加载BPF，不需要因采集权限而切换root。

可先在工作目录做只读检查：

~~~sh
uname -m
go version
go env GOOS GOARCH CGO_ENABLED CC
file lib/libagentsight_test.so
ldd lib/libagentsight_test.so
~~~

预期架构为aarch64/arm64，ldd输出没有“not found”。缺依赖时由环境维护方按发行版提供，
不要通过更改头文件、关闭ABI检查或安装Rust来绕过。

### 2.3 B步骤的额外要求

- 能看到目标进程的宿主PID和/proc，且采集进程处于宿主PID namespace。
- 内核支持所需BPF/tracepoint，具有可用BTF（通常是/sys/kernel/btf/vmlinux）。
- 已获得加载BPF和附加探针所需权限，资源限制允许创建BPF对象。
  root也可能受容器能力、seccomp、SELinux等策略限制，不能只看用户名判断。
- 使用专门测试进程，不选择正在承载业务的PID。本轮先在已批准的宿主机测试环境操作，
  避免混用容器内部PID和宿主PID。

缺少这些条件可以先完成A，但不能将B登记为通过。

## 3. 调用顺序

~~~text
abi_version → init → set_config → get_eventfd
                                  ↓
                           set_test_target
                                  ↓
                         start → 等待READY
                                  ↓
          poll/epoll等待通知 → poll_events → 复制数据 → free_event
                                  ↓
                   stop → poll_events直到返回0 → free
~~~

get_stats和get_last_error用于状态检查与诊断。get_eventfd返回的是可等待的FD，不是回调函数；
宿主不能直接read/write/close它。通知可能是状态变化，通知后poll_events返回0也是正常情况。

**start返回0只表示内部线程启动，必须看get_stats.target_state==3（READY）才开始测试操作。**
准备过程失败可能表现为state==4或target_state==4，而不是start立即返回错误。

## 4. 完整Go示例与编译

将以下完整代码保存为上面目录的main.go。无需从其他文档或源码仓库复制辅助代码。
无参数运行A；传-pid运行B。示例只打印数据，不发送到外部系统。

~~~go
package main

/*
#cgo CFLAGS: -I${SRCDIR}/include
#cgo LDFLAGS: -L${SRCDIR}/lib -lagentsight_test -Wl,-rpath,${SRCDIR}/lib
#include "agentsight_test.h"
#include <stdlib.h>
#include <poll.h>
#include <errno.h>
static int wait_events(int fd) {
    struct pollfd p = { .fd = fd, .events = POLLIN };
    int n = poll(&p, 1, 250);
    if (n < 0) return -errno;
    if (p.revents & (POLLERR | POLLHUP | POLLNVAL)) return -EIO;
    return n;
}
*/
import "C"

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"
	"unsafe"
)

func run() (result error) {
	pid := flag.Int("pid", 0, "host PID of dedicated target; 0 = basic checks only")
	seconds := flag.Int("seconds", 30, "capture seconds after READY, 1..3600")
	flag.Parse()
	if *pid < 0 || *pid > 2147483647 || *seconds < 1 || *seconds > 3600 {
		return fmt.Errorf("invalid pid or seconds")
	}
	version := C.agentsight_abi_version()
	if version != C.AGENTSIGHT_ABI_VERSION {
		return fmt.Errorf("ABI mismatch: library=%d header=%d", version, C.AGENTSIGHT_ABI_VERSION)
	}
	fmt.Printf("ABI=%d\n", version)
	var h *C.AgentsightHandle
	if rc := C.agentsight_init(&h); rc != 0 {
		return fmt.Errorf("init rc=%d", rc)
	}
	defer func() {
		rc := C.agentsight_free(h)
		fmt.Printf("FREE rc=%d\n", rc)
		if rc != 0 {
			result = fmt.Errorf("previous=%v; free rc=%d", result, rc)
		}
	}()
	fmt.Println("INIT rc=0")
	diagnostic := func() string {
		buf := C.malloc(2048)
		if buf == nil {
			return "diagnostic allocation failed"
		}
		defer C.free(buf)
		rc := C.agentsight_get_last_error(h, (*C.char)(buf), 2048)
		if rc != 0 && rc != -C.ENOSPC {
			return fmt.Sprintf("get_last_error rc=%d", rc)
		}
		return C.GoString((*C.char)(buf))
	}
	check := func(name string, rc C.int32_t) error {
		if rc < 0 {
			return fmt.Errorf("%s rc=%d: %s", name, rc, diagnostic())
		}
		return nil
	}
	var cfg C.AgentsightConfig // zero values select defaults
	cfg.struct_size = C.sizeof_AgentsightConfig
	cfg.abi_version = C.AGENTSIGHT_ABI_VERSION
	if err := check("set_config", C.agentsight_set_config(h, &cfg)); err != nil {
		return err
	}
	fmt.Println("CONFIG rc=0")
	fd := C.agentsight_get_eventfd(h)
	if err := check("get_eventfd", fd); err != nil {
		return err
	}
	fmt.Printf("EVENTFD=%d\n", fd)
	stats := func() (C.AgentsightStats, error) {
		var s C.AgentsightStats
		err := check("get_stats", C.agentsight_get_stats(h, &s, C.sizeof_AgentsightStats))
		return s, err
	}
	const batch = 32
	slots := C.malloc(C.size_t(batch) * C.size_t(unsafe.Sizeof(uintptr(0))))
	if slots == nil {
		return fmt.Errorf("event slots allocation failed")
	}
	defer C.free(slots)
	events := unsafe.Slice((**C.AgentsightEvent)(slots), batch)
	total, notifications := 0, 0
	drain := func(maxBatches int) error {
		for i := 0; i < maxBatches; i++ {
			n := C.agentsight_poll_events(h, (**C.AgentsightEvent)(slots), batch)
			if err := check("poll_events", n); err != nil {
				return err
			}
			if n == 0 {
				return nil
			}
			var batchError error
			for _, e := range events[:int(n)] {
				payload := C.GoBytes(unsafe.Pointer(e.payload_json), C.int(e.payload_json_len))
				schema, kind, eventPID := uint32(e.schema_version), uint32(e.event_type), uint32(e.pid)
				C.agentsight_free_event(e) // release ALL returned events, even on invalid JSON
				if schema != C.AGENTSIGHT_TEST_EVENT_SCHEMA_VERSION || !json.Valid(payload) {
					batchError = fmt.Errorf("unexpected schema or invalid JSON")
					continue
				}
				total++
				fmt.Printf("{\"schema_version\":%d,\"event_type\":%d,\"pid\":%d,\"payload\":%s}\n", schema, kind, eventPID, payload)
			}
			if batchError != nil {
				return batchError
			}
		}
		return nil
	}
	if *pid == 0 {
		s, err := stats()
		if err != nil {
			return err
		}
		fmt.Printf("STATS state=%d target=%d queued=%d\n", s.state, s.target_state, s.queued_events)
		fmt.Printf("LAST_ERROR=%q\n", diagnostic())
		if err := drain(1); err != nil {
			return err
		}
		C.agentsight_free_event(nil) // NULL behavior only; no real event was allocated
		if err := check("stop", C.agentsight_stop(h)); err != nil {
			return err
		}
		fmt.Println("BASIC PASS: empty poll, NULL event release, inactive stop; NO capture")
		return nil
	}
	if err := check("set_test_target", C.agentsight_set_test_target(h, C.int32_t(*pid))); err != nil {
		return err
	}
	if err := check("start", C.agentsight_start(h)); err != nil {
		return err
	}
	fmt.Println("START rc=0; waiting for READY")
	// Error paths stop before slots/handle are freed. Normal path checks final statistics.
	defer func() {
		if err := check("stop", C.agentsight_stop(h)); err != nil {
			result = fmt.Errorf("previous=%v; cleanup: %w", result, err)
		} else if err := drain(65536/batch + 1); err != nil {
			result = fmt.Errorf("previous=%v; drain: %w", result, err)
		}
	}()
	interrupt := make(chan os.Signal, 1)
	signal.Notify(interrupt, os.Interrupt, syscall.SIGTERM)
	defer signal.Stop(interrupt)
	pump := func() error {
		n := C.wait_events(C.int(fd))
		if n == -C.EINTR {
			return nil
		}
		if n < 0 {
			return fmt.Errorf("poll(eventfd) rc=%d", n)
		}
		if n > 0 {
			notifications++ // includes lifecycle notifications, not just data
		}
		return drain(64)
	}
	ready := false
	deadline := time.Now().Add(15 * time.Second)
loop:
	for time.Now().Before(deadline) {
		select {
		case <-interrupt:
			break loop
		default:
		}
		s, err := stats()
		if err != nil {
			return err
		}
		if s.state == C.AGENTSIGHT_STATE_FAILED || s.state == C.AGENTSIGHT_STATE_STOPPED || s.target_state == C.AGENTSIGHT_TARGET_DEGRADED {
			return fmt.Errorf("capture unavailable: state=%d target=%d error=%d: %s", s.state, s.target_state, s.target_error, diagnostic())
		}
		if !ready && s.target_state == C.AGENTSIGHT_TARGET_READY {
			ready = true
			deadline = time.Now().Add(time.Duration(*seconds) * time.Second)
			fmt.Printf("STATE target=3 pid=%d error=0; READY, perform test operations now\n", s.target_pid)
		}
		if err := pump(); err != nil {
			return err
		}
	}
	if err := check("stop", C.agentsight_stop(h)); err != nil {
		return err
	}
	if err := drain(65536/batch + 1); err != nil {
		return err
	}
	s, err := stats()
	if err != nil {
		return err
	}
	fmt.Printf("STOPPED delivered=%d queued=%d dropped=%d ring_dropped=%d notifications=%d\n", s.delivered_events, s.queued_events, s.queue_dropped_events, s.kernel_ring_dropped, notifications)
	if !ready || total == 0 || notifications == 0 || s.queued_events != 0 || s.queue_dropped_events != 0 || s.kernel_ring_dropped != 0 || s.backend_errors != 0 {
		return fmt.Errorf("capture check failed: ready=%t events=%d notifications=%d backend_errors=%d", ready, total, notifications, s.backend_errors)
	}
	fmt.Println("CAPTURE PASS: verify printed PID/path matches your test operations")
	return nil
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
~~~

在main.go所在目录执行：

~~~sh
CGO_ENABLED=1 GO111MODULE=off go build -a -o agentsight-smoke main.go
./agentsight-smoke
echo $?
~~~

这里显式编译main.go，不依赖go.mod或额外Go包。-a用于避免更换头文件或.so后误用旧构建缓存。
用于独立验证；迁入ICAgent时沿用其现有Go模块和构建方式，不要求ICAgent关闭模块模式。

示例的rpath指向**编译时**工作目录中的lib。把可执行文件移到另一台机器或移动lib目录后，
需要重新编译，或在有权限的启动环境中明确设置运行时库搜索路径。例如从当前目录启动：

~~~sh
LD_LIBRARY_PATH="$PWD/lib" ./agentsight-smoke
~~~

这不替代libelf等系统运行依赖。某些提权/安全执行模式会忽略LD_LIBRARY_PATH，
此时应由部署方配置受信任的动态库目录，不要把“找不到.so”当成接口返回失败。

## 5. 按步骤验收

### 5.1 A：不启动采集的基础检查

无参数运行，输出大致如下，EVENTFD数值可能不同：

~~~text
ABI=2
INIT rc=0
CONFIG rc=0
EVENTFD=3
STATS state=1 target=0 queued=0
LAST_ERROR=""
BASIC PASS: empty poll, NULL event release, inactive stop; NO capture
FREE rc=0
~~~

同时检查退出码为0。此模式已经调用配置、统计和空队列读取，不仅仅是打印ABI。
但没有调用set_test_target/start，没有加载探针，也没有获得真实事件。
free_event(NULL)只检查允许空指针的调用方式，不能替代B中的真实事件内存释放验证。
未启动时stop成功不会强制把state从CONFIGURED改成STOPPED。

### 5.2 B：两个终端验证启动、通知和取数

终端A：启动一个独立的测试shell，并保持它存活：

~~~sh
bash --noprofile --norc
echo $$
~~~

记下这个shell的宿主PID。不要从这个shell启动采集程序：
库禁止把调用方自身或祖先进程选为目标，以免把采集器也纳入后代范围形成自采集。
容器里的$$可能不是宿主PID；不能直接照抄不匹配的PID。

终端B：在已具备采集权限的环境中，从示例目录启动：

~~~sh
./agentsight-smoke -pid <终端A的宿主PID> -seconds 60
~~~

上面的尖括号内容是占位符，执行时替换成数字，不保留尖括号。
程序等待READY最多15秒；READY后计时60秒，这些是示例行为，不是库API的超时配置。
看到下面输出后，再回到终端A操作：

~~~text
START rc=0; waiting for READY
STATE target=3 pid=实际PID error=0; READY, perform test operations now
~~~

终端A执行：

~~~sh
test_dir=$(mktemp -d /tmp/agentsight-interface.XXXXXX)
printf 'hello\n' > "$test_dir/before.txt"
cat "$test_dir/before.txt"
mv "$test_dir/before.txt" "$test_dir/after.txt"
/bin/true
printf '测试目录：%s\n' "$test_dir"
~~~

终端B应打印schema_version=1001的JSON。检查其中的PID/文件路径确实属于本次操作；
exec和部分文件操作的PID可能是这个shell新建的子进程，不一定等于shell本身。
write/rename是聚合记录，可能稍后或stop时才排出。不要只看终端出现任意一条JSON就结束验证。

等待到期，或在终端B按Ctrl+C正常收尾；不要用kill -9检验正常释放。
示例会stop、排空、读取最终统计，再free。应看到：

~~~text
STOPPED delivered=实际条数 queued=0 dropped=0 ring_dropped=0 notifications=实际次数
CAPTURE PASS: verify printed PID/path matches your test operations
FREE rc=0
~~~

再次检查退出码0。示例要求至少收到一条真实事件、有通知、队列已排空，且列出的丢弃与后台错误
计数无异常。notifications次数包括状态通知，不能当作事件条数；其他未覆盖的内核损失不由零计数证明。
建议按上述命令确认四类事件均出现。具体事件条数不固定，动态库加载、管道等也可能产生记录。

测试shell应保持运行直到采集结束。测试文件只在mktemp创建的目录内，按需自行清理；
退出测试shell不会自动删除文件。若提前退出目标，库会停止采集并标记目标失效，
示例会报错而不是打印通过。

### 5.3 本轮交给ICAgent同事的验收清单

- 用实际测试.so和配套头文件编译成功，无架构或动态链接错误。
- A完整通过；B在满足权限的环境中READY，并读到手动操作对应的数据。
- 收到的每个事件复制后释放一次；stop后继续读到0，再释放句柄。
- 记录返回码、目标状态、错误文本和队列/丢弃统计。失败时不把“程序没有退出”当作成功。
- 迁入ICAgent框架后，以同样顺序验证接口；本轮只需日志打印，不要求实现上报或span增强。

## 6. 全部12个接口

声明来自配套头文件。调用方始终包含agentsight_test.h，不自行重复声明。
h表示init创建的有效句柄；输出参数必须指向可写内存。普通操作成功返回0，
失败返回**负errno**；版本、FD、事件条数和void释放接口分别遵循下面说明。

~~~c
uint32_t agentsight_abi_version(void);
int32_t agentsight_init(AgentsightHandle **out);
int32_t agentsight_set_config(AgentsightHandle *h, const AgentsightConfig *config);
int32_t agentsight_set_test_target(AgentsightHandle *h, int32_t target_pid);
int32_t agentsight_start(AgentsightHandle *h);
int32_t agentsight_get_eventfd(AgentsightHandle *h);
int32_t agentsight_poll_events(AgentsightHandle *h, AgentsightEvent **out_events, uint32_t capacity);
void agentsight_free_event(AgentsightEvent *event);
int32_t agentsight_get_stats(AgentsightHandle *h, AgentsightStats *stats, size_t stats_size);
int32_t agentsight_get_last_error(AgentsightHandle *h, char *buffer, size_t capacity);
int32_t agentsight_stop(AgentsightHandle *h);
int32_t agentsight_free(AgentsightHandle *h);
~~~

### 6.1 创建、配置和选择目标

| 函数 | 输入与输出 | 返回及约束 |
|---|---|---|
| agentsight_abi_version | 无参数；返回.so的C ABI版本 | 本包为2，与AGENTSIGHT_ABI_VERSION比较；不是事件schema |
| agentsight_init | out为句柄指针的地址，如C中的&h；成功填入新句柄，失败时有效out指向的值置NULL | 0成功；负值失败，不使用失败句柄；仅分配资源，不采集 |
| agentsight_set_config | h；config指向零初始化后填写的配置结构；库复制结构内容及字符串 | 0成功；只允许工作线程不存在且队列为空；成功后清除之前的测试目标，必须重新set_test_target |
| agentsight_set_test_target | h；target_pid为当前存在的宿主进程PID，必须>1，且不是调用方或其祖先 | 0成功；保存PID和启动标识，不是只保存一个可复用数字；先set_config，后start；有工作线程/残留队列时返回-EBUSY |

配置失败不会应用新配置。选目标失败不会自动选择其他进程。目标退出后不会跟随复用该PID的新进程。
不包含采集开始前已存在的子进程；探针启用后新建的后代才在范围内。
要改目标或重新配置，先stop并排空，之后按set_config（如需）→set_test_target→start重新调用。

### 6.2 启动、通知、读取和释放事件

| 函数 | 输入与输出 | 返回及约束 |
|---|---|---|
| agentsight_start | h，必须已有配置和有效测试目标 | 0表示工作线程启动，之后检查READY；正在RUNNING时重复调用返回0；旧线程未收尾/队列未排空时可能-EBUSY |
| agentsight_get_eventfd | h；返回借用的非阻塞通知FD | >=0为FD，<0为错误；有效到free；用poll/epoll等待，不得read/write/close |
| agentsight_poll_events | h；out_events为可写的**事件指针槽数组**；capacity为槽数，不是字节数，范围1～65536 | 非阻塞，返回0为空、正数为本次条数、负值为错误；只读取返回条数对应的槽，其余槽不保证清零 |
| agentsight_free_event | event为poll_events返回的一个事件指针 | 无返回值；每个事件恰好调用一次；NULL允许；不得用C.free/Go释放或释放数组中未使用的槽 |

Go中指针槽数组要用C.malloc分配；数组本身用C.free释放，但数组里每个事件用agentsight_free_event。
在释放事件前将payload等内容复制到Go内存。释放后不能继续访问事件或payload。
通知发生后，应读取到0；如果为避免长时间占用而采用有界批处理，要继续消费，不能漏掉剩余队列。
示例正常循环每轮最多64批，停止后按最大队列容量完整排空。

### 6.3 统计、诊断、停止和释放句柄

| 函数 | 输入与输出 | 返回及约束 |
|---|---|---|
| agentsight_get_stats | h；stats指向可写结构；stats_size至少sizeof(AgentsightStats)，单位字节 | 0成功并写入快照；负值失败；大小由配套头文件/C.sizeof取得，不硬编码 |
| agentsight_get_last_error | h；buffer为可写字符缓冲区；capacity为总字节数，必须>0且包含结尾NUL空间 | 0表示完整复制；-ENOSPC表示截断且仍以NUL结尾；其他负值为调用失败；无诊断时为空串 |
| agentsight_stop | h | 停止并等待线程收尾、卸载探针、排出内核聚合；0成功；尚未启动或已停止时可重复调用；用户态队列保留，需要继续poll |
| agentsight_free | h | 内部会stop，随后丢弃未取走队列并释放句柄；0成功；NULL返回0；负值时不得假装释放成功；成功后不再使用h |

last_error是共享的最近诊断，可能被后台线程或其他调用覆盖；成功调用不保证清空旧文本。
判断本次调用是否成功以**该调用的返回码**为准，不要用last_error非空替代返回码。
init失败没有可用句柄，此时直接记录其返回码。

正常路径不能用free替代“stop→排空”：free会丢弃未取走的数据。
已经取出的事件可晚于handle释放，但必须在卸载.so前释放；本示例优先释放事件，再释放handle。

## 7. 配置、事件和状态如何理解

### 7.1 AgentsightConfig：输入配置

必须先将整个结构清零，再填struct_size和abi_version。其余字段使用0/NULL即可开始本轮测试。

| 字段 | 单位/取值 | 说明 |
|---|---|---|
| struct_size | 字节 | 必须等于sizeof(AgentsightConfig)，Go用C.sizeof_AgentsightConfig |
| abi_version | 版本号 | AGENTSIGHT_ABI_VERSION，本包为2 |
| queue_capacity | 事件条数 | 0取默认1024；非零1～65536；满时丢新事件 |
| discovery_interval_ms | 毫秒 | 0取默认1000；非零100～60000。本测试库不做自动目标发现，保留字段仍校验范围；本轮保持0 |
| max_queue_bytes | 字节 | 0取默认16MiB；非零1024～1GiB；仅限制排队数据及事件元数据，不是整个进程内存上限 |
| procfs_root | C字符串 | NULL/空串取/proc；非空须为绝对路径且字节数<4096；库在set_config内复制字符串 |

本轮使用默认/proc。自定义procfs_root不能代替宿主PID namespace要求。
若自行传字符串，使用C.CString，set_config返回后即可C.free；不能让后台保存Go字符串指针。
本测试库内部约100ms轮询并检查目标身份，不由discovery_interval_ms决定。

### 7.2 AgentsightEvent：输出事件

| 字段 | 含义/本轮处理方式 |
|---|---|
| struct_size | 当前事件结构字节数，由库填写 |
| schema_version | 固定1001，与ABI版本2是不同概念 |
| event_type | 1=open，2=write，3=rename，4=exec |
| pid、tid | 事件进程/线程标识；字段缺失时可能为0，不能要求所有tid均非零 |
| timestamp_ns | 内核单调时钟纳秒，不是Unix时间 |
| wall_time_ns | 根据双时钟锚点估算的Unix纳秒 |
| duration_ns | 本版未提供实测耗时，0表示未知，不是“操作耗时为0” |
| trace_id[16]、parent_span_id[8]、trace_flags | 本测试库均为0，本轮不做链路关联，也不能按trace_flags=0把测试事件全部过滤 |
| payload_json、payload_json_len | JSON地址与字节数；长度不含结尾NUL；先复制再free_event |
| reserved | 保留字段，本版为0，调用方忽略 |

JSON带agentsight_test_only=true和agentsight_correlation_source="none"。
本轮按下列四类理解即可：

| event_type / JSON event | 常见payload字段 | 不能据此推断的内容 |
|---|---|---|
| 1 / FILE_OPEN | filepath、flags、pid、comm、count | 是打开尝试，不保证成功，也不代表读取了内容 |
| 2 / FILE_WRITE | count、total_bytes、fd、detail、path_resolved，可能有filepath | 成功写入的FD聚合，不是逐次syscall；FD也可能指向终端/管道 |
| 3 / FILE_RENAME | count、detail（目录）、last_destination_path | 重命名尝试的目录聚合，只保留最近目标，不提供完整源路径或结果 |
| 4 / EXEC | filename、full_command、pid、ppid | 成功程序映像切换，不代表该程序最终退出成功 |

write/rename带aggregation=true。write的path_resolved=true表示排出时按/proc/PID/fd查询到路径，
不是写入时路径快照；FD复用/关闭或进程退出会导致路径缺失或不对应早先操作。
路径和命令长度有限，可能包含敏感数据，只在受控测试环境打印。不要把本测试库当作完整审计工具。

### 7.3 AgentsightStats：输出状态与统计

两个状态字段含义不同：

| 字段 | 值 |
|---|---|
| state：句柄/线程状态 | 0=INITIALIZED，1=CONFIGURED，2=RUNNING，3=STOPPED，4=FAILED |
| target_state：目标探针状态 | 0=IDLE，1=WAITING（本测试流程不使用），2=ATTACHING，3=READY，4=DEGRADED |

至少检查state、target_state、target_error、queued_events、queue_dropped_events和kernel_ring_dropped。
目标退出时可能是state=STOPPED而target_state=DEGRADED，不能只看state判断正常完成。

其他字段的完整含义：

| 字段 | 说明 |
|---|---|
| struct_size、abi_version | 统计结构字节数与ABI版本，由库写入 |
| supported_counters | 位掩码：1=用户队列统计，2=部分聚合/跟踪溢出，4=已接入的ring失败统计；不是“绝对无丢失”保证 |
| received_records、filtered_records、invalid_records | 底层输入记录数、被过滤记录数、无法有效解析/转换的记录数；输入可能包含控制记录，不与最终事件一一对应 |
| enqueued_events、delivered_events | 累计入队数、被poll取走数；delivered不代表上报成功 |
| queue_dropped_events | 用户队列容量/字节限额导致的丢弃 |
| output_dropped_records | 底层格式化输出阶段报告的丢弃 |
| backend_errors、backend_diagnostics | 后台采集错误数、诊断消息数；诊断不一定是致命错误 |
| aggregate_overflow | 部分聚合/跟踪失败的累计统计，不覆盖全部丢失原因 |
| queued_events、queued_bytes | 当前积压条数、字节数 |
| target_pid、target_start_ticks | 目标PID及/proc/PID/stat的启动时钟tick标识；ticks不是纳秒 |
| target_generation | 成功挂载次数形成的代次，stop/start后可增长 |
| discovery_scans、discovery_errors | 本测试库不自动扫描发现目标，通常为0 |
| attach_attempts、attach_failures、target_detaches | 挂载尝试、失败及成功挂载后的卸载累计次数 |
| target_error | 当前目标错误的负errno，0表示该字段当前没有错误 |
| adaptation_method | 本测试库不做Python适配，READY时仍为0是正常的 |
| kernel_ring_dropped | 当前已接入的open/exec/exit等ring申请失败计数，不代表覆盖所有内核证据缺失 |

计数通常按handle累计，stop/start不清零；queued等表示当前值。重新init才建立新的统计周期。

## 8. 必须遵守的调用约束

- 生命周期串行管理，一个handle只用一个消费者读取；free不得与任何同handle调用并发。
- 同一进程内该库只允许一个活跃采集实例，第二个实例start可能返回-EBUSY。
- 事件视为只读；不复制/伪造句柄、不重复释放、不把C指针留给已结束的异步任务。
- 配置后再选目标；修改配置会清除原选择。停止后先排空，再重启或更换目标。
- 库不替宿主设置eBPF权限，不提高RLIMIT_MEMLOCK，不安装信号处理，也不自动提权。
  本文Go示例自行处理Ctrl+C/SIGTERM，仅用于测试程序正常收尾。
- 不把句柄跨fork使用；继承的句柄调用会被拒绝，子进程需exec后再初始化。
- .so与宿主同进程：原生崩溃、abort或进程级OOM仍可能影响宿主。返回错误不等于进程隔离。
- 示例直接链接.so，缺库可能在Go main之前就失败；本轮不实现生产可选加载、重试队列或热更新。

## 9. 常见失败与排查

| 现象 | 优先检查 |
|---|---|
| cannot find -lagentsight_test | lib目录与文件名；必须在本文目录布局中编译 |
| agentsight_test.h / agentsight.h not found | 两个头文件是否同时放入include |
| wrong ELF class / incompatible architecture | 当前库为Linux ARM64；Go/C目标也必须匹配 |
| error while loading shared libraries / GLIBC_x.y not found | .so查找路径、libelf/zlib/libgcc及glibc，不是Go语法或Rust环境问题 |
| undefined reference to agentsight_set_test_target | 是否误链接了其他库或混用了头文件 |
| ABI mismatch / stats buffer too small | 配套文件是否一致；使用C.sizeof，重新go build -a |
| -EINVAL（常见-22） | 结构大小/版本、配置范围、PID0/1/自身/祖先、PID视角或参数是否正确 |
| -ESRCH（常见-3） | 目标是否存在、仍存活，以及是否能读取其身份 |
| -EBUSY（常见-16） | 是否有旧线程/另一个活跃实例，或stop后队列未排空 |
| -ECHILD（常见-10） | 是否在fork后使用继承句柄 |
| get_last_error返回-ENOSPC（常见-28） | 诊断文本被截断；扩大缓冲区，不是磁盘空间不足 |
| -EFAULT（常见-14） | 捕获到的内部异常；记录日志，不意味着能隔离段错误 |
| start=0但无法READY / state=FAILED | 用get_stats和get_last_error看挂载错误，再检查权限、BTF、资源限制和目标生命周期 |
| READY但没有预期文件事件 | 是否在READY之后、选定进程或其新后代中执行操作；是否误选PID；聚合可在stop时出现 |
| BASIC PASS但B失败 | A没有加载探针；分别登记结果，继续排查采集环境 |
| CAPTURE检查无事件失败 | 在READY后手动触发操作；不要仅让目标闲置，或在测试结束前关闭目标shell |
| 队列丢弃不为0 | 是否消费太慢、同步打印太多；先减少测试负载，检查消费与容量配置 |

负errno数字以目标系统定义为准，可用C.EINVAL等常量判断；不要要求所有错误来自同一个errno。
程序失败时先保留原始返回码、last_error和状态，不能忽略错误继续读未初始化的结构。

## 10. 本文示例的验证范围

2026-11-09在Linux ARM64、Go 1.21.4和内核6.6.0-28.0.0.34.oe2403.aarch64上，
使用本包现有.so验证了本文示例的编译、基础接口、真实采集、通知、事件释放、停止排空和句柄释放。
真实Go消费测试收到17条记录，四类事件均出现，最终queued/dropped/ring_dropped均为0。
这些条数是当次观察值，不是每次运行的协议保证；其他系统环境仍须按本文复验。

该结果不代表已接入ICAgent私有框架，也不包含后端上报验收。本轮仅更新说明和验证示例，
没有重编译或修改.so及两个头文件。
