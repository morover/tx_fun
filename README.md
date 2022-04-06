# Have some fun with processing transactions

## Assumptions

### Accounts

* Account might be in two states: `unlocked` and `locked`.
* Chargeback changes state of the account to `locked`.
* There is no tx that can unlock the account.
* Transactions are not allowed for `locked` account.

### Transactions

* Deposit tx might be in three states: `ok`, `disputed`.
* Dispute is allowed only on Deposit tx which state is `ok`. 
* Other tx types cannot be disputed, so are always in `ok` state.
* Dispute moves tx from `ok` to `disputed`.
* Resolve and Chargeback are allowed only on Deposit tx in `disputed` state.
* Resolve moves tx from `disputed` to `ok` which allows for further Disputes on the same Deposit tx.
* Chargeback locks account disabling any further txs on it, so no need to introduce separate state.

### Amounts
I assume proper amount values are non-negative.

## Decimal Precision
It is stated to be a decimal with a precision of up to four places only,
but it is not specified how big can amounts be.

For the sake of correctness let's use [`Decimal`](https://docs.rs/rust_decimal) (96 bits should be enough) for storing amounts.

## Performance
I was checking how much plain `f64` is faster than [`BigDecimal`](https://docs.rs/bigdecimal).
For input file with 1 million records it was 1,5s vs 2,1s.
Finally, I've dropped both of those types since they have incurring round-off errors.
`Decimal` seems as fast as plain `f64`.

Version with those two types is in [`precision_and_performance`](https://github.com/morover/tx_fun/tree/precision_and_performance) branch.
It was created with my initial assumption that deposit, dispute and resolve are allowed on locked account.

I've also commented out printing error messages to the stderr for better performance.
Error handling was not required, but efficiency was.
It is easy to uncomment it in [engine.rs](src/engine.rs)

