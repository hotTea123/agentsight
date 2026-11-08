/* SPDX-License-Identifier: GPL-2.0 OR BSD-3-Clause */
#ifndef __PROCESS_EXT_PYTHON_ASYNC_CONTEXT_H
#define __PROCESS_EXT_PYTHON_ASYNC_CONTEXT_H

#define MAX_PYTHON_TASKS 8192
#define MAX_TASK_PARENT_DEPTH 8

static __always_inline void count_python_ring_drop(void)
{
#ifndef AGENTSIGHT_TEST_CAPTURE
	if (!trace_python_async_context)
		return;
#endif
	u32 zero = 0;
	u64 *count = bpf_map_lookup_elem(&file_open_dropped, &zero);
	if (count)
		(*count)++;
}

/* Sticky invalidation survives a dropped EXEC/EXIT ring-buffer record. */
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, u32);
} python_target_changed SEC(".maps");

static __always_inline void invalidate_python_target(u32 pid)
{
	if (!trace_python_async_context || !python_target_pid || pid != python_target_pid)
		return;
	u32 zero = 0;
	u32 *changed = bpf_map_lookup_elem(&python_target_changed, &zero);
	if (changed)
		*changed = 1;
}

static __always_inline bool python_context_enabled(void)
{
	if (!trace_python_async_context)
		return false;
	u32 zero = 0;
	u32 *changed = bpf_map_lookup_elem(&python_target_changed, &zero);
	return changed && !*changed;
}

struct python_current_task {
	u64 task_id;
	u8 source;
	u8 reserved[7];
};

struct python_context_key {
	u32 tgid;
	u32 reserved;
	u64 context_id;
};

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, MAX_PYTHON_TASKS);
	__type(key, struct python_task_key);
	__type(value, struct tool_span_stack);
} python_task_spans SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, MAX_PYTHON_TASKS);
	__type(key, u64);
	__type(value, struct python_current_task);
} python_current_tasks SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_LRU_HASH);
	__uint(max_entries, MAX_PYTHON_TASKS);
	__type(key, struct python_task_key);
	__type(value, u64);
} python_task_parents SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, MAX_PYTHON_TASKS);
	__type(key, u64);
	__type(value, struct python_current_task);
} python_copy_context_sources SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_LRU_HASH);
	__uint(max_entries, MAX_PYTHON_TASKS);
	__type(key, struct python_context_key);
	__type(value, u64);
} python_context_tasks SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, MAX_PYTHON_TASKS);
	__type(key, u64);
	__type(value, struct python_current_task);
} python_context_run_previous SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_LRU_HASH);
	__uint(max_entries, MAX_TRACKED_PIDS);
	__type(key, u32);
	__type(value, struct tool_span_context);
} python_process_spans SEC(".maps");

static __always_inline bool resolve_python_task_span(
	u32 tgid,
	u64 task_id,
	struct tool_span_context *context,
	u64 *bound_task_id,
	u8 *source)
{
	u64 candidate = task_id;

#pragma unroll
	for (int depth = 0; depth < MAX_TASK_PARENT_DEPTH; depth++) {
		struct python_task_key key = {
			.tgid = tgid,
			.task_id = candidate,
		};
			struct tool_span_stack *found = bpf_map_lookup_elem(&python_task_spans, &key);
			if (found && found->depth > 0 && found->depth <= MAX_TOOL_BINDING_DEPTH) {
				unsigned int top = (found->depth - 1) & (MAX_TOOL_BINDING_DEPTH - 1);
				bpf_probe_read_kernel(context, sizeof(*context), &found->contexts[top]);
			*bound_task_id = candidate;
			if (depth > 0 && *source == ASYNC_CORRELATION_TASK)
				*source = ASYNC_CORRELATION_TASK_PARENT;
			return true;
		}
		u64 *parent = bpf_map_lookup_elem(&python_task_parents, &key);
		if (!parent || !*parent)
			break;
		candidate = *parent;
	}
	return false;
}

static __always_inline bool current_python_span(
	struct tool_span_context *context,
	u64 *task_id,
	u64 *bound_task_id,
	u8 *source)
{
	if (!python_context_enabled())
		return false;
	u64 pid_tgid = bpf_get_current_pid_tgid();
	u32 tgid = pid_tgid >> 32;
	struct python_current_task *current =
		bpf_map_lookup_elem(&python_current_tasks, &pid_tgid);

	if (current) {
		u8 resolved_source = current->source;
		*task_id = current->task_id;
		if (resolve_python_task_span(
			tgid,
			current->task_id,
			context,
			bound_task_id,
			&resolved_source)) {
			*source = resolved_source;
			return true;
		}
	}

	struct tool_span_context *process_context =
		bpf_map_lookup_elem(&python_process_spans, &tgid);
	if (!process_context)
		return false;
	bpf_probe_read_kernel(context, sizeof(*context), process_context);
	*source = ASYNC_CORRELATION_PROCESS;
	return true;
}

