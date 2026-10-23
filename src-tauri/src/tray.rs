// tray.rs — Menu bar / system tray: voice presets that follow the window's voice style.

use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Wry,
};

use crate::commands::AppState;

/// Tray preset ids, in menu order. Values per voice style match the window presets in main.js.
const PRESET_IDS: [&str; 4] = ["preset_subtle", "preset_medium", "preset_deep", "preset_deepest"];
const MASCULINE: [(&str, f32); 4] = [("Subtle", -1.0), ("Medium", -3.0), ("Deep", -5.0), ("Deepest", -8.0)];
const FEMININE: [(&str, f32); 4] = [("Subtle", 1.5), ("Medium", 3.5), ("High", 5.5), ("Highest", 8.0)];

pub struct TrayState {
    feminine: Mutex<bool>,
    presets: Vec<CheckMenuItem<Wry>>,
    natural: CheckMenuItem<Wry>,
}

fn presets_for(feminine: bool) -> &'static [(&'static str, f32); 4] {
    if feminine { &FEMININE } else { &MASCULINE }
}

/// "Subtle (−1)", "High (+5.5)".
fn label(name: &str, value: f32) -> String {
    let sign = if value < 0.0 { "−" } else { "+" };
    format!("{} ({}{})", name, sign, value.abs())
}

/// Pitch a tray item sends, given the current voice style.
fn preset_pitch(id: &str, feminine: bool) -> Option<f32> {
    if id == "preset_natural" {
        return Some(0.0);
    }
    PRESET_IDS.iter().position(|p| *p == id).map(|i| presets_for(feminine)[i].1)
}

/// Index of the preset matching `pitch`; None for a custom slider value or Natural.
fn matching_preset(pitch: f32, feminine: bool) -> Option<usize> {
    presets_for(feminine).iter().position(|(_, v)| (pitch - v).abs() < 0.01)
}

/// Relabels the presets for the voice style and ticks the one matching `pitch`.
fn refresh(tray: &TrayState, pitch: f32) {
    let feminine = tray.feminine.lock().map(|f| *f).unwrap_or(false);
    let current = matching_preset(pitch, feminine);
    for (i, (item, (name, value))) in tray.presets.iter().zip(presets_for(feminine)).enumerate() {
        let _ = item.set_text(label(name, *value));
        let _ = item.set_checked(current == Some(i));
    }
    let _ = tray.natural.set_checked(pitch.abs() < 0.01);
}

fn current_pitch(app: &AppHandle) -> f32 {
    app.try_state::<AppState>()
        .and_then(|s| s.active_pitch.lock().ok().map(|p| *p))
        .unwrap_or(-3.0)
}

/// Keeps the tick in step with the pitch, however it was changed.
pub fn sync(app: &AppHandle, pitch: f32) {
    if let Some(tray) = app.try_state::<TrayState>() {
        refresh(&tray, pitch);
    }
}

/// Called when the window switches between masculine and feminine.
pub fn set_feminine(app: &AppHandle, feminine: bool) {
    if let Some(tray) = app.try_state::<TrayState>() {
        if let Ok(mut f) = tray.feminine.lock() {
            *f = feminine;
        }
        refresh(&tray, current_pitch(app));
    }
}

