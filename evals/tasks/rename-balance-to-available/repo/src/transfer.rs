use crate::account::Account;

/// Moves `amount` from `from` to `to` if `from` can cover it.
pub fn transfer(from: &mut Account, to: &mut Account, amount: i64) -> Result<(), String> {
    if from.balance() < amount {
        return Err(format!("{} has only {} available", from.name, from.balance()));
    }
    from.cents -= amount;
    to.cents += amount;
    Ok(())
}
