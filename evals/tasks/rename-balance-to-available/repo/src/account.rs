/// An account with a total and an amount on hold.
#[derive(Debug, Clone)]
pub struct Account {
    pub name: String,
    pub cents: i64,
    pub held: i64,
}

impl Account {
    /// A new account with nothing on hold.
    pub fn new(name: &str, cents: i64) -> Self {
        Account { name: name.to_string(), cents, held: 0 }
    }

    /// Money the account can spend now: the total minus what is on hold.
    pub fn balance(&self) -> i64 {
        self.cents - self.held
    }
}
