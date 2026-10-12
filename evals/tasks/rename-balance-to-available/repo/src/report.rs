use crate::account::Account;

/// One line per account: `name: amount`.
pub fn summary(accounts: &[Account]) -> String {
    accounts
        .iter()
        .map(|a| format!("{}: {}", a.name, a.balance()))
        .collect::<Vec<_>>()
        .join("\n")
}
