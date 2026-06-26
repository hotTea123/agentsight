# AgentSight Collector

**English** | [中文](README.zh-CN.md)

A high-performance Rust streaming framework for real-time AI agent observability through eBPF-based system monitoring. The collector provides a pluggable architecture for processing SSL/TLS traffic and process lifecycle events with minimal overhead.

## Overview

The AgentSight Collector is the core data processing engine that:

- Executes embedded eBPF programs for system-level monitoring
- Processes event streams through configurable analyzer chains
- Provides multiple runners for different monitoring scenarios
- Offers real-time streaming with async/await architecture
- Supports flexible output formats and destinations

## Architecture

```text
eBPF Programs → JSON Events → Runners → Analyzer Chains → Output
```

### Core Components

- **Runners**: Execute eBPF binaries and create event streams
- **Analyzers**: Process and transform events in configurable chains
- **Events**: Standardized event format with rich metadata
- **Binary Extractor**: Manages embedded eBPF programs with automatic cleanup

## Quick Start

### Installation

```bash
# Install the packaged CLI
cargo install agentsight

# Clone and build
git clone https://github.com/eunomia-bpf/agentsight.git --recursive
cd agentsight/collector
cargo build --release
```

### Basic Usage

```bash
# Live agent session view
sudo cargo run -- top

# Launch and record an agent command
sudo cargo run -- record -- python my_agent.py

# SSL traffic monitoring with HTTP parsing
sudo cargo run -- debug ssl --http-parser

# Process lifecycle monitoring
sudo cargo run -- debug process

# Stdio payload monitoring for a local MCP server or CLI child process
sudo cargo run -- debug stdio --pid 1234

# Combined debug monitoring with the web UI
sudo cargo run -- debug trace --server -c python
```

## Commands

### Debug SSL Monitoring

Monitor SSL/TLS traffic with advanced processing capabilities:

```bash
# Basic SSL monitoring
sudo cargo run -- debug ssl

# Enable Server-Sent Events processing
sudo cargo run -- debug ssl --sse-merge

# Enable HTTP parsing with raw data
sudo cargo run -- debug ssl --http-parser --http-raw-data

# Apply filters to reduce noise
sudo cargo run -- debug ssl --http-parser --http-filter "GET /health" --ssl-filter "handshake"

# Pass arguments to underlying eBPF program
sudo cargo run -- debug ssl -- --port 443 --comm python
```

### Debug Process Monitoring

Track process lifecycle events:

```bash
# Basic process monitoring
sudo cargo run -- debug process

# Filter by process name
sudo cargo run -- debug process -- --comm python

# Filter by PID
sudo cargo run -- debug process -- --pid 1234

# Quiet mode (no console output)
sudo cargo run -- debug process --quiet
```

### Debug Stdio Monitoring

Capture plaintext stdin/stdout/stderr payloads from a target process:

```bash
# Capture stdio payloads from one PID
sudo cargo run -- debug stdio --pid 1234

# Filter by UID or comm
sudo cargo run -- debug stdio --pid 1234 --uid 1000 --comm python3

# Capture all file descriptors instead of only 0/1/2
sudo cargo run -- debug stdio --pid 1234 --all-fds
```

### Debug Trace Monitoring

Comprehensive monitoring with both SSL and process events:

```bash
# Full debug trace monitoring
sudo cargo run -- debug trace

# Filter by process command
sudo cargo run -- debug trace -c python

# SSL-only monitoring
sudo cargo run -- debug trace --process false

# Process-only monitoring
sudo cargo run -- debug trace --ssl false

# Advanced filtering
sudo cargo run -- debug trace --pid 1234 --ssl-uid 1000 --http-filter "POST /api"

# Web UI with visualization
sudo cargo run -- debug trace --server

# Stdio-only monitoring through the trace entrypoint
sudo cargo run -- debug trace --ssl false --process false --stdio --pid 1234
```

## Configuration Options

### SSL Options

- `--sse-merge`: Enable Server-Sent Events processing
- `--http-parser`: Parse HTTP requests/responses from SSL traffic
- `--http-raw-data`: Include raw SSL data in HTTP events
- `--http-filter`: Filter HTTP events by pattern
- `--ssl-filter`: Filter SSL events by pattern

### Process Options

- `--comm`: Filter by process command name
- `--pid`: Filter by process ID
- `--duration`: Minimum process duration in ms
- `--mode`: Process filtering mode (0=all, 1=proc, 2=filter)

### Debug Trace Options

- `--ssl`: Enable/disable SSL monitoring
- `--process`: Enable/disable process monitoring
- `--stdio`: Enable/disable stdio payload monitoring
- `--system`: Enable system resource monitoring
- `--server`: Start the local web UI
- `--stdio-uid`: Filter stdio events by user ID
- `--stdio-all-fds`: Capture all file descriptors instead of only stdin/stdout/stderr
- `--stdio-max-bytes`: Limit captured bytes per stdio event
- `--ssl-uid`: Filter SSL events by user ID
- `--ssl-handshake`: Show SSL handshake events
- `--quiet`: Suppress console output

## Framework Architecture

### Runners

Runners execute eBPF programs and create event streams:

