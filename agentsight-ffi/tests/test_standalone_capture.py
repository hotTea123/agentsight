#!/usr/bin/env python3
"""Real BPF test-library acceptance, no marker/OTel/Jaeger. Run with capture privileges.

Creates only dedicated subprocesses and temporary test files; never selects a business PID.
"""
import argparse
import ctypes as c
import json
import os
import select
import subprocess
import sys
import tempfile
import time
from pathlib import Path


class Config(c.Structure):
    _fields_ = [(n, c.c_uint32) for n in ("struct_size", "abi_version", "queue_capacity", "discovery_interval_ms")]
    _fields_ += [("max_queue_bytes", c.c_uint64), ("procfs_root", c.c_char_p)]


class Event(c.Structure):
    _fields_ = [(n, c.c_uint32) for n in ("struct_size", "schema_version", "event_type", "trace_flags", "pid", "tid")]
    _fields_ += [(n, c.c_uint64) for n in ("timestamp_ns", "wall_time_ns", "duration_ns")]
    _fields_ += [("trace_id", c.c_ubyte * 16), ("parent_span_id", c.c_ubyte * 8), ("payload_json", c.c_void_p), ("payload_json_len", c.c_uint32), ("reserved", c.c_uint32)]


class Stats(c.Structure):
    _fields_ = [(n, c.c_uint32) for n in ("struct_size", "abi_version", "state", "supported_counters")]
    _fields_ += [(n, c.c_uint64) for n in ("received_records", "filtered_records", "invalid_records", "enqueued_events", "delivered_events", "queue_dropped_events", "output_dropped_records", "backend_errors", "backend_diagnostics", "aggregate_overflow", "queued_events", "queued_bytes")]
    _fields_ += [("target_state", c.c_uint32), ("target_pid", c.c_int32)]
    _fields_ += [(n, c.c_uint64) for n in ("target_start_ticks", "target_generation", "discovery_scans", "discovery_errors", "attach_attempts", "attach_failures", "target_detaches")]
    _fields_ += [("target_error", c.c_int32), ("adaptation_method", c.c_uint32), ("kernel_ring_dropped", c.c_uint64)]


