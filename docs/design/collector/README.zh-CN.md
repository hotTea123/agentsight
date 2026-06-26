# AgentSight Collector

[English](README.md) | **中文**

高性能 Rust 流式框架，通过基于 eBPF 的系统监控实现实时 AI 智能体可观测性。Collector 提供可插拔架构，以极低的开销处理 SSL/TLS 流量和进程生命周期事件。

## 概述

AgentSight Collector 是核心数据处理引擎：

- 执行嵌入的 eBPF 程序进行系统级监控
- 通过可配置的 Analyzer 链处理事件流
- 提供适用于不同监控场景的多种 Runner
- 基于 async/await 架构提供实时流式处理
- 支持灵活的输出格式和目标

## 架构

```text
eBPF 程序 → JSON 事件 → Runner → Analyzer 链 → 输出
```

### 核心组件

- **Runner**：执行 eBPF 二进制程序并创建事件流
- **Analyzer**：在可配置的链中处理和转换事件
- **Event**：标准化的事件格式，包含丰富的元数据
- **Binary Extractor**：管理嵌入的 eBPF 程序，自动清理

## 快速开始

### 安装

```bash
# 安装依赖
sudo apt-get update
sudo apt-get install -y clang llvm libelf-dev

# 克隆并构建
git clone https://github.com/eunomia-bpf/agentsight.git --recursive
cd agentsight/collector
cargo build --release
```

### 基本用法

```bash
# 实时智能体 session 视图
sudo cargo run -- top

# 启动并记录一个智能体命令
sudo cargo run -- record -- python my_agent.py

# SSL 流量监控，启用 HTTP 解析
sudo cargo run -- debug ssl --http-parser

# 进程生命周期监控
sudo cargo run -- debug process

# 本地 MCP 服务器或 CLI 子进程的 Stdio 载荷监控
sudo cargo run -- debug stdio --pid 1234

# 组合调试监控并启用 Web 界面
sudo cargo run -- debug trace --server -c python
```

## 命令

### Debug SSL 监控

监控 SSL/TLS 流量，支持高级处理能力：

```bash
# 基本 SSL 监控
sudo cargo run -- debug ssl

# 启用 Server-Sent Events 处理
sudo cargo run -- debug ssl --sse-merge

# 启用 HTTP 解析并保留原始数据
sudo cargo run -- debug ssl --http-parser --http-raw-data

# 应用过滤器减少噪音
sudo cargo run -- debug ssl --http-parser --http-filter "GET /health" --ssl-filter "handshake"

# 传递参数给底层 eBPF 程序
sudo cargo run -- debug ssl -- --port 443 --comm python
```

### Debug 进程监控

追踪进程生命周期事件：

```bash
# 基本进程监控
sudo cargo run -- debug process

# 按进程名过滤
sudo cargo run -- debug process -- --comm python

# 按 PID 过滤
sudo cargo run -- debug process -- --pid 1234

# 静默模式（无控制台输出）
sudo cargo run -- debug process --quiet
```

### Debug Stdio 监控

从目标进程捕获明文 stdin/stdout/stderr 载荷：

```bash
# 捕获一个 PID 的 stdio 载荷
sudo cargo run -- debug stdio --pid 1234

# 按 UID 或命令名过滤
sudo cargo run -- debug stdio --pid 1234 --uid 1000 --comm python3

# 捕获所有文件描述符而非仅 0/1/2
sudo cargo run -- debug stdio --pid 1234 --all-fds
```

### Debug Trace 监控

SSL 和进程事件的综合监控：

```bash
# 完整调试监控
sudo cargo run -- debug trace

# 按进程命令过滤
sudo cargo run -- debug trace -c python

# 仅 SSL 监控
sudo cargo run -- debug trace --process false

# 仅进程监控
sudo cargo run -- debug trace --ssl false

# 高级过滤
sudo cargo run -- debug trace --pid 1234 --ssl-uid 1000 --http-filter "POST /api"

# 启用 Web 界面
sudo cargo run -- debug trace --server

# 通过 trace 入口进行仅 Stdio 监控
sudo cargo run -- debug trace --ssl false --process false --stdio --pid 1234
```

## 配置选项

### SSL 选项

- `--sse-merge`：启用 Server-Sent Events 处理
- `--http-parser`：从 SSL 流量解析 HTTP 请求/响应
- `--http-raw-data`：在 HTTP 事件中包含原始 SSL 数据
- `--http-filter`：按模式过滤 HTTP 事件
- `--ssl-filter`：按模式过滤 SSL 事件

### 进程选项

- `--comm`：按进程命令名过滤
- `--pid`：按进程 ID 过滤
- `--duration`：最小进程持续时间（毫秒）
- `--mode`：进程过滤模式（0=全部, 1=进程, 2=过滤）

### Debug Trace 选项

- `--ssl`：启用/禁用 SSL 监控
- `--process`：启用/禁用进程监控
- `--stdio`：启用/禁用 stdio 载荷监控
- `--system`：启用系统资源监控
- `--server`：启动本地 Web UI
- `--stdio-uid`：按用户 ID 过滤 stdio 事件
- `--stdio-all-fds`：捕获所有文件描述符而非仅 stdin/stdout/stderr
- `--stdio-max-bytes`：限制每个 stdio 事件的捕获字节数
- `--ssl-uid`：按用户 ID 过滤 SSL 事件
- `--ssl-handshake`：显示 SSL 握手事件
- `--quiet`：禁止控制台输出

## 框架架构

### Runner