```rust
// SSL Runner
let ssl_runner = SslRunner::from_binary_extractor(binary_path)
    .with_args(&["--port", "443"])
    .add_analyzer(Box::new(HTTPParser::new()));

// Process Runner
let process_runner = ProcessRunner::from_binary_extractor(binary_path)
    .with_args(&["--comm", "python"]);

// Agent Runner (combines SSL + Process)
let agent_runner = AgentRunner::new("agent")
    .add_runner(Box::new(ssl_runner))
    .add_runner(Box::new(process_runner));
```

### Analyzers

Analyzers process event streams in configurable chains:

- **SSEProcessor**: Merges HTTP chunks and processes Server-Sent Events
- **HTTPParser**: Parses HTTP requests/responses from SSL traffic
- **HTTPFilter**: Filters HTTP events by patterns
- **SSLFilter**: Filters SSL events by patterns

Console output is rendered by the CLI after the runner/analyzer pipeline returns events.

### Event Format

Events use a standardized format:

```rust
pub struct Event {
    pub timestamp: u64,
    pub source: String,
    pub pid: u32,
    pub comm: String,
    pub data: serde_json::Value,
}
```

## Performance

- **Low Overhead**: eBPF monitoring with <3% performance impact
- **Async Processing**: Tokio-based async/await architecture
- **Streaming**: Real-time event processing with minimal memory usage
- **Configurable**: Modular analyzer chains for optimal performance

## Examples

### SSL Traffic Analysis

```bash
# Monitor HTTPS traffic with HTTP parsing
sudo cargo run -- debug ssl --http-parser --http-filter "POST /api" -- --port 443

# Monitor Python processes with SSL
sudo cargo run -- debug ssl --sse-merge -- --comm python
```

### Process Lifecycle Tracking

```bash
# Monitor Python processes
sudo cargo run -- debug process -- --comm python --duration 1000

# Monitor specific PID
sudo cargo run -- debug process -- --pid 1234
```

### Combined Monitoring

```bash
# Monitor web application
sudo cargo run -- debug trace -c nginx --ssl-uid 33 --http-filter "GET /metrics"

# Full system monitoring with web interface
sudo cargo run -- debug trace --system --server

```

## Development

### Building

```bash
# Development build
cargo build

# Release build with optimizations
cargo build --release

# Run tests
cargo test
```

### Adding Analyzers

```rust
use async_trait::async_trait;
use futures::stream::Stream;

#[async_trait]
impl Analyzer for MyAnalyzer {
    async fn process(&mut self, mut stream: EventStream) -> Result<EventStream, AnalyzerError> {
        // Process events and return transformed stream
    }
}
```

### Binary Embedding

The collector automatically embeds eBPF binaries at compile time:

```rust
let binary_extractor = BinaryExtractor::new().await?;
let ssl_path = binary_extractor.get_sslsniff_path();
let process_path = binary_extractor.get_process_path();
```

## Security

- **Root Privileges**: eBPF programs require root access for kernel monitoring
- **Independent Monitoring**: System-level observation operates independently of application code
- **Data Sensitivity**: SSL traffic may contain sensitive information
- **Secure Cleanup**: Automatic cleanup of temporary files and processes

## Troubleshooting

### Common Issues

1. **Permission Denied**: Ensure running with `sudo` or appropriate capabilities
2. **eBPF Not Supported**: Requires Linux kernel 4.1+ with eBPF enabled
3. **Binary Extraction Failed**: Check `/tmp` permissions and disk space
4. **High CPU Usage**: Reduce event volume with filters

### Debug Mode

```bash
# Enable debug logging
sudo env RUST_LOG=debug cargo run -- debug ssl --http-parser

# Verbose eBPF program output
sudo cargo run -- debug ssl -- --verbose
```

## Requirements

- **Rust**: 1.88.0 or later (edition 2024)
- **Linux**: Kernel 4.1+ with eBPF support
- **Libraries**: clang, llvm, libelf-dev
- **Privileges**: Root access for eBPF operations

## Dependencies

- **tokio**: Async runtime and stream processing
- **serde**: JSON serialization and deserialization
- **clap**: Command-line argument parsing
- **chrono**: Timestamp handling
- **futures**: Stream utilities and async processing

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## License

[License information would go here]

## Server Mode

The collector includes an embedded web server with frontend for visualization:

```bash
# Start tracing with the embedded frontend
sudo cargo run -- debug trace --server

# Access web interface
# http://127.0.0.1:7395/timeline
```

### Web Interface Features

- **Timeline View**: Interactive event timeline with zoom and filtering
- **Process Tree**: Hierarchical process visualization
- **Log View**: Raw event inspection with JSON formatting
- **Real-time Updates**: Live data streaming and analysis

## Related Projects

- **AgentSight**: Complete observability framework
- **Frontend**: React/TypeScript web interface (`../frontend/`)
- **Analysis Tools**: Python utilities for data processing (`../script/`)
- **Documentation**: Comprehensive guides and examples (`../docs/`)

## Package Information

- **Package Name**: `agentsight`
- **Repository**: https://github.com/eunomia-bpf/agentsight
- **Binary Name**: `agentsight` (after `cargo build --release`)
