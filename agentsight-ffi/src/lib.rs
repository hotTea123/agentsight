//! Tool-span evidence only. No subprocess launcher, exporter, database or Tokio.
#![allow(clippy::missing_safety_doc)]

use std::{
    collections::VecDeque,
    ffi::{CStr, CString, c_char, c_void},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};

pub const AGENTSIGHT_ABI_VERSION: u32 = 1;
pub const AGENTSIGHT_EVENT_FILE_OPEN: u32 = 1;
pub const AGENTSIGHT_EVENT_FILE_WRITE: u32 = 2;
pub const AGENTSIGHT_EVENT_FILE_RENAME: u32 = 3;
pub const AGENTSIGHT_EVENT_PROCESS_EXEC: u32 = 4;
pub const AGENTSIGHT_STATE_INITIALIZED: u32 = 0;
pub const AGENTSIGHT_STATE_CONFIGURED: u32 = 1;
pub const AGENTSIGHT_STATE_RUNNING: u32 = 2;
pub const AGENTSIGHT_STATE_STOPPED: u32 = 3;
pub const AGENTSIGHT_STATE_FAILED: u32 = 4;
/// Ring-buffer drops and all correlation-map failures are NOT measured in v1.
pub const AGENTSIGHT_STATS_USER_QUEUE: u32 = 1;
/// Existing aggregation-map overflow counter, best effort (not total kernel loss).
pub const AGENTSIGHT_STATS_AGG_OVERFLOW: u32 = 2;

/// Zero-initialize, then set struct_size, abi_version and required fields.
/// Strings are borrowed for set_config only; the library copies them.
#[repr(C)]
pub struct AgentsightConfig {
    pub struct_size: u32,
    pub abi_version: u32,
    /// Host-visible PID, or zero to select a cgroup instead. No implicit trace-all.
    pub target_pid: i32,
    pub include_cgroup_children: u32,
    /// Zero selects 1024 events. Maximum 65536.
    pub queue_capacity: u32,
    pub reserved: u32,
    /// Zero selects 16 MiB. Bounds queued payload + event metadata, not all RSS.
    pub max_queue_bytes: u64,
    pub marker_path: *const c_char,
    pub asyncio_path: *const c_char,
    pub python_path: *const c_char,
    /// Optional absolute cgroup v2 path visible to the host process.
    pub cgroup_path: *const c_char,
    /// Explicit validated ELF file offsets; no universal CPython defaults.
    pub task_step_offset: u64,
    pub task_init_offset: u64,
    pub context_copy_offset: u64,
    pub context_run_offset: u64,
}

/// Immutable owned event. Release exactly once with agentsight_free_event.
/// IDs are bytes in network order, NOT strings. JSON retains operation details.
#[repr(C)]
pub struct AgentsightEvent {
    pub struct_size: u32,
    pub schema_version: u32,
    pub event_type: u32,
    pub trace_flags: u32,
    pub pid: u32,
    pub tid: u32,
    /// Kernel CLOCK_MONOTONIC timestamp; not a Unix timestamp.
    pub timestamp_ns: u64,
    /// Estimated Unix nanoseconds using a paired monotonic/realtime anchor.
    pub wall_time_ns: u64,
    /// Zero means unavailable, not a measured zero-duration operation.
    pub duration_ns: u64,
    pub trace_id: [u8; 16],
    /// The original tool span ID. A host creates a NEW evidence child span ID.
    pub parent_span_id: [u8; 8],
    pub payload_json: *const c_char,
    /// Excludes trailing NUL. Valid until free_event, including after handle free.
    pub payload_json_len: u32,
    pub reserved: u32,
}

/// Cumulative per handle, including across stop/start. Gauges reflect now.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct AgentsightStats {
    pub struct_size: u32,
    pub abi_version: u32,
    pub state: u32,
    pub supported_counters: u32,
    pub received_records: u64,
    pub filtered_records: u64,
    pub invalid_records: u64,
    pub enqueued_events: u64,
    pub delivered_events: u64,
    pub queue_dropped_events: u64,
    pub output_dropped_records: u64,
    pub backend_errors: u64,
    pub backend_diagnostics: u64,
    pub aggregate_overflow: u64,
    pub queued_events: u64,
    pub queued_bytes: u64,
}