pub fn build(app: &tauri::App) -> tauri::Result<()> {
    let header = MenuItem::with_id(app, "header", "Voice Presets", false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let presets = PRESET_IDS
        .iter()
        .zip(MASCULINE.iter())
        .map(|(id, (name, value))| CheckMenuItem::with_id(app, *id, label(name, *value), true, false, None::<&str>))
        .collect::<tauri::Result<Vec<_>>>()?;
    let natural = CheckMenuItem::with_id(app, "preset_natural", "Natural (bypass)", true, false, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let show = MenuItem::with_id(app, "show", "Open MicMorph", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit MicMorph", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &header, &sep1,
            &presets[0], &presets[1], &presets[2], &presets[3], &natural,
            &sep2,
            &show,
            &sep3,
            &quit,
        ],
    )?;

    // macOS: monochrome template icon that the system tints to match the menu bar.
    // Windows: the full-colour icon (template images don't apply in the tray).
    #[cfg(target_os = "macos")]
    let (tray_icon_bytes, tray_icon_is_template): (&[u8], bool) =
        (include_bytes!("../icons/tray-icon-template.png"), true);
    #[cfg(not(target_os = "macos"))]
    let (tray_icon_bytes, tray_icon_is_template): (&[u8], bool) =
        (include_bytes!("../icons/tray-icon.png"), false);

    let tray_image = Image::from_bytes(tray_icon_bytes)
        .expect("Failed to load tray icon");

    app.manage(TrayState { feminine: Mutex::new(false), presets, natural });
    sync(app.handle(), current_pitch(app.handle()));

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(tray_image)
        .icon_as_template(tray_icon_is_template)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("MicMorph — Voice Pitch Tuning")
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "preset_subtle" | "preset_medium" | "preset_deep" | "preset_deepest" | "preset_natural" => {
                    let feminine = app.try_state::<TrayState>()
                        .and_then(|t| t.feminine.lock().ok().map(|f| *f))
                        .unwrap_or(false);
                    let pitch: f32 = preset_pitch(id, feminine).unwrap_or(-3.0);
                    log::info!("Tray menu selected pitch: {} st", pitch);
                    if let Some(state) = app.try_state::<AppState>() {
                        if let Err(e) = state.set_pitch(pitch) {
                            log::error!("Failed to set pitch from tray: {}", e);
                        }
                    }
                    let _ = app.emit("pitch-changed-from-tray", pitch);
                    sync(app, pitch);
                }
                "toggle_engine" => {
                    if let Some(state) = app.try_state::<AppState>() {
                        let is_running = state.engine.lock().map(|e| e.is_running()).unwrap_or(false);
                        if is_running {
                            if let Err(e) = state.stop_engine() {
                                log::error!("Tray toggle stop error: {}", e);
                            } else {
                                let _ = app.emit("auto-stopped", ());
                            }
                        } else {
                            let input_device = if let Some(default_mic) = crate::audio::devices::default_input_device_name() {
                                if !crate::audio::devices::is_virtual_device_name(&default_mic) {
                                    default_mic
                                } else {
                                    let devices = crate::audio::devices::list_input_devices();
                                    devices.iter()
                                        .map(|d| d.name.clone())
                                        .find(|name| !crate::audio::devices::is_virtual_device_name(name))
                                        .unwrap_or_default()
                                }
                            } else {
                                String::new()
                            };
                            let pitch = state.active_pitch.lock().map(|p| *p).unwrap_or(-3.0);
                            if !input_device.is_empty() {
                                if let Err(e) = state.start_engine(&input_device, pitch, app) {
                                    log::error!("Tray toggle start error: {}", e);
                                } else {
                                    let _ = app.emit("auto-started", ());
                                }
                            }
                        }
                    }
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    log::info!("Quit requested from tray");
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_presets_follow_voice_style() {
        assert_eq!(preset_pitch("preset_deep", false), Some(-5.0));
        assert_eq!(preset_pitch("preset_deep", true), Some(5.5));
        assert_eq!(preset_pitch("preset_subtle", true), Some(1.5));
        assert_eq!(preset_pitch("preset_deepest", true), Some(8.0));
        assert_eq!(preset_pitch("preset_natural", true), Some(0.0));
        assert_eq!(preset_pitch("show", false), None);
    }

    #[test]
    fn tick_matches_presets_only() {
        assert_eq!(matching_preset(-3.0, false), Some(1));
        assert_eq!(matching_preset(3.5, true), Some(1));
        assert_eq!(matching_preset(-6.5, false), None); // custom slider value
        assert_eq!(matching_preset(-3.0, true), None);  // masculine value while feminine
        assert_eq!(matching_preset(0.0, false), None);  // Natural has its own tick
    }

    #[test]
    fn labels_show_signed_values() {
        assert_eq!(label("Subtle", -1.0), "Subtle (−1)");
        assert_eq!(label("High", 5.5), "High (+5.5)");
    }
}
