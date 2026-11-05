use super::*;

fn handle() -> *mut AgentsightHandle {
    let mut h = ptr::null_mut();
    assert_eq!(unsafe { agentsight_init(&mut h) }, 0);
    h
}

fn evidence() -> Vec<u8> {
    br#"{"event":"FILE_WRITE","timestamp":110,"pid":12,"tid":13,"agentsight_trace_flags":1,"agentsight_trace_id":"12345678901234567890123456789012","agentsight_parent_span_id":"abcdef1234567890","agentsight_correlation_source":"task","filepath":"/test","count":2,"total_bytes":99}"#.to_vec()
}

fn anchor(s: &Shared) {
    receive(
        s,
        0,
        br#"{"event":"CLOCK_SYNC","mono_ns":100,"wall_time_ns":1000}"#,
    );
}

#[test]
fn lifecycle_validation() {
    unsafe {
        assert_eq!(agentsight_init(ptr::null_mut()), -libc::EINVAL);
        assert_eq!(agentsight_start(ptr::null_mut()), -libc::EINVAL);
        let h = handle();
        assert_eq!(agentsight_start(h), -libc::EINVAL);
        let mut message = [0 as c_char; 256];
        assert_eq!(
            agentsight_get_last_error(h, message.as_mut_ptr(), message.len()),
            0
        );
        assert!(
            CStr::from_ptr(message.as_ptr())
                .to_str()
                .unwrap()
                .contains("set_config")
        );
        assert_eq!(agentsight_set_config(h, ptr::null()), -libc::EINVAL);
        assert_eq!(agentsight_stop(h), 0);
        assert_eq!(agentsight_stop(h), 0);
        assert_eq!(agentsight_free(h), 0);
        assert_eq!(agentsight_free(ptr::null_mut()), 0);
        agentsight_free_event(ptr::null_mut());
    }
}

#[test]
fn event_ownership_and_snapshot() {
    unsafe {
        let h = handle();
        anchor(&(*h).shared);
        receive(&(*h).shared, 0, &evidence());
        let mut event = ptr::null_mut();
        assert_eq!(agentsight_poll_events(h, &mut event, 1), 1);
        assert_eq!((*event).wall_time_ns, 1010);
        assert_eq!(
            (*event).trace_id,
            hex_id::<16>(Some("12345678901234567890123456789012")).unwrap()
        );
        assert_eq!(
            (*event).parent_span_id,
            hex_id::<8>(Some("abcdef1234567890")).unwrap()
        );
        assert_eq!(agentsight_free(h), 0);
        // Returned events own their bytes independently from the instance.
        assert!(
            CStr::from_ptr((*event).payload_json)
                .to_str()
                .unwrap()
                .contains("/test")
        );
        agentsight_free_event(event);
    }
}

#[test]
fn bounded_queue_and_partial_read_rearms_eventfd() {
    unsafe {
        let h = handle();
        let s = &(*h).shared;
        lock(&s.data).capacity = 2;
        anchor(s);
        for _ in 0..3 {
            receive(s, 0, &evidence());
        }
        let mut stats = AgentsightStats::default();
        assert_eq!(
            agentsight_get_stats(h, &mut stats, size_of::<AgentsightStats>()),
            0
        );
        assert_eq!(stats.enqueued_events, 2);
        assert_eq!(stats.queue_dropped_events, 1);
        let mut pollfd = libc::pollfd {
            fd: agentsight_get_eventfd(h),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(libc::poll(&mut pollfd, 1, 0), 1);
        let mut event = ptr::null_mut();
        assert_eq!(agentsight_poll_events(h, &mut event, 1), 1);
        agentsight_free_event(event);
        assert_eq!(libc::poll(&mut pollfd, 1, 0), 1);
        assert_eq!(agentsight_poll_events(h, &mut event, 1), 1);
        agentsight_free_event(event);
        assert_eq!(libc::poll(&mut pollfd, 1, 0), 0);
        assert_eq!(agentsight_poll_events(h, &mut event, 1), 0);
        assert_eq!(agentsight_free(h), 0);
    }
}

#[test]
fn rejects_missing_invalid_unsampled_context_and_bad_json() {
    unsafe {
        let h = handle();
        let s = &(*h).shared;
        anchor(s);
        receive(s, 0, b"not JSON");
        receive(s, 0, br#"{"event":"EXEC","pid":1,"timestamp":110}"#);
        let unsampled = String::from_utf8(evidence()).unwrap().replace(
            "\"agentsight_trace_flags\":1",
            "\"agentsight_trace_flags\":0",
        );
        receive(s, 0, unsampled.as_bytes());
        assert_eq!(lock(&s.data).queue.len(), 0);
        assert_eq!(lock(&s.data).stats.invalid_records, 1);
        assert_eq!(lock(&s.data).stats.filtered_records, 2);
        assert!(hex_id::<8>(Some("0000000000000000")).is_none());
        assert!(hex_id::<8>(Some("abcdefghijklmnop")).is_none());
        agentsight_free(h);
    }
}

#[test]
fn byte_limit_and_truncated_serialization_are_counted() {
    unsafe {
        let h = handle();
        let s = &(*h).shared;
        anchor(s);
        lock(&s.data).byte_limit = 1;
        receive(s, 0, &evidence());
        receive(s, 2, b"truncated");
        receive(
            s,
            0,
            br#"{"event":"WARNING","type":"AGG_MAP_OVERFLOW","overflow_count":4}"#,
        );
        let d = lock(&s.data);
        assert_eq!(d.stats.queue_dropped_events, 1);
        assert_eq!(d.stats.output_dropped_records, 1);
        assert_eq!(d.stats.aggregate_overflow, 4);
        drop(d);
        agentsight_free(h);
    }
}

#[test]
fn config_size_and_copy() {
    unsafe {
        let h = handle();
        let mut c: AgentsightConfig = std::mem::zeroed();
        assert_eq!(agentsight_set_config(h, &c), -libc::EINVAL);
        c.struct_size = size_of::<AgentsightConfig>() as u32;
        c.abi_version = 1;
        c.target_pid = 1;
        let path = CString::new("/not/real.so").unwrap();
        c.marker_path = path.as_ptr();
        c.asyncio_path = path.as_ptr();
        c.python_path = path.as_ptr();
        c.task_step_offset = 1;
        c.task_init_offset = 2;
        c.context_copy_offset = 3;
        c.context_run_offset = 4;
        assert_eq!(agentsight_set_config(h, &c), 0);
        drop(path);
        assert_eq!(
            lock(&(*h).lifecycle).config.as_ref().unwrap().paths[0]
                .to_str()
                .unwrap(),
            "/not/real.so"
        );
        agentsight_free(h);
    }
}
