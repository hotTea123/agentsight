# span_enhance migration baseline

2026-11-05: migrated the existing Jiuwen span enhancement and in-process C ABI
from upstream `934f441eff8c` (v1.0.30) onto
`c1574366122a90c451bca6a84713e28813ab95e2` (v1.0.35).

The original dirty checkout was preserved. Both staged/unstaged changes and the
previously untracked FFI sources are included. This commit does not yet implement
automatic discovery or Python adaptation.

Merge decisions:

- Keep upstream network fields, process-instance aggregation, PID namespace
  filtering, resolved-file-open mode and CLI heartbeat.
- Enhancement uses syscall opens: upstream per-file deduplication would suppress
  evidence for later tool spans and bypass the FD entry bookkeeping.
- Keep ARM64 legacy tracepoints disabled and honor namespace/cgroup hard scope.
- Route new upstream output through the existing in-process output adapter.
- Move oversized BPF aggregation temporaries to per-CPU scratch maps and avoid
  inline copying of the enlarged file-open union; verified by real BPF loading.

Validation in the designated ARM64 build container, CPython 3.11.2:

- `make -C bpf process libagentsight_tool_marker.so`: passed.
- FFI release build and six Rust tests: passed.
- BPF C unit tests and C/C++ ABI checks: passed.
- Production Jiuwen callback/LocalFunction live test: 16 tool spans, 71 evidence
  events; task, task_parent, py_context and process attribution all passed.
- Enhancement-off control: 16 tool spans, zero evidence events.
- No invalid records, queue drops or backend errors in the successful live run.

These are local library tests, not an ICAgent/Jaeger integration certification.
The full upstream collector requires newly released Cargo dependencies not in
the existing offline cache; its build is tracked separately from the FFI tests.