Runner 执行 eBPF 程序并创建事件流：

```rust
// SSL Runner
let ssl_runner = SslRunner::from_binary_extractor(binary_path)
    .with_args(&["--port", "443"])
    .add_analyzer(Box::new(HTTPParser::new()));

// Process Runner
let process_runner = ProcessRunner::from_binary_extractor(binary_path)
    .with_args(&["--comm", "python"]);

// Agent Runner（组合 SSL + Process）
let agent_runner = AgentRunner::new("agent")
    .add_runner(Box::new(ssl_runner))
    .add_runner(Box::new(process_runner));
```

### Analyzer

Analyzer 在可配置的链中处理事件流：

- **SSEProcessor**：合并 HTTP 块并处理 Server-Sent Events
- **HTTPParser**：从 SSL 流量解析 HTTP 请求/响应
- **HTTPFilter**：按模式过滤 HTTP 事件
- **SSLFilter**：按模式过滤 SSL 事件

控制台输出由 CLI 在 runner/analyzer pipeline 产出事件后统一渲染。

### 事件格式

事件使用标准化格式：

```rust
pub struct Event {
    pub timestamp: u64,
    pub source: String,
    pub pid: u32,
    pub comm: String,
    pub data: serde_json::Value,
}
```

## 性能

- **低开销**：eBPF 监控，性能影响 <3%
- **异步处理**：基于 Tokio 的 async/await 架构
- **流式处理**：实时事件处理，内存使用最小
- **可配置**：模块化 Analyzer 链，性能最优

## 示例

### SSL 流量分析

```bash
# 监控 HTTPS 流量并启用 HTTP 解析
sudo cargo run -- debug ssl --http-parser --http-filter "POST /api" -- --port 443

# 监控 Python 进程的 SSL 流量
sudo cargo run -- debug ssl --sse-merge -- --comm python
```

### 进程生命周期追踪

```bash
# 监控 Python 进程
sudo cargo run -- debug process -- --comm python --duration 1000

# 监控特定 PID
sudo cargo run -- debug process -- --pid 1234
```

### 组合监控

```bash
# 监控 Web 应用
sudo cargo run -- debug trace -c nginx --ssl-uid 33 --http-filter "GET /metrics"

# 全系统监控 + Web 界面
sudo cargo run -- debug trace --system --server

```

## 开发

### 构建

```bash
# 开发版本
cargo build

# 优化的发布版本
cargo build --release

# 运行测试
cargo test
```

### 添加 Analyzer

```rust
use async_trait::async_trait;
use futures::stream::Stream;

#[async_trait]
impl Analyzer for MyAnalyzer {
    async fn process(&mut self, mut stream: EventStream) -> Result<EventStream, AnalyzerError> {
        // 处理事件并返回转换后的流
    }
}
```

### 二进制嵌入

Collector 在编译时自动嵌入 eBPF 二进制文件：

```rust
let binary_extractor = BinaryExtractor::new().await?;
let ssl_path = binary_extractor.get_sslsniff_path();
let process_path = binary_extractor.get_process_path();
```

## 安全

- **Root 权限**：eBPF 程序需要 root 访问权限进行内核监控
- **独立监控**：系统级观测独立于应用代码运行
- **数据敏感性**：SSL 流量可能包含敏感信息
- **安全清理**：临时文件和进程的自动清理

## 故障排除

### 常见问题

1. **权限被拒绝**：确保使用 `sudo` 或具有相应能力运行
2. **不支持 eBPF**：需要启用 eBPF 的 Linux 内核 4.1+
3. **二进制提取失败**：检查 `/tmp` 权限和磁盘空间
4. **CPU 使用率高**：使用过滤器减少事件量

### 调试模式

```bash
# 启用调试日志
sudo env RUST_LOG=debug cargo run -- debug ssl --http-parser

# 详细 eBPF 程序输出
sudo cargo run -- debug ssl -- --verbose
```

## 系统要求

- **Rust**：1.88.0 或更高版本（edition 2024）
- **Linux**：内核 4.1+，支持 eBPF
- **库**：clang、llvm、libelf-dev
- **权限**：eBPF 操作需要 root 访问

## 依赖

- **tokio**：异步运行时和流处理
- **serde**：JSON 序列化和反序列化
- **clap**：命令行参数解析
- **chrono**：时间戳处理
- **futures**：Stream 工具和异步处理

## 参与贡献

1. Fork 仓库
2. 创建功能分支
3. 为新功能添加测试
4. 确保所有测试通过：`cargo test`
5. 提交 Pull Request

## 许可证

MIT 许可证

## 服务器模式

Collector 包含嵌入式 Web 服务器和前端用于可视化：

```bash
# 启动带嵌入前端的 Web 服务器
sudo cargo run -- debug trace --server

# 访问 Web 界面
# http://127.0.0.1:7395/timeline
```

### Web 界面功能

- **时间线视图**：交互式事件时间线，支持缩放和过滤
- **进程树**：层级进程可视化
- **日志视图**：原始事件检查，支持 JSON 格式化
- **实时更新**：实时数据流和分析

## 相关项目

- **AgentSight**：完整的可观测性框架
- **前端**：React/TypeScript Web 界面（`../frontend/`）
- **分析工具**：Python 数据处理工具（`../script/`）
- **文档**：综合指南和示例（`../docs/`）

## 包信息

- **包名**：`agentsight`
- **仓库**：https://github.com/eunomia-bpf/agentsight
- **二进制名**：`agentsight`（`cargo build --release` 后）
