use anyhow::{anyhow, bail, ensure};
use rust_decimal::{Decimal, prelude::Zero};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
enum DepositState {
    Ok,
    Dispute,
}

impl Default for DepositState {
    fn default() -> Self {
        DepositState::Ok
    }
}

#[derive(Debug)]
struct Deposit {
    amount: Decimal,
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
    pub(crate) available: Decimal,
    pub(crate) held: Decimal,
    pub(crate) total: Decimal,
    locked: bool,
    // storing only deposits, as only them may be disputed
    deposits: HashMap<u32, Deposit>,
}

impl Serialize for Client {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("Client", 5)?;
        state.serialize_field("client", &self.client_id)?;
        state.serialize_field("available", &self.available.round_dp(4))?;
        state.serialize_field("held", &self.held.round_dp(4))?;
        state.serialize_field("total", &self.total.round_dp(4))?;
        state.serialize_field("locked", &self.locked)?;
        state.end()
    }
}

impl Client {
    pub(crate) fn create(client_id: u16) -> Self {
        Client {
            client_id,
            available: Decimal::zero(),
            held: Decimal::zero(),
            total: Decimal::zero(),
            locked: false,
            deposits: Default::default(),
        }
    }

    /// A deposit increases the available and total funds.
    /// Only positive amounts are accepted.
    /// Deposit is not allowed on locked account.
    pub(crate) fn deposit(&mut self, tx_id: u32, amount: Decimal) -> anyhow::Result<()> {
        ensure!(amount >= 0.into(), "Negative amount {}", amount);
        self.ensure_unlocked()?;
        self.deposits.insert(
            // tx ids are unique
            tx_id,
            Deposit {
                amount: amount.clone(),
                state: DepositState::Ok,
            },
        );

        self.available += &amount;
        self.total += &amount;
        Ok(())
    }

    /// A withdraw decreases the available and total funds.
    /// Only positive amounts are accepted.
    /// It is not allowed to withdraw from locked account or exceeding available funds.
    pub(crate) fn withdraw(&mut self, amount: Decimal) -> anyhow::Result<()> {
        ensure!(amount >= 0.into(), "Negative amount {}", amount);
        self.ensure_unlocked()?;
        ensure!(
            self.available >= amount,
            "Account {}: Not enough funds available: {} > {}",
            self.client_id,
            amount,
            self.available,
        );
        self.available -= &amount;
        self.total -= &amount;
        Ok(())
    }

    /// A dispute decreases available funds by the amount disputed, increases held funds,
    /// total funds remain the same.
    /// It is only allowed to dispute Deposits which are not being disputed nor been chargedback.
    /// It is possible to dispute already resolved Deposits.
    /// It is not allowed to dispute when there is not enough available funds.
    /// Dispute is not allowed for locked account.
    pub(crate) fn dispute(&mut self, tx_id: &u32) -> anyhow::Result<()> {
        self.ensure_unlocked()?;
        let deposit = self
            .deposits
            .get_mut(tx_id)
            .ok_or(anyhow!("Deposit not found {}", tx_id))?;
        deposit.ensure_state(DepositState::Ok)?;
        ensure!(
            self.available >= deposit.amount,
            "Account {}: Not enough funds available: {} > {}",
            self.client_id,
            deposit.amount,
            self.available,
        );
        self.available -= &deposit.amount;
        self.held += &deposit.amount;
        deposit.state = DepositState::Dispute;
        Ok(())
    }

    /// A resolve decreases held funds by the amount no longer disputed, increases available funds,
    /// total funds remain the same.
    /// It is only allowed to resolve Deposits which are being disputed, but not been charged back.
    /// Resolve is not allowed even locked account.
    pub(crate) fn resolve(&mut self, tx_id: &u32) -> anyhow::Result<()> {
        self.ensure_unlocked()?;
        let deposit = self
            .deposits
            .get_mut(tx_id)
            .ok_or(anyhow!("Deposit not found {}", tx_id))?;
        deposit.ensure_state(DepositState::Dispute)?;
        self.available += &deposit.amount;
        // no need to check held funds, bc we had checked state already
        self.held -= &deposit.amount;
        deposit.state = DepositState::Ok;
        Ok(())
    }

