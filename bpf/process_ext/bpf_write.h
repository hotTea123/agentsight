/* SPDX-License-Identifier: GPL-2.0 OR BSD-3-Clause */
#ifndef __PROCESS_EXT_BPF_WRITE_H
#define __PROCESS_EXT_BPF_WRITE_H

/*
 * write-family syscall tracing: write/pwrite64/writev enter/exit pairing for
 * byte count aggregation.
 * write_ctx_map is defined by process_ext/bpf_state.h (temporary context, not aggregation).
 * Aggregation goes into event_agg_map with detail="fd=N" (later resolved to path in userspace).
 */

static __always_inline int trace_write_enter_common(int fd)
{
	if (!trace_fs_mutations)
		return 0;
	if (!is_scope_tracked() || (!is_event_tracked() && !has_python_tool_context()))
		return 0;

	u64 id = bpf_get_current_pid_tgid();
	bpf_map_update_elem(&write_ctx_map, &id, &fd, BPF_ANY);
	return 0;
}

static __always_inline int trace_write_exit_common(long ret)
{
	if (!trace_fs_mutations)
		return 0;

	u64 id = bpf_get_current_pid_tgid();
	int *fd_ptr = bpf_map_lookup_elem(&write_ctx_map, &id);
	if (!fd_ptr)
		return 0;

	int fd = *fd_ptr;
	bpf_map_delete_elem(&write_ctx_map, &id);
	if (ret <= 0)
		return 0;

	u32 zero = 0;
	struct correlated_file_key *correlated_key =
		bpf_map_lookup_elem(&correlated_file_key_scratch, &zero);
	if (!correlated_key)
		return 0;
	__builtin_memset(correlated_key, 0, sizeof(*correlated_key));
	correlated_key->pid = id >> 32;
	correlated_key->event_type = EVENT_TYPE_WRITE;
	bool correlated = fill_python_correlated_file_context(correlated_key);
	struct file_descriptor_key fd_key = {
		.tgid = correlated_key->pid,
		.fd = fd,
	};
	struct tracked_file_descriptor *tracked =
		bpf_map_lookup_elem(&tracked_file_descriptors, &fd_key);
	if (correlated && tracked) {
		bpf_probe_read_kernel_str(
			correlated_key->filepath,
			sizeof(correlated_key->filepath),
			tracked->filepath);
		update_correlated_file_agg_map(
			correlated_key,
			1,
			(u64)ret,
			NULL);
		return 0;
	}
	if (!is_event_tracked())
		return 0;

	struct agg_key key = {};
	key.pid = id >> 32;
	key.event_type = EVENT_TYPE_WRITE;
	format_fd_detail(key.detail, sizeof(key.detail), fd);
	update_agg_map(&key, 1, (u64)ret);
	return 0;
}

SEC("tp/syscalls/sys_enter_write")
int trace_write_enter(struct trace_event_raw_sys_enter *ctx)
{
	return trace_write_enter_common((int)ctx->args[0]);
}

SEC("tp/syscalls/sys_exit_write")
int trace_write_exit(struct trace_event_raw_sys_exit *ctx)
{
	return trace_write_exit_common(ctx->ret);
}

SEC("tp/syscalls/sys_enter_pwrite64")
int trace_pwrite64_enter(struct trace_event_raw_sys_enter *ctx)
{
	return trace_write_enter_common((int)ctx->args[0]);
}

SEC("tp/syscalls/sys_exit_pwrite64")
int trace_pwrite64_exit(struct trace_event_raw_sys_exit *ctx)
{
	return trace_write_exit_common(ctx->ret);
}

SEC("tp/syscalls/sys_enter_writev")
int trace_writev_enter(struct trace_event_raw_sys_enter *ctx)
{
	return trace_write_enter_common((int)ctx->args[0]);
}

SEC("tp/syscalls/sys_exit_writev")
int trace_writev_exit(struct trace_event_raw_sys_exit *ctx)
{
	return trace_write_exit_common(ctx->ret);
}

#endif /* __PROCESS_EXT_BPF_WRITE_H */
