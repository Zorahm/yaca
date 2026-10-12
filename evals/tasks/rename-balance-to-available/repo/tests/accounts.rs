use ledger::account::Account;
use ledger::report::summary;
use ledger::transfer::transfer;

#[test]
fn available_excludes_holds() {
    let mut a = Account::new("alice", 1000);
    a.held = 300;
    assert_eq!(a.available(), 700);
}

#[test]
fn transfer_checks_available_not_total() {
    let mut a = Account::new("alice", 1000);
    a.held = 900;
    let mut b = Account::new("bob", 0);
    assert!(transfer(&mut a, &mut b, 500).is_err());
    assert!(transfer(&mut a, &mut b, 100).is_ok());
    assert_eq!(b.cents, 100);
}

#[test]
fn report_lists_available() {
    let a = Account::new("alice", 250);
    assert_eq!(summary(&[a]), "alice: 250");
}
