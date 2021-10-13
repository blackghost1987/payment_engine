mod csv_handler;
mod transaction;
mod account;

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
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Print data during processing"),
        )
        .get_matches()
}

fn process_file(mut file: File, verbose: bool) {
    let tr_result = csv_handler::read_transactions(&mut file, verbose);
    match tr_result {
        Ok(transactions) => {
            if verbose {
                println!("{} transactions loaded", transactions.len());
            }
            let accounts = account::process_all(transactions);
            println!("{:#?}", accounts);
            // TODO csv output
        }
        Err(e) => eprintln!("Error while loading transactions: {:?}", e),
    }
}

fn main() {
    let opts = parse_args();

    let filename = opts.value_of("INPUT").expect("missing input arg"); // cannot fail here because it's a required arg
    let verbose = opts.is_present("verbose");

    match File::open(filename) {
        Ok(file) => process_file(file, verbose),
        Err(e) => {
            eprintln!("Opening of file failed! Error: {:?}", e);
            process::exit(2)
        }
    }
}
