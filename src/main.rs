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

fn main() {
    let arguments = args::get_arguments();
    let pid: u32 = arguments.pid.unwrap_or_else(|| {
        activewindow::active_window_pid(arguments.debug)
    });
    if arguments.debug {
        println!("op = {:?}", arguments.operation);
        println!("pid: {:?}", pid);
    }

    if !pulseop::pulse_op(pid, &arguments.operation, arguments.debug) &&
        arguments.traverse_children
    {
        let children_pids = childpids::get_children_pids(pid as i32);
        for child_pid in children_pids {
            if arguments.debug {
                println!("Trying with child PID {}", child_pid);
            }
            pulseop::pulse_op(child_pid as u32, &arguments.operation, arguments.debug);
        }
    }
}
