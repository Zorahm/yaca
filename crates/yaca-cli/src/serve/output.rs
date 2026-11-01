//! What `yaca serve` prints: the start-up report, a pairing code, the device list.
//!
//! English, one line where one line will do. The phone needs only the QR code;
//! the address and the fingerprint are for the person reading the terminal.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use yaca_remote::devices::DeviceSummary;
use yaca_remote::pairing::{qr_text, PairingOffer};

use super::commands::HELP;
use super::host::shown;

pub fn startup(addr: SocketAddr, fingerprint: &str, projects: &[PathBuf], lan: &[IpAddr]) {
    println!("Yaca serve listens on {addr} (WebSocket over TLS)");
    println!("certificate fingerprint: {fingerprint}");
    if lan.is_empty() {
        println!("no network address found: a phone can reach the host only by an address typed in by hand");
    } else {
        let reachable: Vec<String> = lan
            .iter()
            .map(|ip| SocketAddr::new(*ip, addr.port()).to_string())
            .collect();
        println!("phone can reach it at: {}", reachable.join(", "));
    }
    println!("projects:");
    for project in projects {
        println!("  {}", shown(project));
    }
    println!("{HELP}");
}

/// The pairing code as the phone scans it: the QR code in text, then the URI
/// under it for a phone whose camera will not read the screen, then how long the
/// code is good for.
pub fn pairing(offer: &PairingOffer) -> Result<()> {
    println!("{}", qr_text(&offer.uri)?);
    println!("{}", offer.uri);
    println!("pairing code expires in {} s, and works once", remaining_secs(offer.expires_at));
    Ok(())
}

pub fn devices(list: &[DeviceSummary]) {
    if list.is_empty() {
        println!("no devices paired");
    }
    for device in list {
        println!("{}  {}  last seen {}", device.id, device.name, ago(device.last_seen));
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn remaining_secs(expires_at_ms: u64) -> u64 {
    expires_at_ms.saturating_sub(now_ms()) / 1000
}

/// "never", or how long ago the device was last seen, in the largest unit that fits.
fn ago(last_seen_ms: Option<u64>) -> String {
    let Some(at) = last_seen_ms else {
        return "never".to_string();
    };
    let secs = now_ms().saturating_sub(at) / 1000;
    match secs {
        0..=59 => format!("{secs} s ago"),
        60..=3599 => format!("{} min ago", secs / 60),
        _ => format!("{} h ago", secs / 3600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_never_seen_says_so() {
        assert_eq!(ago(None), "never");
    }

    #[test]
    fn a_device_seen_just_now_is_in_seconds() {
        assert_eq!(ago(Some(now_ms())), "0 s ago");
    }

    #[test]
    fn a_code_already_past_its_time_has_no_seconds_left() {
        assert_eq!(remaining_secs(0), 0);
    }
}
