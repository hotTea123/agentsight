/* Minimal real capture host. Does not create/export spans or launch a collector. */
#include "agentsight.h"
#include <assert.h>
#include <errno.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

static void check(AgentsightHandle *h, int rc)
{
    if (rc < 0) {
        char error[2048] = {0};
        agentsight_get_last_error(h, error, sizeof(error));
        fprintf(stderr, "AgentSight error %d: %s\n", rc, error);
        agentsight_free(h);
        exit(1);
    }
}

static void drain(AgentsightHandle *h)
{
    for (;;) {
        AgentsightEvent *events[16];
        int count = agentsight_poll_events(h, events, 16);
        check(h, count);
        if (!count) break;
        for (int i = 0; i < count; ++i) {
            const AgentsightEvent *e = events[i];
            printf("{\"schema_version\":%u,\"event_type\":%u,\"wall_time_ns\":%llu,\"timestamp_ns\":%llu,\"trace_id\":\"",
                   e->schema_version, e->event_type, (unsigned long long)e->wall_time_ns, (unsigned long long)e->timestamp_ns);
            for (unsigned j = 0; j < 16; j++) printf("%02x", e->trace_id[j]);
            printf("\",\"parent_span_id\":\"");
            for (unsigned j = 0; j < 8; j++) printf("%02x", e->parent_span_id[j]);
            printf("\",\"payload\":%.*s}\n", (int)e->payload_json_len, e->payload_json);
            agentsight_free_event(events[i]);
        }
        fflush(stdout);
    }
}

int main(int argc, char **argv)
{
    assert(argc <= 2);
    AgentsightHandle *h = NULL;
    check(h, agentsight_init(&h));
    AgentsightConfig cfg = {0};
    cfg.struct_size = sizeof(cfg); cfg.abi_version = AGENTSIGHT_ABI_VERSION;
    cfg.discovery_interval_ms = 100;
    if (argc == 2) cfg.procfs_root = argv[1];
    check(h, agentsight_set_config(h, &cfg));
    check(h, agentsight_start(h));
    AgentsightHandle *second = NULL;
    check(second, agentsight_init(&second));
    check(second, agentsight_set_config(second, &cfg));
    assert(agentsight_start(second) == -EBUSY);
    assert(agentsight_free(second) == 0);
    assert(agentsight_set_config(h, &cfg) < 0);
    assert(agentsight_start(h) == 0);
    puts("STARTED"); fflush(stdout);
    struct pollfd fds[2] = {{ .fd = agentsight_get_eventfd(h), .events = POLLIN }, { .fd = STDIN_FILENO, .events = POLLIN }};
    uint32_t last_state = UINT32_MAX;
    uint64_t last_generation = UINT64_MAX;
    uint64_t last_scan = UINT64_MAX;
    for (;;) {
        AgentsightStats current;
        check(h, agentsight_get_stats(h, &current, sizeof(current)));
        if (last_scan != current.discovery_scans) {
            printf("{\"discovery_scans\":%llu}\n", (unsigned long long)current.discovery_scans);
            last_scan = current.discovery_scans;
            fflush(stdout);
        }
        if (current.target_state != last_state || current.target_generation != last_generation) {
            printf("{\"target_state\":%u,\"target_pid\":%d,\"generation\":%llu,\"adaptation\":%u,\"target_error\":%d}\n",
                current.target_state, current.target_pid, (unsigned long long)current.target_generation,
                current.adaptation_method, current.target_error);
            if (current.target_state == AGENTSIGHT_TARGET_READY) puts("READY");
            if (current.target_error) {
                char message[2048] = {0};
                agentsight_get_last_error(h, message, sizeof(message));
                printf("DIAGNOSTIC %s\n", message);
            }
            last_state = current.target_state;
            last_generation = current.target_generation;
            fflush(stdout);
        }
        int rc = poll(fds, 2, 100);
        if (rc < 0) continue;
        if (fds[1].revents & (POLLIN | POLLHUP)) {
            char command = 0;
            if (rc > 0) (void)read(STDIN_FILENO, &command, 1);
            check(h, agentsight_stop(h));
            drain(h);
            if (command == 'r') {
                check(h, agentsight_start(h));
                last_state = UINT32_MAX;
                puts("STARTED"); fflush(stdout);
                continue;
            }
            break;
        }
        if (fds[0].revents) drain(h);
    }
    AgentsightStats stats;
    check(h, agentsight_get_stats(h, &stats, sizeof(stats)));
    printf("{\"stats\":{\"received\":%llu,\"enqueued\":%llu,\"delivered\":%llu,\"dropped\":%llu,\"invalid\":%llu,\"backend_errors\":%llu,\"kernel_ring_dropped\":%llu}}\n",
           (unsigned long long)stats.received_records, (unsigned long long)stats.enqueued_events,
           (unsigned long long)stats.delivered_events, (unsigned long long)stats.queue_dropped_events,
           (unsigned long long)stats.invalid_records, (unsigned long long)stats.backend_errors,
           (unsigned long long)stats.kernel_ring_dropped);
    check(h, agentsight_free(h));
    return 0;
}
