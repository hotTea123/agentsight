/* SPDX-License-Identifier: GPL-2.0 OR BSD-3-Clause */
#ifndef __PROCESS_EXT_BPF_FS_H
#define __PROCESS_EXT_BPF_FS_H

/*
 * Filesystem mutation tracepoints.
 * Hook both old (unlink, mkdir, rename) and *at variants (unlinkat, mkdirat, renameat2)
 * because glibc/Python may use either depending on version.
 * Uncorrelated mutations retain their legacy aggregation. Correlated renames
 * are individual entry/exit records, including both paths and the real result.
 */

/* Extract parent directory prefix from a path (truncate at last '/') */
static __always_inline void extract_dir_prefix(const char *path, char *out, int out_len)
{
	int last_slash = 0;

	#pragma unroll
	for (int i = 0; i < DETAIL_LEN - 1; i++) {
		if (i >= out_len - 1)
			break;
		char c = path[i];
		if (c == '\0')
			break;
		if (c == '/')
			last_slash = i;
		out[i] = c;
	}

	if (last_slash > 0)
		out[last_slash] = '\0';
}

/* Shared helper: aggregate a file path event with extra info */
static __noinline int agg_path_event(const char *user_path, u32 event_type)
{
	if (!trace_fs_mutations)
		return 0;
	if (!is_event_tracked())
		return 0;

	char filepath[MAX_FILENAME_LEN];
	if (bpf_probe_read_user_str(filepath, sizeof(filepath), user_path) < 0)
		return 0;

	u32 zero = 0;
	struct agg_key *key = bpf_map_lookup_elem(&fs_agg_key_scratch, &zero);
	if (!key)
		return 0;
	__builtin_memset(key, 0, sizeof(*key));
	key->pid = bpf_get_current_pid_tgid() >> 32;
	key->event_type = event_type;
	key->process_start_ns = current_process_start_ns();
	extract_dir_prefix(filepath, key->detail, sizeof(key->detail));

	struct agg_value *val = bpf_map_lookup_elem(&event_agg_map, key);
	if (val) {
		__sync_fetch_and_add(&val->count, 1);
		val->last_ts = bpf_ktime_get_ns();
		bpf_get_current_comm(val->comm, sizeof(val->comm));
		bpf_probe_read_kernel_str(val->extra, sizeof(val->extra), filepath);
	} else {
		struct agg_value new_val = {};
		new_val.count = 1;
		new_val.first_ts = bpf_ktime_get_ns();
		new_val.last_ts = new_val.first_ts;
		bpf_get_current_comm(new_val.comm, sizeof(new_val.comm));
		bpf_probe_read_kernel_str(new_val.extra, sizeof(new_val.extra), filepath);

		if (bpf_map_update_elem(&event_agg_map, key, &new_val, BPF_NOEXIST) < 0) {
			u32 zero = 0;
			u64 *overflow = bpf_map_lookup_elem(&agg_overflow_count, &zero);
			if (overflow)
				__sync_fetch_and_add(overflow, 1);
		}
	}
	return 0;
}

/* --- unlink / unlinkat → FILE_DELETE --- */

SEC("tp/syscalls/sys_enter_unlinkat")
int trace_unlinkat(struct trace_event_raw_sys_enter *ctx)
{
	return agg_path_event((const char *)ctx->args[1], EVENT_TYPE_FILE_DELETE);
}

SEC("tp/syscalls/sys_enter_unlink")
int trace_unlink(struct trace_event_raw_sys_enter *ctx)
{
	return agg_path_event((const char *)ctx->args[0], EVENT_TYPE_FILE_DELETE);
}

static __always_inline int trace_rename_enter_common(
	const char *old_user_path, const char *new_user_path,
	int old_dirfd, int new_dirfd, unsigned int flags, unsigned char variant)
{
	if (!trace_fs_mutations)
		return 0;
	if (!is_scope_tracked() || (!is_event_tracked() && !has_python_tool_context()))
		return 0;

	u64 id = bpf_get_current_pid_tgid();
	bpf_map_delete_elem(&pending_file_renames, &id);
	u32 zero = 0;
	struct event *event = bpf_map_lookup_elem(&rename_event_scratch, &zero);
	if (!event)
		return 0;
	/* The upstream file-open union is now >4 KiB. Only initialize fields
	 * consumed by rename; large inline memset/memcpy is unsupported by BPF. */
	__builtin_memset(event, 0, __builtin_offsetof(struct event, file_rename));
	__builtin_memset(&event->file_rename, 0, sizeof(event->file_rename));
	event->exit_event = false;
	fill_python_async_context(event);
	if (event->correlation_source == ASYNC_CORRELATION_NONE)
		return agg_path_event(new_user_path, EVENT_TYPE_FILE_RENAME);

	event->type = EVENT_TYPE_FILE_RENAME_RESULT;
	event->pid = id >> 32;
	event->timestamp_ns = bpf_ktime_get_ns();
	bpf_get_current_comm(event->comm, sizeof(event->comm));
	event->file_rename.source_dirfd = old_dirfd;
	event->file_rename.destination_dirfd = new_dirfd;
	event->file_rename.flags = flags;
	event->file_rename.variant = variant;
	long old_len = bpf_probe_read_user_str(event->file_rename.source_path,
		sizeof(event->file_rename.source_path), old_user_path);
	long new_len = bpf_probe_read_user_str(event->file_rename.destination_path,
		sizeof(event->file_rename.destination_path), new_user_path);
	/* Fail open rather than export a truncated path as exact evidence. */
	if (old_len <= 0 || old_len >= MAX_FILENAME_LEN ||
	    new_len <= 0 || new_len >= MAX_FILENAME_LEN)
		return 0;
	if (bpf_map_update_elem(&pending_file_renames, &id, event, BPF_ANY) < 0) {
		u64 *overflow = bpf_map_lookup_elem(&agg_overflow_count, &zero);
		if (overflow)
			__sync_fetch_and_add(overflow, 1);
	}
	return 0;
}

