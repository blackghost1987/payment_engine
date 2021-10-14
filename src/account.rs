use crate::transaction::*;

use itertools::Itertools;
use rayon::prelude::*;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Serialize;
use std::collections::HashMap;
use std::ops::Neg;

#[derive(Debug, PartialEq)]
pub struct TransactionStatus {
    pub amount_change: Decimal,
    pub disputed: bool,
    pub chargeback: bool,
}

impl TransactionStatus {
    pub fn new(transaction: &Transaction) -> Result<TransactionStatus> {
        let mut amount_change = transaction.get_amount()?;

        // negate Amount so a disputed Withdrawal increases the available amount
        if transaction.transaction_type == TransactionType::Withdrawal {
            amount_change = amount_change.neg();
        }

        Ok(TransactionStatus {
            amount_change,
            disputed: false,
            chargeback: false,
        })
    }

    pub fn dispute(&mut self) -> Result<Decimal> {
        if self.disputed {
            return Err(Error::AlreadyDisputed);
        }
        self.disputed = true;
        Ok(self.amount_change)
    }

    pub fn resolve(&mut self) -> Result<Decimal> {
        if !self.disputed {
            return Err(Error::NotDisputed);
        }
        self.disputed = false;
        Ok(self.amount_change)
    }

    pub fn chargeback(&mut self) -> Result<Decimal> {
        if !self.disputed {
            return Err(Error::NotDisputed);
        }
        self.chargeback = true;
        Ok(self.amount_change)
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

    pub fn from_transactions(client_id: &ClientId, transactions: &[Transaction], verbose: bool) -> Account {
        let mut acc = Account::new(client_id.to_owned());

        for tr in transactions {
            if let Err(e) = acc.process(tr, verbose) {
                if verbose {
                    println!("Ignoring transaction with ID: {}. Reason: {:?}", tr.transaction_id, e)
                }
            }
        }

        acc
    }

    pub fn total(&self) -> Decimal {
        self.available + self.held
    }

    fn get_transaction_status(&mut self, tr_id: TransactionId) -> Result<&mut TransactionStatus> {
        self.transaction_status
            .get_mut(&tr_id)
            .ok_or(Error::UnknownTransactionId)
    }

    pub fn process(&mut self, tr: &Transaction, verbose: bool) -> Result<()> {
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

                self.available += status.amount_change;
                self.transaction_status.insert(tr.transaction_id, status);
            }
            Withdrawal => {
                if self.transaction_status.contains_key(&tr.transaction_id) {
                    return Err(Error::DuplicatedTransactionId);
                }
                let status = TransactionStatus::new(tr)?;
                if (self.available + status.amount_change).is_sign_negative() {
                    return Err(Error::InsufficientFunds);
                }

                self.available += status.amount_change;
                self.transaction_status.insert(tr.transaction_id, status);
            }
            Dispute => {
                tr.check_amount_empty(verbose);
                let amount_change = {
                    let ref_tr = self.get_transaction_status(tr.transaction_id)?;
                    ref_tr.dispute()?
                };
                self.available -= amount_change;
                self.held += amount_change;
            }
            Resolve => {
                tr.check_amount_empty(verbose);
                let amount_change = {
                    let ref_tr = self.get_transaction_status(tr.transaction_id)?;
                    ref_tr.resolve()?
                };
                self.available += amount_change;
                self.held -= amount_change;
            }
            Chargeback => {
                tr.check_amount_empty(verbose);
                let amount_change = {
                    let ref_tr = self.get_transaction_status(tr.transaction_id)?;
                    ref_tr.chargeback()?
                };
                self.held -= amount_change;
                self.locked = true;
            }
        }

        Ok(())
    }
}

