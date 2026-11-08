//! Explicitly selected, uncorrelated test capture. Never compiled into the production library.
use super::*;
use std::{os::unix::ffi::OsStrExt, path::Path};

unsafe extern "C" {
    fn as_core_test_open(
        pid: i32,
        start_ticks: u64,
        procfs_root: *const c_char,
        output: unsafe extern "C" fn(*mut c_void, i32, *const c_char, usize),
        context: *mut c_void,
    ) -> i32;
}

/// Test-only extension; see include/agentsight_test.h. Requires set_config first.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn agentsight_set_test_target(
    h: *mut AgentsightHandle,
    target_pid: i32,
) -> i32 {
    unsafe {
        with_handle(h, |h| {
            let mut life = lock(&h.lifecycle);
            if life.worker.is_some() || !lock(&h.shared.data).queue.is_empty() {
                return Err(error(
                    libc::EBUSY,
                    "stop and drain before changing the test target",
                ));
            }
            if target_pid <= 1 || target_pid == libc::getpid() {
                return Err(error(
                    libc::EINVAL,
                    "use a dedicated test process, not PID 0/1/self",
                ));
            }
            let cfg = life
                .config
                .as_mut()
                .ok_or_else(|| error(libc::EINVAL, "set_config first"))?;
            let root = Path::new(std::ffi::OsStr::from_bytes(cfg.procfs_root.as_bytes()));
            if std::fs::read_link(root.join("self"))
                .ok()
                .and_then(|p| p.to_str()?.parse::<i32>().ok())
                != Some(libc::getpid())
            {
                return Err(error(
                    libc::EINVAL,
                    "procfs PID view must match the collector",
                ));
            }
            let key = discovery::process_key(root, target_pid)
                .map_err(|e| error(libc::ESRCH, format!("cannot inspect test target: {e}")))?;
            if discovery::collector_descends_from(root, key).map_err(|e| {
                error(
                    libc::EINVAL,
                    format!("cannot verify collector exclusion: {e}"),
                )
            })? {
                return Err(error(
                    libc::EINVAL,
                    "test target must not be an ancestor of the collector",
                ));
            }
            cfg.test_target = Some(key);
            Ok(0)
        })
    }
}

// Keep missing path/result information missing. Legacy summaries are not individual syscalls.
pub(super) fn normalize(mut v: serde_json::Value) -> Option<(serde_json::Value, Vec<u8>)> {
    match (v["event"].as_str()?, v["type"].as_str()) {
        ("FILE_OPEN" | "EXEC", _) => {}
        ("SUMMARY", Some("WRITE")) => {
            v["event"] = "FILE_WRITE".into();
            if v["path_resolved"] == true {
                v["filepath"] = v["detail"].clone();
            }
            v["aggregation"] = true.into();
        }
        ("SUMMARY", Some("FILE_RENAME")) => {
            v["event"] = "FILE_RENAME".into();
            // Legacy directory aggregation retains only the most recent destination.
            v["last_destination_path"] = v["extra"].clone();
            v["aggregation"] = true.into();
        }
        _ => return None,
    }
    v["agentsight_test_only"] = true.into();
    v["agentsight_correlation_source"] = "none".into();
    // Never allow supplied/leftover context to look like a real tool association.
    for key in [
        "agentsight_trace_id",
        "agentsight_parent_span_id",
        "agentsight_trace_flags",
    ] {
        v.as_object_mut()?.remove(key);
    }
    let bytes = serde_json::to_vec(&v).ok()?;
    Some((v, bytes))
}

fn progress(s: &Shared, target: discovery::ProcessKey, state: u32, code: i32, message: &str) {
    let mut d = lock(&s.data);
    d.stats.target_pid = target.pid;
    d.stats.target_start_ticks = target.start;
    d.stats.target_state = state;
    d.stats.target_error = code;
    d.error = message.to_owned();
    drop(d);
    notify(s.notification.as_raw_fd());
}

