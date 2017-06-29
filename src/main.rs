#[macro_use]
extern crate clap;
extern crate libc;
extern crate libpulse_sys;
extern crate sysinfo;
extern crate xcb;
extern crate xcb_util;

mod activewindow;
mod args;
mod childpids;
mod operations;
mod pulseop;

use operations::VolumeOp;
use pulseop::{ConnectedPulseAudio, PulseAudio};

fn pulse_op(pid: u32, op: &VolumeOp, traverse_children: bool, debug: bool) {
    fn do_pulse_op(
        pa_connected: &ConnectedPulseAudio,
        pid: u32,
        op: &VolumeOp,
        debug: bool,
    ) -> bool {
        let infos = pa_connected.get_sink_infos(pid, debug);
        let success = !infos.is_empty();
        if !success {
            println!("PulseAudio sink not found for pid {}", pid);
        }
        for mut info in infos {
            pa_connected.perform_volume_op(op, &mut info, debug);
        }
        success
    }

    // pacmd list-sink-inputs
    let pa = PulseAudio::create(env!("CARGO_PKG_NAME"));
    let pa_connected = pa.connect();

    if let Some(pa_connected) = pa_connected {
        if !do_pulse_op(&pa_connected, pid, op, debug) && traverse_children {
            let children_pids = childpids::get_children_pids(pid as i32);
            for child_pid in children_pids {
                if debug {
                    println!("Trying with child PID {}", child_pid);
                }
                do_pulse_op(&pa_connected, child_pid as u32, op, debug);
            }
        }
    }
}

fn main() {
    let arguments = args::get_arguments();
    let pid: u32 = arguments.pid.unwrap_or_else(|| {
        activewindow::active_window_pid(arguments.debug)
    });
    if arguments.debug {
        println!("op = {:?}", arguments.operation);
        println!("pid: {:?}", pid);
    }

    pulse_op(
        pid,
        &arguments.operation,
        arguments.traverse_children,
        arguments.debug,
    );
}
