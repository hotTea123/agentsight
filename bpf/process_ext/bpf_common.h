/* SPDX-License-Identifier: GPL-2.0 OR BSD-3-Clause */
#ifndef __PROCESS_EXT_BPF_COMMON_H
#define __PROCESS_EXT_BPF_COMMON_H

/*
 * Common BPF helpers for process extension modules: PID filtering and
 * aggregated map updates. Included by process.bpf.c before feature modules.
 * References maps and flags defined in the glue file.
 */

static __always_inline bool is_pid_tracked(void)
{
	if (!filter_pids)
		return true;  /* no filter mode: trace all */
	u32 pid = bpf_get_current_pid_tgid() >> 32;
	return bpf_map_lookup_elem(&tracked_pids, &pid) != NULL;
}

/* Kernel PID namespace nesting is capped at 32 levels (MAX_PID_NS_LEVEL). */
#define PIDNS_MAX_LEVEL 32

/*
 * True when the target PID namespace appears anywhere in the current task's
 * struct pid chain, i.e. when the task is visible from that namespace. That
 * covers the namespace itself and every namespace nested inside it (for
 * example a bubblewrap/unshare --pid sandbox started by the target).
 * bpf_get_ns_current_pid_tgid() is not usable here: it only succeeds when the
 * task's innermost namespace is exactly the target. Namespace inode numbers
 * are unique within nsfs, so the inode alone identifies the namespace.
 */
static __noinline bool task_visible_in_target_pidns(void)
{
	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct pid *thread_pid = BPF_CORE_READ(task, thread_pid);
	unsigned int level;

	if (!thread_pid)
		return false;
	level = BPF_CORE_READ(thread_pid, level);
	for (int i = 0; i < PIDNS_MAX_LEVEL; i++) {
		struct pid_namespace *ns;

		if (i > level)
			break;
		ns = BPF_CORE_READ(thread_pid, numbers[i].ns);
		if (ns && BPF_CORE_READ(ns, ns.inum) == target_pidns_ino)
			return true;
	}
	return false;
}

static __always_inline bool is_scope_tracked(void)
{
	if (target_pidns_ino && !task_visible_in_target_pidns())
		return false;
	if (!filter_cgroup)
		return true;
	u64 cgroup_id = bpf_get_current_cgroup_id();
	if (cgroup_id == target_cgroup_id)
		return true;
	if (!filter_cgroup_children)
		return false;
	return bpf_map_lookup_elem(&tracked_cgroups, &cgroup_id) != NULL;
}

static __always_inline bool is_event_tracked(void)
{
	return is_scope_tracked() && is_pid_tracked();
}

static __always_inline u64 current_process_start_ns(void)
{
	struct task_struct *task = (struct task_struct *)bpf_get_current_task();
	struct task_struct *leader = BPF_CORE_READ(task, group_leader);
	if (!leader)
		leader = task;
	if (bpf_core_field_exists(leader->start_boottime))
		return BPF_CORE_READ(leader, start_boottime);
	return BPF_CORE_READ(leader, start_time);
}

static __always_inline void update_agg_map(struct agg_key *key, u64 count, u64 bytes)
{
	key->process_start_ns = current_process_start_ns();
	struct agg_value *val = bpf_map_lookup_elem(&event_agg_map, key);
	if (val) {
		__sync_fetch_and_add(&val->count, count);
		if (bytes)
			__sync_fetch_and_add(&val->total_bytes, bytes);
		val->last_ts = bpf_ktime_get_ns();
		bpf_get_current_comm(val->comm, sizeof(val->comm));
	} else {
		u32 zero = 0;
		struct agg_value *new_val = bpf_map_lookup_elem(&agg_value_scratch, &zero);
		if (!new_val)
			return;
		__builtin_memset(new_val, 0, sizeof(*new_val));
		new_val->count = count;
		new_val->total_bytes = bytes;
		new_val->first_ts = bpf_ktime_get_ns();
		new_val->last_ts = new_val->first_ts;
		bpf_get_current_comm(new_val->comm, sizeof(new_val->comm));

		if (bpf_map_update_elem(&event_agg_map, key, new_val, BPF_NOEXIST) < 0) {
			/* map full: bump overflow counter */
			u32 zero = 0;
			u64 *overflow = bpf_map_lookup_elem(&agg_overflow_count, &zero);
			if (overflow)
				__sync_fetch_and_add(overflow, 1);
		}
	}
}

