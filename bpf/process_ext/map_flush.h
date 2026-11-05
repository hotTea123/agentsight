/* SPDX-License-Identifier: (LGPL-2.1 OR BSD-2-Clause) */
#ifndef __PROCESS_EXT_MAP_FLUSH_H
#define __PROCESS_EXT_MAP_FLUSH_H

#include "process_output.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <stdbool.h>
#include <limits.h>
#include <bpf/libbpf.h>
#include <bpf/bpf.h>
#include "process_ext/types.h"
#include "jsonl.h"
#include "process_ext/python_async_userspace.h"

static bool print_file_rename_json(const struct event *event)
{
	if (event->type != EVENT_TYPE_FILE_RENAME_RESULT ||
	    event->correlation_source == ASYNC_CORRELATION_NONE)
		return false;
	char comm[TASK_COMM_LEN * 2 + 1];
	char source[MAX_FILENAME_LEN * 2 + 1];
	char destination[MAX_FILENAME_LEN * 2 + 1];
	json_escape(event->comm, comm, sizeof(comm));
	json_escape(event->file_rename.source_path, source, sizeof(source));
	json_escape(event->file_rename.destination_path, destination, sizeof(destination));
	const char *syscall = event->file_rename.variant == 2 ? "renameat2" :
		event->file_rename.variant == 1 ? "renameat" : "rename";
	as_output_printf("{\"timestamp\":%llu,\"event\":\"FILE_RENAME\",\"comm\":\"%s\","
	       "\"pid\":%d,\"filepath\":\"%s\",\"source_path\":\"%s\","
	       "\"destination_path\":\"%s\",\"source_dirfd\":%d,"
	       "\"destination_dirfd\":%d,\"rename_flags\":%u,\"count\":1,"
	       "\"syscall_name\":\"%s\",\"syscall_return\":%lld,"
	       "\"duration_ns\":%llu",
	       event->timestamp_ns, comm, event->pid, destination, source, destination,
	       event->file_rename.source_dirfd, event->file_rename.destination_dirfd,
	       event->file_rename.flags, syscall, event->file_rename.result,
	       event->duration_ns);
	if (event->file_rename.result < 0 && event->file_rename.result >= -4095)
		as_output_printf(",\"syscall_errno\":%lld", -event->file_rename.result);
	print_python_async_context(event);
	as_output_printf("}\n");
	return true;
}

static const char *event_type_name(unsigned int type)
{
	switch (type) {
	case EVENT_TYPE_FILE_DELETE:    return "FILE_DELETE";
	case EVENT_TYPE_FILE_RENAME:   return "FILE_RENAME";
	case EVENT_TYPE_DIR_CREATE:    return "DIR_CREATE";
	case EVENT_TYPE_FILE_TRUNCATE: return "FILE_TRUNCATE";
	case EVENT_TYPE_CHDIR:         return "CHDIR";
	case EVENT_TYPE_WRITE:         return "WRITE";
	case EVENT_TYPE_NET_BIND:      return "NET_BIND";
	case EVENT_TYPE_NET_LISTEN:    return "NET_LISTEN";
	case EVENT_TYPE_NET_CONNECT:   return "NET_CONNECT";
	case EVENT_TYPE_NET_ACCEPT:    return "NET_ACCEPT";
	case EVENT_TYPE_PGRP_CHANGE:   return "PGRP_CHANGE";
	case EVENT_TYPE_SESSION_CREATE:return "SESSION_CREATE";
	case EVENT_TYPE_SIGNAL_SEND:   return "SIGNAL_SEND";
	case EVENT_TYPE_PROC_FORK:     return "PROC_FORK";
	case EVENT_TYPE_MMAP_SHARED:   return "MMAP_SHARED";
	case EVENT_TYPE_COW_FAULT:     return "COW_FAULT";
	default:                       return "UNKNOWN";
	}
}

static bool parse_fd_detail(const char *detail, int *fd_out)
{
	if (!detail || strncmp(detail, "fd=", 3) != 0)
		return false;

	char *end = NULL;
	long fd = strtol(detail + 3, &end, 10);
	if (end == detail + 3 || (end && *end != '\0'))
		return false;

	*fd_out = (int)fd;
	return true;
}

