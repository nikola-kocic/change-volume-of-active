extern crate clap;
extern crate libc;
extern crate libpulse_sys;
extern crate xcb;
extern crate xcb_util;

mod activewindow;
mod args;
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
    pulseop::pulse_op(pid, &arguments.operation, arguments.debug);
}
