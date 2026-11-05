#include "agentsight.h"
#include <assert.h>
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char **argv)
{
    assert(argc == 2);
    void *library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    assert(library);
    const char *symbols[] = {
        "agentsight_init", "agentsight_set_config", "agentsight_start",
        "agentsight_stop", "agentsight_free", "agentsight_get_eventfd",
        "agentsight_poll_events", "agentsight_free_event", "agentsight_get_stats",
        "agentsight_get_last_error", "agentsight_abi_version"
    };
    for (unsigned i = 0; i < sizeof(symbols) / sizeof(symbols[0]); ++i)
        assert(dlsym(library, symbols[i]));
    assert(!dlsym(library, "agentsight_suspend"));
    assert(!dlsym(library, "bpf_object__open"));
    assert(agentsight_abi_version() == 1);
    struct sigaction before, after;
    struct rlimit limit_before, limit_after;
    assert(sigaction(SIGINT, NULL, &before) == 0);
    assert(getrlimit(RLIMIT_MEMLOCK, &limit_before) == 0);
    for (int i = 0; i < 3; ++i) {
        AgentsightHandle *h = NULL;
        assert(agentsight_init(&h) == 0 && h);
        assert(agentsight_start(h) == -EINVAL);
        char message[512];
        assert(agentsight_get_last_error(h, message, sizeof(message)) == 0);
        assert(strstr(message, "set_config"));
        int fd = agentsight_get_eventfd(h);
        assert(fd >= 0 && (fcntl(fd, F_GETFD) & FD_CLOEXEC));
        assert(fcntl(fd, F_GETFL) & O_NONBLOCK);
        AgentsightConfig cfg = {0};
        cfg.struct_size = sizeof(cfg);
        cfg.abi_version = AGENTSIGHT_ABI_VERSION;
        cfg.target_pid = getpid();
        cfg.marker_path = "/nonexistent-agentsight-test/marker.so";
        cfg.asyncio_path = cfg.marker_path;
        cfg.python_path = cfg.marker_path;
        cfg.task_step_offset = cfg.task_init_offset = cfg.context_copy_offset = cfg.context_run_offset = 1;
        assert(agentsight_set_config(h, &cfg) == 0);
        assert(agentsight_start(h) < 0);
        AgentsightStats stats;
        assert(agentsight_get_stats(h, &stats, sizeof(stats)) == 0);
        assert(stats.state == AGENTSIGHT_STATE_FAILED && stats.backend_errors == 1);
        assert(agentsight_stop(h) == 0);
        pid_t child = fork();
        assert(child >= 0);
        if (child == 0) {
            AgentsightHandle *fresh = NULL;
            _exit(agentsight_get_eventfd(h) == -ECHILD &&
                  agentsight_init(&fresh) == -ECHILD && !fresh ? 0 : 1);
        }
        int status;
        assert(waitpid(child, &status, 0) == child && WIFEXITED(status) && WEXITSTATUS(status) == 0);
        assert(agentsight_free(h) == 0);
        assert(fcntl(fd, F_GETFD) == -1 && errno == EBADF);
    }
    assert(sigaction(SIGINT, NULL, &after) == 0);
    assert(before.sa_handler == after.sa_handler);
    assert(getrlimit(RLIMIT_MEMLOCK, &limit_after) == 0);
    assert(limit_before.rlim_cur == limit_after.rlim_cur && limit_before.rlim_max == limit_after.rlim_max);
    assert(dlclose(library) == 0);
    puts("C ABI/load/failure/cleanup/fork checks passed");
    return 0;
}
