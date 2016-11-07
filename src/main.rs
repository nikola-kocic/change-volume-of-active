extern crate libc;

extern crate xcb;
extern crate xcb_util;
extern crate libpulse_sys;

use xcb_util::ewmh;
use std::ptr::{null, null_mut};
use std::ffi::{CStr, CString};
use self::libc::{c_char, c_int, c_void};


fn pulse_info(pid: u32) {
    use libpulse_sys::*;

    struct SinkInputInfo {
        pid: u32,
        found: bool,
        info: pa_sink_input_info,
    }

    unsafe extern "C" fn pa_state_cb(pa_ctx: *mut pa_context, userdata: *mut c_void) -> () {
        let pa_state: pa_context_state_t = pa_context_get_state(pa_ctx);
        let mut pa_ready = userdata as *mut pa_context_state_t;
        *pa_ready = pa_state;
        // println!("Pulse state: {}", pa_state);
    }

    unsafe extern "C" fn pa_sink_input_info_cb(
        _pa_ctx: *mut pa_context, i: *const pa_sink_input_info, eol: c_int, userdata: *mut c_void) -> () {
        // If eol is set to a positive number, you're at the end of the list
        if eol > 0 {
            return;
        }

        let p = (*i).proplist;
        let pa_prop_application_process_id = CString::new("application.process.id").unwrap().as_ptr();
        if pa_proplist_contains(p, pa_prop_application_process_id) == 1 {
            let pid = {
                let pid_c: *const c_char = pa_proplist_gets(p, pa_prop_application_process_id);
                let pid_s = CStr::from_ptr(pid_c).to_str().unwrap();
                pid_s.parse::<u32>().unwrap()
            };
            let mut pa_userdata = userdata as *mut SinkInputInfo;
            if (*pa_userdata).pid == pid {
                (*pa_userdata).found = true;
                (*pa_userdata).info = *i;
            }
        }
    }

    // pacmd list-sink-inputs
    unsafe {
        let client_name = CString::new("test").unwrap();
        let mut pa_ready: pa_context_state_t = 0u32;
        let mut state = 0;
        let mut pa_op: *mut pa_operation = null_mut();
        let mut pa_userdata = SinkInputInfo {
            pid: pid, found: false, info: pa_sink_input_info::default()
        };

        // Create a mainloop API and connection to the default server
        let pa_ml: *mut pa_mainloop = pa_mainloop_new();
        let pa_mlapi: *mut pa_mainloop_api = pa_mainloop_get_api(pa_ml);
        let pa_ctx: *mut pa_context = pa_context_new(pa_mlapi, client_name.as_ptr());
        let pa_ready_ptr = &mut pa_ready as *mut _ as *mut c_void;
        pa_context_set_state_callback(pa_ctx, Some(pa_state_cb), pa_ready_ptr);
        pa_context_connect(pa_ctx, null(), 0, null());

        loop {
            match pa_ready {
                PA_CONTEXT_READY => {
                    match state {
                        0 => {
                            let pa_userdata_ptr = &mut pa_userdata as *mut _ as *mut c_void;
                            pa_op = pa_context_get_sink_input_info_list(
                                pa_ctx, Some(pa_sink_input_info_cb), pa_userdata_ptr);
                            assert!(!pa_op.is_null());
                            state += 1;
                        }
                        1 => {
                            let op_state: pa_operation_state_t = pa_operation_get_state(pa_op);
                            if op_state == PA_OPERATION_DONE {
                                if !pa_userdata.found {
                                    break;
                                } else {
                                    let mute = if pa_userdata.info.mute == 0 { 1 } else { 0 };
                                    // println!("setting mute to {} for {}", mute, pa_userdata.info.index);
                                    pa_operation_unref(pa_op);
                                    pa_op = pa_context_set_sink_input_mute(
                                        pa_ctx, pa_userdata.info.index, mute, None, null_mut());
                                    assert!(!pa_op.is_null());
                                    state += 1;
                                }
                            }
                        }
                        2 => {
                            let op_state: pa_operation_state_t = pa_operation_get_state(pa_op);
                            if op_state == PA_OPERATION_DONE {
                                break;
                            }
                        }
                        _ => {
                            assert!(false);
                        }
                    }
                }
                PA_CONTEXT_FAILED | PA_CONTEXT_TERMINATED => {
                    println!("Failed to connect to PulseAudio!");
                    break;
                }
                _ => {
                }
            }

            // Iterate the main loop and go again.  The second argument is whether
            // or not the iteration should block until something is ready to be
            // done.  Set it to zero for non-blocking.
            pa_mainloop_iterate(pa_ml, 1, null_mut());
        }

        // Cleanup
        if !pa_op.is_null() {
            pa_operation_unref(pa_op);
        }
        pa_context_disconnect(pa_ctx);
        pa_context_unref(pa_ctx);
        pa_mainloop_free(pa_ml);
    }
}

fn active_window_pid() -> u32 {
    let (xcb_con, screen_num) = xcb::Connection::connect(None).unwrap();
    let connection = ewmh::Connection::connect(xcb_con).map_err(|(e, _)| e).unwrap();
    let active_window: xcb::Window = ewmh::get_active_window(&connection, screen_num).get_reply().unwrap();
    let pid = ewmh::get_wm_pid(&connection, active_window).get_reply().unwrap();
    // println!("active_window: {:X}", active_window);
    return pid;
}

fn main() {
    let pid = active_window_pid();
    // println!("pid: {}", pid);
    pulse_info(pid);
}
