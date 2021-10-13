use csv::*;
use std::io;

use crate::account::{Account, AccountOutput};
use crate::transaction::{ClientId, Transaction};
use std::collections::HashMap;

pub fn read_transactions(input: &mut dyn io::Read, verbose: bool) -> Result<Vec<Transaction>> {
    let mut reader = ReaderBuilder::new().trim(Trim::All).from_reader(input);

    let mut res = Vec::with_capacity(100);

    for row in reader.deserialize() {
        let tr: Transaction = row?;
        if verbose {
            println!("{}: {:?}", tr.transaction_id, tr);
        }
        res.push(tr);
    }
    Ok(res)
}

pub fn write_accounts(
    accounts: HashMap<ClientId, Account>,
    output: &mut dyn io::Write,
) -> Result<()> {
    let acc_list: Vec<&Account> = accounts.values().collect();
    let out_list: Vec<AccountOutput> = acc_list.iter().map(|a| (*a).into()).collect();

    // TODO output max 4 decimals

    let mut writer = csv::Writer::from_writer(output);
    for out in out_list {
        writer.serialize(out)?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{Transaction, TransactionType};
    use rust_decimal::Decimal;

    #[test]
    fn test_read() {
        let input = "type, client, tx, amount\ndeposit, 1, 5, 98765.4321";
        let res = read_transactions(&mut input.as_bytes(), false);
        assert!(res.is_ok(), "csv parsing error: {:?}", res);

        if let Ok(transactions) = res {
            let expected = vec![Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 1,
                transaction_id: 5,
                amount: Some(Decimal::new(987654321, 4)),
            }];

            assert_eq!(transactions, expected)
        }
    }
}
