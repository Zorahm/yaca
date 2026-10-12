/// VAT in percent, applied to every line of an invoice.
pub const VAT_PERCENT: u64 = 20;

/// Amount in cents including VAT, rounded down to a whole cent.
pub fn with_vat(cents: u64) -> u64 {
    cents + cents * VAT_PERCENT
}
