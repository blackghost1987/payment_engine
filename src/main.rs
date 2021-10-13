mod csv_parser;
mod transaction;

use clap::{App, Arg, ArgMatches};
use std::fs::File;
use std::process;

const APP_NAME: &str = "Payment Engine";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn parse_args() -> ArgMatches<'static> {
    App::new(APP_NAME)
        .version(APP_VERSION)
        .arg(
            Arg::with_name("INPUT")
                .help("CSV file to use")
                .required(true),
        )
        .arg(
            Arg::with_name("silent")
                .short("s")
                .long("silent")
                .help("Hide errors"),
        )
        .get_matches()
}

fn process_file(mut file: File, verbose: bool) {
    let tr_result = csv_parser::read_transactions(&mut file, verbose);
    match tr_result {
        Ok(trs) => {
            if verbose {
                println!("{} transactions loaded", trs.len());
            }
        }
        Err(e) => eprintln!("Error while loading transactions: {:?}", e),
    }
}

fn main() {
    let opts = parse_args();

    let filename = opts.value_of("INPUT").expect("missing input arg"); // cannot fail here because it's a required arg
    let verbose = opts.is_present("silent");

    match File::open(filename) {
        Ok(file) => process_file(file, verbose),
        Err(e) => {
            eprintln!("Opening of file failed! Error: {:?}", e);
            process::exit(2)
        }
    }
}
