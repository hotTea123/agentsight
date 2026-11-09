# libagentsight_test.so 验证报告

日期：2026-11-06。分支：`test/icagent-capture-smoke`，基于`span_enhance@b682b776`。
本报告针对无marker的独立测试库，不是正式Jiuwen span增强或ICAgent上报验收。

## 构建与交付标识

- 构建环境：已配置C/BPF/Rust工具链的Linux ARM64隔离环境；构建前已核对源码一致性。
- Rust 1.88.0，Linux ARM64；Go消费程序在宿主Go1.21.4构建，在同一宿主内核的构建容器实际运行。
- `test-capture`特性使用独立BPF、PIC和Cargo输出目录；发布文件名libagentsight_test.so。
- 库ABI2，公开基础结构大小32/88/192，测试事件schema1001，新增一个测试扩展接口。
- 测试库SHA256：`09b4998c2db4ff9a894a2a3c4e7e623346c87a4d05a5b4f71a6c15573a0c666f`。
- 动态依赖：libelf.so.1、libz.so.1、libgcc_s.so.1、libc.so.6；最高GLIBC符号为2.34。
- 不依赖marker/Python适配；验收用Python脚本创建普通测试进程，不调用OTel或marker。

## 已通过

| 验证 | 结果 |
|---|---|
| 测试特性Rust单元测试 | 7/7：5项复用discovery测试、2项测试事件/队列/接口测试 |
| 默认正式构建Rust单元测试 | 12/12，包括拒绝无效/未采样/无关联事件 |
| 默认构建C ABI | 动态加载、符号、错误清理、fork防护、信号与RLIMIT检查通过 |
| 默认构建C++头文件/布局 | 通过 |
| 默认构建底层C单元测试 | 通过：3+63+73+53+29项及SSL JSON检查 |
| Go示例强制重编译、ABI检查、go vet | 通过；schema1001，布局32/88/192 |
| 默认库导出符号 | 原11个公开接口，不含agentsight_set_test_target |
| 测试库导出符号 | 原11个接口+agentsight_set_test_target，共12个 |
| 真实eBPF两轮采集/stop/start | 每轮33条事件，均覆盖open/write/rename/exec |
| 无关进程隔离 | 同时运行另一个普通进程，其测试文件/行为不进入结果 |
| 短子进程 | 程序exec和快速文件写入实际被捕获 |
| 通知、读取、释放 | 真实事件触发通知，批量读取，独立事件复制与释放 |
| 停止排空 | stop后读到剩余rename/write聚合，最终队列为0 |
| 队列满 | 容量1真实采集产生可观测丢弃；未排空时重启/改目标被拒绝 |
| 目标退出 | 停止采集并提示目标失效；旧PID重新选择被拒绝 |
| 目标配置保护 | 拒绝PID0、自己、采集器祖先、无效PID；未选择目标不能启动 |
| Go实际消费 | 17条事件，四类齐全，停止后queued=0、dropped=0、ring_dropped=0 |

普通两轮采集的队列丢弃、内核ring失败、invalid和backend错误均为0；容量1场景的
用户态队列丢弃是刻意触发的预期结果，不能将全部测试描述为“零丢弃”。
计数受底层实际行为影响，是本次观察值，不是对每次执行条数的固定协议保证。

## 证据与复现

- 验收脚本：`agentsight-ffi/tests/test_standalone_capture.py`。
- 构建容器原始结果：`/tmp/agentsight-test-capture-final.json`。
- 交付包副本：`reports/verification.json`，含两轮事件、溢出结果和Go输出。
- 命令与手动测试步骤见TEST_CAPTURE.md（交付包README.md）。
- 测试创建的临时业务文件已由脚本清理，专用测试进程已结束；未操作真实Jiuwen业务文件。
- 测试库交付时新增独立测试目录，原正式交付包的两个.so及基础头文件不变。

## 未验证与限制

1. 没有真实ICAgent私有插件接入、后端上报或Jaeger查询；已验证到Go消费和JSON格式化。
2. 没有重跑正式Jiuwen工具增强的全链路；默认构建通过单元/C ABI回归不等于新正式E2E。
3. 没有覆盖x86_64、其他内核/发行版、性能极限和长时间稳定性。
4. 没有通过强制系统PID复用实验覆盖所有竞态；实现包含根PID启动标识校验、退出关闭和子PID内核清理。
5. 普通rename是目录聚合、只保留最近目标；write路径可能是排出时FD查询结果，不能当作完整文件审计。
6. 测试事件没有真实trace/span身份。若需要trace后端展示，接入方必须创建明确的独立测试trace。
7. 本库与宿主同进程，原生崩溃/OOM仍可能影响宿主；测试能力不得误用于生产采集。