#[repr(C)]
struct CoreConfig {
    pid: i32,
    cgroup_children: u32,
    marker_path: *const c_char,
    asyncio_path: *const c_char,
    python_path: *const c_char,
    cgroup_path: *const c_char,
    task_step_offset: u64,
    task_init_offset: u64,
    context_copy_offset: u64,
    context_run_offset: u64,
}
unsafe extern "C" {
    fn as_core_open(
        cfg: *const CoreConfig,
        output: unsafe extern "C" fn(*mut c_void, i32, *const c_char, usize),
        context: *mut c_void,
    ) -> i32;
    fn as_core_poll(timeout_ms: i32, wake_fd: i32) -> i32;
    fn as_core_close();
}

#[derive(Clone)]
struct Config {
    pid: i32,
    children: u32,
    capacity: usize,
    bytes: usize,
    paths: [CString; 4],
    offsets: [u64; 4],
}

#[repr(C)]
struct OwnedEvent {
    public: AgentsightEvent,
    _payload: CString,
}
// payload_json points into the owned CString's stable heap allocation. All
// access is immutable until the sole owner frees it; transfer uses the queue lock.
unsafe impl Send for OwnedEvent {}

struct Data {
    queue: VecDeque<Box<OwnedEvent>>,
    bytes: usize,
    capacity: usize,
    byte_limit: usize,
    stats: AgentsightStats,
    error: String,
    clock: Option<(u64, u64)>,
}

struct Shared {
    data: Mutex<Data>,
    state: AtomicU32,
    stop: AtomicBool,
    notification: OwnedFd,
    wake: OwnedFd,
}

struct Lifecycle {
    config: Option<Config>,
    worker: Option<JoinHandle<()>>,
}

/// Opaque handle. Never copy, dereference or use it after free. Forked children
/// must exec before using this library again; inherited handles return -ECHILD.
pub struct AgentsightHandle {
    creator_pid: i32,
    shared: Arc<Shared>,
    lifecycle: Mutex<Lifecycle>,
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
static LIBRARY_PID: AtomicI32 = AtomicI32::new(0);
struct Reservation;
impl Drop for Reservation {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}
struct CoreGuard;
impl Drop for CoreGuard {
    fn drop(&mut self) {
        unsafe { as_core_close() };
    }
}

type ApiResult = Result<i32, (i32, String)>;
fn error(code: i32, text: impl Into<String>) -> (i32, String) {
    (-code.abs(), text.into())
}
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn eventfd() -> std::io::Result<OwnedFd> {
    let fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK | libc::EFD_CLOEXEC) };
    if fd < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

fn notify(fd: i32) {
    let one: u64 = 1;
    loop {
        let n = unsafe { libc::write(fd, (&one as *const u64).cast(), 8) };
        if n >= 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            break;
        }
    }
    // EAGAIN means it is already readable. The library owns both eventfds.
}

fn clear_notification(fd: i32) {
    let mut value: u64 = 0;
    loop {
        let n = unsafe { libc::read(fd, (&mut value as *mut u64).cast(), 8) };
        if n >= 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            break;
        }
    }
}

unsafe fn with_handle(
    h: *mut AgentsightHandle,
    f: impl FnOnce(&AgentsightHandle) -> ApiResult,
) -> i32 {
    if h.is_null() {
        return -libc::EINVAL;
    }
    // Only checks valid library handles. Arbitrary/dangling pointers are caller UB.
    let handle = unsafe { &*h };
    if handle.creator_pid != unsafe { libc::getpid() } {
        return -libc::ECHILD;
    }
    match catch_unwind(AssertUnwindSafe(|| f(handle))) {
        Ok(Ok(value)) => value,
        Ok(Err((code, message))) => {
            lock(&handle.shared.data).error = message;
            code
        }
        Err(_) => {
            lock(&handle.shared.data).error = "internal Rust panic (operation failed)".into();
            -libc::EFAULT
        }
    }
}