pub fn process_all(transactions: Vec<Transaction>, verbose: bool) -> HashMap<ClientId, Account> {
    let transactions_per_client: Vec<(ClientId, Vec<Transaction>)> = transactions
        .into_iter()
        .group_by(|t| t.client_id)
        .into_iter()
        .map(|(id, items)| (id, items.collect()))
        .collect();

    // using rayon to process clients in parallel
    transactions_per_client.par_iter()
        .map(|(cid, ctr)| {
            let acc = Account::from_transactions(cid, ctr, verbose);
            (cid.to_owned(), acc)
        })
        .collect()
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
            available: a
                .available
                .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero),
            held: a
                .held
                .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero),
            total: a
                .total()
                .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero),
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
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert_eq!(res, Err(Error::ClientIdMismatch), "foreign transaction should fail");
        assert_eq!(acc.total(), Decimal::ZERO);
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_deposit() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "processing error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_duplicate_id() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "processing error: {:?}", res);

        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(3456, 2)),
            },
            false,
        );
        assert_eq!(res, Err(Error::DuplicatedTransactionId), "duplicated id should fail");
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_withdraw() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Withdrawal,
                client_id: 5,
                transaction_id: 2,
                amount: Some(Decimal::new(3456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "withdraw error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(1200, 0));
        assert_eq!(acc.total(), Decimal::new(1200, 0));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_insufficient_funds() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Withdrawal,
                client_id: 5,
                transaction_id: 2,
                amount: Some(Decimal::new(11113456, 2)),
            },
            false,
        );
        assert_eq!(res, Err(Error::InsufficientFunds), "too large withdrawal should fail");
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "dispute error: {:?}", res);

        assert_eq!(acc.held, Decimal::new(123456, 2));
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_double_dispute() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert_eq!(res, Err(Error::AlreadyDisputed), "double dispute should fail");

        assert_eq!(acc.held, Decimal::new(123456, 2));
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute_resolve() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Resolve,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "resolve error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute_after_resolve() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Resolve,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "resolve error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "second dispute error: {:?}", res);

        assert_eq!(acc.held, Decimal::new(123456, 2));
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }

    #[test]
    fn test_dispute_chargeback() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "dispute error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Chargeback,
                client_id: 5,
                transaction_id: 1,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "chargeback error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::ZERO);
        assert_eq!(acc.total(), Decimal::ZERO);
        assert_eq!(acc.locked, true);
    }

    #[test]
    fn test_withdrawal_dispute_chargeback() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Withdrawal,
                client_id: 5,
                transaction_id: 2,
                amount: Some(Decimal::new(1111, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "withdrawal error: {:?}", res);
        assert_eq!(acc.available, Decimal::new(122345, 2));
        assert_eq!(acc.total(), Decimal::new(122345, 2));
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 2,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "dispute error: {:?}", res);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.held, Decimal::new(-1111, 2));
        assert_eq!(acc.total(), Decimal::new(122345, 2));
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Chargeback,
                client_id: 5,
                transaction_id: 2,
                amount: None,
            },
            false,
        );
        assert!(res.is_ok(), "chargeback error: {:?}", res);

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, true);
    }

    #[test]
    fn test_failed_withdrawal_dispute() {
        let mut acc = Account::new(5);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Deposit,
                client_id: 5,
                transaction_id: 1,
                amount: Some(Decimal::new(123456, 2)),
            },
            false,
        );
        assert!(res.is_ok(), "deposit error: {:?}", res);
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Withdrawal,
                client_id: 5,
                transaction_id: 2,
                amount: Some(Decimal::new(999991111, 2)),
            },
            false,
        );
        assert_eq!(res, Err(Error::InsufficientFunds), "too large withdrawal should fail");
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        let res = acc.process(
            &Transaction {
                transaction_type: TransactionType::Dispute,
                client_id: 5,
                transaction_id: 2,
                amount: None,
            },
            false,
        );
        assert_eq!(res, Err(Error::UnknownTransactionId), "failed withdrawal cannot be disputed");

        assert_eq!(acc.held, Decimal::ZERO);
        assert_eq!(acc.available, Decimal::new(123456, 2));
        assert_eq!(acc.total(), Decimal::new(123456, 2));
        assert_eq!(acc.locked, false);
    }
}
