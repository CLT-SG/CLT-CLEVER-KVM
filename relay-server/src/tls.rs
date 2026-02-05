//! TLS Configuration and Self-Signed Certificate Generation
//!
//! This module handles TLS setup for HTTPS support, which is required
//! for WebCodecs API in modern browsers (secure context requirement).

use anyhow::{Context, Result};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

/// TLS configuration for the HTTPS server
pub struct TlsConfig {
    pub server_config: Arc<ServerConfig>,
    pub is_self_signed: bool,
}

/// Load or generate TLS certificates
///
/// If cert_path and key_path are provided and exist, load them.
/// Otherwise, generate self-signed certificates.
pub fn load_or_generate_tls(
    cert_path: Option<&str>,
    key_path: Option<&str>,
    hostname: &str,
) -> Result<TlsConfig> {
    // Try to load existing certificates
    if let (Some(cert_path), Some(key_path)) = (cert_path, key_path) {
        if Path::new(cert_path).exists() && Path::new(key_path).exists() {
            info!("📜 Loading TLS certificates from files");
            return load_certificates(cert_path, key_path).map(|config| TlsConfig {
                server_config: config,
                is_self_signed: false,
            });
        }
    }

    // Generate self-signed certificates
    info!("🔐 Generating self-signed TLS certificate for: {}", hostname);
    generate_self_signed_config(hostname)
}

/// Load certificates from PEM files
fn load_certificates(cert_path: &str, key_path: &str) -> Result<Arc<ServerConfig>> {
    let cert_file = File::open(cert_path).context("Failed to open certificate file")?;
    let key_file = File::open(key_path).context("Failed to open key file")?;

    let mut cert_reader = BufReader::new(cert_file);
    let mut key_reader = BufReader::new(key_file);

    let certs: Vec<CertificateDer<'static>> = certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificates")?;

    let key = private_key(&mut key_reader)
        .context("Failed to read private key")?
        .context("No private key found in file")?;

    let config = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .context("Failed to set protocol versions")?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to configure TLS")?;

    Ok(Arc::new(config))
}

/// Generate self-signed certificate for development/local use
fn generate_self_signed_config(hostname: &str) -> Result<TlsConfig> {
    // Create certificate parameters
    let mut params = CertificateParams::default();

    // Set distinguished name
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, hostname);
    dn.push(DnType::OrganizationName, "CLEVER KVM");
    dn.push(DnType::OrganizationalUnitName, "Relay Server");
    params.distinguished_name = dn;

    // Add Subject Alternative Names
    params.subject_alt_names = vec![
        SanType::DnsName(hostname.try_into()?),
        SanType::DnsName(format!("{}.local", hostname).try_into()?),
        SanType::DnsName("localhost".try_into()?),
        SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
        SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)),
    ];

    // Add local IP addresses to SANs
    if let Ok(addrs) = local_ip_addresses() {
        for addr in addrs {
            params.subject_alt_names.push(SanType::IpAddress(addr));
        }
    }

    // Set validity period (1 year)
    params.not_before = rcgen::date_time_ymd(2024, 1, 1);
    params.not_after = rcgen::date_time_ymd(2027, 12, 31);

    // Generate key pair (uses Ed25519 by default which is widely supported)
    let key_pair = KeyPair::generate()?;

    // Generate certificate
    let cert = params.self_signed(&key_pair)?;

    // Convert to rustls format
    let cert_der = CertificateDer::from(cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(key_pair.serialize_der())
        .map_err(|e| anyhow::anyhow!("Failed to convert private key: {:?}", e))?;

    // Optionally save to disk for reuse
    let data_dir = get_data_directory();
    if let Some(dir) = data_dir {
        let cert_path = dir.join("relay-cert.pem");
        let key_path = dir.join("relay-key.pem");

        if let Err(e) = save_certificates(&cert, &key_pair, &cert_path, &key_path) {
            warn!("Could not save certificates for reuse: {}", e);
        } else {
            info!("📁 Certificates saved to: {}", dir.display());
        }
    }

    let config = ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .context("Failed to set protocol versions")?
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .context("Failed to configure TLS with self-signed cert")?;

    Ok(TlsConfig {
        server_config: Arc::new(config),
        is_self_signed: true,
    })
}

/// Save certificates to PEM files for reuse
fn save_certificates(
    cert: &rcgen::Certificate,
    key_pair: &KeyPair,
    cert_path: &Path,
    key_path: &Path,
) -> Result<()> {
    // Create directory if needed
    if let Some(parent) = cert_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(cert_path, cert.pem())?;
    fs::write(key_path, key_pair.serialize_pem())?;

    Ok(())
}

/// Get data directory for storing certificates
fn get_data_directory() -> Option<std::path::PathBuf> {
    dirs::data_local_dir().map(|p| p.join("clever-relay"))
}

/// Get local IP addresses for certificate SANs
fn local_ip_addresses() -> Result<Vec<std::net::IpAddr>> {
    let mut addresses = Vec::new();

    // Try to get network interfaces
    for iface in get_if_addrs::get_if_addrs()? {
        if !iface.is_loopback() {
            addresses.push(iface.ip());
        }
    }

    Ok(addresses)
}
