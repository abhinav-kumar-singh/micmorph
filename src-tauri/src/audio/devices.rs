// audio/devices.rs — Audio device enumeration for Mac (CoreAudio via cpal)

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

/// Represents an audio device (input or output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub is_blackhole: bool,
}

/// Returns a list of all available audio INPUT devices on this Mac.
/// Flags BlackHole 2ch if detected.
pub fn list_input_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();

    match host.input_devices() {
        Ok(devices) => devices
            .filter_map(|d| {
                d.name().ok().map(|name| {
                    let is_blackhole = name.to_lowercase().contains("blackhole");
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

/// Returns a list of all available audio OUTPUT devices on this Mac.
/// Used to find BlackHole 2ch as the output/virtual mic target.
pub fn list_output_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();

    match host.output_devices() {
        Ok(devices) => devices
            .filter_map(|d| {
                d.name().ok().map(|name| {
                    let is_blackhole = name.to_lowercase().contains("blackhole");
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

/// Checks if BlackHole 2ch is installed and detectable as an output device.
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
