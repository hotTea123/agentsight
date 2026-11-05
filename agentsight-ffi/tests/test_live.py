"""Real BPF acceptance using Jiuwen's production callback + LocalFunction.

Requires the isolated CPython/runtime fixture, not a mock span or an exporter.
Never contacts Jaeger, a cloud endpoint or the original AgentSight collector.
"""
import argparse
import asyncio
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def save(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


async def worker(args):
    from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter
    from openjiuwen.core.foundation.tool import LocalFunction, ToolCard
    from openjiuwen.extensions.observability.config import ObservabilityConfig
    from openjiuwen.extensions.observability.setup import init_observability, force_flush_provider
    from openjiuwen.extensions.observability.span_context import get_current_tool_span
    from jiuwenswarm.agents.harness.agent_observability import (
        sync_agent_observability, open_agent_run_span, close_agent_run_span,
        shutdown_agent_observability,
    )

    work = args.workdir
    exporter = InMemorySpanExporter()
    init_observability(ObservabilityConfig(
        service_name="agentsight-ffi-acceptance", backend="otlp",
        agentsight_tool_enhancement_enabled=args.mode == "on",
        agentsight_marker_library=str(args.marker),
    ), span_exporter_override=exporter)
    sync_agent_observability()
    observed = []

    for round_id in range(2):
        while not (work / f"gate-{round_id}").exists():
            await asyncio.sleep(0.05)
        root = open_agent_run_span(session_id=f"ffi-{args.mode}-{round_id}", mode="agent")
        assert root is not None and root.is_recording()

        def record(label):
            span = get_current_tool_span()
            assert span is not None and span.is_recording(), "missing formal tool span"
            c = span.get_span_context()
            observed.append(dict(label=label, trace_id=f"{c.trace_id:032x}", span_id=f"{c.span_id:016x}", round=round_id))

        def files(label):
            source = work / "input.txt"
            temp = work / f"{label}.tmp"
            output = work / f"{label}.out"
            fd = os.open(source, os.O_RDONLY)
            try:
                data = os.read(fd, 4096)
            finally:
                os.close(fd)
            fd = os.open(temp, os.O_CREAT | os.O_WRONLY | os.O_TRUNC, 0o600)
            try:
                assert os.write(fd, data) == len(data)
            finally:
                os.close(fd)
            os.rename(temp, output)
            return len(data)

        async def invoke(label, mode):
            async def body():
                record(label)
                # Exercise a real Task switch with a live binding; parallel
                # invocations now overlap before performing their file work.
                await asyncio.sleep(0.01)
                if mode == "thread":
                    return await asyncio.to_thread(files, label)
                if mode == "nested":
                    async def child():
                        return files(label)
                    return await asyncio.create_task(child())
                if mode == "process":
                    subprocess.run(["/bin/sh", "-c", 'printf child > "$1"', "sh", str(work / f"{label}.out")], check=True)
                    return 5
                result = files(label)
                if mode == "error":
                    raise RuntimeError("intentional acceptance error")
                return result

            def sync_body():
                record(label)
                return files(label)

            fn = sync_body if mode == "sync" else body
            tool = LocalFunction(ToolCard(id=label, name=label, description="FFI acceptance file operations"), fn)
            try:
                result = await tool.invoke({})
                assert mode != "error", "intentional error was not raised"
                assert result > 0
            except RuntimeError:
                if mode != "error":
                    raise
            assert get_current_tool_span() is None, "tool context leaked"

        try:
            for mode in ("direct", "sync", "nested", "thread", "process", "error"):
                await invoke(f"{mode}-{round_id}", mode)
            await asyncio.gather(invoke(f"parallel-a-{round_id}", "direct"), invoke(f"parallel-b-{round_id}", "thread"))
            files(f"outside-tool-{round_id}")
        finally:
            close_agent_run_span(root, session_id=f"ffi-{args.mode}-{round_id}", output="done")
        force_flush_provider()
        save(work / f"round-{round_id}.json", observed)
    spans = exporter.get_finished_spans()
    save(work / "spans.json", [dict(name=s.name, trace_id=f"{s.context.trace_id:032x}", span_id=f"{s.context.span_id:016x}", parent=f"{s.parent.span_id:016x}" if s.parent else None, status=s.status.status_code.name) for s in spans])
    save(work / "expected.json", observed)
    shutdown_agent_observability()


def wait_until(predicate, processes, timeout=45):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        for p in processes:
            assert p.poll() is None, f"test process {p.pid} exited {p.returncode}"
        time.sleep(0.05)
    raise AssertionError("timed out waiting for readiness or formal tool completion")


def run(args, mode):
    work = args.workdir / mode
    work.mkdir()
    (work / "config").mkdir()
    (work / "input.txt").write_text("real span-enhancement FFI acceptance\n")
    save(work / "config/config.yaml", {
        "agent_observability": {"enabled": True, "service_name": "agentsight-ffi-acceptance", "agentsight_tool_enhancement_enabled": mode == "on", "agentsight_marker_library": str(args.marker)},
        "react": {"evolution": {"skill_evolution": False}},
    })
    env = os.environ | {
        "PYTHONDONTWRITEBYTECODE": "1",
        "JIUWENSWARM_DATA_DIR": str(work), "JIUWENSWARM_CONFIG_DIR": str(work / "config"),
    }
    app = host = None
    try:
        with (work / "python.log").open("w") as pylog, (work / "capture.jsonl").open("w") as capture, (work / "capture.stderr").open("w") as errlog:
            app = subprocess.Popen([sys.executable, "-B", str(Path(__file__).resolve()), "--worker", "--workdir", str(work), "--mode", mode, "--marker", str(args.marker)], env=env, cwd=work, stdout=pylog, stderr=subprocess.STDOUT)
            # Wait for actual marker mapping (on) or Python imports/provider (off).
            if mode == "on":
                wait_until(lambda: str(args.marker) in Path(f"/proc/{app.pid}/maps").read_text(), [app])
            host = subprocess.Popen([str(args.capture), str(app.pid), str(args.marker), str(args.asyncio_lib), str(args.python_lib), *[str(x) for x in args.offsets]], stdin=subprocess.PIPE, stdout=capture, stderr=errlog)
            for round_id in range(2):
                wait_until(lambda: (work / "capture.jsonl").read_text().count("READY") >= round_id + 1, [app, host])
                # The C test host must only have threads, never a collector child.
                children = [p.read_text().strip() for p in Path(f"/proc/{host.pid}/task").glob("*/children")]
                assert not any(children), children
                (work / f"gate-{round_id}").touch()
                wait_until(lambda: (work / f"round-{round_id}.json").exists(), [host])
                if round_id == 0:
                    host.stdin.write(b"r"); host.stdin.flush()
            assert app.wait(timeout=30) == 0
            host.stdin.write(b"q"); host.stdin.flush()
            assert host.wait(timeout=30) == 0
    finally:
        for p in (host, app):
            if p is not None and p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    p.kill(); p.wait()

    expected = json.loads((work / "expected.json").read_text())
    spans = json.loads((work / "spans.json").read_text())
    formal = {(s["trace_id"], s["span_id"]): s for s in spans if s["name"].startswith("tool.")}
    assert len(formal) == len(expected) == 16
    assert all((e["trace_id"], e["span_id"]) in formal for e in expected)
    lines = [json.loads(line) for line in (work / "capture.jsonl").read_text().splitlines() if line.startswith("{")]
    events = [e for e in lines if "payload" in e]
    stats = next(e["stats"] for e in lines if "stats" in e)
    assert stats["invalid"] == stats["backend_errors"] == stats["dropped"] == 0, stats
    assert not (work / "capture.stderr").read_text(), "library wrote to stderr"
    sources = set()
    by_span = {}
    for event in events:
        key = (event["trace_id"], event["parent_span_id"])
        assert key in formal, event
        payload = event["payload"]
        assert event["trace_id"] == payload["agentsight_trace_id"]
        assert event["parent_span_id"] == payload["agentsight_parent_span_id"]
        assert abs(event["wall_time_ns"] - time.time_ns()) < 120_000_000_000
        assert "outside-tool" not in json.dumps(payload), payload
        by_span.setdefault(key, []).append(payload)
        sources.add(payload["agentsight_correlation_source"])
    if mode == "on":
        for e in expected:
            records = by_span.get((e["trace_id"], e["span_id"]), [])
            kinds = {r["event"] for r in records}
            if e["label"].startswith("process"):
                assert "EXEC" in kinds, (e, kinds)
            else:
                assert {"FILE_OPEN", "FILE_WRITE", "FILE_RENAME"} <= kinds, (e, kinds)
        assert {"task", "task_parent", "py_context", "process"} <= sources, sources
    else:
        assert events == [], events
    return dict(mode=mode, passed=True, tool_spans=len(formal), events=len(events), sources=sorted(sources), stats=stats)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--workdir", type=Path, required=True)
    p.add_argument("--marker", type=Path, required=True)
    p.add_argument("--worker", action="store_true")
    p.add_argument("--mode", default="on")
    p.add_argument("--capture", type=Path)
    p.add_argument("--asyncio-lib", type=Path)
    p.add_argument("--python-lib", type=Path)
    p.add_argument("--offsets", nargs=4, type=int)
    args = p.parse_args()
    if args.worker:
        asyncio.run(worker(args))
    else:
        args.workdir.mkdir(parents=True, exist_ok=False)
        result = [run(args, mode) for mode in ("on", "off")]
        save(args.workdir / "verification.json", result)
        print(json.dumps(result, indent=2))
