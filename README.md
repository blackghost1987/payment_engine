# Payment Engine

#### Payment Engine implementation for Rust Coding Test

A CLI tool written in Rust which reads a CSV file containing payment transactions, then outputs account data for each client. 

Use `-h` to get help about the usable arguments.

#### Developer notes

I've implemented the test exercise using the `csv` crate with Serde support and `rayon` for parallel processing of the client transactions.

* **Completeness**: I've implemented all transaction types. There was only one scenario which was not obvious to handle based on the requirements: Dispute of Withdrawals. I assumed it's possible to Dispute the Withdrawal transactions, and it should work symmetrically to Deposits (e.g. disputing a withdrawal increases the available funds). I made it easy to remove this feature, if this should not be possible.   

* **Correctness**: I used automated Unit tests as well as manual Integration tests for the application.

* **Safety and Robustness**: The tool uses human-readable error messages everywhere, and it should not panic. The application only stops on critical errors (e.g failed input parsing), otherwise erroneous transactions are skipped. 

* **Ease of use**: The tool is using Clap for easier command line usage, an auto generated help can be accessed with the "-h" parameter. The "-v" parameter can be used to get some log messages during processing.

* **Performance**: I've tested the performance with CSVs with ~10000 lines, which took around 150 ms on my computer, which seems sufficient. Using Rayon definitely helped with the execution if there are many clients in the input. It caused a 5-10% performance upgrade with 100 clients (for 10000 transactions). The performance and memory usage could be further improved by using async and data streaming.
