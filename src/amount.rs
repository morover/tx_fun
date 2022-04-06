use anyhow::ensure;

/// playing with other decimal types: f64 is much faster
#[cfg(feature = "f64")]
pub(crate) type Amount = f64;
#[cfg(feature = "bigdec")]
pub(crate) type Amount = bigdecimal::BigDecimal;

pub(crate) trait AmountConv {
    fn to_u64(self) -> anyhow::Result<u64>;
    fn from_u64(v: u64) -> Self;
    fn format(v: u64) -> String {
        format!("{:.4}", Amount::from_u64(v))
    }
}

#[cfg(feature = "f64")]
impl AmountConv for Amount {

    fn to_u64(self) -> anyhow::Result<u64> {
        ensure!(
            self >= AmountConv::from_u64(0),
            "Negative amount {:.4}",
            self
        );
        Ok((self * 10_000.).round() as u64)
    }

    fn from_u64(v: u64) -> Self {
        (v as f64) / 10_000.
    }
}

#[cfg(feature = "bigdec")]
impl AmountConv for Amount {

    fn to_u64(self) -> anyhow::Result<u64> {
        use bigdecimal::{BigDecimal, ToPrimitive};
        ensure!(
            self >= BigDecimal::from(0u64),
            "Negative amount {:.4}",
            self
        );
        let (bigint, _exp) = self.round(4).as_bigint_and_exponent();
        bigint.to_u64().ok_or(anyhow::anyhow!("Conversion {} to u64 error", self))
    }

    fn from_u64(v: u64) -> Self {
        bigdecimal::BigDecimal::new(v.into(), 4)
    }
}

#[cfg(test)]
mod tests {
    use crate::amount::{Amount, AmountConv};
    use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};

    #[test]
    fn serialize() -> anyhow::Result<()> {
        let a: Amount = AmountConv::from_u64(31400);
        #[cfg(feature = "f64")]
        let mut buf = [0 as u8; 5];
        #[cfg(feature = "bigdec")]
        let mut buf = [0 as u8; 7];
        {
            let mut wtr = csv::Writer::from_writer(&mut buf[..]);
            wtr.serialize(a)?;
            wtr.flush()?;
        }
        #[cfg(feature = "f64")]
        assert_eq!("3.14\n", String::from_utf8_lossy(&buf));
        #[cfg(feature = "bigdec")]
        assert_eq!("3.1400\n", String::from_utf8_lossy(&buf));
        Ok(())
    }


    #[test]
    fn deserialize() -> anyhow::Result<()> {
        let data = "header\n3.14";
        let mut rdr = csv::Reader::from_reader(data.as_bytes());
        for result in rdr.deserialize() {
            let record: Amount = result?;
            assert_eq!(record, AmountConv::from_u64(31400));
        }
        Ok(())
    }

    /// This is the error that prevented using BigDecimal as amount in clinet
    #[test]
    fn bigdecimal_error() {
        let b = BigDecimal::from_f64(96658.5182).unwrap();
        assert_ne!(96658.5182, b.to_f64().unwrap());
    }
}
