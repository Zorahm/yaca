//! The server a host runs: the listening socket, the accept loop, and the few
//! operations the host's window or CLI needs: pairing, the device list, revoke, stop.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;

use crate::addresses::lan_addresses;
use crate::connection;
use crate::devices::{DeviceSummary, Devices};
use crate::error::Error;
use crate::host::Host;
use crate::pairing::{offer_uri, Pairing, PairingOffer};
use crate::protocol::DeviceId;
use crate::shared::Shared;
use crate::tls::Identity;

/// How long the accept loop waits after a failed accept. A full descriptor table
/// fails every accept, and retrying at once would spin.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);

/// Where and how the server listens.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// The remote directory, `settings::remote_dir(config_dir)`: the certificate,
    /// the devices and the settings live here.
    pub dir: PathBuf,
    /// The port to listen on. 0 picks any free port, which the tests use.
    pub port: u16,
    /// The address to listen on. The desktop and the CLI pass `0.0.0.0`, so a phone
    /// on the same network can reach the host.
    pub bind: IpAddr,
}

/// A running server. Dropping it stops it.
pub struct RemoteServer {
    shared: Arc<Shared>,
    local_addr: SocketAddr,
}

impl RemoteServer {
    /// Loads (or creates) the identity and the devices, binds the port and starts
    /// accepting. Must run inside a Tokio runtime.
    pub async fn start(options: ServerOptions, host: Arc<dyn Host>) -> Result<RemoteServer, Error> {
        let identity = Identity::load_or_create(&options.dir)?;
        let tls = identity.server_config()?;
        let listener = TcpListener::bind(SocketAddr::new(options.bind, options.port)).await?;
        let local_addr = listener.local_addr()?;
        let shared = Arc::new(Shared::new(
            host,
            Devices::load(&options.dir),
            Pairing::new(),
            tls,
            identity.fingerprint().to_string(),
        ));
        tokio::spawn(accept_loop(listener, Arc::clone(&shared)));
        Ok(Self { shared, local_addr })
    }

    /// The address the server listens on, with the port the OS chose if one was asked for.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// The SHA-256 of the server's certificate, which the pairing code carries.
    pub fn fingerprint(&self) -> &str {
        &self.shared.fingerprint
    }

    /// Issues a fresh pairing code, replacing the previous one, and builds the offer
    /// that carries it.
    pub fn offer_pairing(&self) -> Result<PairingOffer, Error> {
        let (code, expires) = self.shared.pairing.issue()?;
        let uri = offer_uri(
            &code,
            &self.shared.fingerprint,
            self.local_addr.port(),
            &lan_addresses(),
            &self.shared.host.name(),
        );
        PairingOffer::new(uri, expires)
    }

    pub fn devices(&self) -> Vec<DeviceSummary> {
        self.shared.devices.list()
    }

    /// Unpairs a device and closes its open connections at once. `false` when there
    /// was no such device.
    pub fn revoke(&self, id: &DeviceId) -> Result<bool, Error> {
        let removed = self.shared.devices.revoke(id)?;
        // After the removal: a connection registers only while its device is listed,
        // so once the list lacks the device, none can be missed here.
        self.shared.cancel_device(id);
        Ok(removed)
    }

    /// Stops accepting and closes every connection.
    pub fn stop(&self) {
        self.shared.cancel.cancel();
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn accept_loop(listener: TcpListener, shared: Arc<Shared>) {
    loop {
        tokio::select! {
            () = shared.cancel.cancelled() => break,
            accepted = listener.accept() => match accepted {
                Ok((socket, peer)) => {
                    tokio::spawn(connection::serve(socket, peer, Arc::clone(&shared)));
                }
                Err(e) => {
                    tracing::warn!(error = %e, "could not accept a connection");
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            }
        }
    }
}