/* A child created by a correlated Python tool can execute and exit before
 * userspace consumes its EXEC event and adds the PID to tracked_pids.  The
 * inherited process context is already an exact trace/span association, so it
 * is a stronger admission signal than the eventually-consistent PID filter.
 */
static __always_inline bool has_python_tool_context(void)
{
	struct tool_span_context context = {};
	u64 task_id = 0;
	u64 bound_task_id = 0;
	u8 source = ASYNC_CORRELATION_NONE;

	if (!trace_python_async_context)
		return false;
	return current_python_span(&context, &task_id, &bound_task_id, &source);
}

static __always_inline void fill_python_async_context(struct event *event)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	event->tid = (u32)pid_tgid;
	event->task_id = 0;
	event->bound_task_id = 0;
	event->correlation_source = ASYNC_CORRELATION_NONE;
	__builtin_memset(&event->tool_context, 0, sizeof(event->tool_context));

	if (!trace_python_async_context)
		return;
	current_python_span(
		&event->tool_context,
		&event->task_id,
		&event->bound_task_id,
		&event->correlation_source);
}

static __noinline bool fill_python_correlated_file_context(
	struct correlated_file_key *key)
{
	key->tid = (u32)bpf_get_current_pid_tgid();
	key->task_id = 0;
	key->bound_task_id = 0;
	key->correlation_source = ASYNC_CORRELATION_NONE;
	__builtin_memset(&key->tool_context, 0, sizeof(key->tool_context));

	if (!trace_python_async_context)
		return false;
	return current_python_span(
		&key->tool_context,
		&key->task_id,
		&key->bound_task_id,
		&key->correlation_source);
}

static __always_inline void propagate_python_context_to_child(u32 child_pid)
{
	struct tool_span_context context = {};
	u64 task_id = 0;
	u64 bound_task_id = 0;
	u8 source = ASYNC_CORRELATION_NONE;
	bpf_map_delete_elem(&python_process_spans, &child_pid);

	if (!trace_python_async_context)
		return;
	if (current_python_span(&context, &task_id, &bound_task_id, &source))
		bpf_map_update_elem(&python_process_spans, &child_pid, &context, BPF_ANY);
}

static __always_inline bool marker_span_id_matches(
	const unsigned char left[SPAN_ID_SIZE],
	const unsigned char right[SPAN_ID_SIZE])
{
#pragma unroll
	for (int i = 0; i < SPAN_ID_SIZE; i++) {
		if (left[i] != right[i])
			return false;
	}
	return true;
}

SEC("uprobe")
int BPF_UPROBE(
	python_tool_context_bind,
	u64 task_id,
	const unsigned char *trace_id,
	const unsigned char *span_id,
	u64 trace_flags)
{
	if (!trace_python_async_context || !task_id || !trace_id || !span_id)
		return 0;

	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_task_key key = {
		.tgid = pid_tgid >> 32,
		.task_id = task_id,
	};
	struct tool_span_context context = {
		.trace_flags = (u8)trace_flags,
	};
	if (bpf_probe_read_user(context.trace_id, TRACE_ID_SIZE, trace_id) < 0 ||
	    bpf_probe_read_user(context.span_id, SPAN_ID_SIZE, span_id) < 0)
		return 0;

	struct tool_span_stack updated = {};
	struct tool_span_stack *existing =
		bpf_map_lookup_elem(&python_task_spans, &key);
	if (existing)
		bpf_probe_read_kernel(&updated, sizeof(updated), existing);
	if (updated.depth >= MAX_TOOL_BINDING_DEPTH)
		return 0;
	unsigned int next = updated.depth & (MAX_TOOL_BINDING_DEPTH - 1);
	updated.contexts[next] = context;
	updated.depth++;
	bpf_map_update_elem(&python_task_spans, &key, &updated, BPF_ANY);
	return 0;
}

