/* SPDX-License-Identifier: (LGPL-2.1 OR BSD-2-Clause) */
#ifndef AGENTSIGHT_PYTHON_RUNTIME_H
#define AGENTSIGHT_PYTHON_RUNTIME_H
#include <stdint.h>

/* Private ELF inspection only: never dlopen or execute a target's library. */
struct as_elf_info {
	uint32_t machine;
	uint32_t python_version;
	uint32_t symbols;
	char build_id[129];
};
int as_inspect_elf(int fd, struct as_elf_info *info);
int as_validate_code_offset(int fd, uint64_t offset);
#endif
