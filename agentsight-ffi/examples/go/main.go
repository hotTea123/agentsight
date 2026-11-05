//go:build linux && (amd64 || arm64)

// Minimal cgo consumer. Use the header and library from the SAME release.
package main

/*
#cgo CFLAGS: -I${SRCDIR}/../../include
#cgo LDFLAGS: -L${SRCDIR}/../../lib -lagentsight -Wl,-rpath,${SRCDIR}/../../lib
#include "agentsight.h"
#include <stdlib.h>
#include <poll.h>
static int wait_agentsight(int fd) {
    struct pollfd p = { .fd = fd, .events = POLLIN };
    return poll(&p, 1, 1000);
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

func run() error {
	pid := flag.Int("pid", 0, "target host PID; omit to run only the ABI smoke check")
	marker := flag.String("marker", "", "absolute marker path")
	asyncio := flag.String("asyncio", "", "absolute _asyncio ELF path")
	python := flag.String("python", "", "absolute Python ELF path")
	step := flag.Uint64("step", 0, "validated task-step file offset")
	init := flag.Uint64("task-init", 0, "validated task-init file offset")
	copy := flag.Uint64("context-copy", 0, "validated context-copy file offset")
	runOffset := flag.Uint64("context-run", 0, "validated context-run file offset")
	seconds := flag.Int("seconds", 10, "capture duration")
	flag.Parse()
	if C.agentsight_abi_version() != C.AGENTSIGHT_ABI_VERSION {
		return fmt.Errorf("AgentSight ABI mismatch")
	}
	var h *C.AgentsightHandle
	if rc := C.agentsight_init(&h); rc != 0 {
		return fmt.Errorf("init: %d", rc)
	}
	defer C.agentsight_free(h)
	check := func(rc C.int32_t) error {
		if rc >= 0 {
			return nil
		}
		buffer := C.malloc(2048)
		if buffer == nil {
			return fmt.Errorf("AgentSight error %d", rc)
		}
		defer C.free(buffer)
		C.agentsight_get_last_error(h, (*C.char)(buffer), 2048)
		return fmt.Errorf("AgentSight error %d: %s", rc, C.GoString((*C.char)(buffer)))
	}
	fd := C.agentsight_get_eventfd(h)
	if err := check(fd); err != nil {
		return err
	}
	if *pid == 0 {
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
	cfg.target_pid = C.int32_t(*pid)
	cfg.marker_path = C.CString(*marker)
	cfg.asyncio_path = C.CString(*asyncio)
	cfg.python_path = C.CString(*python)
	defer C.free(unsafe.Pointer(cfg.marker_path))
	defer C.free(unsafe.Pointer(cfg.asyncio_path))
	defer C.free(unsafe.Pointer(cfg.python_path))
	cfg.task_step_offset = C.uint64_t(*step)
	cfg.task_init_offset = C.uint64_t(*init)
	cfg.context_copy_offset = C.uint64_t(*copy)
	cfg.context_run_offset = C.uint64_t(*runOffset)
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
				C.agentsight_free_event(event)
				fmt.Println(string(payload))
			}
		}
	}
	deadline := time.Now().Add(time.Duration(*seconds) * time.Second)
	for time.Now().Before(deadline) {
		C.wait_agentsight(C.int(fd))
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