unsafe fn copy_path(
    p: *const c_char,
    required: bool,
    limit: usize,
) -> Result<CString, (i32, String)> {
    if p.is_null() {
        return if required {
            Err(error(libc::EINVAL, "required path is NULL"))
        } else {
            Ok(CString::default())
        };
    }
    let value = unsafe { CStr::from_ptr(p) };
    if !required && value.is_empty() {
        return Ok(CString::default());
    }
    if value.to_bytes().first() != Some(&b'/') || value.to_bytes().len() >= limit {
        return Err(error(
            libc::EINVAL,
            "paths must be absolute and fit the documented limits",
        ));
    }
    Ok(value.to_owned())
}

unsafe fn config_from_abi(c: *const AgentsightConfig) -> Result<Config, (i32, String)> {
    if c.is_null() {
        return Err(error(libc::EINVAL, "config is NULL"));
    }
    // Read size before accessing the remaining fields.
    if unsafe { ptr::addr_of!((*c).struct_size).read() } != size_of::<AgentsightConfig>() as u32 {
        return Err(error(libc::EINVAL, "config struct_size mismatch"));
    }
    let c = unsafe { &*c };
    if c.abi_version != AGENTSIGHT_ABI_VERSION
        || c.reserved != 0
        || c.target_pid < 0
        || c.include_cgroup_children > 1
    {
        return Err(error(
            libc::EINVAL,
            "invalid ABI version, reserved field or target",
        ));
    }
    let paths = unsafe {
        [
            copy_path(c.marker_path, true, 4096)?,
            copy_path(c.asyncio_path, true, 4096)?,
            copy_path(c.python_path, true, 4096)?,
            copy_path(c.cgroup_path, false, 256)?,
        ]
    };
    if (c.target_pid == 0 || c.include_cgroup_children != 0) && paths[3].is_empty() {
        return Err(error(
            libc::EINVAL,
            "a target PID or cgroup is required; children requires cgroup",
        ));
    }
    let offsets = [
        c.task_step_offset,
        c.task_init_offset,
        c.context_copy_offset,
        c.context_run_offset,
    ];
    if offsets.contains(&0) {
        return Err(error(
            libc::EINVAL,
            "all four validated CPython offsets are required",
        ));
    }
    let capacity = if c.queue_capacity == 0 {
        1024
    } else {
        c.queue_capacity as usize
    };
    let bytes = if c.max_queue_bytes == 0 {
        16 * 1024 * 1024
    } else {
        c.max_queue_bytes
    };
    if capacity > 65536 || !(1024..=1024 * 1024 * 1024).contains(&bytes) {
        return Err(error(libc::EINVAL, "queue limit outside supported range"));
    }
    Ok(Config {
        pid: c.target_pid,
        children: c.include_cgroup_children,
        capacity,
        bytes: bytes as usize,
        paths,
        offsets,
    })
}

fn hex_id<const N: usize>(s: Option<&str>) -> Option<[u8; N]> {
    let s = s?;
    if s.len() != N * 2 {
        return None;
    }
    let mut out = [0; N];
    for (i, bytes) in s.as_bytes().chunks_exact(2).enumerate() {
        let digit = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        };
        out[i] = digit(bytes[0])? * 16 + digit(bytes[1])?;
    }
    if out.iter().all(|v| *v == 0) {
        None
    } else {
        Some(out)
    }
}

