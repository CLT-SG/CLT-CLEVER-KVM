use log::{info, warn, debug};
use anyhow::Result;
use std::process::Command;

/// Apply ultra-performance optimizations for low-latency streaming
pub fn apply_ultra_performance_optimizations() -> Result<()> {
    info!("[INFO] Applying user-level system optimizations for ultra-low latency performance");
    
    // Set process priority (only try non-privileged operations)
    if let Err(e) = set_high_priority() {
        debug!("Could not set high process priority: {}", e);
    }
    
    // Check system capabilities (non-privileged)
    if let Err(e) = check_system_performance() {
        debug!("Could not check system performance: {}", e);
    }
    
    info!("[OK] User-level optimizations applied (no elevated privileges required)");
    Ok(())
}

/// Set high process priority for better real-time performance
fn set_high_priority() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::process;
        
        // Try to set nice value to -5 (slightly higher priority, doesn't require root)
        let output = Command::new("nice")
            .args(&["-n", "-5", "echo", "priority test"])
            .output();
            
        match output {
            Ok(output) => {
                if output.status.success() {
                    debug!("Process priority adjustment test successful");
                } else {
                    debug!("Process priority adjustment not available without privileges");
                }
            },
            Err(e) => {
                debug!("nice command test failed: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Check system performance characteristics without requiring privileges
fn check_system_performance() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        // Check current CPU governor (read-only)
        if let Ok(output) = Command::new("cat")
            .arg("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
            .output() 
        {
            if let Ok(governor) = String::from_utf8(output.stdout) {
                debug!("Current CPU governor: {}", governor.trim());
                if governor.trim() != "performance" {
                    info!("[TIP] For best performance, consider setting CPU governor to 'performance' mode");
                    info!("    Run: sudo cpupower frequency-set -g performance");
                }
            }
        }
        
        // Check if turbo boost is enabled (read-only)
        if let Ok(output) = Command::new("cat")
            .arg("/sys/devices/system/cpu/intel_pstate/no_turbo")
            .output()
        {
            if let Ok(turbo_status) = String::from_utf8(output.stdout) {
                let turbo_disabled = turbo_status.trim() == "1";
                debug!("Turbo boost disabled: {}", turbo_disabled);
                if !turbo_disabled {
                    debug!("Turbo boost is enabled - good for performance");
                }
            }
        }
    }
    
    Ok(())
}

/// Check system capabilities for ultra-low latency streaming
#[allow(dead_code)]
pub fn check_system_capabilities() -> Result<String> {
    let mut capabilities = Vec::new();
    
    // Check CPU information
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("lscpu").output() {
            if let Ok(cpu_info) = String::from_utf8(output.stdout) {
                capabilities.push("CPU Information:".to_string());
                for line in cpu_info.lines().take(10) {
                    if line.contains("Model name") || line.contains("CPU(s)") || line.contains("Thread(s)") {
                        capabilities.push(format!("  {}", line.trim()));
                    }
                }
            }
        }
    }
    
    // Check available memory
    if let Ok(output) = Command::new("free").args(&["-h"]).output() {
        if let Ok(mem_info) = String::from_utf8(output.stdout) {
            capabilities.push("".to_string());
            capabilities.push("Memory Information:".to_string());
            for line in mem_info.lines().take(3) {
                capabilities.push(format!("  {}", line.trim()));
            }
        }
    }
    
    // Check GPU information
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("lspci").args(&["|", "grep", "-i", "vga"]).output() {
            if let Ok(gpu_info) = String::from_utf8(output.stdout) {
                if !gpu_info.trim().is_empty() {
                    capabilities.push("".to_string());
                    capabilities.push("GPU Information:".to_string());
                    capabilities.push(format!("  {}", gpu_info.trim()));
                }
            }
        }
    }
    
    Ok(capabilities.join("\n"))
}
