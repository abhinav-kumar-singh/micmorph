// main.rs — MicMorph Tauri application entry point

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod tray;

use commands::AppState;
use tauri::Manager;

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
            commands::set_voice_mode,
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
            tray::build(app)?;

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
