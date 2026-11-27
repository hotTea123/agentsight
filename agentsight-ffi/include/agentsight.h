/* AgentSight tool-span evidence ABI v2. See README.md for ownership and threading rules. */

#ifndef AGENTSIGHT_H
#define AGENTSIGHT_H

/* Generated from Rust declarations. Do not edit by hand. */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#define AGENTSIGHT_ABI_VERSION 2

#define AGENTSIGHT_EVENT_FILE_OPEN 1

#define AGENTSIGHT_EVENT_FILE_WRITE 2

#define AGENTSIGHT_EVENT_FILE_RENAME 3

#define AGENTSIGHT_EVENT_PROCESS_EXEC 4

#define AGENTSIGHT_STATE_INITIALIZED 0

#define AGENTSIGHT_STATE_CONFIGURED 1

#define AGENTSIGHT_STATE_RUNNING 2

#define AGENTSIGHT_STATE_STOPPED 3

#define AGENTSIGHT_STATE_FAILED 4

#define AGENTSIGHT_TARGET_IDLE 0

#define AGENTSIGHT_TARGET_WAITING 1

#define AGENTSIGHT_TARGET_ATTACHING 2

#define AGENTSIGHT_TARGET_READY 3

#define AGENTSIGHT_TARGET_DEGRADED 4

#define AGENTSIGHT_ADAPT_SYMBOLS 1

#define AGENTSIGHT_ADAPT_BUILD_ID 2

/**
 * User queue accounting; correlation-map failures are not fully measured.
 */
#define AGENTSIGHT_STATS_USER_QUEUE 1

/**
 * Existing aggregation-map overflow counter, best effort (not total kernel loss).
 */
#define AGENTSIGHT_STATS_AGG_OVERFLOW 2

/**
 * Failed ring reservations in enhancement open/rename/exec/exit probes.
 */
#define AGENTSIGHT_STATS_KERNEL_RING 4

/**
 * Opaque handle. Never copy, dereference or use it after free. Forked children
 * must exec before using this library again; inherited handles return -ECHILD.
 */
typedef struct AgentsightHandle AgentsightHandle;

/**
 * Zero-initialize, then set struct_size, abi_version and required fields.
 * Strings are borrowed for set_config only; the library copies them.
 */
typedef struct AgentsightConfig {
  uint32_t struct_size;
  uint32_t abi_version;
  /**
   * Zero selects 1024 events. Maximum 65536.
   */
  uint32_t queue_capacity;
  /**
   * Zero selects 1000 ms. Supported range: 100..60000 ms.
   */
  uint32_t discovery_interval_ms;
  /**
   * Zero selects 16 MiB. Bounds queued payload + event metadata, not all RSS.
   */
  uint64_t max_queue_bytes;
  /**
   * NULL/empty selects /proc. Requires the host PID namespace, not merely a
   * host procfs bind mount. No implicit command-name or time-window matching.
   */
  const char *procfs_root;
} AgentsightConfig;

/**
 * Immutable owned event. Release exactly once with agentsight_free_event.
 * IDs are bytes in network order, NOT strings. JSON retains operation details.
 */
typedef struct AgentsightEvent {
  uint32_t struct_size;
  uint32_t schema_version;
  uint32_t event_type;
  uint32_t trace_flags;
  uint32_t pid;
  uint32_t tid;
  /**
   * Kernel CLOCK_MONOTONIC timestamp; not a Unix timestamp.
   */
  uint64_t timestamp_ns;
  /**
   * Estimated Unix nanoseconds using a paired monotonic/realtime anchor.
   */
  uint64_t wall_time_ns;
  /**
   * Zero means unavailable, not a measured zero-duration operation.
   */
  uint64_t duration_ns;
  uint8_t trace_id[16];
  /**
   * The original tool span ID. A host creates a NEW evidence child span ID.
   */
  uint8_t parent_span_id[8];
  const char *payload_json;
  /**
   * Excludes trailing NUL. Valid until free_event, including after handle free.
   */
  uint32_t payload_json_len;
  uint32_t reserved;
} AgentsightEvent;

/**
 * Cumulative per handle, including across stop/start. Gauges reflect now.
 */
