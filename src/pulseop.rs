use libpulse_sys::*;

use std::ptr::{null, null_mut};
use std::ffi::{CStr, CString};
use libc::{c_char, c_int, c_void};

use operations::VolumeOp;

// Not available from libpulse_sys
// Normal volume (100%, 0 dB)
const PA_VOLUME_NORM: u32 = 0x10000u32;

const DEBUG_LOG: bool = false;

struct SinkInputInfo {
    pid: u32,
    infos: Vec<pa_sink_input_info>,
    debug: bool,
}

fn volume_to_percent(volume: f32) -> f32 {
    let volume_percent: f32 = volume * 100. / (PA_VOLUME_NORM as f32);
    volume_percent
}

fn percent_to_volume(percent: f32) -> u32 {
    let volume: f32 = percent * (PA_VOLUME_NORM as f32 / 100.);
    volume.round() as u32
}

fn gamma_correction(i: f32, gamma: f32, delta_percent: f32) -> f32 {
    let mut j = i;
    let rel_relta: f32 = delta_percent / 100.0;

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

fn volume_with_delta(delta_percent: f32, old_val: f32) -> u32 {
    let new_val = gamma_correction(old_val, 1.0, delta_percent);
    if new_val < 0.0 {
        0
    } else {
        new_val.round() as u32
    }
}

fn modify_volumes<F>(volume: &mut pa_cvolume, debug: bool, f: F)
where
    F: Fn(f32) -> u32,
{
    for i in 0..(volume.channels as usize) {
        let channel_val = volume.values[i] as f32;
        let new_val = f(channel_val);
        if debug || DEBUG_LOG {
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

unsafe fn connect_pa_context(pa_ml: *mut pa_mainloop, pa_ctx: *mut pa_context) -> bool {
    unsafe extern "C" fn pa_state_cb(pa_ctx: *mut pa_context, userdata: *mut c_void) -> () {
        let pa_state: pa_context_state_t = pa_context_get_state(pa_ctx);
        let pa_ready = userdata as *mut pa_context_state_t;
        *pa_ready = pa_state;
        if DEBUG_LOG {
            println!("Pulse state: {}", pa_state);
        }
    }

    let mut pa_ready: pa_context_state_t = 0u32;
    let pa_ready_ptr = &mut pa_ready as *mut _ as *mut c_void;
    pa_context_set_state_callback(pa_ctx, Some(pa_state_cb), pa_ready_ptr);
    pa_context_connect(pa_ctx, null(), 0, null());
    loop {
        match pa_ready {
            PA_CONTEXT_READY => {
                return true;
            }
            PA_CONTEXT_FAILED |
            PA_CONTEXT_TERMINATED => {
                println!("Failed to connect to PulseAudio!");
                return false;
            }
            _ => {}
        }

        // Iterate the main loop and go again.  The second argument is whether
        // or not the iteration should block until something is ready to be
        // done.  Set it to zero for non-blocking.
        pa_mainloop_iterate(pa_ml, 1, null_mut());
    }
}

unsafe fn do_pa_operation<F>(pa_ml: *mut pa_mainloop, f: F) -> pa_operation_state_t
where
    F: FnOnce() -> *mut pa_operation,
{
    let pa_op: *mut pa_operation = f();
    assert!(!pa_op.is_null());

    loop {
        let op_state: pa_operation_state_t = pa_operation_get_state(pa_op);
        if op_state != PA_OPERATION_RUNNING {
            pa_operation_unref(pa_op);
            return op_state;
        }
        // Iterate the main loop and go again.  The second argument is whether
        // or not the iteration should block until something is ready to be
        // done.  Set it to zero for non-blocking.
        pa_mainloop_iterate(pa_ml, 1, null_mut());
    }
}

unsafe fn perform_volume_pa_op(
    op: &VolumeOp,
    pa_ctx: *mut pa_context,
    info: &mut pa_sink_input_info,
    debug: bool,
) -> *mut pa_operation {

    if debug || DEBUG_LOG {
        println!("Performing operation on sink {}", info.index);
    }

    match *op {
        VolumeOp::ToggleMute => {
            let mute = if info.mute == 0 { 1 } else { 0 };
            if debug || DEBUG_LOG {
                println!("setting mute to {}", mute);
            }
            pa_context_set_sink_input_mute(pa_ctx, info.index, mute, None, null_mut())
        }
        VolumeOp::ChangeVolume(delta_percent) => {
            modify_volumes(&mut info.volume, debug, |channel_val| {
                volume_with_delta(delta_percent, channel_val)
            });
            pa_context_set_sink_input_volume(pa_ctx, info.index, &info.volume, None, null_mut())
        }
        VolumeOp::SetVolume(percent) => {
            modify_volumes(&mut info.volume, debug, |_| percent_to_volume(percent));
            pa_context_set_sink_input_volume(pa_ctx, info.index, &info.volume, None, null_mut())
        }
    }
}

fn get_sink_infos(
    pid: u32,
    debug: bool,
    pa_ml: *mut pa_mainloop,
    pa_ctx: *mut pa_context,
) -> Vec<pa_sink_input_info> {

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
                let pid_c: *const c_char =
                    pa_proplist_gets(p, pa_prop_application_process_id.as_ptr());
                let pid_s = CStr::from_ptr(pid_c).to_str().unwrap();
                pid_s.parse::<u32>().unwrap()
            };
            let pa_userdata_ptr = userdata as *mut SinkInputInfo;
            let pa_userdata: &mut SinkInputInfo = &mut *pa_userdata_ptr;
            if pa_userdata.pid == pid {
                if pa_userdata.debug {
                    println!("Matched pid on sink {}", (*i).index);
                }
                pa_userdata.infos.push(*i);
            }
        }
    }

    let mut pa_userdata = SinkInputInfo {
        pid: pid,
        infos: Vec::new(),
        debug: debug,
    };
    unsafe {
        let pa_userdata_ptr = &mut pa_userdata as *mut _ as *mut c_void;
        do_pa_operation(pa_ml, || {
            pa_context_get_sink_input_info_list(
                pa_ctx,
                Some(pa_sink_input_info_cb),
                pa_userdata_ptr,
            )
        });
    }
    pa_userdata.infos
}

pub struct PulseAudio {
    pa_ml: *mut pa_mainloop,
    pa_ctx: *mut pa_context,
}

impl Drop for PulseAudio {
    fn drop(&mut self) {
        if DEBUG_LOG {
            println!("PulseAudio drop");
        }
        unsafe {
            pa_context_unref(self.pa_ctx);
            pa_mainloop_free(self.pa_ml);
        }
    }
}

impl PulseAudio {
    pub fn create(client_name: &str) -> PulseAudio {
        unsafe {
            let client_name_c = CString::new(client_name).unwrap();
            let pa_ml: *mut pa_mainloop = pa_mainloop_new();
            let pa_mlapi: *mut pa_mainloop_api = pa_mainloop_get_api(pa_ml);
            let pa_ctx: *mut pa_context = pa_context_new(pa_mlapi, client_name_c.as_ptr());
            PulseAudio {
                pa_ml,
                pa_ctx,
            }
        }
    }

    pub fn connect(self) -> Option<ConnectedPulseAudio> {
        unsafe {
            if connect_pa_context(self.pa_ml, self.pa_ctx) {
                Some(ConnectedPulseAudio { data: self })
            } else {
                None
            }
        }
    }
}

pub struct ConnectedPulseAudio {
    data: PulseAudio,
}

impl Drop for ConnectedPulseAudio {
    fn drop(&mut self) {
        if DEBUG_LOG {
            println!("ConnectedPulseAudio drop");
        }
        unsafe {
            pa_context_disconnect(self.data.pa_ctx);
        }
    }
}

impl ConnectedPulseAudio {
    pub fn perform_volume_op(&self, op: &VolumeOp, mut info: &mut pa_sink_input_info, debug: bool) {
        unsafe {
            do_pa_operation(self.data.pa_ml, || {
                perform_volume_pa_op(op, self.data.pa_ctx, &mut info, debug)
            });
        }
    }

    pub fn get_sink_infos(&self, pid: u32, debug: bool) -> Vec<pa_sink_input_info> {
        get_sink_infos(pid, debug, self.data.pa_ml, self.data.pa_ctx)
    }
}
