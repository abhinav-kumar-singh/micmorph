// audio/mac_usage.rs — Native CoreAudio helper to detect if MicMorph virtual mic is active

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::ptr;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::base::TCFType;

type AudioObjectID = u32;
type OSStatus = i32;

#[repr(C)]
struct AudioObjectPropertyAddress {
    mSelector: u32,
    mScope: u32,
    mElement: u32,
}

const K_AUDIO_HARDWARE_PROPERTY_DEVICES: u32 = 0x64657623; // 'dev#'
const K_AUDIO_OBJECT_PROPERTY_SELECTOR_RUNNING: u32 = 0x72756e73; // 'runs' -> kAudioDevicePropertyDeviceIsRunningSomewhere
const K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = 0x676c6f62;    // 'glob'
const K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;             // Element 0
const K_AUDIO_OBJECT_SYSTEM_OBJECT: u32 = 1;
const K_AUDIO_DEVICE_PROPERTY_DEVICE_NAME: u32 = 0x6c6e616d;    // 'lnam' -> kAudioObjectPropertyName

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        outDataSize: *mut u32,
    ) -> OSStatus;

    fn AudioObjectGetPropertyData(
        inObjectID: AudioObjectID,
        inAddress: *const AudioObjectPropertyAddress,
        inQualifierDataSize: u32,
        inQualifierData: *const c_void,
        ioDataSize: *mut u32,
        outData: *mut c_void,
    ) -> OSStatus;
}

// Convert CFStringRef safely to Rust String
unsafe fn cfstring_to_string(cf_string_ptr: *const c_void) -> Option<String> {
    if cf_string_ptr.is_null() {
        return None;
    }
    // wrap_under_create_rule takes ownership (+1 retain count from CoreAudio) and will call CFRelease on drop
    let cf_str = CFString::wrap_under_create_rule(cf_string_ptr as CFStringRef);
    Some(cf_str.to_string())
}

// Enumerate all audio devices and find the ID for the device named "MicMorph"
fn find_micmorph_device_id() -> Option<AudioObjectID> {
    let address = AudioObjectPropertyAddress {
        mSelector: K_AUDIO_HARDWARE_PROPERTY_DEVICES,
        mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut size: u32 = 0;
    let system_obj = K_AUDIO_OBJECT_SYSTEM_OBJECT;
    unsafe {
        let status = AudioObjectGetPropertyDataSize(system_obj, &address, 0, ptr::null(), &mut size);
        if status != 0 {
            return None;
        }

        let count = (size as usize) / std::mem::size_of::<AudioObjectID>();
        if count == 0 {
            return None;
        }

        let mut device_ids: Vec<AudioObjectID> = vec![0; count];
        let status = AudioObjectGetPropertyData(
            system_obj,
            &address,
            0,
            ptr::null(),
            &mut size,
            device_ids.as_mut_ptr() as *mut c_void,
        );
        if status != 0 {
            return None;
        }

        for id in device_ids {
            // Skip device ID 0 (invalid)
            if id == 0 {
                continue;
            }

            let name_address = AudioObjectPropertyAddress {
                mSelector: K_AUDIO_DEVICE_PROPERTY_DEVICE_NAME,
                mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
                mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
            };

            let mut name_size: u32 = 0;
            let status = AudioObjectGetPropertyDataSize(id, &name_address, 0, ptr::null(), &mut name_size);
            if status != 0 || name_size == 0 {
                continue;
            }

            let mut cf_name: *const c_void = ptr::null();
            let status = AudioObjectGetPropertyData(
                id,
                &name_address,
                0,
                ptr::null(),
                &mut name_size,
                &mut cf_name as *mut *const c_void as *mut c_void,
            );

            if status == 0 && !cf_name.is_null() {
                if let Some(name) = cfstring_to_string(cf_name) {
                    let name_lower = name.to_lowercase();
                    if name_lower.contains("micmorph") || name_lower.contains("blackhole") {
                        return Some(id);
                    }
                }
            }
        }
    }
    None
}

// Check if any app is actively reading/streaming from the MicMorph virtual device
pub fn is_micmorph_device_active() -> bool {
    let device_id = match find_micmorph_device_id() {
        Some(id) => id,
        None => return false,
    };

    let address = AudioObjectPropertyAddress {
        mSelector: K_AUDIO_OBJECT_PROPERTY_SELECTOR_RUNNING,
        mScope: K_AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        mElement: K_AUDIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    };

    let mut is_running: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;

    unsafe {
        let status = AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            ptr::null(),
            &mut size,
            &mut is_running as *mut u32 as *mut c_void,
        );
        if status == 0 {
            return is_running == 1;
        }
    }

    false
}