typedef struct AgentsightStats {
  uint32_t struct_size;
  uint32_t abi_version;
  uint32_t state;
  uint32_t supported_counters;
  uint64_t received_records;
  uint64_t filtered_records;
  uint64_t invalid_records;
  uint64_t enqueued_events;
  uint64_t delivered_events;
  uint64_t queue_dropped_events;
  uint64_t output_dropped_records;
  uint64_t backend_errors;
  uint64_t backend_diagnostics;
  uint64_t aggregate_overflow;
  uint64_t queued_events;
  uint64_t queued_bytes;
  /**
   * RUNNING does not imply READY. Inspect this field before claiming capture.
   */
  uint32_t target_state;
  int32_t target_pid;
  /**
   * /proc/PID/stat starttime, in clock ticks, not nanoseconds.
   */
  uint64_t target_start_ticks;
  uint64_t target_generation;
  uint64_t discovery_scans;
  uint64_t discovery_errors;
  uint64_t attach_attempts;
  uint64_t attach_failures;
  uint64_t target_detaches;
  /**
   * Negative errno for the current discovery/attachment failure, or zero.
   */
  int32_t target_error;
  /**
   * Zero until READY, then AGENTSIGHT_ADAPT_*.
   */
  uint32_t adaptation_method;
  uint64_t kernel_ring_dropped;
} AgentsightStats;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Returns the C ABI version, not the separate tool-marker ABI version.
 */
uint32_t agentsight_abi_version(void);

/**
 * Creates an inactive handle. Returns 0 or negative errno; initializes *out to NULL on failure.
 */
int32_t agentsight_init(struct AgentsightHandle **out);

/**
 * Copies configuration. Allowed only with no worker and an empty event queue.
 */
int32_t agentsight_set_config(struct AgentsightHandle *h, const struct AgentsightConfig *config);

/**
 * Starts the discovery worker, even if Jiuwen is absent. Success is NOT target readiness.
 * Inspect get_stats.target_state; unknown runtimes degrade without stopping discovery.
 * Idempotent while running. After stop, drain events before restarting.
 */
int32_t agentsight_start(struct AgentsightHandle *h);

/**
 * Stops and joins the producer, detaches probes and drains kernel aggregates.
 * Queued events survive; stopping does not preserve live tool bindings.
 */
int32_t agentsight_stop(struct AgentsightHandle *h);

/**
 * Stops if needed, discards queued events and frees the handle. NULL is allowed.
 * Must not race ANY call using this handle. Outstanding returned events survive.
 */
int32_t agentsight_free(struct AgentsightHandle *h);

/**
 * Borrowed nonblocking eventfd. Poll/epoll only: do not read, close or write it.
 * The library acknowledges/re-arms it in poll_events. Valid until handle free.
 */
int32_t agentsight_get_eventfd(struct AgentsightHandle *h);

/**
 * Nonblocking batch read. Returns count (0 = empty) or negative errno.
 * out_events points to capacity writable pointer slots (capacity 1..65536).
 * Each returned pointer must be freed exactly once; unused slots are untouched.
 * Use one consumer per handle. For cgo, allocate the pointer array in C memory.
 */
int32_t agentsight_poll_events(struct AgentsightHandle *h,
                               struct AgentsightEvent **out_events,
                               uint32_t capacity);

/**
 * Releases one event allocated by poll_events; NULL is allowed. Never use free().
 */
void agentsight_free_event(struct AgentsightEvent *event);

/**
 * Copies the last diagnostic to caller memory; safe across cgo OS-thread changes.
 * Returns 0, -ENOSPC (NUL-terminated truncation), or another negative errno.
 * Concurrent failures can replace this text; the operation's return code is authoritative.
 */
int32_t agentsight_get_last_error(struct AgentsightHandle *h, char *buffer, size_t capacity);

/**
 * Copies a snapshot into a buffer of at least sizeof(AgentsightStats) bytes.
 */
int32_t agentsight_get_stats(struct AgentsightHandle *h,
                             struct AgentsightStats *stats,
                             size_t stats_size);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* AGENTSIGHT_H */
