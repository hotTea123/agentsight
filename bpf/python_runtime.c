/* SPDX-License-Identifier: (LGPL-2.1 OR BSD-2-Clause) */
#include "python_runtime.h"
#include <elf.h>
#include <gelf.h>
#include <libelf.h>
#include <errno.h>
#include <string.h>
#include <unistd.h>

static Elf *open_elf(int fd)
{
	if (elf_version(EV_CURRENT) == EV_NONE)
		return NULL;
	Elf *elf = elf_begin(fd, ELF_C_READ, NULL);
	GElf_Ehdr hdr;
	if (!elf || !gelf_getehdr(elf, &hdr) ||
	    hdr.e_ident[EI_CLASS] != ELFCLASS64 || hdr.e_ident[EI_DATA] != ELFDATA2LSB ||
	    (hdr.e_type != ET_EXEC && hdr.e_type != ET_DYN)) {
		elf_end(elf);
		return NULL;
	}
	return elf;
}

/* Offsets must lie in an executable, file-backed PT_LOAD segment. */
int as_validate_code_offset(int fd, uint64_t offset)
{
	if (!offset || offset > INT64_MAX)
		return -ENOEXEC;
	Elf *elf = open_elf(fd);
	size_t count;
	int result = -ENOEXEC;
	if (!elf)
		return result;
	if (elf_getphdrnum(elf, &count) == 0) {
		for (size_t i = 0; i < count; i++) {
			GElf_Phdr phdr;
			if (gelf_getphdr(elf, i, &phdr) && phdr.p_type == PT_LOAD &&
			    (phdr.p_flags & PF_X) && offset >= phdr.p_offset &&
			    offset - phdr.p_offset < phdr.p_filesz) {
				unsigned char byte;
				if (offset <= INT64_MAX && pread(fd, &byte, 1, offset) == 1)
					result = 0;
				break;
			}
		}
	}
	elf_end(elf);
	return result;
}

int as_inspect_elf(int fd, struct as_elf_info *info)
{
	static const char *names[] = {
		"task_step_impl", "_asyncio_Task___init__", "PyContext_CopyCurrent", "context_run",
		"agentsight_tool_marker_abi_version", "agentsight_tool_context_bind", "agentsight_tool_context_unbind",
	};
	GElf_Addr addresses[7] = {};
	Elf *elf = open_elf(fd);
	GElf_Ehdr hdr;
	Elf_Scn *scn = NULL;
	int result = -ENOEXEC;
	if (!elf || !info) {
		elf_end(elf);
		return result;
	}
	memset(info, 0, sizeof(*info));
	gelf_getehdr(elf, &hdr);
	info->machine = hdr.e_machine;
	while ((scn = elf_nextscn(elf, scn))) {
		GElf_Shdr shdr;
		Elf_Data *data;
		if (!gelf_getshdr(scn, &shdr))
			goto done;
		data = elf_getdata(scn, NULL);
		if (!data)
			continue;
		if (shdr.sh_type == SHT_NOTE) {
			size_t pos = 0, nameoff, descoff, next;
			GElf_Nhdr note;
			while ((next = gelf_getnote(data, pos, &note, &nameoff, &descoff)) > pos) {
				if (note.n_type == NT_GNU_BUILD_ID && note.n_namesz == 4 &&
				    nameoff <= data->d_size && 4 <= data->d_size - nameoff &&
				    memcmp((char *)data->d_buf + nameoff, "GNU", 4) == 0 &&
				    note.n_descsz > 0 && note.n_descsz <= 64 &&
				    descoff <= data->d_size && note.n_descsz <= data->d_size - descoff) {
					static const char hex[] = "0123456789abcdef";
					unsigned char *id = (unsigned char *)data->d_buf + descoff;
					char build_id[sizeof(info->build_id)] = {};
					for (unsigned i = 0; i < note.n_descsz; i++) {
						build_id[2 * i] = hex[id[i] >> 4];
						build_id[2 * i + 1] = hex[id[i] & 15];
					}
					if (info->build_id[0] && strcmp(info->build_id, build_id))
						goto done;
					memcpy(info->build_id, build_id, sizeof(build_id));
				}
				pos = next;
			}
		}
		if ((shdr.sh_type != SHT_SYMTAB && shdr.sh_type != SHT_DYNSYM) || !shdr.sh_entsize)
			continue;
		for (size_t i = 0; i < data->d_size / shdr.sh_entsize; i++) {
			GElf_Sym sym;
			if (!gelf_getsym(data, i, &sym) || sym.st_shndx == SHN_UNDEF)
				continue;
			const char *name = elf_strptr(elf, shdr.sh_link, sym.st_name);
			if (!name)
				continue;
			if (!strcmp(name, "Py_Version") && GELF_ST_TYPE(sym.st_info) == STT_OBJECT) {
				Elf_Scn *section = elf_getscn(elf, sym.st_shndx);
				GElf_Shdr def;
				if (section && gelf_getshdr(section, &def) && def.sh_type != SHT_NOBITS &&
				    sym.st_size >= 4 && sym.st_value >= def.sh_addr &&
				    sym.st_value - def.sh_addr <= def.sh_size &&
				    def.sh_size - (sym.st_value - def.sh_addr) >= 4 &&
				    def.sh_offset <= UINT64_MAX - (sym.st_value - def.sh_addr)) {
					uint64_t off = sym.st_value - def.sh_addr + def.sh_offset;
					unsigned char v[4];
					if (off <= INT64_MAX && pread(fd, v, sizeof(v), off) == sizeof(v))
						info->python_version = (uint32_t)v[0] | (uint32_t)v[1] << 8 |
							(uint32_t)v[2] << 16 | (uint32_t)v[3] << 24;
				}
			}
			if (GELF_ST_TYPE(sym.st_info) != STT_FUNC)
				continue;
			for (unsigned n = 0; n < 7; n++) {
				if (strcmp(name, names[n]))
					continue;
				if ((info->symbols & (1U << n)) && addresses[n] != sym.st_value)
					goto done; /* Do not silently select an ambiguous function. */
				addresses[n] = sym.st_value;
				info->symbols |= 1U << n;
			}
		}
	}
	result = 0;
done:
	elf_end(elf);
	return result;
}
