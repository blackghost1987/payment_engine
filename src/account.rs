use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;

use crate::transaction::*;
use std::ops::Neg;

#[derive(Debug, PartialEq)]
pub struct TransactionStatus {
    pub amount: Decimal,
    pub disputed: bool,
    pub chargeback: bool,
}

impl TransactionStatus {
    pub fn new(transaction: &Transaction) -> Result<TransactionStatus> {
        let mut amount = transaction.get_amount()?;

        // negate Amount so a disputed Withdrawal increases the available amount
        if transaction.transaction_type == TransactionType::Withdrawal {
            amount = amount.neg();
        }

        Ok(TransactionStatus {
            amount,
            disputed: false,
            chargeback: false,
        })
    }

    pub fn dispute(&mut self) -> Result<Decimal> {
        if self.disputed {
            return Err(Error::AlreadyDisputed);
        }
        self.disputed = true;
        Ok(self.amount)
    }

    pub fn resolve(&mut self) -> Result<Decimal> {
        if !self.disputed {
            return Err(Error::NotDisputed);
        }
        self.disputed = false;
        Ok(self.amount)
    }

    pub fn chargeback(&mut self) -> Result<Decimal> {
        if !self.disputed {
            return Err(Error::NotDisputed);
        }
        self.chargeback = true;
        Ok(self.amount)
    }
}

#[derive(Debug, PartialEq)]
pub struct Account {
    client_id: ClientId,
    available: Decimal,
    held: Decimal,
    locked: bool,
    transaction_status: HashMap<TransactionId, TransactionStatus>, // Deposits and Withdrawals only
}

impl Account {
    pub fn new(client_id: ClientId) -> Account {
        Account {
            client_id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
            transaction_status: HashMap::new(),
        }
    }

    pub fn total(&self) -> Decimal {
        self.available + self.held
    }

    fn get_transaction_status(&mut self, tr_id: TransactionId) -> Result<&mut TransactionStatus> {
        self.transaction_status.get_mut(&tr_id).ok_or(Error::UnknownTransactionId)
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
                if self.transaction_status.contains_key(&tr.transaction_id) {
                    return Err(Error::DuplicatedTransactionId);
                }
                let status = TransactionStatus::new(tr)?;
                self.transaction_status.insert(tr.transaction_id, status);

                let amount = tr.get_amount()?;
                self.available += amount;
            }
            Withdrawal => {
                if self.transaction_status.contains_key(&tr.transaction_id) {
                    return Err(Error::DuplicatedTransactionId);
                }
                let status = TransactionStatus::new(tr)?;
                self.transaction_status.insert(tr.transaction_id, status);

                let amount = tr.get_amount()?;
                if self.available < amount {
                    return Err(Error::InsufficientFunds);
                }
                self.available -= amount;
            }
            Dispute => {
                tr.check_amount_empty();
                let amount = {
                    let ref_tr = self.get_transaction_status(tr.transaction_id)?;
                    ref_tr.dispute()?
                };
                self.available -= amount;
                self.held += amount;
            },
            Resolve => {
                tr.check_amount_empty();
                let amount = {
                    let ref_tr = self.get_transaction_status(tr.transaction_id)?;
                    ref_tr.resolve()?
                };
                self.available += amount;
                self.held -= amount;
            },
            Chargeback => {
                tr.check_amount_empty();
                let amount = {
                    let ref_tr = self.get_transaction_status(tr.transaction_id)?;
                    ref_tr.chargeback()?
                };
                self.held -= amount;
                self.locked = true;
            },
        }

        Ok(())
    }
}

pub fn process_all(transactions: Vec<Transaction>, verbose: bool) -> HashMap<ClientId, Account> {
    let mut accounts = HashMap::new();
    for (i, tr) in transactions.iter().enumerate() {
        // get or create if not yet exists
        let acc = accounts.entry(tr.client_id).or_insert(Account::new(tr.client_id));
        // process transaction
        if let Err(e) = acc.process(&tr) {
            if verbose {
                eprintln!("Ignoring row #{} (transaction_id: {}). Reason: {:?}", i, tr.transaction_id, e)
            }
        }
    }
    accounts
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AccountOutput {
    client: ClientId,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

impl<'a> From<&'a Account> for AccountOutput {
    fn from(a: &'a Account) -> Self {
        AccountOutput {
            client: a.client_id,
            available: a.available,
            held: a.held,
            total: a.total(),
            locked: a.locked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let acc = Account::new(5);
        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::ZERO);
    }

    #[test]
    fn test_foreign() {
        let mut acc = Account::new(10);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_err(), "foreign transaction should fail");
        assert_eq!(acc.total(), Decimal::ZERO);
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_deposit() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "processing error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_duplicate_id() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "processing error: {:?}", res);

        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(3456, 2)),
        });
        assert!(res.is_err(), "duplicated id should fail");
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_withdraw() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Withdrawal,
            client_id: 5,
            transaction_id: 2,
            amount: Some(Decimal::new(3456, 2)),
        });
        assert!(res.is_ok(), "withdraw error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(1200, 0));
        assert_eq!(acc.total(), Decimal::new(1200, 0));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_insufficient_funds() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Withdrawal,
            client_id: 5,
            transaction_id: 2,
            amount: Some(Decimal::new(11113456, 2)),
        });
        assert!(res.is_err(), "too large withdrawal should fail");
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "dispute error: {:?}", res);

        assert_eq!(acc.held, Decimal::new(123456, 2));
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_double_dispute() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_err(), "double dispute should fail");

        assert_eq!(acc.held, Decimal::new(123456, 2));
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute_resolve() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Resolve,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "resolve error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute_after_resolve() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Resolve,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "resolve error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "second dispute error: {:?}", res);

        assert_eq!(acc.held, Decimal::new(123456, 2));
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute_chargeback() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Chargeback,
            client_id: 5,
            transaction_id: 1,
            amount: None,
        });
        assert!(res.is_ok(), "chargeback error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::ZERO);
        assert_eq!(acc.locked, true);
    }

    #[test]
    fn test_withdrawal_dispute_chargeback() {
        let mut acc = Account::new(5);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Deposit,
            client_id: 5,
            transaction_id: 1,
            amount: Some(Decimal::new(123456, 2)),
        });
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Withdrawal,
            client_id: 5,
            transaction_id: 2,
            amount: Some(Decimal::new(1111, 2)),
        });
        assert!(res.is_ok(), "withdrawal error: {:?}", res);
        assert_eq!(acc.available, Decimal::new(122345, 2));
        assert_eq!(acc.total(), Decimal::new(122345, 2));
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Dispute,
            client_id: 5,
            transaction_id: 2,
            amount: None,
        });
        assert!(res.is_ok(), "dispute error: {:?}", res);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.held, Decimal::new(-1111, 2));
        assert_eq!(acc.total(), Decimal::new(122345, 2));
        let res = acc.process(&Transaction {
            transaction_type: TransactionType::Chargeback,
            client_id: 5,
            transaction_id: 2,
            amount: None,
        });
        assert!(res.is_ok(), "chargeback error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, true);
    }
}
