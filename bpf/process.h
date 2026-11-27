/* SPDX-License-Identifier: (LGPL-2.1 OR BSD-2-Clause) */
/* Copyright (c) 2020 Facebook */
#ifndef __PROCESS_H
#define __PROCESS_H

#define TASK_COMM_LEN 16
#define MAX_FILENAME_LEN 127
#define MAX_FILE_PATH_LEN 4096
#define MAX_COMMAND_FILTERS 10
#define MAX_TRACKED_PIDS 4096
#define MAX_COMMAND_LEN 256
#define TRACE_ID_SIZE 16
#define SPAN_ID_SIZE 8
#define MAX_TOOL_BINDING_DEPTH 8

enum async_correlation_source {
	ASYNC_CORRELATION_NONE = 0,
	ASYNC_CORRELATION_TASK = 1,
	ASYNC_CORRELATION_TASK_PARENT = 2,
	ASYNC_CORRELATION_PY_CONTEXT = 3,
	ASYNC_CORRELATION_PROCESS = 4,
};

struct python_task_key {
	unsigned int tgid;
	unsigned int reserved;
	unsigned long long task_id;
};

struct tool_span_context {
	unsigned char trace_id[TRACE_ID_SIZE];
	unsigned char span_id[SPAN_ID_SIZE];
	unsigned char trace_flags;
	unsigned char reserved[7];
};

struct tool_span_stack {
	struct tool_span_context contexts[MAX_TOOL_BINDING_DEPTH];
	unsigned int depth;
	unsigned int reserved;
};

enum filter_mode {
	FILTER_MODE_ALL = 0,      /* Trace all processes and all read/write operations */
	FILTER_MODE_PROC = 1,     /* Trace all processes but only read/write for tracked PIDs */
	FILTER_MODE_FILTER = 2,   /* Only trace processes matching filters and their read/write */
};

enum event_type {
	EVENT_TYPE_PROCESS = 0,
	EVENT_TYPE_BASH_READLINE = 1,
	EVENT_TYPE_FILE_OPERATION = 2,
	EVENT_TYPE_FILE_RENAME_RESULT = 3,
};

struct event {
	enum event_type type;
	int pid;
	int tid;
	int ppid;
	unsigned exit_code;
	unsigned long long duration_ns;
	unsigned long long timestamp_ns;
	char comm[TASK_COMM_LEN];
	char full_command[MAX_COMMAND_LEN];     /* full command line with args */
	unsigned long long task_id;
	unsigned long long bound_task_id;
	struct tool_span_context tool_context;
	unsigned char correlation_source;
	union {
		char filename[MAX_FILENAME_LEN];     /* for process events */
		char command[MAX_COMMAND_LEN];       /* for bash readline events */
		struct {                             /* for file operation events */
			char filepath[MAX_FILE_PATH_LEN];
			int fd;
			int flags;
			bool is_open;  /* true for open/openat, false for close */
			bool resolved; /* security_file_open, not a syscall entry */
			bool layer;
			unsigned char access; /* FILE_ACCESS_* */
			int path_error;
			unsigned int dev;
			unsigned long long ino;
		} file_op;
		struct {
			char source_path[MAX_FILENAME_LEN];
			char destination_path[MAX_FILENAME_LEN];
			long long result;
			int source_dirfd;
			int destination_dirfd;
			unsigned int flags;
			unsigned char variant; /* 0: rename, 1: renameat, 2: renameat2 */
		} file_rename;
	};
	bool exit_event;
};

#define FILE_ACCESS_READ  1
#define FILE_ACCESS_WRITE 2
#define FILE_ACCESS_EXEC  4

struct command_filter {
	char comm[TASK_COMM_LEN];
};

struct pid_info {
	pid_t pid;
	pid_t ppid;
	bool is_tracked;
};

#endif /* __PROCESS_H */
