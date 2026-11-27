/* SPDX-License-Identifier: GPL-2.0 OR BSD-3-Clause */
#ifndef __PROCESS_EXT_PYTHON_ASYNC_USERSPACE_H
#define __PROCESS_EXT_PYTHON_ASYNC_USERSPACE_H

#include "process_output.h"

static const char *python_async_source_name(unsigned char source)
{
	switch (source) {
	case ASYNC_CORRELATION_TASK:
		return "task";
	case ASYNC_CORRELATION_TASK_PARENT:
		return "task_parent";
	case ASYNC_CORRELATION_PY_CONTEXT:
		return "py_context";
	case ASYNC_CORRELATION_PROCESS:
		return "process";
	default:
		return "none";
	}
}

static void bytes_to_hex(const unsigned char *bytes, size_t length, char *output)
{
	static const char digits[] = "0123456789abcdef";

	for (size_t i = 0; i < length; i++) {
		output[i * 2] = digits[bytes[i] >> 4];
		output[i * 2 + 1] = digits[bytes[i] & 0xf];
	}
	output[length * 2] = '\0';
}

static void print_python_async_context(const struct event *event)
{
	if (!event || event->correlation_source == ASYNC_CORRELATION_NONE)
		return;
	char trace_id[TRACE_ID_SIZE * 2 + 1];
	char span_id[SPAN_ID_SIZE * 2 + 1];
	bytes_to_hex(event->tool_context.trace_id, TRACE_ID_SIZE, trace_id);
	bytes_to_hex(event->tool_context.span_id, SPAN_ID_SIZE, span_id);
	as_output_printf(",\"tid\":%u", event->tid);
	as_output_printf(",\"agentsight_trace_id\":\"%s\"", trace_id);
	as_output_printf(",\"agentsight_parent_span_id\":\"%s\"", span_id);
	as_output_printf(",\"agentsight_trace_flags\":%u", event->tool_context.trace_flags);
	as_output_printf(",\"agentsight_task_id\":\"%016llx\"",
	       (unsigned long long)event->task_id);
	as_output_printf(",\"agentsight_bound_task_id\":\"%016llx\"",
	       (unsigned long long)event->bound_task_id);
	as_output_printf(",\"agentsight_correlation_source\":\"%s\"",
	       python_async_source_name(event->correlation_source));
}

#endif /* __PROCESS_EXT_PYTHON_ASYNC_USERSPACE_H */
