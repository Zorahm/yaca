//! The addresses the phone may reach the host at, for the pairing QR.
//!
//! The QR lists every address the host has, and the phone tries them in order,
//! so the list is filtered to the ones a phone on the same network could use at
//! all. Loopback is this machine only; unspecified and link-local addresses are
//! not routes to anywhere. They are dropped here rather than offered and failed.

use std::net::IpAddr;

/// Every address on this machine that a phone could plausibly reach, IPv4
/// first. An interface listing that fails is logged and yields nothing: the
/// pairing still works from an address typed in by hand, and an error here must
/// not stop the window from opening.
pub fn lan_addresses() -> Vec<IpAddr> {
    match if_addrs::get_if_addrs() {
        Ok(interfaces) => reachable(interfaces.into_iter().map(|i| i.ip())),
        Err(e) => {
            tracing::warn!(error = %e, "could not list network interfaces");
            Vec::new()
        }
    }
}

/// The filter behind [`lan_addresses`], without the interface lookup so it can
/// be tested with fixed input. Order is kept within each family, and duplicates
/// are dropped at their first occurrence.
fn reachable(addrs: impl IntoIterator<Item = IpAddr>) -> Vec<IpAddr> {
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    for addr in addrs {
        if !usable(addr) {
            continue;
        }
        let family = match addr {
            IpAddr::V4(_) => &mut v4,
            IpAddr::V6(_) => &mut v6,
        };
        if !family.contains(&addr) {
            family.push(addr);
        }
    }
    v4.extend(v6);
    v4
}

fn usable(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(a) => !(a.is_loopback() || a.is_unspecified() || a.is_link_local()),
        IpAddr::V6(a) => !(a.is_loopback() || a.is_unspecified() || a.is_unicast_link_local()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    fn v6(text: &str) -> IpAddr {
        IpAddr::V6(text.parse::<Ipv6Addr>().unwrap())
    }

    #[test]
    fn drops_loopback_unspecified_and_link_local() {
        let input = [
            v4(127, 0, 0, 1),
            v4(0, 0, 0, 0),
            v4(169, 254, 10, 3),
            v6("::1"),
            v6("::"),
            v6("fe80::1234"),
            v6("febf::1"),
        ];
        assert!(reachable(input).is_empty());
    }

    #[test]
    fn keeps_ordinary_lan_addresses_with_ipv4_first() {
        let input = [
            v6("2001:db8::5"),
            v4(192, 168, 1, 20),
            v6("fd00::7"),
            v4(10, 0, 0, 3),
        ];
        assert_eq!(
            reachable(input),
            vec![
                v4(192, 168, 1, 20),
                v4(10, 0, 0, 3),
                v6("2001:db8::5"),
                v6("fd00::7")
            ]
        );
    }

    #[test]
    fn keeps_the_order_given_within_a_family() {
        let input = [v4(10, 0, 0, 9), v4(10, 0, 0, 2), v4(10, 0, 0, 7)];
        assert_eq!(
            reachable(input),
            vec![v4(10, 0, 0, 9), v4(10, 0, 0, 2), v4(10, 0, 0, 7)]
        );
    }

    #[test]
    fn removes_duplicates_at_their_first_place() {
        let input = [
            v4(10, 0, 0, 2),
            v4(10, 0, 0, 1),
            v4(10, 0, 0, 2),
            v6("2001:db8::1"),
            v6("2001:db8::1"),
        ];
        assert_eq!(
            reachable(input),
            vec![v4(10, 0, 0, 2), v4(10, 0, 0, 1), v6("2001:db8::1")]
        );
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(reachable(Vec::new()).is_empty());
    }

    #[test]
    fn lan_addresses_never_contains_an_unreachable_address() {
        for addr in lan_addresses() {
            assert!(usable(addr), "{addr} should have been filtered out");
        }
    }
}
