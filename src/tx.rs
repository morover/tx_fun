use anyhow::bail;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::client::Client;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum TxType {
    Deposit { amount: Decimal },
    Withdrawal { amount: Decimal },
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Tx {
    #[serde(flatten)]
    pub(crate) tx_type: TxType,
    #[serde(rename = "client")]
    pub(crate) client_id: u16,
    #[serde(rename = "tx")]
    pub(crate) tx_id: u32,
}

impl Tx {
    pub(crate) fn process(self, clients: &mut HashMap<u16, Client>) -> anyhow::Result<()> {
        let client = match self.tx_type {
            TxType::Deposit { .. } => clients
                .entry(self.client_id)
                .or_insert(Client::create(self.client_id)),
            _ => match clients.get_mut(&self.client_id) {
                None => bail!("Account {} not found", self.client_id),
                Some(client) => client,
            },
        };

        match self.tx_type {
            TxType::Deposit { amount } => client.deposit(self.tx_id, amount),
            TxType::Withdrawal { amount } => client.withdraw(amount),
            TxType::Dispute => client.dispute(&self.tx_id),
            TxType::Resolve => client.resolve(&self.tx_id),
            TxType::Chargeback => client.chargeback(&self.tx_id),
        }
    }
}
