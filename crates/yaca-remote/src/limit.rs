//! Failed handshakes per address: the brake on guessing pairing codes and device
//! tokens.
//!
//! A code is 128 bits and a token 256, so a guesser cannot win on the numbers
//! alone. The brake is for scripts that run at full speed against the port and
//! for a phone that keeps retrying a stale token. It counts per address rather
//! than per device, because a guesser does not know a device id to aim at.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How far back a failure still counts.
pub(crate) const WINDOW: Duration = Duration::from_secs(60);
/// Failures within [`WINDOW`] that turn an address away. Five leaves room for a
/// phone whose stored token went stale and was paired again by mistake.
pub(crate) const MAX_FAILURES: usize = 5;

#[derive(Default)]
pub(crate) struct Limiter {
    failures: Mutex<HashMap<IpAddr, Vec<Instant>>>,
}

impl Limiter {
    /// Whether `addr` has used up its failures. Asked before anything is read from
    /// the socket, so a blocked address costs one refusal and no parsing.
    pub(crate) fn blocked(&self, addr: IpAddr, now: Instant) -> bool {
        let mut failures = self.lock();
        prune(&mut failures, now);
        failures
            .get(&addr)
            .is_some_and(|recent| recent.len() >= MAX_FAILURES)
    }

    pub(crate) fn record(&self, addr: IpAddr, now: Instant) {
        let mut failures = self.lock();
        prune(&mut failures, now);
        failures.entry(addr).or_default().push(now);
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<IpAddr, Vec<Instant>>> {
        // The map is only ever pushed to and pruned, so a panic while holding the
        // lock cannot leave it in a state that misreads an address.
        self.failures.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Drops failures older than the window and addresses with none left, so the map
/// holds only what still counts.
fn prune(failures: &mut HashMap<IpAddr, Vec<Instant>>, now: Instant) {
    failures.retain(|_, recent| {
        recent.retain(|at| now.saturating_duration_since(*at) < WINDOW);
        !recent.is_empty()
    });
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn addr() -> IpAddr {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    }

    #[test]
    fn blocked_at_the_max_failure_and_not_before() {
        let limiter = Limiter::default();
        let now = Instant::now();
        for _ in 0..MAX_FAILURES - 1 {
            limiter.record(addr(), now);
        }
        assert!(!limiter.blocked(addr(), now));
        limiter.record(addr(), now);
        assert!(limiter.blocked(addr(), now));
    }

    #[test]
    fn other_addresses_are_not_affected() {
        let limiter = Limiter::default();
        let now = Instant::now();
        for _ in 0..MAX_FAILURES {
            limiter.record(addr(), now);
        }
        assert!(!limiter.blocked(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), now));
    }

    #[test]
    fn failures_age_out_of_the_window() {
        let limiter = Limiter::default();
        let start = Instant::now();
        for _ in 0..MAX_FAILURES {
            limiter.record(addr(), start);
        }
        assert!(limiter.blocked(addr(), start));
        assert!(!limiter.blocked(addr(), start + WINDOW));
    }
}
