/// Errors returned by `divide`.
#[derive(Debug, PartialEq, Eq)]
pub enum MathError {
    DivideByZero,
}

pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

pub fn divide(a: i64, b: i64) -> Result<i64, MathError> {
    if b == 0 {
        return Err(MathError::DivideByZero);
    }
    Ok(a / b)
}

pub fn clamp(value: i64, low: i64, high: i64) -> i64 {
    value.max(low).min(high)
}
