use uploader::upload::{may_retry, MAX_ATTEMPTS};

#[test]
fn budget_matches_the_specification() {
    assert_eq!(MAX_ATTEMPTS, 7);
}

#[test]
fn retries_stop_when_the_budget_is_spent() {
    assert!(may_retry(6));
    assert!(!may_retry(7));
}
