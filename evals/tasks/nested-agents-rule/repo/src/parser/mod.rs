mod header;

pub use header::{parse_header, Header};

/// Why a line could not be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    MissingColon,
}
