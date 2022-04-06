# Have some fun with processing transactions

## Assumptions

### Accounts

* Account might be in two states: unlocked and locked.
* Chargeback changes state of the account to locked.
* There is no tx that can unlock the account.
* Withdrawal and Chargeback are not allowed for locked account.
* Deposit, Dispute and Resolve are allowed even for locked accounts.

### Transactions

* Deposit tx might be in three states: ok, disputed, chargedback.
* Dispute is allowed only on Deposit tx which state is `ok`. 
* Other tx types cannot be disputed, so are always in `ok` state.
* Resolve and Chargeback are alowed only on tx in `disputed` state.
* Resolve moves tx from `disputed` to `ok` which allows for another Dispute on the same tx.
* Chagreback moves tx from `disputed` to `chargedback` and disallows any further txs on this tx.

