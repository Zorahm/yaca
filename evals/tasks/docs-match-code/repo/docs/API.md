# mathkit API

## add

    pub fn add(a: i32, b: i32) -> i32

Adds two numbers.

## divide

    pub fn divide(a: i64, b: i64) -> i64

Divides `a` by `b`. Panics when `b` is zero.

## clamp

    pub fn clamp(value: i64, low: i64, high: i64) -> i64

Keeps `value` between `low` and `high`.
