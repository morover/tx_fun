use crate::amount::{Amount, AmountConv};
use anyhow::{anyhow, bail, ensure};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
enum DepositState {
    Ok,
    Disputed,
    ChargedBack,
}

impl Default for DepositState {
    fn default() -> Self {
        DepositState::Ok
    }
}

#[derive(Debug)]
struct Deposit {
    amount: u64,
    state: DepositState,
}

impl Deposit {
    fn ensure_state(&self, state: DepositState) -> anyhow::Result<()> {
        if self.state != state {
            bail!("Deposit in state {:?} != {:?}", self.state, state)
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(crate) struct Client {
    client_id: u16,
    pub(crate) available: u64,
    pub(crate) held: u64,
    pub(crate) total: u64,
    locked: bool,
    // storing only deposits, as only them may be disputed
    deposits: HashMap<u32, Deposit>,
}

impl Serialize for Client {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Client", 5)?;
        state.serialize_field("client", &self.client_id)?;
        state.serialize_field("available", &Amount::format(self.available))?;
        state.serialize_field("held", &Amount::format(self.held))?;
        state.serialize_field("total", &Amount::format(self.total))?;
        state.serialize_field("locked", &self.locked)?;
        state.end()
    }
}

impl Client {
    pub(crate) fn create(client_id: u16) -> Self {
        Client {
            client_id,
            available: 0,
            held: 0,
            total: 0,
            locked: false,
            deposits: Default::default(),
        }
    }

    /// A deposit increases the available and total funds.
    /// Only positive amounts are accepted.
    /// Deposit is allowed even for locked accounts.
    pub(crate) fn deposit(&mut self, tx_id: u32, amount: Amount) -> anyhow::Result<()> {
        let amount = amount.to_u64()?;
        self.deposits.insert(
            // tx ids are unique
            tx_id,
            Deposit {
                amount: amount,
                state: DepositState::Ok,
            },
        );

        self.available += amount.clone();
        self.total += amount;
        Ok(())
    }

    /// A withdraw decreases the available and total funds.
    /// Only positive amounts are accepted.
    /// It is not allowed to withdraw from locked account or exceeding available funds.
    pub(crate) fn withdraw(&mut self, amount: Amount) -> anyhow::Result<()> {
        let amount = amount.to_u64()?;
        self.ensure_unlocked()?;
        ensure!(
            self.available >= amount,
            "Account {}: Not enough funds available: {} > {}",
            self.client_id,
            Amount::format(amount),
            Amount::format(self.available),
        );
        self.available -= amount.clone();
        self.total -= amount;
        Ok(())
    }

    /// A dispute decreases available funds by the amount disputed, increases held funds,
    /// total funds remain the same.
    /// It is only allowed to dispute Deposits which are not being disputed nor been chargedback.
    /// It is possible to dispute already resolved Deposits.
    /// It is not allowed to dispute when there is not enough available funds.
    pub(crate) fn dispute(&mut self, tx_id: &u32) -> anyhow::Result<()> {
        let deposit = self
            .deposits
            .get_mut(tx_id)
            .ok_or(anyhow!("Deposit not found {}", tx_id))?;
        deposit.ensure_state(DepositState::Ok)?;
        ensure!(
            self.available >= deposit.amount,
            "Account {}: Not enough funds available: {} > {}",
            self.client_id,
            Amount::format(deposit.amount),
            Amount::format(self.available),
        );
        self.available -= deposit.amount.clone();
        self.held += deposit.amount.clone();
        deposit.state = DepositState::Disputed;
        Ok(())
    }

    /// A resolve decreases held funds by the amount no longer disputed, increases available funds,
    /// total funds remain the same.
    /// It is only allowed to dispute Deposits which are not being disputed nor been chargedback.
    /// It is possible to dispute already resolved Deposits.
    pub(crate) fn resolve(&mut self, tx_id: &u32) -> anyhow::Result<()> {
        let deposit = self
            .deposits
            .get_mut(tx_id)
            .ok_or(anyhow!("Deposit not found {}", tx_id))?;
        deposit.ensure_state(DepositState::Disputed)?;
        self.available += deposit.amount.clone();
        // no need to check held funds, bc we had checked state already
        self.held -= deposit.amount.clone();
        deposit.state = DepositState::Ok;
        Ok(())
    }

    /// A chargeback decreases clients held funds and total funds by the amount previously disputed.
    /// A chargeback makes client's account locked / frozen.
    /// It is only allowed to chargeback previously disputed Deposits.
    /// It is not allowed to chargeback when there are not enough total funds available.
    pub(crate) fn chargeback(&mut self, tx_id: &u32) -> anyhow::Result<()> {
        self.ensure_unlocked()?;
        let deposit = self
            .deposits
            .get_mut(tx_id)
            .ok_or(anyhow!("Deposit not found {}", tx_id))?;
        deposit.ensure_state(DepositState::Disputed)?;
        ensure!(
            self.total >= deposit.amount,
            "Account {}: Not enough funds in total: {} > {}",
            self.client_id,
            Amount::format(deposit.amount),
            Amount::format(self.total),
        );
        self.total -= deposit.amount.clone();
        self.held -= deposit.amount.clone();
        deposit.state = DepositState::ChargedBack;
        self.locked = true;
        Ok(())
    }

    fn ensure_unlocked(&self) -> anyhow::Result<()> {
        Ok(ensure!(
            !self.locked,
            "Account {} is locked",
            self.client_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amount::AmountConv;

    trait ClientIs {
        fn is(&self, available: u64, held: u64, total: u64);
        fn is_locked(&self, available: u64, held: u64, total: u64);
    }

    impl ClientIs for Client {
        fn is(&self, available: u64, held: u64, total: u64) {
            assert_eq!(self.available, available);
            assert_eq!(self.held, held);
            assert_eq!(self.total, total);
            assert_ne!(self.locked, true);
        }

        fn is_locked(&self, available: u64, held: u64, total: u64) {
            assert_eq!(self.available, available);
            assert_eq!(self.held, held);
            assert_eq!(self.total, total);
            assert!(self.locked);
        }
    }

    #[test]
    fn should_properly_handle_deposit() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(10000))?;
        c.is(10000, 00000, 10000);
        Ok(())
    }

    #[test]
    fn should_properly_handle_big_deposit() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(4944754876))?;
        c.is(4944754876, 00000, 4944754876);
        c.withdraw(AmountConv::from_u64(966585182))?;
        c.is(4944754876 - 966585182, 00000, 4944754876 - 966585182);
        Ok(())
    }

    #[test]
    fn should_properly_handle_small_deposit() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(31400))?;
        c.is(31400, 00000, 31400);
        c.deposit(2, AmountConv::from_u64(11400))?;
        c.is(42800, 00000, 42800);
        c.dispute(&1)?;
        c.is(11400, 31400, 42800);
        Ok(())
    }

    #[test]
    fn should_not_allow_disputes_for_unknown_id() -> anyhow::Result<()> {
        let mut c = Client::default();
        assert_eq!(
            c.dispute(&2).unwrap_err().to_string(),
            "Deposit not found 2"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_resolve_for_unknown_id() -> anyhow::Result<()> {
        let mut c = Client::default();
        assert_eq!(
            c.resolve(&3).unwrap_err().to_string(),
            "Deposit not found 3"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_chargeback_for_unknown_id() -> anyhow::Result<()> {
        let mut c = Client::default();
        assert_eq!(
            c.chargeback(&4).unwrap_err().to_string(),
            "Deposit not found 4"
        );
        Ok(())
    }

    #[test]
    fn should_deposit_multiple() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(10000))?;
        c.is(10000, 00000, 10000);
        c.deposit(2, AmountConv::from_u64(10000))?;
        c.is(20000, 00000, 20000);
        c.deposit(3, AmountConv::from_u64(30000))?;
        c.is(50000, 00000, 50000);
        Ok(())
    }

    #[test]
    fn should_deposit_multiple_and_withdraw() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(10000))?;
        c.is(10000, 00000, 10000);
        c.deposit(2, AmountConv::from_u64(10000))?;
        c.is(20000, 00000, 20000);
        c.deposit(3, AmountConv::from_u64(30000))?;
        c.is(50000, 00000, 50000);

        c.withdraw(AmountConv::from_u64(40000))?;
        c.is(10000, 00000, 10000);
        Ok(())
    }

    #[test]
    fn should_deposit_multiple_and_withdraw_ingnoring_failed_dispute() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(10000))?;
        c.is(10000, 00000, 10000);
        assert_eq!(
            c.dispute(&2).unwrap_err().to_string(),
            "Deposit not found 2"
        );
        c.deposit(2, AmountConv::from_u64(10000))?;
        c.is(20000, 00000, 20000);
        c.deposit(3, AmountConv::from_u64(30000))?;
        c.is(50000, 00000, 50000);

        c.withdraw(AmountConv::from_u64(40000))?;
        c.is(10000, 00000, 10000);
        Ok(())
    }

    #[test]
    fn should_not_allow_dispute_on_disputed() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, AmountConv::from_u64(30000))?;
        c.is(30000, 00000, 30000);
        c.dispute(&3)?;
        c.is(00000, 30000, 30000);
        assert_eq!(
            c.dispute(&3).unwrap_err().to_string(),
            "Deposit in state Disputed != Ok"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_resolve_on_resolved() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, AmountConv::from_u64(50000))?;
        c.is(50000, 00000, 50000);
        c.dispute(&3)?;
        c.is(00000, 50000, 50000);
        c.resolve(&3)?;
        c.is(50000, 00000, 50000);
        assert_eq!(
            c.resolve(&3).unwrap_err().to_string(),
            "Deposit in state Ok != Disputed"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_chargeback_on_resolved() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, AmountConv::from_u64(50000))?;
        c.is(50000, 00000, 50000);
        c.dispute(&3)?;
        c.is(00000, 50000, 50000);
        c.resolve(&3)?;
        c.is(50000, 00000, 50000);
        assert_eq!(
            c.chargeback(&3).unwrap_err().to_string(),
            "Deposit in state Ok != Disputed"
        );
        Ok(())
    }

    #[test]
    fn should_allow_dispute_on_resolved() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, AmountConv::from_u64(50000))?;
        c.is(50000, 00000, 50000);

        c.withdraw(AmountConv::from_u64(40000))?;
        c.is(10000, 00000, 10000);
        c.deposit(4, AmountConv::from_u64(70000))?;
        c.is(80000, 00000, 80000);
        c.dispute(&3)?;
        c.is(30000, 50000, 80000);
        c.resolve(&3)?;
        c.is(80000, 00000, 80000);
        c.dispute(&3)?;
        c.is(30000, 50000, 80000);
        Ok(())
    }

    #[test]
    fn should_not_allow_dispute_when_not_enough_available() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, AmountConv::from_u64(50000))?;
        c.is(50000, 00000, 50000);
        c.withdraw(AmountConv::from_u64(40000))?;
        c.is(10000, 00000, 10000);
        assert_eq!(
            c.dispute(&3).unwrap_err().to_string(),
            "Account 0: Not enough funds available: 5.0000 > 1.0000"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_withdraw_when_not_enough() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(10000))?;
        c.is(10000, 00000, 10000);
        c.deposit(2, AmountConv::from_u64(20000))?;
        c.is(30000, 00000, 30000);
        assert_eq!(
            c.withdraw(AmountConv::from_u64(40000))
                .unwrap_err()
                .to_string(),
            "Account 0: Not enough funds available: 4.0000 > 3.0000"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_withdraw_when_not_enough_in_dispute() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(10000))?;
        c.is(10000, 00000, 10000);
        c.deposit(2, AmountConv::from_u64(20000))?;
        c.is(30000, 00000, 30000);
        c.dispute(&2)?;
        c.is(10000, 20000, 30000);
        assert_eq!(
            c.withdraw(AmountConv::from_u64(20000))
                .unwrap_err()
                .to_string(),
            "Account 0: Not enough funds available: 2.0000 > 1.0000"
        );
        Ok(())
    }

    #[test]
    fn should_allow_only_one_chargeback() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, AmountConv::from_u64(30000))?;
        c.is(30000, 00000, 30000);
        c.dispute(&3)?;
        c.is(00000, 30000, 30000);
        c.chargeback(&3)?;
        c.is_locked(00000, 00000, 00000);
        assert_eq!(
            c.chargeback(&3).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_dispute_nor_resolve_on_chargedback() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, AmountConv::from_u64(30000))?;
        c.is(30000, 00000, 30000);
        c.dispute(&3)?;
        c.is(00000, 30000, 30000);
        c.chargeback(&3)?;
        c.is_locked(00000, 00000, 00000);
        assert_eq!(
            c.dispute(&3).unwrap_err().to_string(),
            "Deposit in state ChargedBack != Ok"
        );
        assert_eq!(
            c.resolve(&3).unwrap_err().to_string(),
            "Deposit in state ChargedBack != Disputed"
        );
        Ok(())
    }

    #[test]
    fn should_allow_deposit_and_dispute_and_resolve_but_not_chargeback_nor_withdrawal_on_locked(
    ) -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, AmountConv::from_u64(10000))?;
        c.is(10000, 00000, 10000);
        c.deposit(2, AmountConv::from_u64(10000))?;
        c.is(20000, 00000, 20000);
        c.deposit(3, AmountConv::from_u64(30000))?;
        c.is(50000, 00000, 50000);
        c.dispute(&3)?;
        c.is(20000, 30000, 50000);
        c.chargeback(&3)?;
        c.is_locked(20000, 00000, 20000);
        c.deposit(4, AmountConv::from_u64(40000))?;
        c.is_locked(60000, 00000, 60000);
        c.dispute(&2)?;
        c.is_locked(50000, 10000, 60000);
        assert_eq!(
            c.chargeback(&2).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        assert_eq!(
            c.withdraw(AmountConv::from_u64(10000))
                .unwrap_err()
                .to_string(),
            "Account 0 is locked"
        );

        c.resolve(&2)?;
        c.is_locked(60000, 00000, 60000);
        c.dispute(&4)?;
        c.is_locked(20000, 40000, 60000);
        c.resolve(&4)?;
        c.is_locked(60000, 00000, 60000);
        Ok(())
    }
}
