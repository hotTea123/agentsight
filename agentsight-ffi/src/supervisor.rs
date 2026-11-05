use super::*;
use discovery::{ProcessKey, Scanner, Target};
use std::{
    os::unix::ffi::OsStrExt,
    path::Path,
    time::{Duration, Instant},
};

struct Active {
    // Detach and flush before releasing pinned target ELF files.
    _guard: CoreGuard,
    target: Target,
}

fn progress(s: &Shared, state: u32, target: Option<ProcessKey>, code: i32, message: &str) {
    let mut d = lock(&s.data);
    let pid = target.map_or(0, |t| t.pid);
    let changed = d.stats.target_state != state
        || d.stats.target_pid != pid
        || d.stats.target_error != code
        || (code != 0 && d.error != message);
    d.stats.target_state = state;
    d.stats.target_pid = pid;
    d.stats.target_start_ticks = target.map_or(0, |t| t.start);
    d.stats.target_error = code;
    if state != AGENTSIGHT_TARGET_READY {
        d.stats.adaptation_method = 0;
    }
    if changed {
        d.error = message.into();
    }
    drop(d);
    if changed {
        notify(s.notification.as_raw_fd());
    }
}

fn detach(s: &Shared, active: &mut Option<Active>) {
    if active.take().is_some() {
        lock(&s.data).stats.target_detaches += 1;
    }
}

pub(super) fn run(s: &Arc<Shared>, cfg: &Config, ready: &mpsc::SyncSender<i32>) -> i32 {
    let root = Path::new(std::ffi::OsStr::from_bytes(cfg.procfs_root.as_bytes()));
    let mut scanner = Scanner::default();
    let mut active: Option<Active> = None;
    let mut previous = None;
    let interval = Duration::from_millis(cfg.interval_ms as u64);
    let mut next_scan = Instant::now();
    s.state.store(AGENTSIGHT_STATE_RUNNING, Ordering::Release);
    progress(s, AGENTSIGHT_TARGET_WAITING, None, 0, "");
    if ready.send(0).is_err() {
        return -libc::ECANCELED;
    }
    while !s.stop.load(Ordering::Acquire) {
        if let Some(current) = active.as_ref() {
            let rc = unsafe { as_core_poll(100, s.wake.as_raw_fd()) };
            let changed = unsafe { as_core_target_changed() };
            if rc < 0 || changed != 0 || !current.target.unchanged(root) {
                detach(s, &mut active);
                if rc < 0 || changed < 0 {
                    lock(&s.data).stats.backend_errors += 1;
                    progress(
                        s,
                        AGENTSIGHT_TARGET_DEGRADED,
                        previous,
                        rc.min(changed),
                        "capture backend failed; retrying",
                    );
                    next_scan = Instant::now() + interval;
                } else {
                    progress(s, AGENTSIGHT_TARGET_WAITING, None, 0, "");
                    next_scan = Instant::now();
                }
            }
        }
        if s.stop.load(Ordering::Acquire) {
            break;
        }
        if Instant::now() >= next_scan {
            lock(&s.data).stats.discovery_scans += 1;
            // A large node scan must not starve the same thread's ring reader.
            let discovered = scanner.scan(root, previous, &s.stop, || {
                if active.is_some() {
                    let rc = unsafe { as_core_poll(0, s.wake.as_raw_fd()) };
                    if rc < 0 {
                        lock(&s.data).stats.backend_errors += 1;
                        return Err(format!("capture poll failed during discovery: {rc}"));
                    }
                }
                Ok(())
            });
            next_scan = Instant::now() + interval;
            match discovered {
                Err(message) => {
                    detach(s, &mut active);
                    lock(&s.data).stats.discovery_errors += 1;
                    progress(
                        s,
                        AGENTSIGHT_TARGET_DEGRADED,
                        previous,
                        -libc::EAGAIN,
                        &message,
                    );
                }
                Ok(None) => {
                    detach(s, &mut active);
                    progress(s, AGENTSIGHT_TARGET_WAITING, None, 0, "");
                }
                Ok(Some(target)) => {
                    if active
                        .as_ref()
                        .is_some_and(|a| a.target.same_instance(&target))
                    {
                        continue;
                    }
                    detach(s, &mut active);
                    previous = Some(target.process);
                    let adaptation = match target.adapt() {
                        Ok(a) => a,
                        Err(message) => {
                            progress(
                                s,
                                AGENTSIGHT_TARGET_DEGRADED,
                                previous,
                                -libc::ENOTSUP,
                                &message,
                            );
                            continue;
                        }
                    };
                    progress(s, AGENTSIGHT_TARGET_ATTACHING, previous, 0, "");
                    let paths = target.paths();
                    let raw = CoreConfig {
                        pid: target.process.pid,
                        cgroup_children: 0,
                        marker_path: paths[0].as_ptr(),
                        asyncio_path: paths[1].as_ptr(),
                        python_path: paths[2].as_ptr(),
                        cgroup_path: ptr::null(),
                        task_step_offset: adaptation.offsets[0],
                        task_init_offset: adaptation.offsets[1],
                        context_copy_offset: adaptation.offsets[2],
                        context_run_offset: adaptation.offsets[3],
                        symbol_mask: adaptation.symbol_mask,
                        procfs_root: cfg.procfs_root.as_ptr(),
                    };
                    lock(&s.data).stats.attach_attempts += 1;
                    let guard = CoreGuard;
                    let rc =
                        unsafe { as_core_open(&raw, core_output, Arc::as_ptr(s) as *mut c_void) };
                    if rc != 0
                        || unsafe { as_core_target_changed() } != 0
                        || !target.unchanged(root)
                    {
                        drop(guard);
                        let message = lock(&s.data).error.clone();
                        lock(&s.data).stats.attach_failures += 1;
                        progress(
                            s,
                            AGENTSIGHT_TARGET_DEGRADED,
                            previous,
                            if rc == 0 { -libc::EAGAIN } else { rc },
                            &format!("attach failed or target changed: {message}"),
                        );
                        continue;
                    }
                    {
                        let mut d = lock(&s.data);
                        d.stats.target_generation += 1;
                        d.stats.adaptation_method = adaptation.method;
                    }
                    active = Some(Active {
                        _guard: guard,
                        target,
                    });
                    progress(s, AGENTSIGHT_TARGET_READY, previous, 0, "");
                }
            }
        }
        if active.is_none() {
            let timeout = next_scan
                .saturating_duration_since(Instant::now())
                .as_millis()
                .min(250) as i32;
            let mut fd = libc::pollfd {
                fd: s.wake.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            unsafe {
                libc::poll(&mut fd, 1, timeout);
            }
        }
    }
    detach(s, &mut active);
    progress(s, AGENTSIGHT_TARGET_IDLE, None, 0, "");
    0
}
