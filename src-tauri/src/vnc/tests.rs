//! Unit tests for VNC modules
//! 
//! Tests for VNC server, input handling, audio, and registration

#[cfg(test)]
mod vnc_tests {
    use crate::vnc::*;

    #[test]
    fn test_vnc_server_config_default() {
        let config = VncServerConfig::default();
        assert_eq!(config.port, 5900);
        assert_eq!(config.monitor_id, 0);
        assert_eq!(config.enable_audio, true);
        assert_eq!(config.audio_port, Some(5901));
        assert_eq!(config.max_clients, 10);
        assert_eq!(config.password, None);
    }

    #[test]
    fn test_vnc_server_config_custom() {
        let config = VncServerConfig {
            port: 5901,
            monitor_id: 1,
            enable_audio: false,
            audio_port: None,
            max_clients: 5,
            password: Some("test123".to_string()),
        };
        
        assert_eq!(config.port, 5901);
        assert_eq!(config.monitor_id, 1);
        assert_eq!(config.enable_audio, false);
        assert_eq!(config.audio_port, None);
        assert_eq!(config.max_clients, 5);
        assert_eq!(config.password, Some("test123".to_string()));
    }

    #[test]
    fn test_vnc_client_creation() {
        let client = VncClient {
            id: 1,
            address: "192.168.1.100:5900".to_string(),
            connected_at: std::time::Instant::now(),
        };
        
        assert_eq!(client.id, 1);
        assert_eq!(client.address, "192.168.1.100:5900");
    }
}

#[cfg(test)]
mod audio_tests {
    use crate::vnc::audio::*;

    #[test]
    fn test_separate_audio_stream_creation() {
        let result = SeparateAudioStream::new(5901);
        assert!(result.is_ok());
        
        if let Ok(stream) = result {
            assert!(stream.get_stream_url().contains("5901"));
            assert!(stream.get_stream_url().contains("rtsp://"));
            assert!(stream.get_stream_url().ends_with("/audio"));
        }
    }

    #[test]
    fn test_separate_audio_stream_running_state() {
        let result = SeparateAudioStream::new(5901);
        assert!(result.is_ok());
        
        if let Ok(stream) = result {
            assert!(!stream.is_running());
        }
    }

    #[test]
    fn test_rfb_audio_extension_creation() {
        let result = RfbAudioExtension::new(48000, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rfb_audio_extension_disabled_by_default() {
        let audio_ext = RfbAudioExtension::new(48000, 2).unwrap();
        let samples = vec![0.5f32, -0.5f32, 0.25f32, -0.25f32];
        
        // Should return empty when disabled
        let packet = audio_ext.encode_audio_packet(&samples);
        assert_eq!(packet.len(), 0);
    }

    #[test]
    fn test_rfb_audio_extension_enabled() {
        let mut audio_ext = RfbAudioExtension::new(48000, 2).unwrap();
        audio_ext.enable();
        
        let samples = vec![0.5f32, -0.5f32];
        let packet = audio_ext.encode_audio_packet(&samples);
        
        // Should have header + audio data
        // Header: 4 (pseudo-encoding) + 4 (sample count) + 4 (sample rate) + 2 (channels) = 14 bytes
        // Audio data: 2 samples * 2 bytes per sample = 4 bytes
        // Total: 18 bytes
        assert_eq!(packet.len(), 18);
        
        // Check pseudo-encoding type (first 4 bytes)
        assert_eq!(&packet[0..4], &0x52706C41u32.to_be_bytes());
    }

    #[test]
    fn test_rfb_audio_packet_format() {
        let mut audio_ext = RfbAudioExtension::new(48000, 2).unwrap();
        audio_ext.enable();
        
        let samples = vec![1.0f32]; // Maximum positive sample
        let packet = audio_ext.encode_audio_packet(&samples);
        
        // Verify packet structure
        assert!(packet.len() >= 14);
        
        // Extract and verify pseudo-encoding
        let pseudo_encoding = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);
        assert_eq!(pseudo_encoding, 0x52706C41);
        
        // Extract and verify sample count
        let sample_count = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
        assert_eq!(sample_count, 1);
        
        // Extract and verify sample rate
        let sample_rate = u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]);
        assert_eq!(sample_rate, 48000);
        