static bool resolve_fd_path(uint32_t pid, int fd, char *out, size_t out_size)
{
	char fd_link[PATH_MAX];
	char link_target[MAX_FILENAME_LEN];

	snprintf(fd_link, sizeof(fd_link), "%s/%u/fd/%d", as_procfs_root, pid, fd);
	ssize_t n = readlink(fd_link, link_target, sizeof(link_target) - 1);
	if (n <= 0)
		return false;

	link_target[n] = '\0';
	strncpy(out, link_target, out_size - 1);
	out[out_size - 1] = '\0';
	return true;
}

static bool endpoint_address(const char *endpoint, char *address, size_t size)
{
	const char *end;
	const char *begin = endpoint;
	if (*begin == '[') {
		begin++;
		end = strchr(begin, ']');
	} else {
		end = strrchr(begin, ':');
	}
	if (!end || end <= begin || (size_t)(end - begin) >= size)
		return false;
	memcpy(address, begin, end - begin);
	address[end - begin] = '\0';
	return true;
}

static const char *aggregate_correlation_source_name(unsigned char source)
{
	switch (source) {
	case ASYNC_CORRELATION_TASK:        return "task";
	case ASYNC_CORRELATION_TASK_PARENT: return "task_parent";
	case ASYNC_CORRELATION_PY_CONTEXT:  return "py_context";
	case ASYNC_CORRELATION_PROCESS:     return "process";
	default:                            return "none";
	}
}

static void aggregate_bytes_to_hex(
	const unsigned char *bytes,
	size_t length,
	char *output)
{
	static const char digits[] = "0123456789abcdef";

	for (size_t i = 0; i < length; i++) {
		output[i * 2] = digits[bytes[i] >> 4];
		output[i * 2 + 1] = digits[bytes[i] & 0xf];
	}
	output[length * 2] = '\0';
}

static void print_aggregate_python_context(const struct correlated_file_key *key)
{
	char trace_id[TRACE_ID_SIZE * 2 + 1];
	char span_id[SPAN_ID_SIZE * 2 + 1];

	aggregate_bytes_to_hex(key->tool_context.trace_id, TRACE_ID_SIZE, trace_id);
	aggregate_bytes_to_hex(key->tool_context.span_id, SPAN_ID_SIZE, span_id);
	as_output_printf(",\"tid\":%u", key->tid);
	as_output_printf(",\"agentsight_trace_id\":\"%s\"", trace_id);
	as_output_printf(",\"agentsight_parent_span_id\":\"%s\"", span_id);
	as_output_printf(",\"agentsight_trace_flags\":%u", key->tool_context.trace_flags);
	as_output_printf(",\"agentsight_task_id\":\"%016llx\"",
	       (unsigned long long)key->task_id);
	as_output_printf(",\"agentsight_bound_task_id\":\"%016llx\"",
	       (unsigned long long)key->bound_task_id);
	as_output_printf(",\"agentsight_correlation_source\":\"%s\"",
	       aggregate_correlation_source_name(key->correlation_source));
}

static bool print_correlated_file_json(
	const struct correlated_file_key *key,
	const struct agg_value *val)
{
	if (key->correlation_source == ASYNC_CORRELATION_NONE)
		return false;
	if (key->event_type != EVENT_TYPE_WRITE &&
	    key->event_type != EVENT_TYPE_FILE_RENAME)
		return false;

	char comm_esc[TASK_COMM_LEN * 2 + 1];
	char source_esc[MAX_FILENAME_LEN * 2 + 1];
	char destination_esc[MAX_FILENAME_LEN * 2 + 1];
	json_escape(val->comm, comm_esc, sizeof(comm_esc));
	json_escape(key->filepath, source_esc, sizeof(source_esc));
	json_escape(val->extra, destination_esc, sizeof(destination_esc));

	as_output_printf("{\"timestamp\":%llu,\"event\":\"%s\","
	       "\"comm\":\"%s\",\"pid\":%u",
	       (unsigned long long)val->last_ts,
	       key->event_type == EVENT_TYPE_WRITE ? "FILE_WRITE" : "FILE_RENAME",
	       comm_esc,
	       key->pid);
	if (key->event_type == EVENT_TYPE_WRITE) {
		as_output_printf(",\"filepath\":\"%s\",\"count\":%llu,\"total_bytes\":%llu",
		       source_esc,
		       (unsigned long long)val->count,
		       (unsigned long long)val->total_bytes);
	} else {
		as_output_printf(",\"filepath\":\"%s\",\"source_path\":\"%s\","
		       "\"destination_path\":\"%s\",\"count\":%llu",
		       destination_esc,
		       source_esc,
		       destination_esc,
		       (unsigned long long)val->count);
	}
	print_aggregate_python_context(key);
	as_output_printf("}\n");
	return true;
}

