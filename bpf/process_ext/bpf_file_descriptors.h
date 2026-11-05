/* SPDX-License-Identifier: GPL-2.0 OR BSD-3-Clause */
#ifndef __PROCESS_EXT_BPF_FILE_DESCRIPTORS_H
#define __PROCESS_EXT_BPF_FILE_DESCRIPTORS_H

static __always_inline void track_file_open_enter(const char *user_path, int flags)
{
	if (!trace_fs_mutations)
		return;
	if (!is_scope_tracked() || (!is_event_tracked() && !has_python_tool_context()))
		return;

	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct pending_file_open pending = { .flags = flags };
	if (bpf_probe_read_user_str(
		pending.filepath,
		sizeof(pending.filepath),
		user_path) < 0)
		return;
	bpf_map_update_elem(&pending_file_opens, &pid_tgid, &pending, BPF_ANY);
}

static __always_inline int track_file_open_exit(long fd)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct pending_file_open *pending =
		bpf_map_lookup_elem(&pending_file_opens, &pid_tgid);
	if (!pending)
		return 0;

	if (fd >= 0) {
		struct file_descriptor_key key = {
			.tgid = pid_tgid >> 32,
			.fd = (int)fd,
		};
		struct tracked_file_descriptor tracked = { .flags = pending->flags };
		bpf_probe_read_kernel_str(
			tracked.filepath,
			sizeof(tracked.filepath),
			pending->filepath);
		bpf_map_update_elem(&tracked_file_descriptors, &key, &tracked, BPF_ANY);
	}
	bpf_map_delete_elem(&pending_file_opens, &pid_tgid);
	return 0;
}

/*
 * Shell redirection commonly opens the destination on a temporary descriptor
 * and then uses dup2()/dup3() to make it stdout or stderr.  Preserve the path
 * across that descriptor copy so the later write(1/2, ...) can still be
 * attributed to the file.  fixed_newfd is -1 for dup(), whose destination is
 * only known from the syscall return value.
 */
static __always_inline void track_file_dup_enter(int oldfd, int fixed_newfd)
{
	if (!trace_fs_mutations)
		return;

	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct file_descriptor_key source_key = {
		.tgid = pid_tgid >> 32,
		.fd = oldfd,
	};
	struct tracked_file_descriptor *source =
		bpf_map_lookup_elem(&tracked_file_descriptors, &source_key);
	bool destination_tracked = false;
	if (fixed_newfd >= 0) {
		struct file_descriptor_key destination_key = {
			.tgid = pid_tgid >> 32,
			.fd = fixed_newfd,
		};
		destination_tracked =
			bpf_map_lookup_elem(&tracked_file_descriptors, &destination_key) != NULL;
	}
	if (!source && !destination_tracked)
		return;

	struct pending_file_dup pending = {};
	if (source) {
		pending.source_tracked = 1;
		pending.tracked.flags = source->flags;
		bpf_probe_read_kernel_str(
			pending.tracked.filepath,
			sizeof(pending.tracked.filepath),
			source->filepath);
	}
	bpf_map_update_elem(&pending_file_dups, &pid_tgid, &pending, BPF_ANY);
}

static __always_inline int track_file_dup_exit(long newfd)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct pending_file_dup *pending =
		bpf_map_lookup_elem(&pending_file_dups, &pid_tgid);
	if (!pending)
		return 0;

	if (newfd >= 0) {
		struct file_descriptor_key destination_key = {
			.tgid = pid_tgid >> 32,
			.fd = (int)newfd,
		};
		if (pending->source_tracked)
			bpf_map_update_elem(
				&tracked_file_descriptors,
				&destination_key,
				&pending->tracked,
				BPF_ANY);
		else
			bpf_map_delete_elem(&tracked_file_descriptors, &destination_key);
	}
	bpf_map_delete_elem(&pending_file_dups, &pid_tgid);
	return 0;
}

SEC("tp/syscalls/sys_exit_openat")
int trace_openat_exit(struct trace_event_raw_sys_exit *ctx)
{
	return track_file_open_exit(ctx->ret);
}

SEC("tp/syscalls/sys_exit_open")
int trace_open_exit(struct trace_event_raw_sys_exit *ctx)
{
	return track_file_open_exit(ctx->ret);
}

SEC("tp/syscalls/sys_enter_dup")
int trace_dup_enter(struct trace_event_raw_sys_enter *ctx)
{
	track_file_dup_enter((int)ctx->args[0], -1);
	return 0;
}

SEC("tp/syscalls/sys_exit_dup")
int trace_dup_exit(struct trace_event_raw_sys_exit *ctx)
{
	return track_file_dup_exit(ctx->ret);
}

SEC("tp/syscalls/sys_enter_dup2")
int trace_dup2_enter(struct trace_event_raw_sys_enter *ctx)
{
	track_file_dup_enter((int)ctx->args[0], (int)ctx->args[1]);
	return 0;
}

SEC("tp/syscalls/sys_exit_dup2")
int trace_dup2_exit(struct trace_event_raw_sys_exit *ctx)
{
	return track_file_dup_exit(ctx->ret);
}

SEC("tp/syscalls/sys_enter_dup3")
int trace_dup3_enter(struct trace_event_raw_sys_enter *ctx)
{
	track_file_dup_enter((int)ctx->args[0], (int)ctx->args[1]);
	return 0;
}

SEC("tp/syscalls/sys_exit_dup3")
int trace_dup3_exit(struct trace_event_raw_sys_exit *ctx)
{
	return track_file_dup_exit(ctx->ret);
}

SEC("tp/syscalls/sys_enter_close")
int trace_close(struct trace_event_raw_sys_enter *ctx)
{
	if (!trace_fs_mutations)
		return 0;
	struct file_descriptor_key key = {
		.tgid = bpf_get_current_pid_tgid() >> 32,
		.fd = (int)ctx->args[0],
	};
	bpf_map_delete_elem(&tracked_file_descriptors, &key);
	return 0;
}

#endif /* __PROCESS_EXT_BPF_FILE_DESCRIPTORS_H */
