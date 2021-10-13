use csv;
use std::fs::File;
use crate::transaction::{Transaction, TransactionType};
use rust_decimal::Decimal;

pub fn load_file(file: File, silent: bool) -> Vec<Transaction> {
    // TODO load csv
    vec![Transaction {
        transaction_type: TransactionType::Deposit,
        client_id: 1,
        transaction_id: 5,
        amount: Decimal::new(987654321, 4)
    }]
}