static void print_summary_json(const struct agg_key *key, const struct agg_value *val)
{
	char detail_esc[MAX_FILENAME_LEN * 2];
	char detail_path[MAX_FILENAME_LEN];
	char extra_esc[MAX_FILENAME_LEN * 2];
	char comm_esc[TASK_COMM_LEN * 2];
	int fd = -1;
	bool parsed_fd = false;
	bool resolved_path = false;

	json_escape(val->extra, extra_esc, sizeof(extra_esc));
	json_escape(val->comm, comm_esc, sizeof(comm_esc));

	if (key->event_type == EVENT_TYPE_WRITE) {
		parsed_fd = parse_fd_detail(key->detail, &fd);
		if (parsed_fd)
			resolved_path = resolve_fd_path(key->pid, fd, detail_path, sizeof(detail_path));
	}

	if (resolved_path)
		json_escape(detail_path, detail_esc, sizeof(detail_esc));
	else
		json_escape(key->detail, detail_esc, sizeof(detail_esc));

	as_output_printf("{\"timestamp\":%llu,\"event\":\"SUMMARY\","
	       "\"comm\":\"%s\",\"pid\":%u,\"process_start_ns\":%llu,"
	       "\"type\":\"%s\",\"detail\":\"%s\","
	       "\"count\":%llu",
	       (unsigned long long)val->last_ts, comm_esc, key->pid,
	       (unsigned long long)key->process_start_ns,
	       event_type_name(key->event_type), detail_esc,
	       (unsigned long long)val->count);

	if (val->total_bytes > 0)
		as_output_printf(",\"total_bytes\":%llu", (unsigned long long)val->total_bytes);
	if (key->protocol) {
		const char *protocol = key->protocol == 6 ? "tcp" :
			key->protocol == 17 ? "udp" :
			key->protocol == 136 ? "udplite" :
			key->protocol == 1 ? "icmp" : "icmpv6";
		as_output_printf(",\"protocol\":\"%s\",\"port\":%u", protocol, key->port);
	}
	else if (key->event_type == EVENT_TYPE_NET_BIND)
		as_output_printf(",\"port\":%u", key->port);

	if (key->event_type == EVENT_TYPE_NET_LISTEN)
		as_output_printf(",\"local_endpoint\":\"%s\"", detail_esc);
	else if (key->event_type == EVENT_TYPE_NET_ACCEPT) {
		char local_esc[DETAIL_LEN * 2];
		json_escape(key->local_endpoint, local_esc, sizeof(local_esc));
		as_output_printf(",\"local_endpoint\":\"%s\",\"peer_endpoint\":\"%s\"", local_esc, detail_esc);
	}
	if (key->event_type == EVENT_TYPE_NET_BIND ||
	    key->event_type == EVENT_TYPE_NET_LISTEN ||
	    key->event_type == EVENT_TYPE_NET_ACCEPT) {
		char address[DETAIL_LEN];
		const char *local = key->event_type == EVENT_TYPE_NET_ACCEPT ?
			key->local_endpoint : key->detail;
		if (endpoint_address(local, address, sizeof(address)))
			as_output_printf(",\"address\":\"%s\"", address);
		if (key->event_type == EVENT_TYPE_NET_ACCEPT) {
			if (strchr(key->detail, ':') == strrchr(key->detail, ':') &&
			    endpoint_address(key->detail, address, sizeof(address)))
				as_output_printf(",\"peer\":\"%s\"", address);
			else if (strchr(key->detail, ':') != strrchr(key->detail, ':'))
				as_output_printf(",\"peer\":\"%s\"", detail_esc);
		}
	}

	if (key->event_type == EVENT_TYPE_WRITE && parsed_fd) {
		as_output_printf(",\"fd\":%d", fd);
		as_output_printf(",\"path_resolved\":%s", resolved_path ? "true" : "false");
	}

	if (extra_esc[0])
		as_output_printf(",\"extra\":\"%s\"", extra_esc);

	as_output_printf("}\n");
}