TARGET = r'''
import json, os, subprocess, sys
from pathlib import Path
root = Path(sys.argv[1])
print("STARTED", flush=True)
for line in sys.stdin:
    name = line.strip()
    if name == "quit": break
    before, after = root / (name + ".before"), root / (name + ".after")
    fd = os.open(before, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.write(fd, b"test-evidence\n")
    os.close(fd)
    os.rename(before, after)
    child = subprocess.Popen(["/bin/sh", "-c", 'printf child-evidence > "$1"', "as-test", str(root / (name + ".child"))])
    child.wait(timeout=5)
    print(json.dumps({"pid":os.getpid(), "child":child.pid, "name":name}), flush=True)
'''


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--library", required=True)
    ap.add_argument("--report", required=True)
    ap.add_argument("--go-demo")
    args = ap.parse_args()
    lib = c.CDLL(str(Path(args.library).resolve()))
    handle = c.c_void_p()
    pe = c.POINTER(Event)
    signatures = {
        "agentsight_abi_version": ([], c.c_uint32),
        "agentsight_init": ([c.POINTER(c.c_void_p)], c.c_int32),
        "agentsight_set_config": ([c.c_void_p, c.POINTER(Config)], c.c_int32),
        "agentsight_set_test_target": ([c.c_void_p, c.c_int32], c.c_int32),
        "agentsight_poll_events": ([c.c_void_p, c.POINTER(pe), c.c_uint32], c.c_int32),
        "agentsight_free_event": ([pe], None),
        "agentsight_get_stats": ([c.c_void_p, c.POINTER(Stats), c.c_size_t], c.c_int32),
        "agentsight_get_last_error": ([c.c_void_p, c.c_void_p, c.c_size_t], c.c_int32),
    }
    for name in ("start", "stop", "free", "get_eventfd"):
        signatures["agentsight_" + name] = ([c.c_void_p], c.c_int32)
    for name, (argv, ret) in signatures.items():
        getattr(lib, name).argtypes, getattr(lib, name).restype = argv, ret

    def check(rc):
        if rc < 0:
            buf = c.create_string_buffer(2048)
            lib.agentsight_get_last_error(handle, buf, len(buf))
            raise AssertionError((rc, buf.value.decode(errors="replace")))
        return rc

    def stats():
        s = Stats()
        check(lib.agentsight_get_stats(handle, c.byref(s), c.sizeof(s)))
        return s

    def drain():
        events = []
        while True:
            slots = (pe * 32)()
            n = check(lib.agentsight_poll_events(handle, slots, 32))
            if not n: return events
            batch = []
            # Free every returned pointer even if validation later fails.
            for i in range(n):
                e = slots[i].contents
                batch.append((e.schema_version, bytes(e.trace_id), bytes(e.parent_span_id), e.trace_flags,
                              e.event_type, e.pid, e.wall_time_ns, c.string_at(e.payload_json, e.payload_json_len)))
                lib.agentsight_free_event(slots[i])
            for schema, trace, parent, flags, kind, pid, wall, raw in batch:
                assert schema == 1001 and trace == bytes(16) and parent == bytes(8) and flags == 0
                assert 0 < wall < int((time.time() + 60) * 1e9)
                payload = json.loads(raw)
                assert payload["agentsight_test_only"] is True
                assert payload["agentsight_correlation_source"] == "none"
                events.append({"event_type":kind, "pid":pid, "payload":payload})

    def ready():
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            s = stats()
            if s.target_state == 3: return
            if s.state == 4: check(s.target_error or -5)
            time.sleep(.02)
        raise AssertionError("READY timeout")

    def command(p, name):
        p.stdin.write(name + "\n")
        p.stdin.flush()
        assert select.select([p.stdout], [], [], 10)[0], "target timeout"
        return json.loads(p.stdout.readline())

    assert (c.sizeof(Config), c.sizeof(Event), c.sizeof(Stats)) == (32, 88, 192)
    assert lib.agentsight_abi_version() == 2
    check(lib.agentsight_init(c.byref(handle)))
    report = {"scope":"test library only; no ICAgent/Jaeger", "rounds":[]}
    processes = []
    try:
        with tempfile.TemporaryDirectory(prefix="agentsight-test-evidence-") as tmp:
            def spawn():
                p = subprocess.Popen([sys.executable, "-u", "-c", TARGET, tmp], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
                processes.append(p)
                assert p.stdout.readline().strip() == "STARTED"
                return p
            target, outsider = spawn(), spawn()
            cfg = Config(c.sizeof(Config), 2, 1024, 100, 0, b"/proc")
            check(lib.agentsight_set_config(handle, c.byref(cfg)))
            assert lib.agentsight_start(handle) == -22
            assert lib.agentsight_set_test_target(handle, os.getpid()) == -22
            assert lib.agentsight_set_test_target(handle, os.getppid()) == -22
            assert lib.agentsight_set_test_target(handle, 0) == -22
            assert lib.agentsight_set_test_target(handle, 2147483647) == -3
            check(lib.agentsight_set_test_target(handle, target.pid))
            fd = check(lib.agentsight_get_eventfd(handle))
            poller = select.poll()
            poller.register(fd, select.POLLIN)
            for round_no in range(2):
                check(lib.agentsight_start(handle))
                ready()
                check(lib.agentsight_start(handle))
                assert lib.agentsight_set_test_target(handle, outsider.pid) == -16
                drain()  # Also acknowledges READY notification before measuring event notification.
                outside = command(outsider, "outside" + str(round_no))
                inside = command(target, "inside" + str(round_no))
                assert poller.poll(2000), "missing event notification"
                events = []
                deadline = time.monotonic() + 8
                while time.monotonic() < deadline:
                    events.extend(drain())
                    if {e["event_type"] for e in events} == {1,2,3,4}: break
                    poller.poll(100)
                # New pending aggregate must be retained on stop/drain, too.
                shutdown = command(target, "shutdown" + str(round_no))
                check(lib.agentsight_stop(handle))
                events.extend(drain())
                check(lib.agentsight_stop(handle))
                assert {e["event_type"] for e in events} == {1,2,3,4}
                assert all(e["pid"] in {target.pid, inside["child"], shutdown["child"]} for e in events)
                assert not any("outside" in json.dumps(e) for e in events), outside
                assert any(e["pid"] == inside["child"] and e["event_type"] == 2 for e in events), "short-child write missing"
                assert any(e["event_type"] == 3 and "shutdown" in e["payload"].get("last_destination_path", "") for e in events)
                s = stats()
                assert s.queued_events == 0 and s.target_generation == round_no + 1
                assert s.queue_dropped_events == s.kernel_ring_dropped == s.invalid_records == s.backend_errors == 0
                report["rounds"].append({"count":len(events), "events":events, "generation":s.target_generation})
            # Verify real bounded-queue drops, then restore normal config. Reconfiguration
            # deliberately clears the target, so accidental start cannot retain an old scope.
            cfg.queue_capacity = 1
            check(lib.agentsight_set_config(handle, c.byref(cfg)))
            assert lib.agentsight_start(handle) == -22
            check(lib.agentsight_set_test_target(handle, target.pid))
            check(lib.agentsight_start(handle)); ready()
            command(target, "overflow")
            check(lib.agentsight_stop(handle))
            assert stats().queued_events == 1 and stats().queue_dropped_events > 0
            assert lib.agentsight_start(handle) == -16
            assert lib.agentsight_set_test_target(handle, target.pid) == -16
            report["queue_overflow_dropped"] = stats().queue_dropped_events
            drain()
            cfg.queue_capacity = 1024
            check(lib.agentsight_set_config(handle, c.byref(cfg)))
            check(lib.agentsight_set_test_target(handle, target.pid))
            check(lib.agentsight_start(handle))
            ready()
            target.stdin.write("quit\n"); target.stdin.flush(); target.wait(timeout=5)
            deadline = time.monotonic() + 3
            while stats().state != 3 and time.monotonic() < deadline: time.sleep(.02)
            s = stats()
            assert s.state == 3 and s.target_state == 4 and s.target_error == -3
            check(lib.agentsight_stop(handle)); drain()
            assert lib.agentsight_set_test_target(handle, target.pid) == -3
            report["target_exit_stops_capture"] = True

            if args.go_demo:
                target2 = spawn()
                with tempfile.TemporaryFile(mode="w+") as out:
                    go = subprocess.Popen([args.go_demo, "-pid", str(target2.pid), "-seconds", "3"], stdout=out, stderr=subprocess.STDOUT)
                    processes.append(go)
                    deadline = time.monotonic() + 10
                    while time.monotonic() < deadline:
                        out.seek(0)
                        if "STATE target=3" in out.read(): break
                        assert go.poll() is None, "Go host failed before READY"
                        time.sleep(.05)
                    else: raise AssertionError("Go READY timeout")
                    command(target2, "go-consumer")
                    assert go.wait(timeout=10) == 0
                    out.seek(0); text = out.read()
                    rows = [json.loads(line) for line in text.splitlines() if line.startswith("{")]
                    assert {r["event_type"] for r in rows} == {1,2,3,4}
                    assert all(r["schema_version"] == 1001 for r in rows)
                    assert "queued=0 dropped=0 ring_dropped=0" in text
                    report["go_consumer"] = {"events":len(rows), "output":text}
    finally:
        lib.agentsight_stop(handle)
        lib.agentsight_free(handle)
        for p in processes:
            if p.poll() is None:
                p.terminate()
                try: p.wait(timeout=5)
                except subprocess.TimeoutExpired: p.kill(); p.wait()
    report["passed"] = True
    Path(args.report).write_text(json.dumps(report, indent=2, ensure_ascii=False))
    print(json.dumps({"passed":True, "counts":[r["count"] for r in report["rounds"]], "go":report.get("go_consumer", {}).get("events"), "report":args.report}))


if __name__ == "__main__":
    main()
