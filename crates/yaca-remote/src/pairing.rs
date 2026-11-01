//! The one-time code a phone scans to pair, and the offer that carries it.
//!
//! Redeeming a code is the only way to get a device token without already having
//! one, so a code is a password with a short life: single use, expiring, and
//! replaced whenever a new one is issued. The code on screen is then the only
//! one that can work, and a screenshot of an old one is worth nothing.

use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use qrcode::render::{svg, unicode};
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::files::epoch_ms;
use crate::secret::{constant_time_eq, random_token};
use crate::error::Error;

pub const PAIRING_TTL: Duration = Duration::from_secs(300);
/// 128 bits: enough that a code cannot be guessed inside its lifetime, even
/// with every attempt the server accepts.
const CODE_BYTES: usize = 16;
/// The QR is read by a phone camera from a screen: a floor on its size keeps
/// modules large enough to scan at arm's length.
const QR_MIN_PX: u32 = 256;
const SCHEME: &str = "yaca://pair";

struct Active {
    code: String,
    expires: SystemTime,
}

/// The host's current pairing code, if any. Only one exists at a time.
pub struct Pairing {
    ttl: Duration,
    active: Mutex<Option<Active>>,
}

impl Pairing {
    pub fn new() -> Self {
        Self::with_ttl(PAIRING_TTL)
    }

    /// A shorter life, for tests that need an expiry without waiting minutes.
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            active: Mutex::new(None),
        }
    }

    /// Issues a new code and drops the previous one, even if it had not expired:
    /// the screen shows one code at a time, and that is the one that should work.
    pub fn issue(&self) -> Result<(String, SystemTime), Error> {
        let code = random_token(CODE_BYTES)?;
        let expires = SystemTime::now()
            .checked_add(self.ttl)
            .ok_or_else(|| Error::Clock(format!("pairing lifetime of {:?} does not fit the clock", self.ttl)))?;
        *self.lock() = Some(Active {
            code: code.clone(),
            expires,
        });
        Ok((code, expires))
    }

    /// Whether `code` is the live one. A right code is spent by this call, so a
    /// second attempt with it fails. A wrong code leaves the live one in place:
    /// otherwise anyone could cancel a pairing by guessing.
    pub fn redeem(&self, code: &str) -> bool {
        let mut active = self.lock();
        let Some(current) = active.as_ref() else {
            return false;
        };
        if SystemTime::now() >= current.expires {
            *active = None;
            return false;
        }
        if !constant_time_eq(current.code.as_bytes(), code.as_bytes()) {
            return false;
        }
        *active = None;
        true
    }

    fn lock(&self) -> MutexGuard<'_, Option<Active>> {
        self.active.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Default for Pairing {
    fn default() -> Self {
        Self::new()
    }
}

/// What the desktop window shows for pairing: the URI, when it stops working,
/// and the same URI as an SVG QR code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PairingOffer {
    pub uri: String,
    #[ts(type = "number")]
    pub expires_at: u64,
    pub qr_svg: String,
}

impl PairingOffer {
    pub fn new(uri: String, expires: SystemTime) -> Result<Self, Error> {
        let qr_svg = code_for(&uri)?
            .render::<svg::Color>()
            .min_dimensions(QR_MIN_PX, QR_MIN_PX)
            .build();
        Ok(Self {
            uri,
            expires_at: epoch_ms(expires),
            qr_svg,
        })
    }
}

/// The text a terminal shows for `yaca serve`, the same code in half-height
/// Unicode blocks.
pub fn qr_text(uri: &str) -> Result<String, Error> {
    Ok(code_for(uri)?.render::<unicode::Dense1x2>().build())
}

/// The `yaca://pair` URI the phone reads from the QR. Every value is
/// percent-encoded, so a name with spaces or an address with colons survives the
/// trip; `host` repeats once per address in the order given, which is the order
/// the phone tries them in.
pub fn offer_uri(code: &str, fingerprint: &str, port: u16, hosts: &[IpAddr], name: &str) -> String {
    let mut uri = format!(
        "{SCHEME}?v=1&code={}&fp={}&port={port}&name={}",
        encode(code),
        encode(fingerprint),
        encode(name),
    );
    for host in hosts {
        // IPv6 is written bare, without the brackets of a URL authority: the
        // phone reads these as plain addresses, and the brackets are not part
        // of the address.
        uri.push_str("&host=");
        uri.push_str(&encode(&host.to_string()));
    }
    uri
}

