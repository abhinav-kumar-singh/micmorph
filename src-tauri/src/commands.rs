// commands.rs — Tauri IPC commands exposed to the frontend UI

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, Manager, State};
use serde::{Deserialize, Serialize};

use crate::audio::{
    devices::{default_input_device_name, is_blackhole_available, list_input_devices, AudioDevice},
    engine::{AudioEngine, EngineState},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UsageConfig {
    pub last_used_date: String,
    pub seconds_used_today: u32,
    pub is_pro: bool,
}

pub struct AppState {
    pub engine: Mutex<AudioEngine>,
    pub active_pitch: Mutex<f32>,
    /// RMS * 1000 stored as integer — written by audio thread, read by get_audio_level command
    pub audio_level: Arc<AtomicU32>,

    // Usage tracking fields
    pub seconds_used_today: Arc<Mutex<u32>>,
    pub last_used_date: Arc<Mutex<String>>,
    pub is_pro: Arc<Mutex<bool>>,
    pub bypass_active: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            engine: Mutex::new(AudioEngine::new()),
            active_pitch: Mutex::new(-3.0),
            audio_level: Arc::new(AtomicU32::new(0)),
            seconds_used_today: Arc::new(Mutex::new(0)),
            last_used_date: Arc::new(Mutex::new(String::new())),
            is_pro: Arc::new(Mutex::new(true)),
            bypass_active: Arc::new(AtomicBool::new(false)),
        }
    }

    fn get_config_path(&self, app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
        let mut path = app_handle.path().app_config_dir().map_err(|e| e.to_string())?;
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        path.push("usage_config.json");
        Ok(path)
    }

    pub fn load_from_disk(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let path = self.get_config_path(app_handle)?;
        let current_date = chrono::Local::now().format("%Y-%m-%d").to_string();

        if !path.exists() {
            *self.last_used_date.lock().unwrap() = current_date;
            *self.seconds_used_today.lock().unwrap() = 0;
            *self.is_pro.lock().unwrap() = true;
            self.bypass_active.store(false, Ordering::SeqCst);
            self.save_to_disk(app_handle)?;
            return Ok(());
        }

        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let config: UsageConfig = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        
        let mut last_date = self.last_used_date.lock().unwrap();
        let mut used_seconds = self.seconds_used_today.lock().unwrap();
        let mut pro_status = self.is_pro.lock().unwrap();

        *pro_status = true; // Unlimited access for initial release
        *last_date = current_date.clone();
        *used_seconds = config.seconds_used_today;
        self.bypass_active.store(false, Ordering::SeqCst);
        
        drop(last_date);
        drop(used_seconds);
        drop(pro_status);
        self.save_to_disk(app_handle)?;

        Ok(())
    }

    pub fn save_to_disk(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let path = self.get_config_path(app_handle)?;
        let config = UsageConfig {
            last_used_date: self.last_used_date.lock().unwrap().clone(),
            seconds_used_today: *self.seconds_used_today.lock().unwrap(),
            is_pro: *self.is_pro.lock().unwrap(),
        };
        let pretty = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
        fs::write(path, pretty).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn start_engine(&self, input_device: &str, pitch_semitones: f32, app_handle: &tauri::AppHandle) -> Result<(), String> {
        let mut engine = self.engine.lock().map_err(|e| e.to_string())?;
        if engine.is_running() {
            *self.active_pitch.lock().map_err(|e| e.to_string())? = pitch_semitones;
            engine.set_pitch(pitch_semitones);
            return Ok(());
        }

        // Refresh today's usage from disk (to handle dates reset)
        let _ = self.load_from_disk(app_handle);

        // Apply bypass check
        if !*self.is_pro.lock().unwrap() && *self.seconds_used_today.lock().unwrap() >= 3600 {
            self.bypass_active.store(true, Ordering::SeqCst);
        } else {
            self.bypass_active.store(false, Ordering::SeqCst);
        }

        *self.active_pitch.lock().map_err(|e| e.to_string())? = pitch_semitones;
        engine.set_app_handle(app_handle.clone());
        engine.set_audio_level(Arc::clone(&self.audio_level));
        engine.set_bypass_active(Arc::clone(&self.bypass_active));

        engine.start(input_device, pitch_semitones)?;

        // Spawn the background time tracker thread
        let app_handle_clone = app_handle.clone();
        let seconds_used_today = Arc::clone(&self.seconds_used_today);
        let is_pro = Arc::clone(&self.is_pro);
        let bypass_active = Arc::clone(&state_bypass_active(self));
        let stop_flag = Arc::clone(&engine.stop_flag);

        std::thread::spawn(move || {
            let mut save_counter = 0;
            while !stop_flag.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }

                let is_pro_val = *is_pro.lock().unwrap();
                if !is_pro_val {
                    let limit_reached = {
                        let mut used = seconds_used_today.lock().unwrap();
                        *used = used.saturating_add(1);
                        *used >= 3600
                    };

                    if limit_reached {
                        if !bypass_active.load(Ordering::SeqCst) {
                            bypass_active.store(true, Ordering::SeqCst);
                            let _ = app_handle_clone.emit("free-limit-reached", ());
                        }
                    }

                    save_counter += 1;
                    if save_counter >= 60 {
                        save_counter = 0;
                        if let Some(s) = app_handle_clone.try_state::<AppState>() {
                            let _ = s.save_to_disk(&app_handle_clone);
                        }
                    }
                }
            }

            // Final save on stop
            if let Some(s) = app_handle_clone.try_state::<AppState>() {
                let _ = s.save_to_disk(&app_handle_clone);
            }
        });

        Ok(())
    }

    pub fn stop_engine(&self) -> Result<(), String> {
        let mut engine = self.engine.lock().map_err(|e| e.to_string())?;
        engine.stop();
        self.audio_level.store(0, Ordering::Relaxed);
        Ok(())
    }

    pub fn set_pitch(&self, semitones: f32) -> Result<(), String> {
        let clamped = semitones.clamp(-12.0, 12.0);
        *self.active_pitch.lock().map_err(|e| e.to_string())? = clamped;
        self.engine.lock().map_err(|e| e.to_string())?.set_pitch(clamped);
        Ok(())
    }
}

