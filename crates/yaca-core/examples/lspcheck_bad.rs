//! Every line below is something a language server should complain about.

struct Money {
    amount: i64,
}

fn times(&self, factor: i64) -> Money {
    Money {
        amount: self.amount * factor,
    }
}

fn main() {
    let money = Money { amount: 100 };
    let doubled: Money = money.times("2");
    let unused = 42;
    let text: i64 = "receipt";
    let missing = undefined_helper(3);
    println!(
        "{} {} {} {}",
        doubled.amount, text, missing, money.amount.extra
    );
}
