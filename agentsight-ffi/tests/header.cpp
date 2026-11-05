#include "agentsight.h"
#include <cstddef>
#include <type_traits>
static_assert(sizeof(AgentsightConfig) == 96);
static_assert(sizeof(AgentsightEvent) == 88);
static_assert(sizeof(AgentsightStats) == 112);
static_assert(offsetof(AgentsightEvent, trace_id) == 48);
static_assert(offsetof(AgentsightEvent, payload_json) == 72);
static_assert(std::is_same_v<decltype(&agentsight_start), int32_t (*)(AgentsightHandle *)>);
int main() { return agentsight_abi_version() == 1 ? 0 : 1; }
