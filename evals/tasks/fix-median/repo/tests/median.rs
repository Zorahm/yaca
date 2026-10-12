use median_fix::median;

#[test]
fn empty_is_none() {
    assert_eq!(median(&[]), None);
}

#[test]
fn odd_length_takes_the_middle_of_the_sorted_values() {
    assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
}

#[test]
fn even_length_averages_middle_pair() {
    assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
}
