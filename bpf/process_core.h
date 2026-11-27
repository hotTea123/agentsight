/* SPDX-License-Identifier: (LGPL-2.1 OR BSD-2-Clause) */
#ifndef AGENTSIGHT_PROCESS_CORE_H
#define AGENTSIGHT_PROCESS_CORE_H
#include <stdint.h>
#include <stddef.h>

/* Internal C/Rust contract, NOT part of the public agentsight.h ABI.
 * Calls are serialized on one worker thread. Only one active core per process. */
struct as_core_config {
	int32_t pid;
	uint32_t cgroup_children;
	const char *marker_path;
	const char *asyncio_path;
	const char *python_path;
	const char *cgroup_path;
	uint64_t task_step_offset;
	uint64_t task_init_offset;
	uint64_t context_copy_offset;
	uint64_t context_run_offset;
	uint32_t symbol_mask;
	const char *procfs_root;
};
typedef void (*as_core_output_fn)(void *, int, const char *, size_t);
int as_core_open(const struct as_core_config *, as_core_output_fn, void *);
int as_core_poll(int timeout_ms, int wake_fd);
void as_core_close(void);
int as_core_target_changed(void);
#endif
