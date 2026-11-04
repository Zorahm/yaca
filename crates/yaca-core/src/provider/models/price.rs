//! OpenRouter prices use decimal dollars per token; shift the decimal text
//! before parsing so multiplying a binary float does not introduce artifacts.

pub(super) fn per_million(raw: Option<&str>) -> Option<f64> {
    let text = raw?.trim();
    if text == "-1" {
        return Some(-1.0);
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let mut shifted = if whole.is_empty() {
        "0".into()
    } else {
        whole.to_string()
    };
    for at in 0..6 {
        shifted.push(fraction.as_bytes().get(at).map_or('0', |b| char::from(*b)));
    }
    let rest = fraction.get(6..).unwrap_or("").trim_end_matches('0');
    if !rest.is_empty() {
        shifted.push('.');
        shifted.push_str(rest);
    }
    shifted.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[cfg(test)]
mod tests {
    use super::per_million;

    #[test]
    fn exact_decimal_prices_and_variable_routes_survive_conversion() {
        assert_eq!(per_million(Some("0.00000015")), Some(0.15));
        assert_eq!(per_million(Some("0.000003")), Some(3.0));
        assert_eq!(per_million(Some("0")), Some(0.0));
        assert_eq!(per_million(Some("-1")), Some(-1.0));
        for invalid in ["", ".", "NaN", "-2", "5oops"] {
            assert_eq!(per_million(Some(invalid)), None);
        }
    }
}
