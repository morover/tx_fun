use anyhow::bail;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;

use crate::amount::Amount;
use crate::client::Client;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum TxType {
    Deposit { amount: Amount },
    Withdrawal { amount: Amount },
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Tx {
    #[serde(flatten)]
    pub(crate) tx_type: TxType,
    #[serde(rename = "client")]
    pub(crate) client_id: u16,
    #[serde(rename = "tx")]
    pub(crate) tx_id: u32,
}

impl Tx {
    pub(crate) fn process(&self, clients: &mut HashMap<u16, Client>) -> anyhow::Result<()> {
        let client = if let TxType::Deposit { .. } = self.tx_type {
            clients
                .entry(self.client_id)
                .or_insert(Client::create(self.client_id))
        } else {
            match clients.get_mut(&self.client_id) {
                None => bail!("Account {} not found", self.client_id),
                Some(client) => client,
            }
        };

        match &self.tx_type {
            TxType::Deposit { amount } => client.deposit(self.tx_id, amount.clone()),
            TxType::Withdrawal { amount } => client.withdraw(amount.clone()),
            TxType::Dispute => client.dispute(&self.tx_id),
            TxType::Resolve => client.resolve(&self.tx_id),
            TxType::Chargeback => client.chargeback(&self.tx_id),
        }
    }
}
