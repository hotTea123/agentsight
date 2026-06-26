# Process Tracer Extension Architecture

The process tracer has one product path: `bpf/process.c` plus `bpf/process.bpf.c`.
Extended tracing lives in header-only modules under `bpf/process_ext/`; there is
no second process tracer binary or copied implementation.

## Structure

- `bpf/process.c`: userspace CLI, skeleton setup, PID/session/cgroup tracking,
  JSON output, aggregation flushing, and resource sampling loop.
- `bpf/process.bpf.c`: base exec/exit/bash/file-open programs plus extension
  module includes.
- `bpf/process_ext/types.h`: shared aggregation keys, values, and extension
  event type constants.
- `bpf/process_ext/bpf_state.h`: BPF maps and volatile feature flags used by
  extension modules.
- `bpf/process_ext/bpf_*.h`: BPF tracepoint modules for filesystem, write,
  network, signals/fork/session, shared memory, and CoW events.
- `bpf/process_ext/map_flush.h`: userspace JSON rendering and aggregation map
  flushing.
- `bpf/process_ext/mem_info.h`: userspace `/proc/<pid>` memory helpers.
- `bpf/process_ext/resource_sampler.h`: userspace process-tree and cgroup
  resource sampling.
- `bpf/process_ext/userspace.h`: userspace clock sync and cgroup filter helpers.

## CLI Surface

The base tracer still supports:

```text
./process [-v] [-d MS] [-c COMMANDS] [-p PID] [--session SID] [-m MODE]
```

Optional extension flags:

```text
--trace-fs
--trace-net
--trace-signals
--trace-mem
--trace-cow
--trace-all
--trace-resources
--resource-detail
--sample-interval MS
--cgroup PATH
--cgroup-filter PATH
--cgroup-filter-children
```

`--trace-all` enables filesystem, network, signals, and shared-memory tracing.
It intentionally excludes `--trace-cow` because the CoW kprobe is kernel-symbol
dependent and higher overhead.

## Maintenance Rules

1. Do not add another process tracer binary.
2. Do not copy `process.c` or `process.bpf.c` into a parallel implementation.
3. New BPF extension logic goes in a `bpf/process_ext/*.h` module and is included
   once from `process.bpf.c`.
4. New userspace extension helpers go in `bpf/process_ext/*.h` and are called
   from `process.c`.
5. Optional extension programs must be controlled with `bpf_program__set_autoload`
   in `process.c`, so default tracing stays small and unavailable optional
   kernel hooks do not break base capture.
6. Session and PID attribution must update both the userspace `pid_tracker` and
   BPF `tracked_pids` map.
7. All process tracer JSON strings must pass through `json_escape`.

## Tests

Unit tests:

```text
make -C bpf test
```

`make test` runs header/unit tests and then automatically runs the process
runtime suite when the current user is root or `sudo -n` is available. If root
is unavailable, runtime tests are skipped explicitly instead of prompting for a
password.

Runtime coverage includes:

- default `./process -m 0`
- JSON parsing and escaping for quoted commands
- `-p` target process tree attribution
- `--session` process-session attribution
- `-m 2` without selectors does not fall back to global tracing
- `--trace-fs` SUMMARY events
- `--trace-net` SUMMARY events
- `--trace-resources` RESOURCE_SAMPLE events
- JSON parsing of commands and paths containing quotes and backslashes

Full root integration tests:

```text
make -C bpf integration-test
```

Collector regression tests:

```text
cd collector && cargo test
```