static __noinline void update_correlated_file_agg_map(
	struct correlated_file_key *key,
	u64 count,
	u64 bytes,
	const char *extra_user_path)
{
	struct agg_value *val = bpf_map_lookup_elem(&correlated_file_agg_map, key);
	if (val) {
		__sync_fetch_and_add(&val->count, count);
		if (bytes)
			__sync_fetch_and_add(&val->total_bytes, bytes);
		val->last_ts = bpf_ktime_get_ns();
		bpf_get_current_comm(val->comm, sizeof(val->comm));
		if (extra_user_path)
			bpf_probe_read_user_str(val->extra, sizeof(val->extra), extra_user_path);
	} else {
		u32 zero = 0;
		struct agg_value *new_val = bpf_map_lookup_elem(&agg_value_scratch, &zero);
		if (!new_val)
			return;
		__builtin_memset(new_val, 0, sizeof(*new_val));
		new_val->count = count;
		new_val->total_bytes = bytes;
		new_val->first_ts = bpf_ktime_get_ns();
		new_val->last_ts = new_val->first_ts;
		bpf_get_current_comm(new_val->comm, sizeof(new_val->comm));
		if (extra_user_path &&
		    bpf_probe_read_user_str(
			new_val->extra,
			sizeof(new_val->extra),
			extra_user_path) < 0)
			return;
		if (bpf_map_update_elem(
			&correlated_file_agg_map,
			key,
			new_val,
			BPF_NOEXIST) < 0) {
			u32 zero = 0;
			u64 *overflow = bpf_map_lookup_elem(&agg_overflow_count, &zero);
			if (overflow)
				__sync_fetch_and_add(overflow, 1);
		}
	}
}

/* Format "fd=N" into a detail buffer without bpf_snprintf */
static __always_inline void format_fd_detail(char *buf, int buf_len, int fd)
{
	/* Keep LLVM from folding stack pointer arithmetic into bitwise OR. */
	barrier_var(buf);
	/* Constant offsets keep older BPF verifiers away from variable stack
	 * pointer arithmetic. File descriptors passed by these syscalls are
	 * non-negative; retain a deterministic marker for invalid values. */
	if (buf_len < 14)
		return;
	buf[0] = 'f';
	buf[1] = 'd';
	buf[2] = '=';
	if (fd < 0) {
		buf[3] = '-';
		buf[4] = '1';
		buf[5] = '\0';
		return;
	}

	unsigned int value = (unsigned int)fd;
	if (value >= 1000000000U) {
		buf[3] = '0' + (value / 1000000000U) % 10;
		buf[4] = '0' + (value / 100000000U) % 10;
		buf[5] = '0' + (value / 10000000U) % 10;
		buf[6] = '0' + (value / 1000000U) % 10;
		buf[7] = '0' + (value / 100000U) % 10;
		buf[8] = '0' + (value / 10000U) % 10;
		buf[9] = '0' + (value / 1000U) % 10;
		buf[10] = '0' + (value / 100U) % 10;
		buf[11] = '0' + (value / 10U) % 10;
		buf[12] = '0' + value % 10;
		buf[13] = '\0';
	} else if (value >= 100000000U) {
		buf[3] = '0' + value / 100000000U;
		buf[4] = '0' + (value / 10000000U) % 10;
		buf[5] = '0' + (value / 1000000U) % 10;
		buf[6] = '0' + (value / 100000U) % 10;
		buf[7] = '0' + (value / 10000U) % 10;
		buf[8] = '0' + (value / 1000U) % 10;
		buf[9] = '0' + (value / 100U) % 10;
		buf[10] = '0' + (value / 10U) % 10;
		buf[11] = '0' + value % 10;
		buf[12] = '\0';
	} else if (value >= 10000000U) {
		buf[3] = '0' + value / 10000000U;
		buf[4] = '0' + (value / 1000000U) % 10;
		buf[5] = '0' + (value / 100000U) % 10;
		buf[6] = '0' + (value / 10000U) % 10;
		buf[7] = '0' + (value / 1000U) % 10;
		buf[8] = '0' + (value / 100U) % 10;
		buf[9] = '0' + (value / 10U) % 10;
		buf[10] = '0' + value % 10;
		buf[11] = '\0';
	} else if (value >= 1000000U) {
		buf[3] = '0' + value / 1000000U;
		buf[4] = '0' + (value / 100000U) % 10;
		buf[5] = '0' + (value / 10000U) % 10;
		buf[6] = '0' + (value / 1000U) % 10;
		buf[7] = '0' + (value / 100U) % 10;
		buf[8] = '0' + (value / 10U) % 10;
		buf[9] = '0' + value % 10;
		buf[10] = '\0';
	} else if (value >= 100000U) {
		buf[3] = '0' + value / 100000U;
		buf[4] = '0' + (value / 10000U) % 10;
		buf[5] = '0' + (value / 1000U) % 10;
		buf[6] = '0' + (value / 100U) % 10;
		buf[7] = '0' + (value / 10U) % 10;
		buf[8] = '0' + value % 10;
		buf[9] = '\0';
	} else if (value >= 10000U) {
		buf[3] = '0' + value / 10000U;
		buf[4] = '0' + (value / 1000U) % 10;
		buf[5] = '0' + (value / 100U) % 10;
		buf[6] = '0' + (value / 10U) % 10;
		buf[7] = '0' + value % 10;
		buf[8] = '\0';
	} else if (value >= 1000U) {
		buf[3] = '0' + value / 1000U;
		buf[4] = '0' + (value / 100U) % 10;
		buf[5] = '0' + (value / 10U) % 10;
		buf[6] = '0' + value % 10;
		buf[7] = '\0';
	} else if (value >= 100U) {
		buf[3] = '0' + value / 100U;
		buf[4] = '0' + (value / 10U) % 10;
		buf[5] = '0' + value % 10;
		buf[6] = '\0';
	} else if (value >= 10U) {
		buf[3] = '0' + value / 10U;
		buf[4] = '0' + value % 10;
		buf[5] = '\0';
	} else {
		buf[3] = '0' + value;
		buf[4] = '\0';
	}
}

