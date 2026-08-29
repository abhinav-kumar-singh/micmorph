// audio/mod.rs — Audio module root
pub mod devices;
pub mod engine;
pub mod processor;

#[cfg(target_os = "macos")]
pub mod mac_usage;

#[cfg(target_os = "windows")]
pub mod win_usage;

pub fn is_micmorph_device_active() -> bool {
    #[cfg(target_os = "macos")]
    return mac_usage::is_micmorph_device_active();

    #[cfg(target_os = "windows")]
    return win_usage::is_micmorph_device_active();

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return false;
}
