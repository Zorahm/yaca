/// An amount in minor units of some currency.
pub struct Money {
    pub amount: i64,
    pub currency: String,
}

impl Money {
    pub fn new(amount: i64, currency: &str) -> Self {
        Self {
            amount,
            currency: currency.to_string(),
        }
    }

    /// A new amount, `factor` times this one.
    pub fn times(&self, factor: i64) -> Money {
        Money::new(self.amount * factor, &self.currency)
    }

    pub fn doubled(&self) -> Money {
        self.times(DEFAULT_FACTOR)
    }
}

pub const DEFAULT_FACTOR: i64 = 2;

pub fn total(prices: &[Money]) -> i64 {
    prices.iter().map(|price| price.amount).sum()
}

fn main() {
    let prices = [Money::new(100, "EUR"), Money::new(250, "EUR")];
    println!("{} {}", total(&prices), prices[0].doubled().amount);
}
