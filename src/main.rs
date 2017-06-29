extern crate clap;
extern crate xcb;
extern crate xcb_util;

use clap::{App, Arg};
use xcb_util::ewmh;

mod pulseop;

fn active_window_pid(debug: bool) -> u32 {
    let (xcb_con, screen_num) = xcb::Connection::connect(None).unwrap();
    let connection = ewmh::Connection::connect(xcb_con)
        .map_err(|(e, _)| e)
        .unwrap();
    let active_window: xcb::Window = ewmh::get_active_window(&connection, screen_num)
        .get_reply()
        .unwrap();
    let pid = ewmh::get_wm_pid(&connection, active_window)
        .get_reply()
        .unwrap();
    if debug {
        println!("active_window: {:X}", active_window);
    }
    pid
}

fn main() {
    let matches = App::new("Change Volume of Active App")
        .version("0.1.0")
        .author("Nikola Kocić. <nikolakocic@gmail.com>")
        .about("Changes volume of active application")
        .arg(
            Arg::with_name("mute")
                .long("mute")
                .short("m")
                .help("Toggle mute")
                .takes_value(false)
                .conflicts_with("volume"),
        )
        .arg(
            Arg::with_name("volume")
                .long("volume")
                .short("v")
                .help("Adjusts volume (in percent)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("pid")
                .short("p")
                .long("pid")
                .help("PID of window, get active window if not specified")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .short("d")
                .help("Turn on debug output")
                .takes_value(false),
        )
        .get_matches();
    let op = {
        let mute: bool = matches.is_present("mute");
        if mute {
            pulseop::VolumeOp::ToggleMute
        } else {
            let volume_present: bool = matches.is_present("volume");
            if volume_present {
                let volume_delta_s: &str = matches.value_of("volume").unwrap();
                let volume_delta = volume_delta_s.parse::<f32>().unwrap();
                pulseop::VolumeOp::ChangeVolume(volume_delta)
            } else {
                pulseop::VolumeOp::ChangeVolume(0.0)
            }
        }
    };

    let debug = matches.is_present("debug");

    let pid = {
        if matches.is_present("pid") {
            let pid_s: &str = matches.value_of("pid").unwrap();
            pid_s.parse::<u32>().unwrap()
        } else {
            active_window_pid(debug)
        }
    };
    if debug {
        println!("op = {:?}", op);
        println!("pid: {}", pid);
    }
    pulseop::pulse_op(pid, &op, debug);
}