// Helper to extract arc pointer from self for the thread
fn state_bypass_active(state: &AppState) -> Arc<AtomicBool> {
    Arc::clone(&state.bypass_active)
}

#[tauri::command]
pub fn get_input_devices(_state: State<AppState>) -> Vec<AudioDevice> {
    list_input_devices()
}

#[tauri::command]
pub fn get_default_input_device() -> Option<String> {
    default_input_device_name()
}

#[tauri::command]
pub fn check_blackhole() -> bool {
    is_blackhole_available()
}

#[tauri::command]
pub fn start_processing(
    input_device: String,
    pitch_semitones: f32,
    state: State<AppState>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    state.start_engine(&input_device, pitch_semitones, &app_handle)?;
    Ok("Started".to_string())
}

#[tauri::command]
pub fn stop_processing(state: State<AppState>) -> Result<String, String> {
    state.stop_engine()?;
    Ok("Stopped".to_string())
}

/// Polled by frontend every 50ms to drive the waveform visualizer.
/// Returns RMS level 0.0–1.0.
#[tauri::command]
pub fn get_audio_level(state: State<AppState>) -> f32 {
    let raw = state.audio_level.load(Ordering::Relaxed);
    raw as f32 / 1000.0
}

#[tauri::command]
pub fn set_pitch(semitones: f32, state: State<AppState>) -> Result<(), String> {
    state.set_pitch(semitones)
}

#[tauri::command]
pub fn toggle_preview(enabled: bool, state: State<AppState>) -> Result<(), String> {
    let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
    engine.set_preview_enabled(enabled)?;
    Ok(())
}

#[tauri::command]
pub fn get_engine_state(state: State<AppState>) -> String {
    match state.engine.lock() {
        Ok(engine) => match &engine.state {
            EngineState::Running => "running".to_string(),
            EngineState::Stopped => "stopped".to_string(),
            EngineState::Error(e) => format!("error:{}", e),
        },
        Err(_) => "error:lock_failed".to_string(),
    }
}

#[tauri::command]
pub fn get_current_pitch(state: State<AppState>) -> f32 {
    state.active_pitch.lock().map(|p| *p).unwrap_or(-3.0)
}

#[derive(Debug, serde::Serialize)]
pub struct UsageStatus {
    pub seconds_used_today: u32,
    pub limit_seconds: u32,
    pub is_pro: bool,
    pub bypass_active: bool,
}

