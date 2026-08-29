// audio/devices.rs — Audio device enumeration for Mac (CoreAudio via cpal)

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

/// Represents an audio device (input or output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_blackhole: bool,
}

pub fn is_virtual_device_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("blackhole")
        || lower.contains("cable input")
        || lower.contains("cable output")
        || lower.contains("vb-audio")
        || lower.contains("virtual audio")
        || lower.contains("micmorph")
}

/// Returns a list of all available audio INPUT devices on this system.
/// Flags virtual bridge devices (BlackHole on macOS, VB-CABLE on Windows).
pub fn list_input_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();

    match host.input_devices() {
        Ok(devices) => devices
            .filter_map(|d| {
                d.name().ok().map(|name| {
                    let is_blackhole = is_virtual_device_name(&name);
                    AudioDevice { name, is_blackhole }
                })
            })
            .collect(),
        Err(e) => {
            log::error!("Failed to enumerate input devices: {}", e);
            vec![]
        }
    }
}

/// Returns a list of all available audio OUTPUT devices on this system.
/// Used to find BlackHole (macOS) or VB-CABLE (Windows) as the virtual mic stream target.
pub fn list_output_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();

    match host.output_devices() {
        Ok(devices) => devices
            .filter_map(|d| {
                d.name().ok().map(|name| {
                    let is_blackhole = is_virtual_device_name(&name);
                    AudioDevice { name, is_blackhole }
                })
            })
            .collect(),
        Err(e) => {
            log::error!("Failed to enumerate output devices: {}", e);
            vec![]
        }
    }
}

/// Checks if a virtual audio cable (BlackHole on macOS or VB-CABLE on Windows) is available.
pub fn is_blackhole_available() -> bool {
    list_output_devices()
        .iter()
        .any(|d| d.is_blackhole)
}

/// Gets the default input device name (physical mic).
pub fn default_input_device_name() -> Option<String> {
    let host = cpal::default_host();
    host.default_input_device()
        .and_then(|d| d.name().ok())
}
