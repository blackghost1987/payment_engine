use rust_decimal::Decimal;
use serde::Deserialize;

pub type ClientId = u16;
pub type TransactionId = u32;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Clone, Debug)]
pub struct Transaction {
    pub transaction_type: TransactionType,
    pub client_id: ClientId,
    pub transaction_id: TransactionId,
    pub amount: Decimal,
}