static inline void flush_agg_map(int map_fd)
{
	struct agg_key key = {}, next_key;
	struct agg_value val;

	while (bpf_map_get_next_key(map_fd, &key, &next_key) == 0) {
		if (bpf_map_lookup_elem(map_fd, &next_key, &val) == 0 && val.count > 0) {
			print_summary_json(&next_key, &val);
			bpf_map_delete_elem(map_fd, &next_key);
		}
		key = next_key;
	}
	as_output_flush();
}

static inline void flush_pid_from_agg_map(int map_fd, uint32_t target_pid)
{
	struct agg_key key = {}, next_key;
	struct agg_value val;
	struct agg_key to_delete[256];
	int del_count = 0;

	while (bpf_map_get_next_key(map_fd, &key, &next_key) == 0) {
		if (next_key.pid == target_pid) {
			if (bpf_map_lookup_elem(map_fd, &next_key, &val) == 0 && val.count > 0) {
				print_summary_json(&next_key, &val);
			}
			if (del_count < 256)
				to_delete[del_count++] = next_key;
		}
		key = next_key;
	}

	for (int i = 0; i < del_count; i++)
		bpf_map_delete_elem(map_fd, &to_delete[i]);
	as_output_flush();
}

static inline void flush_correlated_file_agg_map(int map_fd)
{
	struct correlated_file_key key = {}, next_key;
	struct agg_value val;

	while (bpf_map_get_next_key(map_fd, &key, &next_key) == 0) {
		if (bpf_map_lookup_elem(map_fd, &next_key, &val) == 0 && val.count > 0) {
			print_correlated_file_json(&next_key, &val);
			bpf_map_delete_elem(map_fd, &next_key);
		}
		key = next_key;
	}
	as_output_flush();
}

static inline void flush_pid_from_correlated_file_agg_map(
	int map_fd,
	uint32_t target_pid)
{
	struct correlated_file_key key = {}, next_key;
	struct agg_value val;
	struct correlated_file_key to_delete[64];
	int del_count = 0;

	while (bpf_map_get_next_key(map_fd, &key, &next_key) == 0) {
		if (next_key.pid == target_pid) {
			if (bpf_map_lookup_elem(map_fd, &next_key, &val) == 0 && val.count > 0)
				print_correlated_file_json(&next_key, &val);
			if (del_count < 64)
				to_delete[del_count++] = next_key;
		}
		key = next_key;
	}

	for (int i = 0; i < del_count; i++)
		bpf_map_delete_elem(map_fd, &to_delete[i]);
	as_output_flush();
}

/* Check and report overflow count, returns the total count across all CPUs */
static inline uint64_t check_overflow(int overflow_map_fd)
{
	uint32_t zero = 0;
	int num_cpus = libbpf_num_possible_cpus();
	if (num_cpus <= 0) {
		/* A guessed CPU count can underallocate a per-CPU map lookup buffer. */
		as_output_diagnostic("Cannot read aggregation counters: unknown CPU count\n");
		return 0;
	}

	uint64_t *counts = calloc(num_cpus, sizeof(uint64_t));
	if (!counts)
		return 0;

	uint64_t total = 0;
	if (bpf_map_lookup_elem(overflow_map_fd, &zero, counts) == 0) {
		for (int i = 0; i < num_cpus; i++)
			total += counts[i];
	}

	if (total > 0) {
		as_output_printf("{\"timestamp\":0,\"event\":\"WARNING\",\"comm\":\"\",\"pid\":0,"
		       "\"type\":\"AGG_MAP_OVERFLOW\",\"overflow_count\":%llu}\n",
		       (unsigned long long)total);
		as_output_flush();
		/* Reset counters */
		memset(counts, 0, num_cpus * sizeof(uint64_t));
		bpf_map_update_elem(overflow_map_fd, &zero, counts, BPF_ANY);
	}

	free(counts);
	return total;
}

#endif /* __PROCESS_EXT_MAP_FLUSH_H */
