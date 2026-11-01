//! What every connection of one server shares: the host, the devices, the pairing
//! code, the failed-handshake counts, and the live connections that revoking a
//! device has to reach.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use rustls::ServerConfig;
use tokio_util::sync::CancellationToken;

use crate::devices::Devices;
use crate::host::Host;
use crate::limit::Limiter;
use crate::pairing::Pairing;
use crate::protocol::DeviceId;

pub(crate) struct Shared {
    pub(crate) host: Arc<dyn Host>,
    pub(crate) devices: Devices,
    pub(crate) pairing: Pairing,
    pub(crate) tls: Arc<ServerConfig>,
    pub(crate) fingerprint: String,
    /// Every connection is a child of this token, so stopping the server stops them
    /// all at once.
    pub(crate) cancel: CancellationToken,
    pub(crate) limiter: Limiter,
    live: Mutex<LiveTable>,
}

#[derive(Default)]
struct LiveTable {
    next: u64,
    connections: HashMap<u64, Live>,
}

struct Live {
    device: DeviceId,
    token: CancellationToken,
}

impl Shared {
    pub(crate) fn new(
        host: Arc<dyn Host>,
        devices: Devices,
        pairing: Pairing,
        tls: Arc<ServerConfig>,
        fingerprint: String,
    ) -> Self {
        Self {
            host,
            devices,
            pairing,
            tls,
            fingerprint,
            cancel: CancellationToken::new(),
            limiter: Limiter::default(),
            live: Mutex::new(LiveTable::default()),
        }
    }

    /// Registers a connection of `device` so that revoking the device can reach it.
    /// `None` when the device is not paired (any more).
    ///
    /// The check and the insert happen under the table's lock, and the revoke
    /// cancels under the same lock after removing the device. So whichever comes
    /// first, a connection is either refused here or found by the revoke: none can
    /// slip between the two.
    pub(crate) fn register(self: &Arc<Self>, device: &DeviceId, token: CancellationToken) -> Option<LiveGuard> {
        let mut table = self.table();
        if !self.devices.contains(device) {
            return None;
        }
        let key = table.next;
        table.next += 1;
        table.connections.insert(
            key,
            Live {
                device: device.clone(),
                token,
            },
        );
        Some(LiveGuard {
            shared: Arc::clone(self),
            key,
        })
    }

    /// Closes every live connection of `device`. The device must already be gone
    /// from [`Devices`], or a connection that registers in between would live on.
    pub(crate) fn cancel_device(&self, device: &DeviceId) {
        for live in self.table().connections.values() {
            if live.device == *device {
                live.token.cancel();
            }
        }
    }

    fn table(&self) -> MutexGuard<'_, LiveTable> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A registered connection. Dropping it unregisters the connection.
pub(crate) struct LiveGuard {
    shared: Arc<Shared>,
    key: u64,
}

impl Drop for LiveGuard {
    fn drop(&mut self) {
        self.shared.table().connections.remove(&self.key);
    }
}
