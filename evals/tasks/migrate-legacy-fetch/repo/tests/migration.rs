use std::fs;
use std::path::Path;

use service::handlers::{orders, stock, users};

#[test]
fn handlers_return_the_same_bodies() {
    assert_eq!(users::profile(7), "body of https://api.example/users/7");
    assert_eq!(orders::summary(3), "body of https://api.example/orders/3/summary");
    assert_eq!(stock::level("A-1"), "body of https://api.example/stock/A-1");
}

#[test]
fn handlers_no_longer_call_legacy_fetch() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers");
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("legacy_fetch"), "{} still calls legacy_fetch", path.display());
    }
}