fn code_for(uri: &str) -> Result<QrCode, Error> {
    QrCode::new(uri.as_bytes()).map_err(|e| Error::Qr(e.to_string()))
}

/// Percent-encodes everything except the RFC 3986 unreserved set. Written here
/// rather than pulled in as a crate: the URI has three values to encode, and the
/// set is small enough to state in full.
fn encode(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn a_code_pairs_once() {
        let pairing = Pairing::new();
        let (code, _) = pairing.issue().unwrap();
        assert!(pairing.redeem(&code));
        assert!(!pairing.redeem(&code));
    }

    #[test]
    fn a_wrong_code_fails_and_leaves_the_live_one_usable() {
        let pairing = Pairing::new();
        let (code, _) = pairing.issue().unwrap();
        assert!(!pairing.redeem("wrong"));
        assert!(!pairing.redeem(""));
        assert!(pairing.redeem(&code));
    }

    #[test]
    fn nothing_redeems_before_anything_is_issued() {
        assert!(!Pairing::new().redeem("anything"));
    }

    #[test]
    fn an_expired_code_fails() {
        let pairing = Pairing::with_ttl(Duration::ZERO);
        let (code, expires) = pairing.issue().unwrap();
        assert!(expires <= SystemTime::now());
        assert!(!pairing.redeem(&code));
    }

    #[test]
    fn a_code_expires_in_real_time() {
        let pairing = Pairing::with_ttl(Duration::from_millis(20));
        let (code, _) = pairing.issue().unwrap();
        std::thread::sleep(Duration::from_millis(60));
        assert!(!pairing.redeem(&code));
    }

    #[test]
    fn issuing_again_invalidates_the_previous_code() {
        let pairing = Pairing::new();
        let (first, _) = pairing.issue().unwrap();
        let (second, _) = pairing.issue().unwrap();
        assert_ne!(first, second);
        assert!(!pairing.redeem(&first));
        assert!(pairing.redeem(&second));
    }

    #[test]
    fn a_code_is_long_enough_and_url_safe() {
        let (code, _) = Pairing::new().issue().unwrap();
        assert_eq!(code.len(), 22);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn offer_uri_has_the_exact_shape() {
        let hosts = [
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)),
            IpAddr::V6("2001:db8::1".parse::<Ipv6Addr>().unwrap()),
        ];
        let uri = offer_uri("abc_DEF-123", "ab12", 47820, &hosts, "Pixel 8 Pro");
        assert_eq!(
            uri,
            "yaca://pair?v=1&code=abc_DEF-123&fp=ab12&port=47820&name=Pixel%208%20Pro\
             &host=192.168.1.20&host=2001%3Adb8%3A%3A1"
        );
    }

    #[test]
    fn offer_uri_without_hosts_has_no_host_parameter() {
        let uri = offer_uri("c", "f", 1, &[], "n");
        assert_eq!(uri, "yaca://pair?v=1&code=c&fp=f&port=1&name=n");
    }

    #[test]
    fn offer_uri_encodes_non_ascii_names_as_utf8() {
        let uri = offer_uri("c", "f", 1, &[], "Ж");
        assert!(uri.ends_with("&name=%D0%96"), "{uri}");
    }

    #[test]
    fn offer_uri_keeps_the_host_order() {
        let hosts = [
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        ];
        let uri = offer_uri("c", "f", 1, &hosts, "n");
        assert!(uri.ends_with("&host=10.0.0.2&host=10.0.0.1"), "{uri}");
    }

    #[test]
    fn pairing_offer_renders_an_svg_and_carries_the_expiry() {
        let expires = SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_000_000);
        let offer = PairingOffer::new("yaca://pair?v=1&code=c".to_string(), expires).unwrap();
        assert!(offer.qr_svg.contains("<svg"));
        assert_eq!(offer.expires_at, 1_700_000_000_000);
        assert_eq!(offer.uri, "yaca://pair?v=1&code=c");
    }

    #[test]
    fn qr_text_draws_blocks_for_a_terminal() {
        let text = qr_text("yaca://pair?v=1&code=c").unwrap();
        assert!(text.lines().count() > 10);
        assert!(text.chars().any(|c| c == '█' || c == '▀' || c == '▄'));
    }
}
