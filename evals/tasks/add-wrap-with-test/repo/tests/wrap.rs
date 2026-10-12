use textwrap_lite::wrap::wrap;

#[test]
fn fills_lines_greedily() {
    assert_eq!(wrap("aa bb cc dd", 5), vec!["aa bb", "cc dd"]);
}

#[test]
fn long_word_gets_its_own_line() {
    assert_eq!(wrap("a verylongword b", 4), vec!["a", "verylongword", "b"]);
}

#[test]
fn empty_text_has_no_lines() {
    assert!(wrap("", 10).is_empty());
}