    /// A chargeback decreases clients held funds and total funds by the amount previously disputed.
    /// A chargeback makes client's account locked / frozen.
    /// It is only allowed to chargeback Deposits which are being disputed.
    /// It is not allowed to chargeback when there are not enough total funds available.
    /// Chargeback is not allowed for locked account.
    pub(crate) fn chargeback(&mut self, tx_id: &u32) -> anyhow::Result<()> {
        self.ensure_unlocked()?;
        let deposit = self
            .deposits
            .get_mut(tx_id)
            .ok_or(anyhow!("Deposit not found {}", tx_id))?;
        deposit.ensure_state(DepositState::Dispute)?;
        ensure!(
            self.total >= deposit.amount,
            "Account {}: Not enough funds in total: {} > {}",
            self.client_id,
            deposit.amount,
            self.total,
        );
        self.total -= &deposit.amount;
        self.held -= &deposit.amount;
        self.locked = true;
        Ok(())
    }

    fn ensure_unlocked(&self) -> anyhow::Result<()> {
        ensure!(!self.locked, "Account {} is locked", self.client_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::{Decimal, prelude::FromPrimitive};

    trait ClientIs {
        fn is(&self, available: f64, held: f64, total: f64);
        fn is_locked(&self, available: f64, held: f64, total: f64);
    }

    impl ClientIs for Client {
        fn is(&self, available: f64, held: f64, total: f64) {
            assert_eq!(self.available, Decimal::from_f64(available).unwrap());
            assert_eq!(self.held, Decimal::from_f64(held).unwrap());
            assert_eq!(self.total, Decimal::from_f64(total).unwrap());
            assert_ne!(self.locked, true);
        }

        fn is_locked(&self, available: f64, held: f64, total: f64) {
            assert_eq!(self.available, Decimal::from_f64(available).unwrap());
            assert_eq!(self.held, Decimal::from_f64(held).unwrap());
            assert_eq!(self.total, Decimal::from_f64(total).unwrap());
            assert!(self.locked);
        }
    }

    #[test]
    fn should_properly_handle_deposit() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, 1.into())?;
        c.is(1., 0., 1.);
        Ok(())
    }

