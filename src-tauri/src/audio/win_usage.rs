// audio/win_usage.rs — Windows WASAPI audio device activity detection

#![cfg(target_os = "windows")]

/// Checks if any application is currently recording/capturing from the virtual microphone (VB-CABLE / MicMorph).
pub fn is_micmorph_device_active() -> bool {
    // Default safe fallback on Windows — will be connected to WASAPI IAudioSessionManager2
    false
}
