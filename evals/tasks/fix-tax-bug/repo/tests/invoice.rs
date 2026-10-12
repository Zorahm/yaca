use billing::invoice::{total, Line};

#[test]
fn a_single_line_includes_vat() {
    assert_eq!(total(&[Line { cents: 1000, quantity: 1 }]), 1200);
}

#[test]
fn quantities_multiply_before_vat() {
    assert_eq!(total(&[Line { cents: 250, quantity: 4 }]), 1200);
}

#[test]
fn lines_add_up() {
    assert_eq!(
        total(&[Line { cents: 1000, quantity: 1 }, Line { cents: 500, quantity: 2 }]),
        2400
    );
}
