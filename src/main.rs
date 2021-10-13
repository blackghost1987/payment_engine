use clap::{App, Arg, ArgMatches};
use std::fs::File;
use std::process;

const APP_NAME:    &'static str = "Payment Engine";
const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn parse_args() -> ArgMatches<'static> {
    App::new(APP_NAME)
        .version(APP_VERSION)
        .arg(Arg::with_name("INPUT")
            .help("CSV file to use")
            .required(true)
        )
        .arg(Arg::with_name("silent")
            .short("s")
            .long("silent")
            .help("Hide errors")
        )
        .get_matches()
}

fn main() {
    let opts = parse_args();

    let filename = opts.value_of("INPUT").expect("missing input arg"); // cannot fail here because it's a required arg
    let silent = opts.is_present("silent");

    match File::open(filename) {
        Ok(file) => process_file(file, silent),
        Err(e) => {
            if !silent { eprintln!("Opening of file failed! Error: {:?}", e); }
            process::exit(2)
        }
    }
}
