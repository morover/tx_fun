use anyhow::anyhow;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::client::Client;
use crate::tx::Tx;

#[derive(Default)]
pub(crate) struct Engine {
    clients: HashMap<u16, Client>,
}

impl Engine {
    pub(crate) fn run(&mut self, input_file: PathBuf) -> anyhow::Result<()> {
        self.process_file(input_file)?;
        Ok(self.output()?)
    }

    fn process_file(&mut self, input_file: PathBuf) -> anyhow::Result<()> {
        let mut rdr = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_path(input_file)?;

        for result in rdr.deserialize() {
            if let Err(_e) = self.process_row(result) {
                // commenting out for better performance
                // eprintln!("Error: {}", _e)
            }
        }
        Ok(())
    }

    fn process_row(&mut self, row: csv::Result<Tx>) -> anyhow::Result<()> {
        let tx = row?;
        tx.process(&mut self.clients)
            .map_err(|e| anyhow!("Cannot process {:?}({}); {}", tx.tx_type, tx.tx_id, e))
    }

    fn output(&self) -> anyhow::Result<()> {
        let mut wtr = csv::Writer::from_writer(std::io::stdout());
        for c in self.clients.values() {
            wtr.serialize(c)?;
        }

        Ok(wtr.flush()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::TxType;
    use rust_decimal::{Decimal, prelude::FromPrimitive};
    use rand::{thread_rng, Rng};
    use serde::Serialize;

    fn random() -> Decimal {
        let r: Decimal = thread_rng().gen_range(1..1_000_000_000).into();
        r / Decimal::from(10_000)
    }

    fn assert_example_result(engine: &mut Engine) {
        let client = engine.clients.get(&1).unwrap();
        assert_eq!(client.available, Decimal::from_f32(1.5).unwrap());
        assert_eq!(client.held, 0.into());
        assert_eq!(client.total, Decimal::from_f32(1.5).unwrap());
        let client = engine.clients.get(&2).unwrap();
        assert_eq!(client.available, 2.into());
        assert_eq!(client.held, 0.into());
        assert_eq!(client.total, 2.into());
    }

    #[test]
    fn should_handle_example() -> anyhow::Result<()> {
        let mut engine = Engine::default();
        engine.process_file("test_samples/example.csv".into())?;
        assert_example_result(&mut engine);
        Ok(())
    }

    #[test]
    fn should_handle_spaceless_format() -> anyhow::Result<()> {
        let mut engine = Engine::default();
        engine.process_file("test_samples/spaceless.csv".into())?;
        assert_example_result(&mut engine);
        Ok(())
    }

    #[test]
    fn should_handle_spacefull_format() -> anyhow::Result<()> {
        let mut engine = Engine::default();
        engine.process_file("test_samples/spacefull.csv".into())?;
        assert_example_result(&mut engine);
        Ok(())
    }

    #[test]
    fn should_skip_wrong_lines_in_csv_but_process_rest() -> anyhow::Result<()> {
        let mut engine = Engine::default();
        engine.process_file("test_samples/wrong.csv".into())?;
        let client = engine.clients.get(&1).unwrap();
        assert_eq!(client.available, 1.into());
        assert_eq!(client.held, 0.into());
        assert_eq!(client.total, 1.into());
        let client = engine.clients.get(&2).unwrap();
        assert_eq!(client.available, 2.into());
        assert_eq!(client.held, 0.into());
        assert_eq!(client.total, 2.into());
        Ok(())
    }

    #[test]
    fn should_skip_nonexistent_accounts() -> anyhow::Result<()> {
        let mut engine = Engine::default();
        engine.process_file("test_samples/nonexistent.csv".into())?;
        let client = engine.clients.get(&1).unwrap();
        assert_eq!(client.available, Decimal::from_f32(0.49).unwrap());
        assert_eq!(client.held, 0.into());
        assert_eq!(client.total, Decimal::from_f32(0.49).unwrap());
        let client = engine.clients.get(&2).unwrap();
        assert_eq!(client.available, Decimal::from_f32(0.).unwrap());
        assert_eq!(client.held, Decimal::from_f32(0.).unwrap());
        assert_eq!(client.total, Decimal::from_f32(0.).unwrap());
        let client = engine.clients.get(&3).unwrap();
        assert_eq!(client.available, Decimal::from_f32(1.14).unwrap());
        assert_eq!(client.held, Decimal::from_f32(3.14).unwrap());
        assert_eq!(client.total, Decimal::from_f32(4.28).unwrap());
        Ok(())
    }

    #[test]
    #[ignore]
    fn performance_test() -> anyhow::Result<()> {
        let mut engine = Engine::default();
        let mut rng = thread_rng();

        for _ in 0..10_000_000 {
            let tx_type = match rng.gen_range(0..5) {
                0 => TxType::Deposit { amount: random() },
                1 => TxType::Withdrawal { amount: random() },
                2 => TxType::Dispute,
                3 => TxType::Resolve,
                4 => TxType::Chargeback,
                _ => unreachable!(),
            };

            let client_id = rng.gen_range(1..10_000);
            let tx_id = rng.gen_range(1..100_000);
            let tx = Tx {
                tx_type: tx_type.clone(),
                client_id,
                tx_id,
            };

            if let Err(_e) = engine.process_row(csv::Result::Ok(tx)) {
                // eprintln!("Error: {}", _e)
            }
        }
        Ok(())
    }

    #[test]
    #[ignore]
    fn generate_test_file() -> anyhow::Result<()> {
        #[derive(Clone, Debug, Serialize)]
        pub(crate) struct Tx {
            r#type: String,
            client: u16,
            tx: u32,
            amount: String,
        }

        let mut wtr = csv::Writer::from_path("tst.csv")?;
        let mut rng = thread_rng();
        for _ in 0..1_000_000 {
            let (tx_type, amount) = match rng.gen_range(0..5) {
                0 => ("deposit", format!("{:0.4}", random())),
                1 => ("withdrawal", format!("{:0.4}", random())),
                2 => ("dispute", "".into()),
                3 => ("resolve", "".into()),
                4 => ("chargeback", "".into()),
                _ => unreachable!(),
            };
            let client = rng.gen_range(1..1_000);
            let tx: u32 = rng.gen_range(1..10_000);

            wtr.serialize(Tx {
                r#type: tx_type.to_string(),
                client,
                tx,
                amount,
            })?
        }
        Ok(wtr.flush()?)
    }
}