/* Format "N.N.N.N:PORT" for IPv4 addresses without bpf_snprintf */
/* Fully unrolled for BPF verifier (no loops / back-edges) */
static __always_inline void write_octet(char *buf, int buf_len, int *pos, u8 val)
{
	/* Keep LLVM from folding stack pointer arithmetic into bitwise OR. */
	barrier_var(buf);
	if (val >= 100 && *pos < buf_len - 1) buf[(*pos)++] = '0' + val / 100;
	if (val >= 10  && *pos < buf_len - 1) buf[(*pos)++] = '0' + (val / 10) % 10;
	if (*pos < buf_len - 1)               buf[(*pos)++] = '0' + val % 10;
}

static __always_inline void format_ipv4_port(char *buf, int buf_len, u32 ip, u16 port)
{
	/* Keep LLVM from folding stack pointer arithmetic into bitwise OR. */
	barrier_var(buf);
	int pos = 0;
	u8 o0 = ip & 0xFF;
	u8 o1 = (ip >> 8) & 0xFF;
	u8 o2 = (ip >> 16) & 0xFF;
	u8 o3 = (ip >> 24) & 0xFF;

	/* Octet 0 */
	write_octet(buf, buf_len, &pos, o0);
	/* Octet 1 */
	if (pos < buf_len - 1) buf[pos++] = '.';
	write_octet(buf, buf_len, &pos, o1);
	/* Octet 2 */
	if (pos < buf_len - 1) buf[pos++] = '.';
	write_octet(buf, buf_len, &pos, o2);
	/* Octet 3 */
	if (pos < buf_len - 1) buf[pos++] = '.';
	write_octet(buf, buf_len, &pos, o3);

	/* :PORT — max 5 digits, write manually */
	if (pos < buf_len - 1) buf[pos++] = ':';
	unsigned int p = port;
	/* Extract each digit (max 65535 = 5 digits) */
	char d4 = '0' + (p / 10000) % 10;
	char d3 = '0' + (p / 1000) % 10;
	char d2 = '0' + (p / 100) % 10;
	char d1 = '0' + (p / 10) % 10;
	char d0 = '0' + p % 10;
	if (p >= 10000 && pos < buf_len - 1) buf[pos++] = d4;
	if (p >= 1000  && pos < buf_len - 1) buf[pos++] = d3;
	if (p >= 100   && pos < buf_len - 1) buf[pos++] = d2;
	if (p >= 10    && pos < buf_len - 1) buf[pos++] = d1;
	if (pos < buf_len - 1) buf[pos++] = d0;

	buf[pos] = '\0';
}

#endif /* __PROCESS_EXT_BPF_COMMON_H */
