mod account;
mod csv_handler;
mod transaction;

use clap::{App, Arg, ArgMatches};
use std::fs::File;
use std::{io, process};

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
                .help("Print progress data"),
        )
        .get_matches()
}

fn process_file(mut file: File, verbose: bool) {
    let tr_result = csv_handler::read_transactions(&mut file, verbose);
    match tr_result {
        Ok(transactions) => {
            if verbose {
                println!("Transactions loaded: {}", transactions.len());
            }

            let accounts = account::process_all(transactions, verbose);
            if verbose {
                println!("Client accounts processed: {}", accounts.len());
            }

            let write_res = csv_handler::write_accounts(accounts, &mut io::stdout());
            if let Err(e) = write_res {
                eprintln!("Error while writing output: {:?}", e);
                process::exit(4)
            }
        }
        Err(e) => {
            eprintln!("Error while loading transactions: {:?}", e);
            process::exit(3)
        }
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
