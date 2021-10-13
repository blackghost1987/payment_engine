use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;

use crate::transaction::*;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Account {
    client_id: ClientId,
    available: Decimal,
    held: Decimal,
    locked: bool,
    #[serde(skip)]
    transactions: HashMap<TransactionId, Transaction>, // Deposits and Withdrawals only
}

impl Account {
    pub fn new(client_id: ClientId) -> Account {
        Account {
            client_id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
            transactions: HashMap::new(),
        }
    }

    fn get_transaction(&mut self, tr_id: &TransactionId) -> Result<&mut Transaction> {
        self.transactions.get_mut(&tr_id).ok_or(Error::UnknownTransactionId)
    }

    pub fn process(&mut self, tr: &Transaction) -> Result<()> {
        use TransactionType::*;

        if self.client_id != tr.client_id {
            return Err(Error::ClientIdMismatch);
        }

        if self.locked {
            return Err(Error::AccountLocked);
        }

        match tr.transaction_type {
            Deposit => {
                if self.transactions.contains_key(&tr.transaction_id) {
                    return Err(Error::DuplicatedTransactionId);
                }
                self.transactions.insert(tr.transaction_id, tr.clone());

                let amount = tr.amount.ok_or(Error::MissingAmount)?;
                self.available += amount;
            }
            Withdrawal => {
                if self.transactions.contains_key(&tr.transaction_id) {
                    return Err(Error::DuplicatedTransactionId);
                }
                self.transactions.insert(tr.transaction_id, tr.clone());

                let amount = tr.amount.ok_or(Error::MissingAmount)?;
                if self.available < amount {
                    return Err(Error::InsufficientFunds);
                }
                self.available -= amount;
            }
            Dispute => {
                tr.check_amount_empty();

                let ref_tr = self.get_transaction(&tr.transaction_id)?;
                if ref_tr.disputed {
                    return Err(Error::AlreadyDisputed);
                }
                let amount = ref_tr.get_amount()?;

                ref_tr.disputed = true;
                self.available -= amount;
                self.held += amount;
            },
            Resolve => {
                tr.check_amount_empty();

                let ref_tr = self.get_transaction(&tr.transaction_id)?;
                if !ref_tr.disputed {
                    return Err(Error::NotDisputed);
                }
                let amount = ref_tr.get_amount()?;

                ref_tr.disputed = false;
                self.available += amount;
                self.held -= amount;
            },
            Chargeback => {
                tr.check_amount_empty();

                let ref_tr = self.get_transaction(&tr.transaction_id)?;
                if !ref_tr.disputed {
                    return Err(Error::NotDisputed);
                }
                let amount = ref_tr.get_amount()?;

                ref_tr.chargeback = true;
                self.held -= amount;
                self.locked = true;
            },
        }

        Ok(())
    }
}

pub fn process_all(transactions: Vec<Transaction>) -> HashMap<ClientId, Account> {
    let mut accounts = HashMap::new();
    for (i, tr) in transactions.iter().enumerate() {
        // get or create if not yet exists
        let acc = accounts.entry(tr.client_id).or_insert(Account::new(tr.client_id));
        // process transaction
        if let Err(e) = acc.process(&tr) {
            eprintln!("Error during processing row #{} (transaction_id: {}): {:?}", i, tr.transaction_id, e)
        }
    }
    accounts
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AccountOutput {
    total: Decimal,
    #[serde(flatten)]
    account: Account,
}

impl From<Account> for AccountOutput {
    fn from(a: Account) -> Self {
        AccountOutput {
            total: a.available + a.held,
            account: a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_it() {
        // TODO implement tests
    }
}
