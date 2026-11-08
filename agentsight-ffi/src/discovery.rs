//! One marker-bearing Python root per node. No command-name guessing or Python execution.
use std::{
    collections::{HashMap, HashSet},
    ffi::{CString, c_int},
    fs::{self, File, Metadata},
    io::{self, Read},
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

const MARKER: &str = "libagentsight_tool_marker.so";
const MAX_PROC_BYTES: u64 = 4 * 1024 * 1024;
const KNOWN_PYTHON: &str = "d190c32fa97c85641233f1a813582deecb960b8c";
const KNOWN_ASYNCIO: &str = "1007e17e3911f3b90839a8823bc3228925b817d0";
const KNOWN_OFFSETS: [u64; 4] = [24496, 19600, 2528016, 2546048];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct ProcessKey {
    pub pid: i32,
    pub start: u64,
}

#[derive(Clone, Copy, Debug)]
struct Process {
    key: ProcessKey,
    parent: i32,
}

fn process_stat(text: &str) -> Option<Process> {
    let (pid, _) = text.split_once(' ')?;
    // comm can contain spaces and parentheses; the final ')' terminates it.
    let (_, rest) = text.rsplit_once(')')?;
    let fields: Vec<_> = rest.split_whitespace().collect();
    if fields.len() < 20 || matches!(fields[0], "Z" | "X") {
        return None;
    }
    Some(Process {
        key: ProcessKey {
            pid: pid.parse().ok()?,
            start: fields[19].parse().ok()?,
        },
        parent: fields[1].parse().ok()?,
    })
}

fn read_bounded(path: &Path) -> io::Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_PROC_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PROC_BYTES {
        return Err(io::Error::other("proc file exceeds inspection limit"));
    }
    String::from_utf8(bytes).map_err(|_| io::Error::other("non-UTF8 proc entry"))
}

fn read_process(root: &Path, pid: i32) -> io::Result<Process> {
    process_stat(&read_bounded(&root.join(format!("{pid}/stat")))?)
        .filter(|p| p.key.pid == pid)
        .ok_or_else(|| io::Error::other("process exited or invalid stat"))
}

#[cfg(feature = "test-capture")]
pub(super) fn process_key(root: &Path, pid: i32) -> io::Result<ProcessKey> {
    read_process(root, pid).map(|p| p.key)
}