fn receive(shared: &Shared, kind: i32, bytes: &[u8]) {
    let mut d = lock(&shared.data);
    if kind == 1 {
        d.stats.backend_diagnostics += 1;
        d.error = String::from_utf8_lossy(bytes).trim().to_owned();
        return;
    }
    if kind == 2 {
        d.stats.output_dropped_records += 1;
        return;
    }
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    d.stats.received_records += 1;
    let v: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => {
            d.stats.invalid_records += 1;
            return;
        }
    };
    let name = v["event"].as_str().unwrap_or("");
    if name == "CLOCK_SYNC" {
        if let (Some(m), Some(w)) = (v["mono_ns"].as_u64(), v["wall_time_ns"].as_u64()) {
            d.clock = Some((m, w));
        } else {
            d.stats.invalid_records += 1;
        }
        return;
    }
    if v["type"] == "AGG_MAP_OVERFLOW" {
        d.stats.aggregate_overflow += v["overflow_count"].as_u64().unwrap_or(0);
        return;
    }
    let event_type = match name {
        "FILE_OPEN" => AGENTSIGHT_EVENT_FILE_OPEN,
        "FILE_WRITE" => AGENTSIGHT_EVENT_FILE_WRITE,
        "FILE_RENAME" => AGENTSIGHT_EVENT_FILE_RENAME,
        "EXEC" => AGENTSIGHT_EVENT_PROCESS_EXEC,
        _ => {
            d.stats.filtered_records += 1;
            return;
        }
    };
    let flags = v["agentsight_trace_flags"].as_u64().unwrap_or(0);
    let ids = (
        hex_id::<16>(v["agentsight_trace_id"].as_str()),
        hex_id::<8>(v["agentsight_parent_span_id"].as_str()),
    );
    let (Some(trace_id), Some(parent_span_id)) = ids else {
        d.stats.filtered_records += 1;
        return;
    };
    if flags & 1 == 0 {
        d.stats.filtered_records += 1;
        return;
    }
    let Some(timestamp) = v["timestamp"].as_u64() else {
        d.stats.invalid_records += 1;
        return;
    };
    let Some((mono, wall)) = d.clock else {
        d.stats.invalid_records += 1;
        return;
    };
    let epoch = if timestamp >= mono {
        wall.checked_add(timestamp - mono)
    } else {
        wall.checked_sub(mono - timestamp)
    };
    let Some(epoch) = epoch else {
        d.stats.invalid_records += 1;
        return;
    };
    let footprint = bytes.len() + 1 + size_of::<OwnedEvent>();
    if d.queue.len() >= d.capacity || footprint > d.byte_limit.saturating_sub(d.bytes) {
        d.stats.queue_dropped_events += 1;
        return;
    }
    let payload = match CString::new(bytes) {
        Ok(p) => p,
        Err(_) => {
            d.stats.invalid_records += 1;
            return;
        }
    };
    let event = Box::new(OwnedEvent {
        public: AgentsightEvent {
            struct_size: size_of::<AgentsightEvent>() as u32,
            schema_version: 1,
            event_type,
            trace_flags: flags as u32,
            pid: v["pid"].as_u64().unwrap_or(0) as u32,
            tid: v["tid"].as_u64().unwrap_or(0) as u32,
            timestamp_ns: timestamp,
            wall_time_ns: epoch,
            duration_ns: v["duration_ns"].as_u64().unwrap_or(0),
            trace_id,
            parent_span_id,
            payload_json: payload.as_ptr(),
            payload_json_len: bytes.len() as u32,
            reserved: 0,
        },
        _payload: payload,
    });
    let empty = d.queue.is_empty();
    d.bytes += footprint;
    d.queue.push_back(event);
    d.stats.enqueued_events += 1;
    if empty {
        notify(shared.notification.as_raw_fd());
    }
}

unsafe extern "C" fn core_output(context: *mut c_void, kind: i32, data: *const c_char, len: usize) {
    // Callback is private: C invokes it synchronously on the Rust worker only.
    let s = unsafe { &*(context as *const Shared) };
    let result = catch_unwind(AssertUnwindSafe(|| {
        if !data.is_null() && len <= 65536 {
            receive(s, kind, unsafe {
                std::slice::from_raw_parts(data.cast(), len)
            });
        }
    }));
    if result.is_err() {
        lock(&s.data).stats.invalid_records += 1;
    }
}

