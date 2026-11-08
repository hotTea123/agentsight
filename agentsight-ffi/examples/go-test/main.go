//go:build linux && (amd64 || arm64)

// Test-library consumer. No tool instrumentation, marker or trace exporter.
package main

/*
#cgo CFLAGS: -I${SRCDIR}/../../include
#cgo LDFLAGS: -L${SRCDIR}/../../lib -lagentsight_test -Wl,-rpath,${SRCDIR}/../../lib
#include "agentsight_test.h"
#include <stdlib.h>
#include <poll.h>
#include <errno.h>
static int wait_test_events(int fd) {
    struct pollfd p = { .fd = fd, .events = POLLIN };
    int rc = poll(&p, 1, 250);
    if (rc < 0) return -errno;
    if (p.revents & (POLLERR | POLLNVAL | POLLHUP)) return -EIO;
    return rc;
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
	pid := flag.Int("pid", 0, "dedicated test shell PID; 0 runs ABI check only")
	seconds := flag.Int("seconds", 30, "capture duration after start")
	procfs := flag.String("procfs-root", "/proc", "host procfs (matching host PID namespace required)")
	flag.Parse()
	if *pid < 0 || *pid > 2147483647 || *seconds <= 0 {
		return fmt.Errorf("invalid pid or seconds")
	}
	if C.agentsight_abi_version() != C.AGENTSIGHT_ABI_VERSION {
		return fmt.Errorf("ABI mismatch")
	}
	var h *C.AgentsightHandle
	if rc := C.agentsight_init(&h); rc != 0 {
		return fmt.Errorf("init: %d", rc)
	}
	defer func() {
		if rc := C.agentsight_free(h); rc != 0 {
			result = fmt.Errorf("previous error=%v; free: %d", result, rc)
		}
	}()
	check := func(rc C.int32_t) error {
		if rc >= 0 {
			return nil
		}
		buf := C.malloc(2048)
		if buf == nil {
			return fmt.Errorf("AgentSight error %d", rc)
		}
		defer C.free(buf)
		diag := C.agentsight_get_last_error(h, (*C.char)(buf), 2048)
		if diag != 0 && diag != -C.ENOSPC {
			return fmt.Errorf("AgentSight error %d; diagnostic=%d", rc, diag)
		}
		return fmt.Errorf("AgentSight error %d: %s (diagnostic=%d)", rc, C.GoString((*C.char)(buf)), diag)
	}
	fd := C.agentsight_get_eventfd(h)
	if err := check(fd); err != nil {
		return err
	}
	if *pid == 0 {
		fmt.Printf("TEST ABI passed: config=%d event=%d stats=%d schema=%d\n", C.sizeof_AgentsightConfig, C.sizeof_AgentsightEvent, C.sizeof_AgentsightStats, C.AGENTSIGHT_TEST_EVENT_SCHEMA_VERSION)
		return nil
	}
	var cfg C.AgentsightConfig
	cfg.struct_size = C.sizeof_AgentsightConfig
	cfg.abi_version = C.AGENTSIGHT_ABI_VERSION
	cfg.procfs_root = C.CString(*procfs)
	rc := C.agentsight_set_config(h, &cfg)
	C.free(unsafe.Pointer(cfg.procfs_root))
	if err := check(rc); err != nil {
		return err
	}
	if err := check(C.agentsight_set_test_target(h, C.int32_t(*pid))); err != nil {
		return err
	}
	const batch = 32
	slots := C.malloc(C.size_t(batch) * C.size_t(unsafe.Sizeof(uintptr(0))))
	if slots == nil {
		return fmt.Errorf("allocate event slots")
	}
	defer C.free(slots)
	events := unsafe.Slice((**C.AgentsightEvent)(slots), batch)
	// All fetched events are released, even if one payload fails validation.
	drain := func(maxBatches int) error {
		for i := 0; i < maxBatches; i++ {
			n := C.agentsight_poll_events(h, (**C.AgentsightEvent)(slots), batch)
			if err := check(n); err != nil {
				return err
			}
			if n == 0 {
				return nil
			}
			var firstError error
			for _, e := range events[:int(n)] {
				payload := C.GoBytes(unsafe.Pointer(e.payload_json), C.int(e.payload_json_len))
				schema, kind := uint32(e.schema_version), uint32(e.event_type)
				wall, duration := uint64(e.wall_time_ns), uint64(e.duration_ns)
				C.agentsight_free_event(e)
				if schema != C.AGENTSIGHT_TEST_EVENT_SCHEMA_VERSION || !json.Valid(payload) {
					firstError = fmt.Errorf("invalid test event schema or JSON")
					continue
				}
				// Host-owned JSON envelope. Replace stdout with ICAgent's test reporting path.
				fmt.Printf("{\"schema_version\":%d,\"event_type\":%d,\"wall_time_ns\":%d,\"duration_ns\":%d,\"payload\":%s}\n", schema, kind, wall, duration, payload)
			}
			if firstError != nil {
				return firstError
			}
		}
		return nil
	}
	if err := check(C.agentsight_start(h)); err != nil {
		return err
	}
	interrupt := make(chan os.Signal, 1)
	signal.Notify(interrupt, os.Interrupt, syscall.SIGTERM)
	defer signal.Stop(interrupt)
	deadline := time.Now().Add(time.Duration(*seconds) * time.Second)
	lastState := C.uint32_t(^uint32(0))
loop:
	for time.Now().Before(deadline) {
		select {
		case <-interrupt:
			break loop
		default:
		}
		wait := C.wait_test_events(C.int(fd))
		if wait == -C.EINTR {
			continue
		}
		if wait < 0 {
			return fmt.Errorf("eventfd wait: %d", wait)
		}
		if err := drain(64); err != nil {
			return err
		}
		var stats C.AgentsightStats
		if err := check(C.agentsight_get_stats(h, &stats, C.sizeof_AgentsightStats)); err != nil {
			return err
		}
		if stats.target_state != lastState {
			fmt.Printf("STATE target=%d pid=%d error=%d\n", stats.target_state, stats.target_pid, stats.target_error)
			lastState = stats.target_state
		}
		if stats.state == C.AGENTSIGHT_STATE_FAILED {
			return check(-C.EIO)
		}
		if stats.state == C.AGENTSIGHT_STATE_STOPPED {
			break
		}
	}
	if err := check(C.agentsight_stop(h)); err != nil {
		return err
	}
	if err := drain(65536/batch + 1); err != nil {
		return err
	}
	var stats C.AgentsightStats
	if err := check(C.agentsight_get_stats(h, &stats, C.sizeof_AgentsightStats)); err != nil {
		return err
	}
	fmt.Printf("STOPPED delivered=%d queued=%d dropped=%d ring_dropped=%d\n", stats.delivered_events, stats.queued_events, stats.queue_dropped_events, stats.kernel_ring_dropped)
	return nil
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
