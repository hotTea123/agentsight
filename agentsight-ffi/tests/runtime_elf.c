#include "python_runtime.h"
#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
int main(int argc, char **argv)
{
    assert(argc == 4);
    struct as_elf_info full, stripped, unknown;
    int a = open(argv[1], O_RDONLY), b = open(argv[2], O_RDONLY), c = open(argv[3], O_RDONLY);
    assert(a >= 0 && b >= 0 && c >= 0);
    assert(as_inspect_elf(a, &full) == 0 && full.symbols == 127);
    assert(as_inspect_elf(b, &stripped) == 0 && stripped.symbols == 116);
    assert(full.python_version == 0x030b02f0 && stripped.python_version == full.python_version);
    assert(strlen(full.build_id) > 0 && !strcmp(full.build_id, stripped.build_id));
    assert(as_inspect_elf(c, &unknown) == 0 && unknown.build_id[0] == 0);
    assert(as_validate_code_offset(a, 0) < 0);
    assert(as_validate_code_offset(a, UINT64_MAX) < 0);
    assert(as_inspect_elf(-1, &unknown) < 0);
    close(a); close(b); close(c);
    puts("ELF symbols, stripped symbols, Build ID, version and offset bounds passed");
    return 0;
}