fn worker(s: Arc<Shared>, cfg: Config, ready: mpsc::SyncSender<i32>, _reservation: Reservation) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _core = CoreGuard;
        let raw = CoreConfig {
            pid: cfg.pid,
            cgroup_children: cfg.children,
            marker_path: cfg.paths[0].as_ptr(),
            asyncio_path: cfg.paths[1].as_ptr(),
            python_path: cfg.paths[2].as_ptr(),
            cgroup_path: cfg.paths[3].as_ptr(),
            task_step_offset: cfg.offsets[0],
            task_init_offset: cfg.offsets[1],
            context_copy_offset: cfg.offsets[2],
            context_run_offset: cfg.offsets[3],
        };
        let rc = unsafe { as_core_open(&raw, core_output, Arc::as_ptr(&s) as *mut c_void) };
        if rc != 0 {
            return rc;
        }
        s.state.store(AGENTSIGHT_STATE_RUNNING, Ordering::Release);
        if ready.send(0).is_err() {
            return -libc::ECANCELED;
        }
        while !s.stop.load(Ordering::Acquire) {
            let rc = unsafe { as_core_poll(250, s.wake.as_raw_fd()) };
            if rc < 0 {
                return rc;
            }
        }
        0
    }));
    let rc = result.unwrap_or(-libc::EFAULT);
    if rc != 0 {
        let mut d = lock(&s.data);
        d.stats.backend_errors += 1;
        let diagnostic = d.error.clone();
        d.error = format!("capture failed ({rc}): {diagnostic}");
        s.state.store(AGENTSIGHT_STATE_FAILED, Ordering::Release);
    } else {
        s.state.store(AGENTSIGHT_STATE_STOPPED, Ordering::Release);
    }
    // On startup failure, cleanup has finished before the caller is released.
    if rc != 0 {
        let _ = ready.try_send(rc);
    }
    notify(s.notification.as_raw_fd());
}

fn stop(h: &AgentsightHandle, life: &mut Lifecycle) -> ApiResult {
    h.shared.stop.store(true, Ordering::Release);
    notify(h.shared.wake.as_raw_fd());
    if let Some(t) = life.worker.take() {
        if t.join().is_err() {
            return Err(error(libc::EFAULT, "capture worker panicked"));
        }
    }
    Ok(0)
}

/// Returns the C ABI version, not the separate tool-marker ABI version.
#[unsafe(no_mangle)]
pub extern "C" fn agentsight_abi_version() -> u32 {
    AGENTSIGHT_ABI_VERSION
}

/// Creates an inactive handle. Returns 0 or negative errno; initializes *out to NULL on failure.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_init(out: *mut *mut AgentsightHandle) -> i32 {
    if out.is_null() {
        return -libc::EINVAL;
    }
    unsafe { out.write(ptr::null_mut()) };
    let pid = unsafe { libc::getpid() };
    if let Err(owner) = LIBRARY_PID.compare_exchange(0, pid, Ordering::AcqRel, Ordering::Acquire) {
        if owner != pid {
            return -libc::ECHILD;
        }
    }
    catch_unwind(AssertUnwindSafe(|| {
        let notification = match eventfd() {
            Ok(v) => v,
            Err(e) => return -e.raw_os_error().unwrap_or(libc::EIO),
        };
        let wake = match eventfd() {
            Ok(v) => v,
            Err(e) => return -e.raw_os_error().unwrap_or(libc::EIO),
        };
        let h = Box::new(AgentsightHandle {
            creator_pid: unsafe { libc::getpid() },
            lifecycle: Mutex::new(Lifecycle {
                config: None,
                worker: None,
            }),
            shared: Arc::new(Shared {
                data: Mutex::new(Data {
                    queue: VecDeque::new(),
                    bytes: 0,
                    capacity: 1024,
                    byte_limit: 16 * 1024 * 1024,
                    stats: AgentsightStats::default(),
                    error: String::new(),
                    clock: None,
                }),
                state: AtomicU32::new(AGENTSIGHT_STATE_INITIALIZED),
                stop: AtomicBool::new(false),
                notification,
                wake,
            }),
        });
        unsafe { out.write(Box::into_raw(h)) };
        0
    }))
    .unwrap_or(-libc::EFAULT)
}

