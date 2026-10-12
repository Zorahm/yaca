use wire::parser::{parse_header, ParseError};

#[test]
fn splits_on_the_first_colon_and_trims() {
    let h = parse_header("  Host :  a:b  ").unwrap();
    assert_eq!(h.name, "Host");
    assert_eq!(h.value, "a:b");
}

#[test]
fn an_empty_line_is_an_error_not_a_panic() {
    assert_eq!(parse_header("   ").err(), Some(ParseError::Empty));
}

#[test]
fn a_line_without_a_colon_is_an_error() {
    assert_eq!(parse_header("Host example").err(), Some(ParseError::MissingColon));
}