    #[test]
    fn should_properly_handle_big_deposit() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, Decimal::from_f64(494475.4876).unwrap())?;
        c.is(494475.4876, 0., 494475.4876);
        c.withdraw(Decimal::from_f64(96658.5182).unwrap())?;
        c.is(494475.4876 - 96658.5182, 0., 494475.4876 - 96658.5182);
        Ok(())
    }

    #[test]
    fn should_properly_handle_small_deposit() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, Decimal::from_f64(3.14).unwrap())?;
        c.is(3.14, 0., 3.14);
        c.deposit(2, Decimal::from_f64(1.14).unwrap())?;
        c.is(4.28, 0., 4.28);
        c.dispute(&1)?;
        c.is(1.14, 3.14, 4.28);
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
        c.deposit(1, 1.into())?;
        c.is(1., 0., 1.);
        c.deposit(2, 1.into())?;
        c.is(2., 0., 2.);
        c.deposit(3, 3.into())?;
        c.is(5., 0., 5.);
        Ok(())
    }

    #[test]
    fn should_deposit_multiple_and_withdraw() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, 1.into())?;
        c.is(1., 0., 1.);
        c.deposit(2, 1.into())?;
        c.is(2., 0., 2.);
        c.deposit(3, 3.into())?;
        c.is(5., 0., 5.);

        c.withdraw(4.into())?;
        c.is(1., 0., 1.);
        Ok(())
    }

    #[test]
    fn should_deposit_multiple_and_withdraw_ingnoring_failed_dispute() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, 1.into())?;
        c.is(1., 0., 1.);
        assert_eq!(
            c.dispute(&2).unwrap_err().to_string(),
            "Deposit not found 2"
        );
        c.deposit(2, 1.into())?;
        c.is(2., 0., 2.);
        c.deposit(3, 3.into())?;
        c.is(5., 0., 5.);

        c.withdraw(4.into())?;
        c.is(1., 0., 1.);
        Ok(())
    }

    #[test]
    fn should_not_allow_dispute_on_disputed() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, 3.into())?;
        c.is(3., 0., 3.);
        c.dispute(&3)?;
        c.is(0., 3., 3.);
        assert_eq!(
            c.dispute(&3).unwrap_err().to_string(),
            "Deposit in state Dispute != Ok"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_resolve_on_resolved() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, 5.into())?;
        c.is(5., 0., 5.);
        c.dispute(&3)?;
        c.is(0., 5., 5.);
        c.resolve(&3)?;
        c.is(5., 0., 5.);
        assert_eq!(
            c.resolve(&3).unwrap_err().to_string(),
            "Deposit in state Ok != Dispute"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_chargeback_on_resolved() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, 5.into())?;
        c.is(5., 0., 5.);
        c.dispute(&3)?;
        c.is(0., 5., 5.);
        c.resolve(&3)?;
        c.is(5., 0., 5.);
        assert_eq!(
            c.chargeback(&3).unwrap_err().to_string(),
            "Deposit in state Ok != Dispute"
        );
        Ok(())
    }

    #[test]
    fn should_allow_dispute_on_resolved() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, 5.into())?;
        c.is(5., 0., 5.);

        c.withdraw(4.into())?;
        c.is(1., 0., 1.);
        c.deposit(4, 7.into())?;
        c.is(8., 0., 8.);
        c.dispute(&3)?;
        c.is(3., 5., 8.);
        c.resolve(&3)?;
        c.is(8., 0., 8.);
        c.dispute(&3)?;
        c.is(3., 5., 8.);
        Ok(())
    }

    #[test]
    fn should_not_allow_dispute_when_not_enough_available() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, 5.into())?;
        c.is(5., 0., 5.);
        c.withdraw(4.into())?;
        c.is(1., 0., 1.);
        assert_eq!(
            c.dispute(&3).unwrap_err().to_string(),
            "Account 0: Not enough funds available: 5 > 1"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_withdraw_when_not_enough() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, 1.into())?;
        c.is(1., 0., 1.);
        c.deposit(2, 2.into())?;
        c.is(3., 0., 3.);
        assert_eq!(
            c.withdraw(4.into()).unwrap_err().to_string(),
            "Account 0: Not enough funds available: 4 > 3"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_withdraw_when_not_enough_in_dispute() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, 1.into())?;
        c.is(1., 0., 1.);
        c.deposit(2, 2.into())?;
        c.is(3., 0., 3.);
        c.dispute(&2)?;
        c.is(1., 2., 3.);
        assert_eq!(
            c.withdraw(2.into()).unwrap_err().to_string(),
            "Account 0: Not enough funds available: 2 > 1"
        );
        Ok(())
    }

    #[test]
    fn should_allow_only_one_chargeback() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, 3.into())?;
        c.is(3., 0., 3.);
        c.dispute(&3)?;
        c.is(0., 3., 3.);
        c.chargeback(&3)?;
        c.is_locked(0., 0., 0.);
        assert_eq!(
            c.chargeback(&3).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_dispute_nor_resolve_on_chargedback() -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(3, 3.into())?;
        c.is(3., 0., 3.);
        c.dispute(&3)?;
        c.is(0., 3., 3.);
        c.chargeback(&3)?;
        c.is_locked(0., 0., 0.);
        assert_eq!(
            c.dispute(&3).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        assert_eq!(
            c.resolve(&3).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        Ok(())
    }

    #[test]
    fn should_not_allow_any_tx_on_locked(
    ) -> anyhow::Result<()> {
        let mut c = Client::default();
        c.deposit(1, 1.into())?;
        c.is(1., 0., 1.);
        c.deposit(2, 1.into())?;
        c.is(2., 0., 2.);
        c.deposit(3, 3.into())?;
        c.is(5., 0., 5.);
        c.dispute(&3)?;
        c.is(2., 3., 5.);
        c.chargeback(&3)?;
        c.is_locked(2., 0., 2.);
        assert_eq!(
            c.deposit(4, 4.into()).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        assert_eq!(
            c.withdraw(1.into()).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        assert_eq!(
            c.dispute(&2).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        assert_eq!(
            c.resolve(&3).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        assert_eq!(
            c.chargeback(&2).unwrap_err().to_string(),
            "Account 0 is locked"
        );
        Ok(())
    }
}
