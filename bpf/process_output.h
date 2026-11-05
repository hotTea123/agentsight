/* SPDX-License-Identifier: (LGPL-2.1 OR BSD-2-Clause) */
#ifndef AGENTSIGHT_PROCESS_OUTPUT_H
#define AGENTSIGHT_PROCESS_OUTPUT_H
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <stdbool.h>

/* Private, single-producer output adapter. Never redirects process stdio.
 * kind: 0 = complete JSON line, 1 = diagnostic, 2 = discarded record.
 * The receiver must copy data before returning and must not unwind into C. */
typedef void (*as_output_fn)(void *, int, const char *, size_t);
static as_output_fn as_output_sink;
static void *as_output_context;
static const char *as_procfs_root = "/proc";
static char as_output_buffer[65536];
static size_t as_output_length;
static bool as_output_truncated;

static inline void as_output_set_sink(as_output_fn sink, void *context)
{
	as_output_sink = sink;
	as_output_context = context;
	as_output_length = 0;
	as_output_truncated = false;
}

static inline int as_output_printf(const char *format, ...)
{
	va_list args;
	va_start(args, format);
	if (!as_output_sink) {
		int n = vprintf(format, args);
		va_end(args);
		return n;
	}
	size_t available = sizeof(as_output_buffer) - as_output_length;
	int n = vsnprintf(as_output_buffer + as_output_length, available, format, args);
	va_end(args);
	if (n < 0 || (size_t)n >= available)
		as_output_truncated = true;
	else
		as_output_length += (size_t)n;
	/* All serializers terminate each record with a literal newline. */
	if (strchr(format, '\n')) {
		if (as_output_truncated)
			as_output_sink(as_output_context, 2, "JSON record exceeds 65535 bytes", 31);
		else
			as_output_sink(as_output_context, 0, as_output_buffer, as_output_length);
		as_output_length = 0;
		as_output_truncated = false;
	}
	return n;
}

static inline void as_output_flush(void)
{
	if (!as_output_sink)
		fflush(stdout);
}

static inline int as_output_diagnostic(const char *format, ...)
{
	va_list args;
	va_start(args, format);
	int n;
	if (!as_output_sink) {
		n = vfprintf(stderr, format, args);
	} else {
		char message[2048];
		n = vsnprintf(message, sizeof(message), format, args);
		if (n >= 0)
			as_output_sink(as_output_context, 1, message, strnlen(message, sizeof(message)));
	}
	va_end(args);
	return n;
}
#endif
