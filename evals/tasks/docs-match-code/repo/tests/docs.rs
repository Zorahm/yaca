const DOCS: &str = include_str!("../docs/API.md");
const SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn every_public_signature_is_documented_exactly() {
    for line in SOURCE.lines().filter(|l| l.starts_with("pub fn ")) {
        let signature = line.trim_end_matches(" {").trim();
        assert!(DOCS.contains(signature), "docs/API.md does not document `{signature}`");
    }
}

#[test]
fn the_error_of_divide_is_documented() {
    assert!(DOCS.contains("DivideByZero"));
    assert!(!DOCS.contains("Panics"));
}