pub(super) fn run(s: &Arc<Shared>, cfg: &Config, ready: &mpsc::SyncSender<i32>) -> i32 {
    let target = cfg.test_target.expect("start validates test target");
    let root = Path::new(std::ffi::OsStr::from_bytes(cfg.procfs_root.as_bytes()));
    let unchanged = || discovery::process_key(root, target.pid).ok() == Some(target);
    s.state.store(AGENTSIGHT_STATE_RUNNING, Ordering::Release);
    if ready.send(0).is_err() {
        return -libc::ECANCELED;
    }
    if !unchanged() {
        progress(
            s,
            target,
            AGENTSIGHT_TARGET_DEGRADED,
            -libc::ESRCH,
            "test target exited or was replaced",
        );
        return -libc::ESRCH;
    }
    progress(s, target, AGENTSIGHT_TARGET_ATTACHING, 0, "");
    lock(&s.data).stats.attach_attempts += 1;
    let guard = CoreGuard;
    let rc = unsafe {
        as_core_test_open(
            target.pid,
            target.start,
            cfg.procfs_root.as_ptr(),
            core_output,
            Arc::as_ptr(s) as *mut c_void,
        )
    };
    if rc != 0 {
        let message = lock(&s.data).error.clone();
        drop(guard);
        lock(&s.data).stats.attach_failures += 1;
        progress(s, target, AGENTSIGHT_TARGET_DEGRADED, rc, &message);
        return rc;
    }
    lock(&s.data).stats.target_generation += 1;
    progress(s, target, AGENTSIGHT_TARGET_READY, 0, "");
    let mut result = 0;
    while !s.stop.load(Ordering::Acquire) {
        if !unchanged() {
            progress(
                s,
                target,
                AGENTSIGHT_TARGET_DEGRADED,
                -libc::ESRCH,
                "test target exited; select a new target explicitly",
            );
            break;
        }
        let rc = unsafe { as_core_poll(100, s.wake.as_raw_fd()) };
        if rc < 0 {
            progress(
                s,
                target,
                AGENTSIGHT_TARGET_DEGRADED,
                rc,
                "test capture polling failed",
            );
            result = rc;
            break;
        }
    }
    drop(guard); // Detach, drain ring and flush aggregates before STOPPED.
    lock(&s.data).stats.target_detaches += 1;
    if s.stop.load(Ordering::Acquire) {
        progress(s, target, AGENTSIGHT_TARGET_IDLE, 0, "");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_events_have_no_fake_context_and_keep_unknown_fields_absent() {
        let (v, _) = normalize(serde_json::json!({"event":"SUMMARY","type":"FILE_RENAME",
            "extra":"/tmp/new","count":2,"agentsight_trace_id":"fake"}))
        .unwrap();
        assert_eq!(v["event"], "FILE_RENAME");
        assert_eq!(v["last_destination_path"], "/tmp/new");
        assert!(v.get("syscall_return").is_none());
        assert!(v.get("source_path").is_none());
        assert!(v.get("agentsight_trace_id").is_none());
        assert_eq!(v["agentsight_test_only"], true);
        assert!(normalize(serde_json::json!({"event":"SUMMARY","type":"NET_CONNECT"})).is_none());
    }

    #[test]
    fn test_queue_notification_bounds_ownership_and_target_validation() {
        unsafe {
            let mut h = ptr::null_mut();
            assert_eq!(agentsight_init(&mut h), 0);
            assert_eq!(agentsight_set_test_target(h, 0), -libc::EINVAL);
            assert_eq!(agentsight_set_test_target(h, libc::getpid()), -libc::EINVAL);
            let mut cfg: AgentsightConfig = std::mem::zeroed();
            cfg.struct_size = size_of::<AgentsightConfig>() as u32;
            cfg.abi_version = AGENTSIGHT_ABI_VERSION;
            cfg.queue_capacity = 1;
            assert_eq!(agentsight_set_config(h, &cfg), 0);
            assert_eq!(agentsight_start(h), -libc::EINVAL);
            assert_eq!(agentsight_set_test_target(h, i32::MAX), -libc::ESRCH);
            let s = &(*h).shared;
            receive(
                s,
                0,
                br#"{"event":"CLOCK_SYNC","mono_ns":100,"wall_time_ns":1000}"#,
            );
            for _ in 0..2 {
                receive(
                    s,
                    0,
                    br#"{"event":"FILE_OPEN","timestamp":110,"pid":12,"filepath":"/test"}"#,
                );
            }
            assert_eq!(lock(&s.data).stats.queue_dropped_events, 1);
            let mut p = libc::pollfd {
                fd: agentsight_get_eventfd(h),
                events: libc::POLLIN,
                revents: 0,
            };
            assert_eq!(libc::poll(&mut p, 1, 0), 1);
            let mut e = ptr::null_mut();
            assert_eq!(agentsight_poll_events(h, &mut e, 1), 1);
            assert_eq!((*e).schema_version, 1001);
            assert_eq!((*e).trace_id, [0; 16]);
            assert_eq!((*e).parent_span_id, [0; 8]);
            assert_eq!((*e).trace_flags, 0);
            assert_eq!((*e).wall_time_ns, 1010);
            assert_eq!(libc::poll(&mut p, 1, 0), 0);
            assert_eq!(agentsight_free(h), 0);
            assert!(
                CStr::from_ptr((*e).payload_json)
                    .to_str()
                    .unwrap()
                    .contains("agentsight_test_only")
            );
            agentsight_free_event(e);
        }
    }
}