/// Copies configuration. Allowed only with no worker and an empty event queue.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_set_config(
    h: *mut AgentsightHandle,
    config: *const AgentsightConfig,
) -> i32 {
    unsafe {
        with_handle(h, |h| {
            let mut life = lock(&h.lifecycle);
            if life.worker.is_some() {
                return Err(error(libc::EBUSY, "stop before changing configuration"));
            }
            let cfg = config_from_abi(config)?;
            let mut d = lock(&h.shared.data);
            if !d.queue.is_empty() {
                return Err(error(
                    libc::EBUSY,
                    "drain events before changing configuration",
                ));
            }
            d.capacity = cfg.capacity;
            d.byte_limit = cfg.bytes;
            life.config = Some(cfg);
            h.shared
                .state
                .store(AGENTSIGHT_STATE_CONFIGURED, Ordering::Release);
            Ok(0)
        })
    }
}

/// Starts one in-process worker. Success means BPF/probes/ring-buffer are ready.
/// Idempotent while running. After stop, drain events before restarting.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_start(h: *mut AgentsightHandle) -> i32 {
    unsafe {
        with_handle(h, |h| {
            let mut life = lock(&h.lifecycle);
            if life.worker.is_some() {
                return if h.shared.state.load(Ordering::Acquire) == AGENTSIGHT_STATE_RUNNING {
                    Ok(0)
                } else {
                    Err(error(
                        libc::EBUSY,
                        "stop the previous worker before restarting",
                    ))
                };
            }
            let cfg = life
                .config
                .clone()
                .ok_or_else(|| error(libc::EINVAL, "set_config is required before start"))?;
            {
                let mut d = lock(&h.shared.data);
                if !d.queue.is_empty() {
                    return Err(error(libc::EBUSY, "drain events before restarting"));
                }
                d.clock = None;
                d.error.clear();
            }
            if ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Err(error(
                    libc::EBUSY,
                    "another capture instance is active in this process",
                ));
            }
            let reservation = Reservation;
            clear_notification(h.shared.wake.as_raw_fd());
            clear_notification(h.shared.notification.as_raw_fd());
            h.shared.stop.store(false, Ordering::Release);
            let s = Arc::clone(&h.shared);
            let (tx, rx) = mpsc::sync_channel(1);
            let t = thread::Builder::new()
                .name("agentsight-bpf".into())
                .spawn(move || worker(s, cfg, tx, reservation))
                .map_err(|e| error(e.raw_os_error().unwrap_or(libc::EAGAIN), e.to_string()))?;
            life.worker = Some(t);
            let rc = rx.recv().unwrap_or(-libc::EFAULT);
            if rc != 0 {
                let _ = stop(h, &mut life);
                return Ok(rc);
            }
            Ok(0)
        })
    }
}

/// Stops and joins the producer, detaches probes and drains kernel aggregates.
/// Queued events survive; stopping does not preserve live tool bindings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_stop(h: *mut AgentsightHandle) -> i32 {
    unsafe { with_handle(h, |h| stop(h, &mut lock(&h.lifecycle))) }
}

/// Stops if needed, discards queued events and frees the handle. NULL is allowed.
/// Must not race ANY call using this handle. Outstanding returned events survive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_free(h: *mut AgentsightHandle) -> i32 {
    if h.is_null() {
        return 0;
    }
    let rc = unsafe { agentsight_stop(h) };
    if rc != 0 {
        return rc;
    }
    catch_unwind(AssertUnwindSafe(|| {
        unsafe { drop(Box::from_raw(h)) };
        0
    }))
    .unwrap_or(-libc::EFAULT)
}

