//! CLEVER Service Registration Module
//! 
//! Handles auto-registration with clever-service API
//! for video wall integration

use anyhow::Result;
use log::{info, warn, error};
use serde::{Deserialize, Serialize};

/// Screencast registration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreencastRegistration {
    pub id: u64,
    pub vnc_url: String,
    pub audio_url: Option<String>,
    pub hostname: String,
    pub registered_at: String,
}

/// Register VNC server with clever-service
pub async fn register_vnc_with_clever_service(
    clever_service_url: &str,
    vnc_port: u16,
    audio_port: Option<u16>,
    hostname: &str,
) -> Result<ScreencastRegistration> {
    info!("📡 Registering VNC server with clever-service at {}", clever_service_url);

    // Get local IP address
    let local_ip = match local_ip_address::local_ip() {
        Ok(ip) => ip.to_string(),
        Err(e) => {
            warn!("Failed to get local IP: {}, using localhost", e);
            "localhost".to_string()
        }
    };

    // Build VNC and audio URLs
    let vnc_url = format!("vnc://{}:{}", local_ip, vnc_port);
    let audio_url = audio_port.map(|port| format!("rtsp://{}:{}/audio", local_ip, port));

    // Build registration payload
    #[derive(Serialize)]
    struct RegistrationPayload {
        vnc_url: String,
        audio_url: Option<String>,
        hostname: String,
        r#type: String,
    }

    let payload = RegistrationPayload {
        vnc_url: vnc_url.clone(),
        audio_url: audio_url.clone(),
        hostname: hostname.to_string(),
        r#type: "vnc-kvm".to_string(),
    };

    // Send registration request
    let client = reqwest::Client::new();
    let registration_url = format!("{}/api/screencasts/register", clever_service_url);
    
    info!("📤 Sending registration to {}", registration_url);
    
    match client.post(&registration_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<RegistrationResponse>().await {
                    Ok(resp) => {
                        let registration = ScreencastRegistration {
                            id: resp.id,
                            vnc_url: vnc_url.clone(),
                            audio_url: audio_url.clone(),
                            hostname: hostname.to_string(),
                            registered_at: chrono::Utc::now().to_rfc3339(),
                        };
                        
                        info!("✅ Successfully registered with clever-service (ID: {})", resp.id);
                        info!("   VNC URL: {}", vnc_url);
                        if let Some(ref audio) = audio_url {
                            info!("   Audio URL: {}", audio);
                        }
                        
                        Ok(registration)
                    }
                    Err(e) => {
                        error!("❌ Failed to parse registration response: {}", e);
                        Err(anyhow::anyhow!("Failed to parse registration response: {}", e))
                    }
                }
            } else {
                error!("❌ Registration failed with status: {}", response.status());
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                Err(anyhow::anyhow!("Registration failed: {}", error_text))
            }
        }
        Err(e) => {
            error!("❌ Failed to send registration request: {}", e);
            Err(anyhow::anyhow!("Failed to send registration request: {}", e))
        }
    }
}

/// Unregister from clever-service
pub async fn unregister_from_clever_service(
    clever_service_url: &str,
    registration_id: u64,
) -> Result<()> {
    info!("📡 Unregistering from clever-service (ID: {})", registration_id);

    let client = reqwest::Client::new();
    let unregister_url = format!("{}/api/screencasts/{}", clever_service_url, registration_id);
    
    match client.delete(&unregister_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                info!("✅ Successfully unregistered from clever-service");
                Ok(())
            } else {
                warn!("⚠️  Unregistration returned status: {}", response.status());
                Ok(()) // Still return Ok since this is cleanup
            }
        }
        Err(e) => {
            warn!("⚠️  Failed to unregister: {}", e);
            Ok(()) // Still return Ok since this is cleanup
        }
    }
}

/// Response from clever-service registration
#[derive(Debug, Deserialize)]
struct RegistrationResponse {
    id: u64,
    #[allow(dead_code)]
    status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screencast_registration_serialization() {
        let registration = ScreencastRegistration {
            id: 123,
            vnc_url: "vnc://192.168.1.100:5900".to_string(),
            audio_url: Some("rtsp://192.168.1.100:5901/audio".to_string()),
            hostname: "test-host".to_string(),
            registered_at: "2024-01-01T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&registration);
        assert!(json.is_ok());
    }
}
