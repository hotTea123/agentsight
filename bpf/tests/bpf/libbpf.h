/* Stub bpf/libbpf.h for unit tests */
#ifndef __LIBBPF_LIBBPF_H
#define __LIBBPF_LIBBPF_H

#ifndef LIBBPF_API
#define LIBBPF_API
#endif

static inline int libbpf_num_possible_cpus(void)
{
	return 1;
}

#endif /* __LIBBPF_LIBBPF_H */
