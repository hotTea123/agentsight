/* SPDX-License-Identifier: MIT */
#include <stdint.h>

#if defined(__GNUC__)
#define AGENTSIGHT_MARKER_API __attribute__((visibility("default"), noinline, used))
#else
#define AGENTSIGHT_MARKER_API
#endif

AGENTSIGHT_MARKER_API uint32_t agentsight_tool_marker_abi_version(void)
{
	return 1;
}

AGENTSIGHT_MARKER_API int agentsight_tool_context_bind(
	uint64_t task_id,
	const unsigned char *trace_id,
	const unsigned char *span_id,
	uint64_t trace_flags)
{
	(void)task_id;
	(void)trace_id;
	(void)span_id;
	(void)trace_flags;
	return 0;
}

AGENTSIGHT_MARKER_API int agentsight_tool_context_unbind(
	uint64_t task_id,
	const unsigned char *span_id)
{
	(void)task_id;
	(void)span_id;
	return 0;
}