/// Borrowed nonblocking eventfd. Poll/epoll only: do not read, close or write it.
/// The library acknowledges/re-arms it in poll_events. Valid until handle free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_get_eventfd(h: *mut AgentsightHandle) -> i32 {
    unsafe { with_handle(h, |h| Ok(h.shared.notification.as_raw_fd())) }
}

/// Nonblocking batch read. Returns count (0 = empty) or negative errno.
/// out_events points to capacity writable pointer slots (capacity 1..65536).
/// Each returned pointer must be freed exactly once; unused slots are untouched.
/// Use one consumer per handle. For cgo, allocate the pointer array in C memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_poll_events(
    h: *mut AgentsightHandle,
    out_events: *mut *mut AgentsightEvent,
    capacity: u32,
) -> i32 {
    unsafe {
        with_handle(h, |h| {
            if out_events.is_null() || capacity == 0 || capacity > 65536 {
                return Err(error(libc::EINVAL, "invalid output array or capacity"));
            }
            let mut d = lock(&h.shared.data);
            let count = (capacity as usize).min(d.queue.len());
            for i in 0..count {
                let event = d.queue.pop_front().unwrap();
                d.bytes -= event.public.payload_json_len as usize + 1 + size_of::<OwnedEvent>();
                out_events.add(i).write(Box::into_raw(event).cast());
            }
            d.stats.delivered_events += count as u64;
            clear_notification(h.shared.notification.as_raw_fd());
            if !d.queue.is_empty() {
                notify(h.shared.notification.as_raw_fd());
            }
            Ok(count as i32)
        })
    }
}

/// Releases one event allocated by poll_events; NULL is allowed. Never use free().
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_free_event(event: *mut AgentsightEvent) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !event.is_null() {
            unsafe { drop(Box::from_raw(event.cast::<OwnedEvent>())) };
        }
    }));
}

/// Copies the last diagnostic to caller memory; safe across cgo OS-thread changes.
/// Returns 0, -ENOSPC (NUL-terminated truncation), or another negative errno.
/// Concurrent failures can replace this text; the operation's return code is authoritative.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_get_last_error(
    h: *mut AgentsightHandle,
    buffer: *mut c_char,
    capacity: usize,
) -> i32 {
    unsafe {
        with_handle(h, |h| {
            if buffer.is_null() || capacity == 0 {
                return Ok(-libc::EINVAL);
            }
            let d = lock(&h.shared.data);
            let text = d.error.as_bytes();
            let n = text.len().min(capacity - 1);
            ptr::copy_nonoverlapping(text.as_ptr(), buffer.cast(), n);
            buffer.add(n).write(0);
            Ok(if n == text.len() { 0 } else { -libc::ENOSPC })
        })
    }
}

/// Copies a snapshot into a buffer of at least sizeof(AgentsightStats) bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_get_stats(
    h: *mut AgentsightHandle,
    stats: *mut AgentsightStats,
    stats_size: usize,
) -> i32 {
    unsafe {
        with_handle(h, |h| {
            if stats.is_null() || stats_size < size_of::<AgentsightStats>() {
                return Err(error(libc::EINVAL, "stats buffer too small"));
            }
            let d = lock(&h.shared.data);
            let mut snapshot = d.stats;
            snapshot.struct_size = size_of::<AgentsightStats>() as u32;
            snapshot.abi_version = AGENTSIGHT_ABI_VERSION;
            snapshot.state = h.shared.state.load(Ordering::Acquire);
            snapshot.supported_counters =
                AGENTSIGHT_STATS_USER_QUEUE | AGENTSIGHT_STATS_AGG_OVERFLOW;
            snapshot.queued_events = d.queue.len() as u64;
            snapshot.queued_bytes = d.bytes as u64;
            stats.write(snapshot);
            Ok(0)
        })
    }
}

#[cfg(test)]
mod tests;
