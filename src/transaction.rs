use rust_decimal::Decimal;
use serde::Deserialize;

pub type ClientId = u16;
pub type TransactionId = u32;

#[derive(Debug, PartialEq)]
pub enum Error {
    MissingAmount,
    InsufficientFunds,
    ClientIdMismatch,
    AccountLocked,
    UnknownTransactionId,
    DuplicatedTransactionId,
    AlreadyDisputed,
    NotDisputed,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    #[serde(rename = "client")]
    pub client_id: ClientId,
    #[serde(rename = "tx")]
    pub transaction_id: TransactionId,
    pub amount: Option<Decimal>,
}

impl Transaction {
    pub fn get_amount(&self) -> Result<Decimal> {
        self.amount.ok_or(Error::MissingAmount)
    }

    pub fn check_amount_empty(&self, verbose: bool) {
        if let Some(_) = self.amount {
            if verbose {
                println!("Unexpected amount in transaction! ID: {}", self.transaction_id)
            }
        }
    }
}
