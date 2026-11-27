"""Real procfs/BPF lifecycle tests; fixture never creates synthetic tool spans."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import select
import signal
import subprocess
import sys
import time


def fixture(args):
    import asyncio  # Load the actual _asyncio ELF before the delayed marker.
    assert asyncio.new_event_loop
    marker = ctypes.CDLL(str(args.marker)) if args.loaded else None
    print("FIXTURE", flush=True)
    for command in sys.stdin:
        command = command.strip()
        if command == "load":
            marker = ctypes.CDLL(str(args.marker))
            print("LOADED", flush=True)
        elif command == "exec":
            os.execv(sys.executable, [sys.executable, "-B", __file__, "--fixture", "--loaded", "--marker", str(args.marker)])
        elif command == "fork":
            child = os.fork()
            if child == 0:
                # Remain mapped after the root exits, without consuming stdin.
                while True:
                    signal.pause()
            print(child, flush=True)
        elif command == "quit":
            return


def line(p, expected=None):
    assert select.select([p.stdout], [], [], 20)[0], "fixture response timeout"
    result = p.stdout.readline().decode().strip()
    assert result and (expected is None or result == expected), result
    return result


def send(p, command, expected=None):
    p.stdin.write((command + "\n").encode()); p.stdin.flush()
    return line(p, expected) if command != "quit" else None


def run(args):
    args.workdir.mkdir(parents=True, exist_ok=False)
    path = args.workdir / "capture.jsonl"
    processes, children = [], []
    def app(loaded=False, marker=None):
        cmd = [sys.executable, "-B", str(Path(__file__).resolve()), "--fixture", "--marker", str(marker or args.marker)]
        if loaded:
            cmd.append("--loaded")
        p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        processes.append(p)
        line(p, "FIXTURE")
        return p
    def states():
        return [v for text in path.read_text().splitlines() if text.startswith("{")
                if "target_state" in (v := json.loads(text))]
    def scan_count():
        return max([0] + [v["discovery_scans"] for text in path.read_text().splitlines()
                          if text.startswith("{") and "discovery_scans" in (v := json.loads(text))])
    def observe_scans():
        # Count starts of three scans, so two whole scans have completed.
        target = scan_count() + 3
        deadline = time.monotonic() + 30
        while scan_count() < target:
            assert time.monotonic() < deadline, "discovery scans stalled"
            assert host.poll() is None
            time.sleep(0.05)
    def wait_state(state, pid=None, generation=None, after=0):
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            for v in states()[after:]:
                if v["target_state"] == state and (pid is None or v["target_pid"] == pid) and (generation is None or v["generation"] >= generation):
                    return v
            assert host.poll() is None, "capture host exited"
            time.sleep(0.05)
        raise AssertionError(path.read_text())
    try:
        with path.open("w") as output, (args.workdir / "stderr").open("w") as error:
            host = subprocess.Popen([str(args.capture)], stdin=subprocess.PIPE, stdout=output, stderr=error)
            processes.append(host)
            wait_state(1)
            if args.decoy_marker:
                app(loaded=True, marker=args.decoy_marker)
                observe_scans()
                assert states()[-1]["target_state"] == 1, "non-marker namesake selected"
            first = app()
            observe_scans()
            assert not any(v["target_state"] == 3 for v in states())
            send(first, "load", "LOADED")
            ready = wait_state(3, first.pid)
            send(first, "exec", "FIXTURE")
            ready = wait_state(3, first.pid, ready["generation"] + 1)
            child = int(send(first, "fork")); children.append(child)
            observe_scans()  # Confirm ancestry scans before reparenting.
            assert states()[-1]["target_state"] == 3
            checkpoint = len(states())
            send(first, "quit"); assert first.wait(timeout=10) == 0
            wait_state(1, after=checkpoint)
            observe_scans()
            assert states()[-1]["target_state"] == 1, "orphan child promoted to root"
            second = app(loaded=True)
            ready = wait_state(3, second.pid, ready["generation"] + 1)
            checkpoint = len(states())
            conflicting = app(loaded=True)
            wait_state(4, after=checkpoint)
            assert "multiple independent" in path.read_text()
            send(conflicting, "quit"); assert conflicting.wait(timeout=10) == 0
            wait_state(3, second.pid, ready["generation"] + 1)
            send(second, "quit"); assert second.wait(timeout=10) == 0
            host.stdin.write(b"q"); host.stdin.flush()
            assert host.wait(timeout=10) == 0
        assert not (args.workdir / "stderr").read_text()
        result = dict(passed=True, cases=["host-first", "delayed-marker", "same-PID-exec", "fork-child", "observed-orphan-excluded", "root-restart", "ambiguous-roots", "ambiguity-recovery"])
        if args.decoy_marker:
            result["cases"].append("non-marker-namesake-excluded")
        (args.workdir / "verification.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result))
    finally:
        for p in processes:
            if p.poll() is None:
                p.terminate()
                try:
                    p.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    p.kill(); p.wait()
        for pid in children:
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", action="store_true")
    parser.add_argument("--loaded", action="store_true")
    parser.add_argument("--marker", type=Path, required=True)
    parser.add_argument("--workdir", type=Path)
    parser.add_argument("--capture", type=Path)
    parser.add_argument("--decoy-marker", type=Path)
    args = parser.parse_args()
    fixture(args) if args.fixture else run(args)
