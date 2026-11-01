//! The secrets' arithmetic: random tokens, their digests, and comparing them
//! without leaking through timing. Shared by the device tokens, the pairing code
//! and the certificate fingerprint.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::Error;

/// `bytes` random bytes, base64url without padding: a string that is safe in a
/// JSON field, a URL and a QR code alike.
pub(crate) fn random_token(bytes: usize) -> Result<String, Error> {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).map_err(|e| Error::Random(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(buf))
}

/// Equality without an early exit, so the time taken does not reveal how many
/// leading bytes matched. The length check may exit early: the lengths of the
/// values compared here (a fixed-size digest, a fixed-size code) are public.
pub(crate) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Used for the certificate fingerprint and for hashing device tokens, which
/// are both one-way digests in hex form.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
