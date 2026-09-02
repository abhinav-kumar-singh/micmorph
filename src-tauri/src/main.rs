// main.rs — MicMorph Tauri application entry point

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;

use commands::AppState;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

fn main() {
    // Initialize logging (visible in terminal during dev)
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("MicMorph starting up...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // Register global app state (audio engine)
        .manage(AppState::new())
        // Register all IPC commands callable from JS
        .invoke_handler(tauri::generate_handler![
            commands::get_input_devices,
            commands::get_default_input_device,
            commands::check_blackhole,
            commands::start_processing,
            commands::stop_processing,
            commands::get_audio_level,
            commands::set_pitch,
            commands::toggle_preview,
            commands::get_engine_state,
            commands::get_current_pitch,
            commands::get_usage_status,
            commands::simulate_use_minutes,
            commands::install_virtual_driver,
        ])
        .setup(|app| {
            // Load state from disk
            let state = app.state::<AppState>();
            if let Err(e) = state.load_from_disk(app.handle()) {
                log::error!("Failed to load config: {}", e);
            }

            // Config loaded cleanly

            // ── System Tray ──────────────────────────────────────────────
            let header = MenuItem::with_id(app, "header", "MicMorph Voice Control", false, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let subtle = MenuItem::with_id(app, "preset_subtle", "🎙️ Subtle (-1 st)", true, None::<&str>)?;
            let medium = MenuItem::with_id(app, "preset_medium", "🎙️ Medium (-3 st)", true, None::<&str>)?;
            let deep = MenuItem::with_id(app, "preset_deep", "🎙️ Deep (-5 st)", true, None::<&str>)?;
            let deepest = MenuItem::with_id(app, "preset_deepest", "🎙️ Deepest (-8 st)", true, None::<&str>)?;
            let natural = MenuItem::with_id(app, "preset_natural", "⚪ Natural / Bypass (0 st)", true, None::<&str>)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let toggle = MenuItem::with_id(app, "toggle_engine", "⚡ Toggle Active / Idle", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "🪟 Open MicMorph Window", true, None::<&str>)?;
            let sep3 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "❌ Quit MicMorph", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &header, &sep1,
                    &subtle, &medium, &deep, &deepest, &natural,
                    &sep2,
                    &toggle, &show,
                    &sep3,
                    &quit,
                ],
            )?;

            let tray_image = Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                .expect("Failed to load tray icon");

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_image)
                .icon_as_template(false)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .tooltip("MicMorph — Voice Pitch Tuning")
                .on_menu_event(|app, event| {
                    let id = event.id.as_ref();
                    match id {
                        "preset_subtle" | "preset_medium" | "preset_deep" | "preset_deepest" | "preset_natural" => {
                            let pitch: f32 = match id {
                                "preset_subtle" => -1.0,
                                "preset_medium" => -3.0,
                                "preset_deep" => -5.0,
                                "preset_deepest" => -8.0,
                                "preset_natural" => 0.0,
                                _ => -3.0,
                            };
                            log::info!("Tray menu selected pitch: {} st", pitch);
                            if let Some(state) = app.try_state::<AppState>() {
                                if let Err(e) = state.set_pitch(pitch) {
                                    log::error!("Failed to set pitch from tray: {}", e);
                                }
                            }
                            let _ = app.emit("pitch-changed-from-tray", pitch);
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

            log::info!("MicMorph tray icon created with voice preset menu");
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close button → hide to tray instead of quitting
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("Error running MicMorph");
}
