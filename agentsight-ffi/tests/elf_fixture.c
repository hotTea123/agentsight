/* ELF metadata fixture, not a CPython implementation or an attachment target. */
#include <stdint.h>
const unsigned long Py_Version = 0x030b02f0;
__attribute__((used, noinline)) static void task_step_impl(void *task) { (void)task; }
__attribute__((used, noinline)) static void _asyncio_Task___init__(void *task) { (void)task; }
__attribute__((used, noinline)) static void context_run(void *context) { (void)context; }
void *PyContext_CopyCurrent(void) { return 0; }
uint32_t agentsight_tool_marker_abi_version(void) { return 1; }
void agentsight_tool_context_bind(void) {}
void agentsight_tool_context_unbind(void) {}
