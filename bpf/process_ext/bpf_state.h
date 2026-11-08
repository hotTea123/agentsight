/* SPDX-License-Identifier: GPL-2.0 OR BSD-3-Clause */
#ifndef __PROCESS_EXT_BPF_STATE_H
#define __PROCESS_EXT_BPF_STATE_H

#include "process_ext/types.h"

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 16384);
	__type(key, struct agg_key);
	__type(value, struct agg_value);
} event_agg_map SEC(".maps");

/* The upstream network identity enlarged agg_key. Keep path aggregation
 * below the BPF 512-byte combined stack limit. */
struct {
	__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, struct agg_key);
} fs_agg_key_scratch SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, struct agg_value);
} agg_value_scratch SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 8192);
	__type(key, struct correlated_file_key);
	__type(value, struct agg_value);
} correlated_file_agg_map SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, struct correlated_file_key);
} correlated_file_key_scratch SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, MAX_TRACKED_PIDS);
	__type(key, u32);
	__type(value, u8);
} tracked_pids SEC(".maps");

#ifdef AGENTSIGHT_TEST_CAPTURE
/* Test build only: bounded PID scope, never a global capture switch. */
const volatile unsigned int test_root_pid = 0;
const volatile unsigned long long test_root_start_ticks = 0;
const volatile unsigned long long test_ns_per_tick = 10000000;
struct {
	__uint(type, BPF_MAP_TYPE_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, u32);
} test_root_exited SEC(".maps");
#endif

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, MAX_TRACKED_CGROUPS);
	__type(key, u64);
	__type(value, u8);
} tracked_cgroups SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, u64);
} agg_overflow_count SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 4096);
	__type(key, u32);
	__type(value, struct exit_mem_info);
} exit_mem SEC(".maps");

struct accept_peer_key {
	__u64 start_time;
	__u32 pid;
	__u16 port;
	__u16 family;
	__u8 peer[16];
};

struct {
	__uint(type, BPF_MAP_TYPE_LRU_HASH);
	__uint(max_entries, 16384);
	__type(key, struct accept_peer_key);
	__type(value, __u8);
} accept_peers_seen SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 8192);
	__type(key, u64);
	__type(value, int);
} write_ctx_map SEC(".maps");

/* Keep the socket across inet_listen entry/return so only successful listens
 * are reported, with the assigned port (including implicit/ephemeral binds). */
struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 4096);
	__type(key, u64);
	__type(value, u64);
} listen_socket_map SEC(".maps");

/* Snapshot the entry context; no lookup of the current Tool at syscall exit. */
struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 8192);
	__type(key, u64);
	__type(value, struct event);
} pending_file_renames SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_PERCPU_ARRAY);
	__uint(max_entries, 1);
	__type(key, u32);
	__type(value, struct event);
} rename_event_scratch SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 8192);
	__type(key, u64);
	__type(value, struct pending_file_open);
} pending_file_opens SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_LRU_HASH);
	__uint(max_entries, 16384);
	__type(key, struct file_descriptor_key);
	__type(value, struct tracked_file_descriptor);
} tracked_file_descriptors SEC(".maps");

struct {
	__uint(type, BPF_MAP_TYPE_HASH);
	__uint(max_entries, 8192);
	__type(key, u64);
	__type(value, struct pending_file_dup);
} pending_file_dups SEC(".maps");

const volatile bool filter_pids = false;
const volatile bool filter_cgroup = false;
const volatile bool filter_cgroup_children = false;
const volatile unsigned long long target_cgroup_id = 0;
const volatile unsigned long long target_pidns_dev = 0;
const volatile unsigned long long target_pidns_ino = 0;
const volatile bool trace_fs_mutations = false;
const volatile bool trace_network = false;
const volatile bool trace_signals = false;
const volatile bool trace_memory = false;
const volatile bool trace_cow = false;
const volatile bool trace_python_async_context = false;
const volatile unsigned int python_target_pid = 0;

#endif /* __PROCESS_EXT_BPF_STATE_H */
