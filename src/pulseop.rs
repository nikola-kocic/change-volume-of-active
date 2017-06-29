use libpulse_sys::*;

use std::ptr::{null, null_mut};
use std::ffi::{CStr, CString};
use libc::{c_char, c_int, c_void};

use operations::VolumeOp;

// Not available from libpulse_sys
const PA_VOLUME_NORM: u32 = 0x10000u32;

struct SinkInputInfo {
    pid: u32,
    infos: Vec<pa_sink_input_info>,
    debug: bool,
}

fn volume_to_percent(volume: f32) -> f32 {
    let volume_percent: f32 = volume * 100. / (PA_VOLUME_NORM as f32);
    volume_percent
}

fn gamma_correction(i: f32, gamma: f32, delta: f32) -> f32 {
    let mut j = i;
    let rel_relta: f32 = delta / 100.0;

    j /= PA_VOLUME_NORM as f32;
    j = j.powf(1.0 / gamma);

    j += rel_relta;
    if j < 0.0 {
        j = 0.0;
    }

    j = j.powf(gamma);
    j *= PA_VOLUME_NORM as f32;

    j
}

fn calculate_new_volume(delta: f32, old_val: f32) -> u32 {
    let new_val = gamma_correction(old_val, 1.0, delta);
    if new_val < 0.0 {
        0
    } else {
        new_val.round() as u32
    }
}

fn calculate_new_volumes(delta: f32, volume: &mut pa_cvolume, debug: bool) {
    for i in 0..(volume.channels as usize) {
        let channel_val = volume.values[i] as f32;
        let new_val = calculate_new_volume(delta, channel_val);
        if debug {
            let perc_vol = volume_to_percent(channel_val);
            let new_perc_vol = volume_to_percent(new_val as f32);
            println!(
                "setting volume of channel {} from {} to {}",
                i,
                perc_vol,
                new_perc_vol
            );
        }
        volume.values[i] = new_val;
    }
}

unsafe extern "C" fn pa_state_cb(pa_ctx: *mut pa_context, userdata: *mut c_void) -> () {
    let pa_state: pa_context_state_t = pa_context_get_state(pa_ctx);
    let mut pa_ready = userdata as *mut pa_context_state_t;
    *pa_ready = pa_state;
    // println!("Pulse state: {}", pa_state);
}

unsafe extern "C" fn pa_sink_input_info_cb(
    _pa_ctx: *mut pa_context,
    i: *const pa_sink_input_info,
    eol: c_int,
    userdata: *mut c_void,
) -> () {
    // If eol is set to a positive number, you're at the end of the list
    if eol > 0 {
        return;
    }

    let p = (*i).proplist;
    let pa_prop_application_process_id = CString::new("application.process.id").unwrap();
    if pa_proplist_contains(p, pa_prop_application_process_id.as_ptr()) == 1 {
        let pid = {
            let pid_c: *const c_char = pa_proplist_gets(p, pa_prop_application_process_id.as_ptr());
            let pid_s = CStr::from_ptr(pid_c).to_str().unwrap();
            pid_s.parse::<u32>().unwrap()
        };
        let pa_userdata_ptr = userdata as *mut SinkInputInfo;
        let mut pa_userdata: &mut SinkInputInfo = &mut *pa_userdata_ptr;
        if pa_userdata.pid == pid {
            if pa_userdata.debug {
                println!("Matched pid on sink {}", (*i).index);
            }
            pa_userdata.infos.push(*i);
        }
    }
}

unsafe fn perform_op(
    op: &VolumeOp,
    pa_ctx: *mut pa_context,
    info: &mut pa_sink_input_info,
    debug: bool,
) -> *mut pa_operation {
    if debug {
        println!("Performing operation on sink {}", info.index);
    }

    match *op {
        VolumeOp::ToggleMute => {
            let mute = if info.mute == 0 { 1 } else { 0 };
            if debug {
                println!("setting mute to {}", mute);
            }
            pa_context_set_sink_input_mute(pa_ctx, info.index, mute, None, null_mut())
        }
        VolumeOp::ChangeVolume(val) => {
            calculate_new_volumes(val, &mut info.volume, debug);
            pa_context_set_sink_input_volume(pa_ctx, info.index, &info.volume, None, null_mut())
        }
    }
}

pub fn pulse_op(pid: u32, op: &VolumeOp, debug: bool) {
    // pacmd list-sink-inputs
    let client_name = CString::new("test").unwrap();
    let mut pa_ready: pa_context_state_t = 0u32;
    let mut state = 0;
    let mut pa_op: *mut pa_operation = null_mut();
    let mut pa_userdata = SinkInputInfo {
        pid: pid,
        infos: Vec::new(),
        debug: debug,
    };
    let mut info: Option<pa_sink_input_info> = None;

    unsafe {
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
                                pa_ctx,
                                Some(pa_sink_input_info_cb),
                                pa_userdata_ptr,
                            );
                            assert!(!pa_op.is_null());
                            state += 1;
                        }
                        1 => {
                            let op_state: pa_operation_state_t = pa_operation_get_state(pa_op);
                            if op_state == PA_OPERATION_DONE {
                                let previous_info = info;
                                info = pa_userdata.infos.pop();
                                match info {
                                    None => {
                                        if previous_info.is_none() {
                                            println!("PulseAudio sink not found for pid {}", pid);
                                        }
                                        break;
                                    }
                                    Some(ref mut info) => {
                                        pa_operation_unref(pa_op);
                                        pa_op = perform_op(op, pa_ctx, info, debug);
                                        assert!(!pa_op.is_null());
                                    }
                                }
                            }
                        }
                        _ => {
                            assert!(false);
                        }
                    }
                }
                PA_CONTEXT_FAILED |
                PA_CONTEXT_TERMINATED => {
                    println!("Failed to connect to PulseAudio!");
                    break;
                }
                _ => {}
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