SEC("uprobe")
int BPF_UPROBE(
	python_tool_context_unbind,
	u64 task_id,
	const unsigned char *span_id)
{
	if (!trace_python_async_context || !task_id || !span_id)
		return 0;

	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_task_key key = {
		.tgid = pid_tgid >> 32,
		.task_id = task_id,
	};
	struct tool_span_stack *existing =
		bpf_map_lookup_elem(&python_task_spans, &key);
	if (!existing || existing->depth == 0 ||
	    existing->depth > MAX_TOOL_BINDING_DEPTH)
		return 0;

	struct tool_span_stack updated = {};
	unsigned char requested_span_id[SPAN_ID_SIZE] = {};
	if (bpf_probe_read_kernel(&updated, sizeof(updated), existing) < 0 ||
	    bpf_probe_read_user(requested_span_id, SPAN_ID_SIZE, span_id) < 0)
		return 0;
	/* The verifier cannot carry the map-value range proof across the helper
	 * copy into stack memory. Re-prove the bound on the copied value before
	 * using it as an array index.
	 */
	if (updated.depth == 0 || updated.depth > MAX_TOOL_BINDING_DEPTH)
		return 0;
	unsigned int top = (updated.depth - 1) & (MAX_TOOL_BINDING_DEPTH - 1);
	if (!marker_span_id_matches(updated.contexts[top].span_id, requested_span_id))
		return 0;
	updated.depth = top;
	if (updated.depth == 0)
		bpf_map_delete_elem(&python_task_spans, &key);
	else
		bpf_map_update_elem(&python_task_spans, &key, &updated, BPF_ANY);
	return 0;
}

SEC("uprobe")
int BPF_UPROBE(python_task_step_enter, void *task)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_current_task current = {
		.task_id = (u64)task,
		.source = ASYNC_CORRELATION_TASK,
	};
	bpf_map_update_elem(&python_current_tasks, &pid_tgid, &current, BPF_ANY);
	return 0;
}

SEC("uretprobe")
int BPF_URETPROBE(python_task_step_exit)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	bpf_map_delete_elem(&python_current_tasks, &pid_tgid);
	return 0;
}

SEC("uprobe")
int BPF_UPROBE(python_task_init_enter, void *task)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_current_task *current =
		bpf_map_lookup_elem(&python_current_tasks, &pid_tgid);
	if (!current || !current->task_id)
		return 0;
	struct python_task_key child = {
		.tgid = pid_tgid >> 32,
		.task_id = (u64)task,
	};
	bpf_map_update_elem(&python_task_parents, &child, &current->task_id, BPF_ANY);
	return 0;
}

SEC("uprobe")
int BPF_UPROBE(python_copy_context_enter)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_current_task *current =
		bpf_map_lookup_elem(&python_current_tasks, &pid_tgid);
	if (current)
		bpf_map_update_elem(&python_copy_context_sources, &pid_tgid, current, BPF_ANY);
	return 0;
}

SEC("uretprobe")
int BPF_URETPROBE(python_copy_context_exit, void *context)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_current_task *source =
		bpf_map_lookup_elem(&python_copy_context_sources, &pid_tgid);
	if (source && context) {
		struct python_context_key key = {
			.tgid = pid_tgid >> 32,
			.context_id = (u64)context,
		};
		bpf_map_update_elem(&python_context_tasks, &key, &source->task_id, BPF_ANY);
	}
	bpf_map_delete_elem(&python_copy_context_sources, &pid_tgid);
	return 0;
}

SEC("uprobe")
int BPF_UPROBE(python_context_run_enter, void *context)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_context_key key = {
		.tgid = pid_tgid >> 32,
		.context_id = (u64)context,
	};
	u64 *source_task = bpf_map_lookup_elem(&python_context_tasks, &key);
	if (!source_task)
		return 0;
	struct python_current_task *previous =
		bpf_map_lookup_elem(&python_current_tasks, &pid_tgid);
	if (previous)
		bpf_map_update_elem(&python_context_run_previous, &pid_tgid, previous, BPF_ANY);
	else
		bpf_map_delete_elem(&python_context_run_previous, &pid_tgid);
	struct python_current_task current = {
		.task_id = *source_task,
		.source = ASYNC_CORRELATION_PY_CONTEXT,
	};
	bpf_map_update_elem(&python_current_tasks, &pid_tgid, &current, BPF_ANY);
	return 0;
}

SEC("uretprobe")
int BPF_URETPROBE(python_context_run_exit)
{
	u64 pid_tgid = bpf_get_current_pid_tgid();
	struct python_current_task *previous =
		bpf_map_lookup_elem(&python_context_run_previous, &pid_tgid);
	if (previous)
		bpf_map_update_elem(&python_current_tasks, &pid_tgid, previous, BPF_ANY);
	else
		bpf_map_delete_elem(&python_current_tasks, &pid_tgid);
	bpf_map_delete_elem(&python_context_run_previous, &pid_tgid);
	return 0;
}

#endif /* __PROCESS_EXT_PYTHON_ASYNC_CONTEXT_H */
