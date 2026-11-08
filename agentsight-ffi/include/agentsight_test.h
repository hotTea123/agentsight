/* Test-only extension. Link libagentsight_test.so, NEVER the production library. */
#ifndef AGENTSIGHT_TEST_H
#define AGENTSIGHT_TEST_H
#include "agentsight.h"
#define AGENTSIGHT_TEST_EVENT_SCHEMA_VERSION 1001
#ifdef __cplusplus
extern "C" {
#endif
/* After set_config, before start. pid > 1, not the collector or its ancestor, must exist.
 * Captures this process and children born while probes are active. No marker required.
 * Returns 0 or negative errno: EINVAL (configuration/PID view), ESRCH (target),
 * EBUSY (worker/queued events), plus base handle errors. Records PID + start ticks.
 * set_config resets the selection. stop + drain is required before replacing it.
 * Test events have schema 1001, zero trace/span/flags and JSON agentsight_test_only=true.
 * Do NOT treat these events as attributed production tool evidence. */
int32_t agentsight_set_test_target(AgentsightHandle *h, int32_t target_pid);
#ifdef __cplusplus
}
#endif
#endif
