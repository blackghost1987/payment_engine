use csv::*;
use rust_decimal::Decimal;
use std::io;

use crate::transaction::Transaction;

pub fn read_transactions(input: &mut dyn io::Read, verbose: bool) -> Result<Vec<Transaction>> {
    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .from_reader(input);

    let mut res = Vec::with_capacity(100);

    for row in reader.deserialize() {
        let tr: Transaction = row?;
        if verbose { println!("{:?}", tr); }
        res.push(tr);
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{Transaction, TransactionType};

    #[test]
    fn test_parse() {
        let input = "type, client, tx, amount\ndeposit, 1, 5, 98765.4321";
        let res = read_transactions(&mut input.as_bytes(), true);
        assert!(res.is_ok());

        if let Ok(transactions) = res {
            let expected = vec![Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 5,
                amount: Decimal::new(987654321, 4)
            }];

            assert_eq!(transactions, expected)
        }
    }
}