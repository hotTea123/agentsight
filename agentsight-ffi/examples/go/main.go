//go:build linux && (amd64 || arm64)

// Minimal cgo consumer. Use the header and library from the SAME release.
package main

/*
#cgo CFLAGS: -I${SRCDIR}/../../include
#cgo LDFLAGS: -L${SRCDIR}/../../lib -lagentsight -Wl,-rpath,${SRCDIR}/../../lib
#include "agentsight.h"
#include <stdlib.h>
#include <poll.h>
#include <errno.h>
static int wait_agentsight(int fd) {
    struct pollfd p = { .fd = fd, .events = POLLIN };
    int rc = poll(&p, 1, 1000);
    if (rc < 0) return -errno;
    if (p.revents & (POLLERR | POLLHUP | POLLNVAL)) return -EIO;
    return rc;
}
*/
import "C"

import (
	"flag"
	"fmt"
	"os"
	"time"
	"unsafe"
)

func run() (result error) {
	capture := flag.Bool("capture", false, "discover Jiuwen and capture; otherwise only check the ABI")
	procfs := flag.String("procfs-root", "/proc", "host procfs path; host PID namespace is required")
	seconds := flag.Int("seconds", 10, "capture duration")
	flag.Parse()
	if *seconds <= 0 {
		return fmt.Errorf("seconds must be positive")
	}
	if C.agentsight_abi_version() != C.AGENTSIGHT_ABI_VERSION {
		return fmt.Errorf("AgentSight ABI mismatch")
	}
	var h *C.AgentsightHandle
	if rc := C.agentsight_init(&h); rc != 0 {
		return fmt.Errorf("init: %d", rc)
	}
	check := func(rc C.int32_t) error {
		if rc >= 0 {
			return nil
		}
		buffer := C.malloc(2048)
		if buffer == nil {
			return fmt.Errorf("AgentSight error %d", rc)
		}
		defer C.free(buffer)
		diagnosticRC := C.agentsight_get_last_error(h, (*C.char)(buffer), 2048)
		if diagnosticRC != 0 && diagnosticRC != -C.ENOSPC {
			return fmt.Errorf("AgentSight error %d; diagnostic read failed: %d", rc, diagnosticRC)
		}
		message := C.GoString((*C.char)(buffer))
		if diagnosticRC == -C.ENOSPC {
			message += " [truncated]"
		}
		return fmt.Errorf("AgentSight error %d: %s", rc, message)
	}
	defer func() {
		// On success h is invalid; do not query its last_error afterwards.
		if rc := C.agentsight_free(h); rc < 0 {
			if result == nil {
				result = fmt.Errorf("agentsight_free: %d", rc)
			} else {
				result = fmt.Errorf("%w; agentsight_free: %d", result, rc)
			}
		}
	}()
	fd := C.agentsight_get_eventfd(h)
	if err := check(fd); err != nil {
		return err
	}
	if !*capture {
		var stats C.AgentsightStats
		if err := check(C.agentsight_get_stats(h, &stats, C.sizeof_AgentsightStats)); err != nil {
			return err
		}
		fmt.Printf("Go/cgo ABI smoke passed: eventfd=%d state=%d config=%d event=%d stats=%d\n", fd, stats.state, C.sizeof_AgentsightConfig, C.sizeof_AgentsightEvent, C.sizeof_AgentsightStats)
		return nil
	}
	var cfg C.AgentsightConfig
	cfg.struct_size = C.sizeof_AgentsightConfig
	cfg.abi_version = C.AGENTSIGHT_ABI_VERSION
	cfg.procfs_root = C.CString(*procfs)
	defer C.free(unsafe.Pointer(cfg.procfs_root))
	if err := check(C.agentsight_set_config(h, &cfg)); err != nil {
		return err
	}
	if err := check(C.agentsight_start(h)); err != nil {
		return err
	}
	const batch = 16
	slots := C.malloc(C.size_t(batch) * C.size_t(unsafe.Sizeof(uintptr(0))))
	if slots == nil {
		return fmt.Errorf("allocate event slots")
	}
	defer C.free(slots)
	events := unsafe.Slice((**C.AgentsightEvent)(slots), batch)
	drain := func() error {
		for {
			n := C.agentsight_poll_events(h, (**C.AgentsightEvent)(slots), batch)
			if err := check(n); err != nil {
				return err
			}
			if n == 0 {
				return nil
			}
			for _, event := range events[:int(n)] {
				// Copy before release; do not retain a slice into C/Rust memory.
				payload := C.GoBytes(unsafe.Pointer(event.payload_json), C.int(event.payload_json_len))
				traceID := C.GoBytes(unsafe.Pointer(&event.trace_id[0]), 16)
				parentID := C.GoBytes(unsafe.Pointer(&event.parent_span_id[0]), 8)
				eventType, flags := uint32(event.event_type), uint32(event.trace_flags)
				pid, tid := uint32(event.pid), uint32(event.tid)
				wallTime, duration := uint64(event.wall_time_ns), uint64(event.duration_ns)
				C.agentsight_free_event(event)
				// These are copied values. A real host converts them into a NEW child span.
				fmt.Printf("type=%d trace_id=%x parent_span_id=%x flags=%d pid=%d tid=%d wall_time_ns=%d duration_ns=%d\n",
					eventType, traceID, parentID, flags, pid, tid, wallTime, duration)
				fmt.Println(string(payload))
			}
		}
	}
	deadline := time.Now().Add(time.Duration(*seconds) * time.Second)
	lastState := C.uint32_t(^uint32(0))
	for time.Now().Before(deadline) {
		waitRC := C.wait_agentsight(C.int(fd))
		if waitRC == -C.EINTR {
			continue
		}
		if waitRC < 0 {
			return fmt.Errorf("eventfd wait: %d", waitRC)
		}
		if err := drain(); err != nil {
			return err
		}
		var stats C.AgentsightStats
		if err := check(C.agentsight_get_stats(h, &stats, C.sizeof_AgentsightStats)); err != nil {
			return err
		}
		if stats.state == C.AGENTSIGHT_STATE_FAILED {
			return fmt.Errorf("capture failed; consult get_last_error/get_stats")
		}
		if stats.target_state != lastState {
			fmt.Printf("target_state=%d target_pid=%d generation=%d adaptation=%d error=%d\n",
				stats.target_state, stats.target_pid, stats.target_generation, stats.adaptation_method, stats.target_error)
			lastState = stats.target_state
		}
	}
	if err := check(C.agentsight_stop(h)); err != nil {
		return err
	}
	return drain()
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
