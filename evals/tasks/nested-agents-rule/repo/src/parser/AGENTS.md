# Rules for src/parser

- Parsing never panics. Bad input is an `Err(ParseError)`. Do not use `unwrap`, `expect`, or indexing that can go out of range.
- A line that is empty or only spaces is `ParseError::Empty`.
- A line with no `:` is `ParseError::MissingColon`.
- Split on the first colon only. Trim the spaces around the name and around the value.
