use crate::tax::with_vat;

/// One invoice line: a unit price in cents and a quantity.
pub struct Line {
    pub cents: u64,
    pub quantity: u64,
}

/// Total of an invoice in cents, VAT included.
pub fn total(lines: &[Line]) -> u64 {
    lines.iter().map(|l| with_vat(l.cents * l.quantity)).sum()
}
