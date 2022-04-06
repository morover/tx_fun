# Have some fun with processing transactions

## Assumptions

### Accounts

* Account might be in two states: `unlocked` and `locked`.
* Chargeback changes state of the account to `locked`.
* There is no tx that can unlock the account.
* Withdrawal and Chargeback are not allowed for `locked` account.
* Deposit, Dispute and Resolve are allowed even for `locked` accounts.

### Transactions

* Deposit tx might be in three states: `ok`, `disputed`, `chargedback`.
* Dispute is allowed only on Deposit tx which state is `ok`. 
* Other tx types cannot be disputed, so are always in `ok` state.
* Resolve and Chargeback are allowed only on Deposit tx in `disputed` state.
* Resolve moves tx from `disputed` to `ok` which allows for another Dispute on the same Deposit tx.
* Chargeback moves tx from `disputed` to `chargedback` and disallows any further txs on this Deposit tx.

### Amounts
I assume proper amount values are non-negative.

## Decimal Precision
It is stated to be a decimal with a precision of up to four places only,
but it is not specified how big can amounts be.

For the sake of correctness let's use BigDecimal for storing amounts.

## Performance
I've commented printing out error messages to the stderr for better performance.
Error handling was not required, but efficiency was.
It is easy to uncomment in [engine.rs](src/engine.rs)