#[cfg(feature = "test-capture")]
pub(super) fn collector_descends_from(root: &Path, target: ProcessKey) -> io::Result<bool> {
    let mut pid = unsafe { libc::getpid() };
    for _ in 0..128 {
        if pid <= 1 {
            return Ok(false);
        }
        let p = read_process(root, pid)?;
        if p.key == target {
            return Ok(true);
        }
        if p.parent == pid {
            break;
        }
        pid = p.parent;
    }
    Err(io::Error::other(
        "cannot establish a bounded collector ancestry",
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileStamp {
    dev: u64,
    ino: u64,
    len: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}
impl From<Metadata> for FileStamp {
    fn from(m: Metadata) -> Self {
        Self {
            dev: m.dev(),
            ino: m.ino(),
            len: m.len(),
            mtime: (m.mtime(), m.mtime_nsec()),
            ctime: (m.ctime(), m.ctime_nsec()),
        }
    }
}

#[derive(Debug)]
struct Mapping {
    range: String,
    major: u64,
    minor: u64,
    inode: u64,
    path: PathBuf,
}

fn mapping(line: &str) -> Option<Mapping> {
    let mut rest = line;
    let mut fields = Vec::new();
    for _ in 0..5 {
        rest = rest.trim_start();
        let n = rest.find(char::is_whitespace)?;
        fields.push(&rest[..n]);
        rest = &rest[n..];
    }
    if !fields[1].contains('x') || !rest.trim_start().starts_with('/') {
        return None;
    }
    let (major, minor) = fields[3].split_once(':')?;
    let (begin, end) = fields[0].split_once('-')?;
    let begin = u64::from_str_radix(begin, 16).ok()?;
    let end = u64::from_str_radix(end, 16).ok()?;
    let path = rest.trim_start();
    if path.ends_with(" (deleted)") {
        return None; // No guessing a replacement file at the old pathname.
    }
    Some(Mapping {
        range: format!("{begin:x}-{end:x}"),
        major: u64::from_str_radix(major, 16).ok()?,
        minor: u64::from_str_radix(minor, 16).ok()?,
        inode: fields[4].parse().ok()?,
        path: PathBuf::from(path.replace("\\012", "\n")),
    })
}

struct MappedFile {
    file: File,
    stamp: FileStamp,
}
impl MappedFile {
    fn open(root: &Path, pid: i32, map: &Mapping) -> io::Result<Self> {
        let (file, from_mapping) =
            match File::open(root.join(format!("{pid}/map_files/{}", map.range))) {
                Ok(file) => (file, true),
                Err(_) => (
                    File::open(
                        root.join(format!("{pid}/root"))
                            .join(map.path.strip_prefix("/").unwrap()),
                    )?,
                    false,
                ),
            };
        let stamp = FileStamp::from(file.metadata()?);
        // Overlayfs maps may report the backing device while fstat reports the
        // overlay device. A successful kernel map_files open selects the actual
        // mapping; a pathname fallback MUST match both device and inode.
        if stamp.ino != map.inode
            || (!from_mapping
                && (libc::major(stamp.dev) != map.major as u32
                    || libc::minor(stamp.dev) != map.minor as u32))
        {
            return Err(io::Error::other("mapped file identity changed"));
        }
        Ok(Self { file, stamp })
    }
    fn pinned_path(&self) -> CString {
        CString::new(format!("/proc/self/fd/{}", self.file.as_raw_fd())).unwrap()
    }
}

pub(super) struct Target {
    pub process: ProcessKey,
    exe: FileStamp,
    marker: MappedFile,
    asyncio: MappedFile,
    runtime: MappedFile,
}
impl Target {
    pub fn same_instance(&self, other: &Self) -> bool {
        self.process == other.process
            && self.exe == other.exe
            && self.marker.stamp == other.marker.stamp
            && self.asyncio.stamp == other.asyncio.stamp
            && self.runtime.stamp == other.runtime.stamp
    }
    pub fn paths(&self) -> [CString; 3] {
        [
            self.marker.pinned_path(),
            self.asyncio.pinned_path(),
            self.runtime.pinned_path(),
        ]
    }
    pub fn unchanged(&self, root: &Path) -> bool {
        if read_process(root, self.process.pid).map(|p| p.key).ok() != Some(self.process) {
            return false;
        }
        fs::metadata(root.join(format!("{}/exe", self.process.pid)))
            .map(FileStamp::from)
            .is_ok_and(|stamp| stamp == self.exe)
            && [&self.marker, &self.asyncio, &self.runtime]
                .iter()
                .all(|f| {
                    f.file
                        .metadata()
                        .map(FileStamp::from)
                        .is_ok_and(|stamp| stamp == f.stamp)
                })
    }
    pub fn adapt(&self) -> Result<Adaptation, String> {
        let marker = inspect(&self.marker.file)?;
        let asyncio = inspect(&self.asyncio.file)?;
        let runtime = inspect(&self.runtime.file)?;
        let machine = if cfg!(target_arch = "aarch64") {
            183
        } else if cfg!(target_arch = "x86_64") {
            62
        } else {
            0
        };
        if machine == 0
            || [marker.machine, asyncio.machine, runtime.machine]
                .iter()
                .any(|m| *m != machine)
        {
            return Err("target ELF architecture does not match this library".into());
        }
        if marker.symbols & 0x70 != 0x70 {
            return Err("marker is missing the agreed ABI symbols".into());
        }
        let adaptation = select_adaptation(&asyncio, &runtime)?;
        for (i, off) in adaptation.offsets.iter().enumerate() {
            if adaptation.symbol_mask & (1 << i) == 0 {
                let fd = if i < 2 {
                    self.asyncio.file.as_raw_fd()
                } else {
                    self.runtime.file.as_raw_fd()
                };
                if unsafe { as_validate_code_offset(fd, *off) } != 0 {
                    return Err("profile offset is outside executable file data".into());
                }
            }
        }
        Ok(adaptation)
    }
}

#[repr(C)]
struct ElfInfo {
    machine: u32,
    python_version: u32,
    symbols: u32,
    build_id: [u8; 129],
}
unsafe extern "C" {
    fn as_inspect_elf(fd: c_int, info: *mut ElfInfo) -> c_int;
    fn as_validate_code_offset(fd: c_int, offset: u64) -> c_int;
}
fn inspect(file: &File) -> Result<ElfInfo, String> {
    let mut info = ElfInfo {
        machine: 0,
        python_version: 0,
        symbols: 0,
        build_id: [0; 129],
    };
    if unsafe { as_inspect_elf(file.as_raw_fd(), &mut info) } != 0 {
        return Err("invalid ELF or ambiguous probe symbols".into());
    }
    Ok(info)
}
fn build_id(info: &ElfInfo) -> &str {
    let n = info
        .build_id
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(info.build_id.len());
    std::str::from_utf8(&info.build_id[..n]).unwrap_or("")
}
pub(super) struct Adaptation {
    pub offsets: [u64; 4],
    pub symbol_mask: u32,
    pub method: u32,
}
fn select_adaptation(asyncio: &ElfInfo, runtime: &ElfInfo) -> Result<Adaptation, String> {
    // This probe model is CPython 3.11 only. Symbol names alone do not validate
    // other Python implementations, minor versions or free-threaded runtimes.
    if runtime.python_version >> 16 != 0x030b {
        return Err(format!(
            "unsupported Python probe model: version=0x{:08x}",
            runtime.python_version
        ));
    }
    let symbol_mask = (asyncio.symbols & 3) | (runtime.symbols & 12);
    if symbol_mask == 15 {
        return Ok(Adaptation {
            offsets: [0; 4],
            symbol_mask,
            method: 1,
        });
    }
    if asyncio.machine == 183
        && runtime.machine == 183
        && build_id(asyncio) == KNOWN_ASYNCIO
        && build_id(runtime) == KNOWN_PYTHON
    {
        return Ok(Adaptation {
            offsets: KNOWN_OFFSETS,
            symbol_mask,
            method: 2,
        });
    }
    Err(format!(
        "missing Python probe symbols; no verified Build ID profile (asyncio={}, runtime={})",
        build_id(asyncio),
        build_id(runtime)
    ))
}

fn descendant(pid: i32, root: ProcessKey, processes: &HashMap<i32, Process>) -> bool {
    let mut current = pid;
    for _ in 0..128 {
        let Some(p) = processes.get(&current) else {
            return false;
        };
        if p.key == root {
            return pid != root.pid;
        }
        if p.parent <= 0 || p.parent == current {
            return false;
        }
        current = p.parent;
    }
    false
}

#[derive(Default)]
pub(super) struct Scanner {
    descendants: HashSet<ProcessKey>,
}
impl Scanner {
    pub fn scan(
        &mut self,
        root: &Path,
        previous: Option<ProcessKey>,
        stop: &AtomicBool,
        mut maintain_capture: impl FnMut() -> Result<(), String>,
    ) -> Result<Option<Target>, String> {
        // A host procfs mount alone cannot translate PID-scoped uprobe attachment.
        if fs::read_link(root.join("self"))
            .ok()
            .and_then(|p| p.to_str()?.parse::<i32>().ok())
            != Some(unsafe { libc::getpid() })
        {
            return Err(
                "procfs PID view differs from collector; host PID namespace is required".into(),
            );
        }
        let entries = fs::read_dir(root).map_err(|e| format!("cannot scan procfs: {e}"))?;
        let mut processes = HashMap::new();
        for (index, entry) in entries.flatten().enumerate() {
            if stop.load(Ordering::Acquire) {
                return Ok(None);
            }
            if index % 64 == 0 {
                maintain_capture()?;
            }
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|s| s.parse::<i32>().ok())
            else {
                continue;
            };
            if let Ok(p) = read_process(root, pid) {
                processes.insert(pid, p);
            }
        }
        self.descendants
            .retain(|k| processes.get(&k.pid).is_some_and(|p| p.key == *k));
        if let Some(parent) = previous {
            for p in processes.values() {
                if descendant(p.key.pid, parent, &processes) {
                    self.descendants.insert(p.key);
                }
            }
        }
        let mut candidates = Vec::new();
        let mut unreadable = 0;
        let mut inaccessible_markers = 0;
        for (index, p) in processes.values().enumerate() {
            if stop.load(Ordering::Acquire) {
                return Ok(None);
            }
            if index % 64 == 0 {
                maintain_capture()?;
            }
            if self.descendants.contains(&p.key) {
                continue;
            }
            match read_bounded(&root.join(format!("{}/maps", p.key.pid))) {
                Ok(text) => {
                    let maps: Vec<_> = text.lines().filter_map(mapping).collect();
                    if maps
                        .iter()
                        .filter(|m| m.path.file_name().is_some_and(|n| n == MARKER))
                        .any(|m| match MappedFile::open(root, p.key.pid, m) {
                            Ok(f) => inspect(&f.file).is_ok_and(|elf| elf.symbols & 0x70 == 0x70),
                            Err(_) => {
                                inaccessible_markers += 1;
                                false
                            }
                        })
                    {
                        candidates.push((*p, maps));
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => unreadable += 1,
                Err(_) => {} // Exiting processes race scanning normally.
            }
        }
        if inaccessible_markers != 0 {
            return Err(format!(
                "cannot access or verify {inaccessible_markers} mapped marker files"
            ));
        }
        let roots: Vec<_> = candidates
            .iter()
            .filter(|(p, _)| {
                !candidates
                    .iter()
                    .any(|(other, _)| descendant(p.key.pid, other.key, &processes))
            })
            .collect();
        if roots.len() > 1 {
            return Err(
                "multiple independent marker-bearing processes; refusing ambiguous target".into(),
            );
        }
        let Some((p, maps)) = roots.first().copied() else {
            return if unreadable == 0 {
                Ok(None)
            } else {
                Err(format!("cannot inspect {unreadable} process maps"))
            };
        };
        for child in processes.values() {
            if descendant(child.key.pid, p.key, &processes) {
                self.descendants.insert(child.key);
            }
        }
        let exe_file =
            File::open(root.join(format!("{}/exe", p.key.pid))).map_err(|e| e.to_string())?;
        let exe = FileStamp::from(exe_file.metadata().map_err(|e| e.to_string())?);
        let unique = |selected: Vec<&Mapping>| -> Result<Option<MappedFile>, String> {
            let mut identities = HashSet::new();
            let mut result = None;
            for map in selected {
                if identities.insert((map.major, map.minor, map.inode)) {
                    if result.is_some() {
                        return Err("ambiguous runtime mapping".into());
                    }
                    result =
                        Some(MappedFile::open(root, p.key.pid, map).map_err(|e| e.to_string())?);
                }
            }
            Ok(result)
        };
        let marker = unique(
            maps.iter()
                .filter(|m| m.path.file_name().is_some_and(|n| n == MARKER))
                .collect(),
        )?;
        let asyncio = unique(
            maps.iter()
                .filter(|m| {
                    m.path.file_name().is_some_and(|n| {
                        n.as_bytes().starts_with(b"_asyncio.") && n.as_bytes().ends_with(b".so")
                    })
                })
                .collect(),
        )?;
        let shared = maps
            .iter()
            .filter(|m| {
                m.path
                    .file_name()
                    .is_some_and(|n| n.as_bytes().starts_with(b"libpython3."))
            })
            .collect::<Vec<_>>();
        let runtime = if inspect(&exe_file)
            .is_ok_and(|elf| elf.python_version != 0 && elf.symbols & 4 != 0)
        {
            Some(MappedFile {
                file: exe_file,
                stamp: exe.clone(),
            })
        } else {
            unique(shared)?
        };
        let (Some(marker), Some(asyncio), Some(runtime)) = (marker, asyncio, runtime) else {
            return Ok(None);
        };
        let target = Target {
            process: p.key,
            exe,
            marker,
            asyncio,
            runtime,
        };
        if !target.unchanged(root) {
            return Err("process changed during discovery".into());
        }
        Ok(Some(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info(id: &str, symbols: u32) -> ElfInfo {
        let mut v = ElfInfo {
            machine: 183,
            python_version: 0x030b02f0,
            symbols,
            build_id: [0; 129],
        };
        v.build_id[..id.len()].copy_from_slice(id.as_bytes());
        v
    }
    #[test]
    fn profile_requires_both_exact_build_ids() {
        let a = info(KNOWN_ASYNCIO, 0);
        let r = info(KNOWN_PYTHON, 4);
        assert_eq!(select_adaptation(&a, &r).unwrap().offsets, KNOWN_OFFSETS);
        assert!(select_adaptation(&info("different", 0), &r).is_err());
        assert!(select_adaptation(&a, &info("different", 4)).is_err());
        assert!(select_adaptation(&info("", 0), &info("", 4)).is_err());
    }
    #[test]
    fn symbols_require_the_supported_probe_model() {
        assert_eq!(
            select_adaptation(&info("other", 3), &info("other", 12))
                .unwrap()
                .method,
            1
        );
        let mut r = info("other", 12);
        r.python_version = 0x030c00f0;
        assert!(select_adaptation(&info("other", 3), &r).is_err());
    }
    #[test]
    fn parses_proc_stat_with_parentheses_and_detects_pid_generation() {
        let stat = format!("42 (some ) odd name) S 7 {} 12345 0", ["0"; 17].join(" "));
        let p = process_stat(&stat).unwrap();
        assert_eq!(
            p.key,
            ProcessKey {
                pid: 42,
                start: 12345
            }
        );
        assert_eq!(p.parent, 7);
        assert_ne!(
            p.key,
            process_stat(&stat.replace("12345", "12346")).unwrap().key
        );
        assert!(process_stat(&stat.replace(") S ", ") Z ")).is_none());
    }
    #[test]
    fn parses_maps_with_spaces_and_rejects_deleted_mapping() {
        let line = "1000-2000 r-xp 00000000 08:01 100    /some path/libagentsight_tool_marker.so";
        assert_eq!(
            mapping(line).unwrap().path,
            PathBuf::from("/some path/libagentsight_tool_marker.so")
        );
        assert!(mapping(&format!("{line} (deleted)")).is_none());
        assert!(mapping(&line.replace("r-xp", "r--p")).is_none());
    }
    #[test]
    fn excludes_fork_descendants_through_intermediate_processes() {
        let root = ProcessKey { pid: 1, start: 10 };
        let processes = [
            (
                1,
                Process {
                    key: root,
                    parent: 0,
                },
            ),
            (
                2,
                Process {
                    key: ProcessKey { pid: 2, start: 20 },
                    parent: 1,
                },
            ),
            (
                3,
                Process {
                    key: ProcessKey { pid: 3, start: 30 },
                    parent: 2,
                },
            ),
        ]
        .into();
        assert!(descendant(3, root, &processes));
        assert!(!descendant(1, root, &processes));
        assert!(!descendant(3, ProcessKey { pid: 1, start: 11 }, &processes));
    }
}