#[tauri::command]
pub fn get_usage_status(_state: State<AppState>) -> UsageStatus {
    UsageStatus {
        seconds_used_today: 0,
        limit_seconds: u32::MAX,
        is_pro: true,
        bypass_active: false,
    }
}

#[tauri::command]
pub fn simulate_use_minutes(minutes: i32, state: State<AppState>, app_handle: tauri::AppHandle) -> Result<(), String> {
    let mut used = state.seconds_used_today.lock().unwrap();
    if minutes >= 0 {
        *used = (*used).saturating_add((minutes as u32) * 60);
    } else {
        let sub_seconds = (minutes.abs() as u32) * 60;
        *used = (*used).saturating_sub(sub_seconds);
    }
    
    if *used >= 3600 {
        state.bypass_active.store(true, Ordering::SeqCst);
        let _ = app_handle.emit("free-limit-reached", ());
    } else {
        state.bypass_active.store(false, Ordering::SeqCst);
    }
    drop(used);
    state.save_to_disk(&app_handle)?;
    Ok(())
}

#[tauri::command]
pub async fn install_virtual_driver() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        // 1-Click driver installer for Windows
        // Downloads VB-Audio Cable zip to %TEMP% if not present, extracts, runs setup with admin elevation,
        // and renames the audio endpoint to "MicMorph" in Windows Registry
        let script = r#"
            $tempDir = [System.IO.Path]::GetTempPath()
            $zipPath = Join-Path $tempDir "VBCABLE_Driver_Pack43.zip"
            $extractPath = Join-Path $tempDir "VBCABLE_Driver"
            
            if (-not (Test-Path $zipPath)) {
                Invoke-WebRequest -Uri "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack43.zip" -OutFile $zipPath -UseBasicParsing
            }
            if (-not (Test-Path $extractPath)) {
                Expand-Archive -Path $zipPath -DestinationPath $extractPath -Force
            }
            
            $setupExe = Join-Path $extractPath "VBCABLE_Setup_x64.exe"
            if (-not (Test-Path $setupExe)) {
                $setupExe = Join-Path $extractPath "VBCABLE_Setup.exe"
            }
            
            Start-Process -FilePath $setupExe -ArgumentList "-i", "-h" -Verb RunAs -Wait

            # Rename VB-Audio Cable capture/render devices to "MicMorph"
            $renderPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render"
            $capturePath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture"

            foreach ($base in @($renderPath, $capturePath)) {
                if (Test-Path $base) {
                    Get-ChildItem $base | ForEach-Object {
                        $props = Join-Path $_.PSPath "Properties"
                        if (Test-Path $props) {
                            $p = Get-ItemProperty -Path $props -ErrorAction SilentlyContinue
                            $desc = $p."{a45c254e-df1c-4efd-8020-67d146a850e0},2"
                            if ($desc -like "*VB-Audio*" -or $desc -like "*CABLE*") {
                                Set-ItemProperty -Path $props -Name "{b3f8fa53-0004-438e-9003-51a46e139bfc},6" -Value "MicMorph" -ErrorAction SilentlyContinue
                            }
                        }
                    }
                }
            }
        "#;

        let output = std::process::Command::new("powershell")
            .args(&["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
            .output()
            .map_err(|e| format!("Failed to execute PowerShell driver installer: {}", e))?;

        if output.status.success() {
            Ok("Virtual audio driver installed successfully as MicMorph".to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(format!("Driver installation failed: {}", err))
        }
    }

    #[cfg(target_os = "macos")]
    {
        // 1-Click driver installer for macOS
        // Runs osascript with administrator privileges to install BlackHole or triggers brew
        let script = r#"
            do shell script "/usr/local/bin/brew install blackhole-2ch || /opt/homebrew/bin/brew install blackhole-2ch || true; killall coreaudiod 2>/dev/null || true" with administrator privileges
        "#;
        
        let output = std::process::Command::new("osascript")
            .args(&["-e", script])
            .output()
            .map_err(|e| format!("Failed to execute macOS installer: {}", e))?;

        if output.status.success() {
            Ok("BlackHole driver installed and CoreAudio reloaded".to_string())
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Err(format!("macOS driver installation error: {}", err))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Err("Unsupported operating system".to_string())
    }
}
