//! TLS Certificate Management Module
//! 
//! Provides self-signed certificate generation and management for WebSocket TLS connections.
//! Certificates are persisted to disk and reused across application restarts.

use anyhow::{Context, Result};
use log::{info, warn};
use rcgen::{Certificate, CertificateParams, DistinguishedName, DnType, KeyPair};
use std::fs;
use std::path::{Path, PathBuf};
use time::{Duration, OffsetDateTime};

/// TLS certificate manager
pub struct TlsCertificateManager {
    cert_dir: PathBuf,
}

impl TlsCertificateManager {
    /// Create a new certificate manager
    pub fn new() -> Result<Self> {
        // Get application data directory
        let cert_dir = Self::get_cert_directory()?;
        
        // Ensure directory exists
        fs::create_dir_all(&cert_dir)
            .context("Failed to create certificate directory")?;
        
        Ok(Self { cert_dir })
    }
    
    /// Get the certificate directory path
    fn get_cert_directory() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Failed to get config directory")?;
        
        Ok(config_dir.join("clever-kvm").join("certs"))
    }
    
    /// Get paths for certificate and key files
    fn get_cert_paths(&self) -> (PathBuf, PathBuf) {
        let cert_path = self.cert_dir.join("server.crt");
        let key_path = self.cert_dir.join("server.key");
        (cert_path, key_path)
    }
    
    /// Check if certificates exist
    pub fn certificates_exist(&self) -> bool {
        let (cert_path, key_path) = self.get_cert_paths();
        cert_path.exists() && key_path.exists()
    }
    
    /// Generate a new self-signed certificate
    pub fn generate_self_signed_certificate(&self) -> Result<(String, String)> {
        info!("🔐 Generating self-signed TLS certificate...");
        
        // Get hostname for certificate CN
        let hostname = gethostname::gethostname()
            .into_string()
            .unwrap_or_else(|_| "localhost".to_string());
        
        // Create certificate parameters
        let mut params = CertificateParams::default();
        
        // Set distinguished name
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, &hostname);
        dn.push(DnType::OrganizationName, "CLT-CLEVER-KVM");
        dn.push(DnType::CountryName, "SG");
        params.distinguished_name = dn;
        
        // Set validity period (365 days)
        params.not_before = OffsetDateTime::now_utc();
        params.not_after = OffsetDateTime::now_utc() + Duration::days(365);
        
        // Add subject alternative names
        params.subject_alt_names = vec![
            rcgen::SanType::DnsName(hostname.clone()),
            rcgen::SanType::DnsName("localhost".to_string()),
            rcgen::SanType::IpAddress(std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
            rcgen::SanType::IpAddress(std::net::IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1))),
        ];
        
        // Generate key pair and certificate
        let key_pair = KeyPair::generate(&rcgen::PKCS_ECDSA_P256_SHA256)
            .context("Failed to generate key pair")?;
        params.key_pair = Some(key_pair);
        
        let cert = Certificate::from_params(params)
            .context("Failed to create certificate")?;
        
        // Serialize to PEM format
        let cert_pem = cert.serialize_pem()
            .context("Failed to serialize certificate")?;
        let key_pem = cert.serialize_private_key_pem();
        
        info!("✅ Generated TLS certificate for hostname: {}", hostname);
        
        Ok((cert_pem, key_pem))
    }
    
    /// Save certificate and key to disk
    pub fn save_certificate(&self, cert_pem: &str, key_pem: &str) -> Result<()> {
        let (cert_path, key_path) = self.get_cert_paths();
        
        info!("💾 Saving certificate to: {:?}", cert_path);
        fs::write(&cert_path, cert_pem)
            .context("Failed to write certificate file")?;
        
        fs::write(&key_path, key_pem)
            .context("Failed to write key file")?;
        
        // Set restrictive permissions on key file (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&key_path)?.permissions();
            perms.set_mode(0o600); // Read/write for owner only
            fs::set_permissions(&key_path, perms)?;
        }
        
        info!("✅ Certificate saved successfully");
        Ok(())
    }
    
    /// Load existing certificate and key from disk
    pub fn load_certificate(&self) -> Result<(String, String)> {
        let (cert_path, key_path) = self.get_cert_paths();
        
        info!("📂 Loading certificate from: {:?}", cert_path);
        
        let cert_pem = fs::read_to_string(&cert_path)
            .context("Failed to read certificate file")?;
        let key_pem = fs::read_to_string(&key_path)
            .context("Failed to read key file")?;
        
        info!("✅ Certificate loaded successfully");
        Ok((cert_pem, key_pem))
    }
    
    /// Get or create certificate (load existing or generate new)
    pub fn get_or_create_certificate(&self) -> Result<(String, String)> {
        if self.certificates_exist() {
            info!("📜 Using existing TLS certificate");
            self.load_certificate()
        } else {
            info!("🆕 No existing certificate found, generating new one");
            let (cert_pem, key_pem) = self.generate_self_signed_certificate()?;
            self.save_certificate(&cert_pem, &key_pem)?;
            Ok((cert_pem, key_pem))
        }
    }
    
    /// Get certificate directory path for user reference
    pub fn get_cert_dir_path(&self) -> &Path {
        &self.cert_dir
    }
    
    /// Delete existing certificates (for regeneration)
    pub fn delete_certificates(&self) -> Result<()> {
        let (cert_path, key_path) = self.get_cert_paths();
        
        if cert_path.exists() {
            fs::remove_file(&cert_path)
                .context("Failed to delete certificate file")?;
        }
        
        if key_path.exists() {
            fs::remove_file(&key_path)
                .context("Failed to delete key file")?;
        }
        
        info!("🗑️ Certificates deleted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_certificate_generation() {
        let manager = TlsCertificateManager::new().unwrap();
        let (cert, key) = manager.generate_self_signed_certificate().unwrap();
        
        assert!(cert.contains("BEGIN CERTIFICATE"));
        assert!(cert.contains("END CERTIFICATE"));
        assert!(key.contains("BEGIN PRIVATE KEY") || key.contains("BEGIN EC PRIVATE KEY"));
        assert!(key.contains("END PRIVATE KEY") || key.contains("END EC PRIVATE KEY"));
    }
}
