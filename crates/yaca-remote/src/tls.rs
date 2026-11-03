//! The server's identity: a self-signed certificate that the phone pins.
//!
//! No certificate authority is involved. The pairing code carries the SHA-256
//! fingerprint of this certificate, so the phone trusts exactly this one and
//! nothing that a CA might issue for the same name. The name in the certificate
//! is therefore a label for humans, not something a client checks.
//!
//! The certificate is created once and kept: regenerating it would change the
//! fingerprint, and every phone paired against the old one would refuse the host.

use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;

use crate::error::Error;
use crate::files::write_atomic;
use crate::secret::sha256_hex;

const CERT_FILE: &str = "cert.pem";
const KEY_FILE: &str = "key.pem";
/// Only a label: no client checks the name, because trust comes from the pinned
/// fingerprint.
const SUBJECT: &str = "yaca.local";

pub struct Identity {
    cert: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
    fingerprint: String,
}

impl Identity {
    /// Reads `cert.pem` and `key.pem` from `dir`, or creates both when either is
    /// missing. A lone file is not an identity: pairing it with a freshly minted
    /// partner would give a certificate and a key that were never made together,
    /// so both are regenerated instead.
    ///
    /// Two files that exist but do not parse are an error, not a regeneration:
    /// a fresh certificate would silently break every paired phone, and a human
    /// should decide that, not a fallback.
    pub fn load_or_create(dir: &Path) -> Result<Self, Error> {
        let cert_path = dir.join(CERT_FILE);
        let key_path = dir.join(KEY_FILE);
        if cert_path.exists() && key_path.exists() {
            let cert_pem = std::fs::read(&cert_path)?;
            let key_pem = std::fs::read(&key_path)?;
            return Self::from_pem(&cert_pem, &key_pem).map_err(|e| {
                Error::Certificate(format!(
                    "{} and {}: {e}",
                    cert_path.display(),
                    key_path.display()
                ))
            });
        }

        let generated = rcgen::generate_simple_self_signed(vec![SUBJECT.to_string()])
            .map_err(|e| Error::Certificate(e.to_string()))?;
        let cert_pem = generated.cert.pem();
        let key_pem = generated.key_pair.serialize_pem();
        write_atomic(&cert_path, cert_pem.as_bytes())?;
        write_atomic(&key_path, key_pem.as_bytes())?;
        // Parsed back from the PEMs just written, so a fresh identity and a
        // loaded one take the same path and cannot differ.
        Self::from_pem(cert_pem.as_bytes(), key_pem.as_bytes())
            .map_err(|e| Error::Certificate(e.to_string()))
    }

    fn from_pem(cert_pem: &[u8], key_pem: &[u8]) -> Result<Self, rustls::pki_types::pem::Error> {
        let cert = CertificateDer::from_pem_slice(cert_pem)?;
        let key = PrivateKeyDer::from_pem_slice(key_pem)?;
        let fingerprint = fingerprint(cert.as_ref());
        Ok(Self {
            cert,
            key,
            fingerprint,
        })
    }

    /// The fingerprint the pairing code carries, computed once at load.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The certificate as DER, for tests and for anything that pins it.
    pub fn cert_der(&self) -> &[u8] {
        self.cert.as_ref()
    }

    /// The TLS configuration for the listener. The ring provider is pinned
    /// explicitly: rustls would otherwise pick a provider from the features
    /// linked into the whole binary, and aws-lc-rs needs cmake and nasm to build
    /// on Windows.
    pub fn server_config(&self) -> Result<Arc<ServerConfig>, Error> {
        let config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()?
                .with_no_client_auth()
                .with_single_cert(vec![self.cert.clone()], self.key.clone_key())?;
        Ok(Arc::new(config))
    }
}

/// SHA-256 of a certificate's DER, lowercase hex. Shown in the pairing code so
/// the phone can compare it with what the server presents.
pub fn fingerprint(der: &[u8]) -> String {
    sha256_hex(der)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::TestDir;

    #[test]
    fn fingerprint_is_sha256_hex_of_the_der() {
        assert_eq!(
            fingerprint(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(fingerprint(b"abc").len(), 64);
    }

    #[test]
    fn create_then_load_keeps_the_same_identity() {
        let dir = TestDir::new();
        let created = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(created.fingerprint().len(), 64);
        assert!(created
            .fingerprint()
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(created.fingerprint(), fingerprint(created.cert_der()));

        let loaded = Identity::load_or_create(dir.path()).unwrap();
        assert_eq!(loaded.fingerprint(), created.fingerprint());
        assert_eq!(loaded.cert_der(), created.cert_der());
    }

    #[test]
    fn server_config_builds_from_a_fresh_identity() {
        let dir = TestDir::new();
        let identity = Identity::load_or_create(dir.path()).unwrap();
        identity.server_config().unwrap();
    }

    #[test]
    fn missing_key_regenerates_both_files() {
        let dir = TestDir::new();
        let first = Identity::load_or_create(dir.path()).unwrap();
        std::fs::remove_file(dir.path().join(KEY_FILE)).unwrap();

        let second = Identity::load_or_create(dir.path()).unwrap();
        assert_ne!(second.fingerprint(), first.fingerprint());
        assert!(dir.path().join(KEY_FILE).exists());
        // And the new pair is stable from now on.
        assert_eq!(
            Identity::load_or_create(dir.path()).unwrap().fingerprint(),
            second.fingerprint()
        );
    }

    #[test]
    fn missing_certificate_regenerates_both_files() {
        let dir = TestDir::new();
        let first = Identity::load_or_create(dir.path()).unwrap();
        std::fs::remove_file(dir.path().join(CERT_FILE)).unwrap();
        let second = Identity::load_or_create(dir.path()).unwrap();
        assert_ne!(second.fingerprint(), first.fingerprint());
    }

    #[test]
    fn unparsable_pair_is_an_error_not_a_regeneration() {
        let dir = TestDir::new();
        Identity::load_or_create(dir.path()).unwrap();
        let cert_before = std::fs::read(dir.path().join(CERT_FILE)).unwrap();
        std::fs::write(dir.path().join(KEY_FILE), b"not a key").unwrap();

        assert!(matches!(
            Identity::load_or_create(dir.path()),
            Err(Error::Certificate(_))
        ));
        // Nothing was rewritten: the certificate on disk is the one it was.
        assert_eq!(
            std::fs::read(dir.path().join(CERT_FILE)).unwrap(),
            cert_before
        );
    }
}
