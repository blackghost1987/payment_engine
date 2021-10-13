use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};

pub type ClientId = u16;
pub type TransactionId = u32;

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

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Account {
    client_id: ClientId,
    available: Decimal,
    held: Decimal,
    total: Decimal, // TODO make this a calculated value
    locked: bool,
}