static __always_inline int trace_rename_exit_common(long result)
{
	u64 id = bpf_get_current_pid_tgid();
	struct event *pending = bpf_map_lookup_elem(&pending_file_renames, &id);
	if (!pending)
		return 0;
	struct event *event = bpf_ringbuf_reserve(&rb, sizeof(*event), 0);
	if (event) {
		if (bpf_probe_read_kernel(event, sizeof(*event), pending) < 0) {
			bpf_ringbuf_discard(event, 0);
			bpf_map_delete_elem(&pending_file_renames, &id);
			return 0;
		}
		event->file_rename.result = result;
		event->duration_ns = bpf_ktime_get_ns() - event->timestamp_ns;
		bpf_ringbuf_submit(event, 0);
	}
	bpf_map_delete_elem(&pending_file_renames, &id);
	return 0;
}

/* --- rename / renameat / renameat2 → FILE_RENAME --- */

SEC("tp/syscalls/sys_enter_renameat2")
int trace_renameat2(struct trace_event_raw_sys_enter *ctx)
{
	return trace_rename_enter_common(
		(const char *)ctx->args[1], (const char *)ctx->args[3],
		(int)ctx->args[0], (int)ctx->args[2], (unsigned int)ctx->args[4], 2);
}

SEC("tp/syscalls/sys_enter_renameat")
int trace_renameat(struct trace_event_raw_sys_enter *ctx)
{
	return trace_rename_enter_common(
		(const char *)ctx->args[1], (const char *)ctx->args[3],
		(int)ctx->args[0], (int)ctx->args[2], 0, 1);
}

SEC("tp/syscalls/sys_enter_rename")
int trace_rename(struct trace_event_raw_sys_enter *ctx)
{
	return trace_rename_enter_common(
		(const char *)ctx->args[0], (const char *)ctx->args[1],
		-100, -100, 0, 0); /* AT_FDCWD */
}

SEC("tp/syscalls/sys_exit_rename")
int trace_rename_exit(struct trace_event_raw_sys_exit *ctx)
{
	return trace_rename_exit_common(ctx->ret);
}

SEC("tp/syscalls/sys_exit_renameat")
int trace_renameat_exit(struct trace_event_raw_sys_exit *ctx)
{
	return trace_rename_exit_common(ctx->ret);
}

SEC("tp/syscalls/sys_exit_renameat2")
int trace_renameat2_exit(struct trace_event_raw_sys_exit *ctx)
{
	return trace_rename_exit_common(ctx->ret);
}

/* --- mkdir / mkdirat → DIR_CREATE --- */

SEC("tp/syscalls/sys_enter_mkdirat")
int trace_mkdirat(struct trace_event_raw_sys_enter *ctx)
{
	return agg_path_event((const char *)ctx->args[1], EVENT_TYPE_DIR_CREATE);
}

SEC("tp/syscalls/sys_enter_mkdir")
int trace_mkdir(struct trace_event_raw_sys_enter *ctx)
{
	return agg_path_event((const char *)ctx->args[0], EVENT_TYPE_DIR_CREATE);
}

SEC("tp/syscalls/sys_enter_ftruncate")
int trace_ftruncate(struct trace_event_raw_sys_enter *ctx)
{
	if (!trace_fs_mutations)
		return 0;
	if (!is_event_tracked())
		return 0;

	int fd = (int)ctx->args[0];

	struct agg_key key = {};
	key.pid = bpf_get_current_pid_tgid() >> 32;
	key.event_type = EVENT_TYPE_FILE_TRUNCATE;
	format_fd_detail(key.detail, sizeof(key.detail), fd);

	update_agg_map(&key, 1, 0);
	return 0;
}

SEC("tp/syscalls/sys_enter_chdir")
int trace_chdir(struct trace_event_raw_sys_enter *ctx)
{
	if (!trace_fs_mutations)
		return 0;
	if (!is_event_tracked())
		return 0;

	char path[DETAIL_LEN];
	const char *user_path = (const char *)ctx->args[0];
	if (bpf_probe_read_user_str(path, sizeof(path), user_path) < 0)
		return 0;

	struct agg_key key = {};
	key.pid = bpf_get_current_pid_tgid() >> 32;
	key.event_type = EVENT_TYPE_CHDIR;
	bpf_probe_read_kernel_str(key.detail, sizeof(key.detail), path);

	update_agg_map(&key, 1, 0);
	return 0;
}

#endif /* __PROCESS_EXT_BPF_FS_H */