        // Extract and verify channels
        let channels = u16::from_be_bytes([packet[12], packet[13]]);
        assert_eq!(channels, 2);
    }
}

#[cfg(test)]
mod input_tests {
    use crate::vnc::input::*;

    #[test]
    fn test_vnc_keyboard_ascii_characters() {
        // Test ASCII character conversion
        let result = handle_vnc_keyboard(0x61, true); // 'a'
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0x41, true); // 'A'
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0x30, true); // '0'
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_keyboard_special_keys() {
        // Test special keys
        let result = handle_vnc_keyboard(0xff0d, true); // Enter
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0xff08, true); // Backspace
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0xff1b, true); // Escape
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_keyboard_function_keys() {
        // Test function keys F1-F12
        for i in 0..12 {
            let key = 0xffbe + i;
            let result = handle_vnc_keyboard(key, true);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_vnc_keyboard_modifiers() {
        // Test modifier keys
        let result = handle_vnc_keyboard(0xffe1, true); // Shift
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0xffe3, true); // Control
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0xffe9, true); // Alt
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_keyboard_arrow_keys() {
        // Test arrow keys
        let result = handle_vnc_keyboard(0xff51, true); // Left
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0xff52, true); // Up
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0xff53, true); // Right
        assert!(result.is_ok());
        
        let result = handle_vnc_keyboard(0xff54, true); // Down
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_mouse_movement() {
        // Test mouse movement
        let result = handle_vnc_mouse(0x00, 100, 100);
        assert!(result.is_ok());
        
        let result = handle_vnc_mouse(0x00, 200, 200);
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_mouse_buttons() {
        // Test mouse button clicks
        let result = handle_vnc_mouse(0x01, 100, 100); // Left button
        assert!(result.is_ok());
        
        let result = handle_vnc_mouse(0x02, 100, 100); // Middle button
        assert!(result.is_ok());
        
        let result = handle_vnc_mouse(0x04, 100, 100); // Right button
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_mouse_scroll() {
        // Test mouse scroll
        let result = handle_vnc_mouse(0x08, 100, 100); // Scroll up
        assert!(result.is_ok());
        
        let result = handle_vnc_mouse(0x10, 100, 100); // Scroll down
        assert!(result.is_ok());
    }

    #[test]
    fn test_vnc_mouse_combined_buttons() {
        // Test multiple buttons pressed simultaneously
        let result = handle_vnc_mouse(0x03, 100, 100); // Left + Middle
        assert!(result.is_ok());
        
        let result = handle_vnc_mouse(0x05, 100, 100); // Left + Right
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod registration_tests {
    use crate::vnc::registration::*;

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
        
        if let Ok(json_str) = json {
            assert!(json_str.contains("123"));
            assert!(json_str.contains("vnc://192.168.1.100:5900"));
            assert!(json_str.contains("rtsp://192.168.1.100:5901/audio"));
            assert!(json_str.contains("test-host"));
        }
    }

    #[test]
    fn test_screencast_registration_deserialization() {
        let json_str = r#"{
            "id": 456,
            "vnc_url": "vnc://10.0.0.1:5900",
            "audio_url": "rtsp://10.0.0.1:5901/audio",
            "hostname": "workstation-1",
            "registered_at": "2024-01-01T12:00:00Z"
        }"#;
        
        let registration: Result<ScreencastRegistration, _> = serde_json::from_str(json_str);
        assert!(registration.is_ok());
        
        if let Ok(reg) = registration {
            assert_eq!(reg.id, 456);
            assert_eq!(reg.vnc_url, "vnc://10.0.0.1:5900");
            assert_eq!(reg.audio_url, Some("rtsp://10.0.0.1:5901/audio".to_string()));
            assert_eq!(reg.hostname, "workstation-1");
        }
    }

    #[test]
    fn test_screencast_registration_without_audio() {
        let registration = ScreencastRegistration {
            id: 789,
            vnc_url: "vnc://192.168.1.200:5900".to_string(),
            audio_url: None,
            hostname: "test-host-2".to_string(),
            registered_at: "2024-01-02T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&registration);
        assert!(json.is_ok());
        
        if let Ok(json_str) = json {
            assert!(json_str.contains("789"));
            assert!(json_str.contains("vnc://192.168.1.200:5900"));
            assert!(json_str.contains("null") || json_str.contains("audio_url"));
        }
    }
}
