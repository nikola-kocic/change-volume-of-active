use clap::{App, Arg, ArgGroup};
use operations::VolumeOp;

pub struct Arguments {
    pub debug: bool,
    pub operation: VolumeOp,
    pub pid: Option<u32>,
}

pub fn get_arguments() -> Arguments {
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
        .group(
            ArgGroup::with_name("operation")
                .args(&["mute", "volume"])
                .required(true),
        )
        .get_matches();
    let op = {
        let mute: bool = matches.is_present("mute");
        if mute {
            VolumeOp::ToggleMute
        } else {
            let volume_present: bool = matches.is_present("volume");
            if volume_present {
                let volume_delta_s: &str = matches.value_of("volume").unwrap();
                let volume_delta = volume_delta_s.parse::<f32>().unwrap();
                VolumeOp::ChangeVolume(volume_delta)
            } else {
                VolumeOp::ChangeVolume(0.0)
            }
        }
    };

    let debug = matches.is_present("debug");

    let pid = {
        if matches.is_present("pid") {
            let pid_s: &str = matches.value_of("pid").unwrap();
            let pid_val = pid_s.parse::<u32>().unwrap();
            Some(pid_val)
        } else {
            None
        }
    };
    Arguments {
        debug: debug,
        pid: pid,
        operation: op,
    }
}
