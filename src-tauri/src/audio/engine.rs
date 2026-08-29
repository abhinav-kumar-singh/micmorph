// audio/engine.rs — Core audio pipeline

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::collections::VecDeque;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, SampleRate, StreamConfig};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::processor::PitchProcessor;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EngineState {
    Stopped,
    Running,
    Error(String),
}

pub struct AudioEngine {
    pub state: EngineState,
    pitch_semitones: Arc<Mutex<f32>>,
    pub(crate) stop_flag: Arc<AtomicBool>,
    _input_stream: Option<cpal::Stream>,
    _output_stream: Option<cpal::Stream>,
    _app_handle: Option<AppHandle>,
    /// Shared with AppState — audio thread writes RMS*1000 here, JS polls via get_audio_level
    audio_level: Option<Arc<AtomicU32>>,

    // Real-time dynamic updates and preview voice fields
    active_processor: Option<Arc<Mutex<PitchProcessor>>>,
    sample_rate: u32,
    source_channels: u16,
    preview_stream: Option<cpal::Stream>,
    preview_enabled: Arc<AtomicBool>,
    preview_buffer: Arc<Mutex<VecDeque<f32>>>,
    bypass_buffer: Arc<Mutex<VecDeque<f32>>>,
    pub(crate) bypass_active: Option<Arc<AtomicBool>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            state: EngineState::Stopped,
            pitch_semitones: Arc::new(Mutex::new(-3.0)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            _input_stream: None,
            _output_stream: None,
            _app_handle: None,
            audio_level: None,
            active_processor: None,
            sample_rate: 0,
            source_channels: 0,
            preview_stream: None,
            preview_enabled: Arc::new(AtomicBool::new(false)),
            preview_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(8192))),
            bypass_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(8192))),
            bypass_active: None,
        }
    }

    pub fn set_bypass_active(&mut self, flag: Arc<AtomicBool>) {
        self.bypass_active = Some(flag);
    }

    pub fn set_app_handle(&mut self, handle: AppHandle) {
        self._app_handle = Some(handle);
    }

    pub fn set_audio_level(&mut self, level: Arc<AtomicU32>) {
        self.audio_level = Some(level);
    }

    pub fn start(&mut self, input_device_name: &str, pitch_semitones: f32) -> Result<(), String> {
        if self.state == EngineState::Running {
            return Err("Engine already running".to_string());
        }

        log::info!("Starting audio engine: input='{}', pitch={:.1}", input_device_name, pitch_semitones);

        let host = cpal::default_host();

        let input_device = host
            .input_devices()
            .map_err(|e| format!("Cannot enumerate input devices: {}", e))?
            .find(|d| d.name().map(|n| n == input_device_name).unwrap_or(false))
            .ok_or_else(|| format!("Input device '{}' not found", input_device_name))?;

        let output_device = host
            .output_devices()
            .map_err(|e| format!("Cannot enumerate output devices: {}", e))?
            .find(|d| d.name().map(|n| crate::audio::devices::is_virtual_device_name(&n)).unwrap_or(false))
            .ok_or_else(|| {
                #[cfg(target_os = "macos")]
                return "BlackHole 2ch not found. Please install it with: brew install blackhole-2ch".to_string();
                #[cfg(target_os = "windows")]
                return "VB-CABLE Virtual Audio Device not found. Please install VB-CABLE Driver.".to_string();
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                return "Virtual audio cable driver not found.".to_string();
            })?;

        log::info!("Output device: {}", output_device.name().unwrap_or_default());

        let input_config = input_device
            .default_input_config()
            .map_err(|e| format!("Cannot get input config: {}", e))?;

        let sample_rate = input_config.sample_rate().0;
        let channels = input_config.channels();

        log::info!("Audio config: {}Hz, {} channels, {:?}", sample_rate, channels, input_config.sample_format());

        let stream_config = StreamConfig {
            channels,
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Fixed(512),
        };

        let processor = Arc::new(Mutex::new(PitchProcessor::new(sample_rate, channels, pitch_semitones)));
        let processor_for_output = Arc::clone(&processor);

        // Store active properties
        self.sample_rate = sample_rate;
        self.source_channels = channels;
        self.active_processor = Some(processor.clone());
        self.preview_buffer.lock().clear();
        self.bypass_buffer.lock().clear();

        *self.pitch_semitones.lock() = pitch_semitones;
        self.stop_flag = Arc::new(AtomicBool::new(false));

        // Share audio_level with the input callback
        let audio_level = self.audio_level.clone();
        let stop_flag = self.stop_flag.clone();

        let input_stream = self.build_input_stream(
            &input_device,
            &stream_config,
            input_config.sample_format(),
            processor.clone(),
            self.bypass_buffer.clone(),
            audio_level,
            self.bypass_active.clone(),
            stop_flag.clone(),
        )?;

        let output_config = StreamConfig {
            channels: 2, // Always open loopback virtual device in stereo (2 channels)
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Fixed(512),
        };

        // Share preview states with output stream
        let preview_enabled = self.preview_enabled.clone();
        let preview_buffer = self.preview_buffer.clone();
        let bypass_buffer = self.bypass_buffer.clone();

        let output_stream = self.build_output_stream(
            &output_device,
            &output_config,
            processor_for_output,
            bypass_buffer,
            preview_enabled,
            preview_buffer,
            self.bypass_active.clone(),
            stop_flag.clone(),
        )?;

        input_stream.play().map_err(|e| format!("Failed to start input stream: {}", e))?;
        output_stream.play().map_err(|e| format!("Failed to start output stream: {}", e))?;

        self._input_stream = Some(input_stream);
        self._output_stream = Some(output_stream);
        self.state = EngineState::Running;

        log::info!("Audio engine started successfully");

        // Auto-start preview if enabled in UI config
        if self.preview_enabled.load(Ordering::Relaxed) {
            if let Err(e) = self.start_preview_stream() {
                log::error!("Failed to auto-start preview stream: {}", e);
            }
        }

        Ok(())
    }

    fn build_input_stream(
        &self,
        device: &Device,
        config: &StreamConfig,
        sample_format: SampleFormat,
        processor: Arc<Mutex<PitchProcessor>>,
        bypass_buffer: Arc<Mutex<VecDeque<f32>>>,
        audio_level: Option<Arc<AtomicU32>>,
        bypass_active: Option<Arc<AtomicBool>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<cpal::Stream, String> {
        let err_fn = |err| log::error!("Input stream error: {}", err);

        use std::sync::atomic::AtomicUsize;
        let call_count = Arc::new(AtomicUsize::new(0));
        let bypass_active_clone = bypass_active.clone();

        let stream = match sample_format {
            SampleFormat::F32 => {
                let processor = processor.clone();
                let audio_level_clone = audio_level.clone();
                let call_count = call_count.clone();
                let bypass_active_clone = bypass_active_clone.clone();
                let bypass_buf = bypass_buffer.clone();
                let stop_flag = stop_flag.clone();
                device.build_input_stream(
                    config,
                    move |data: &[f32], _| {
                        if stop_flag.load(Ordering::Relaxed) {
                            return;
                        }
                        let is_bypassed = bypass_active_clone.as_ref().map(|f| f.load(Ordering::Relaxed)).unwrap_or(false);
                        if is_bypassed {
                            let mut buf = bypass_buf.lock();
                            if buf.len() < 16384 {
                                buf.extend(data);
                            }
                        } else {
                            let mut proc = processor.lock();
                            proc.feed_samples(data);
                        }
                        
                        let sum_sq: f32 = data.iter().map(|&s| s * s).sum();
                        let rms = (sum_sq / data.len() as f32).sqrt();
                        let c = call_count.fetch_add(1, Ordering::Relaxed);
                        if c % 100 == 0 {
                            log::info!("Input callback F32 #{} - RMS: {:.6}", c, rms);
                        }

                        if let Some(ref level) = audio_level_clone {
                            let val = (rms * 1000.0).min(u32::MAX as f32) as u32;
                            level.store(val, Ordering::Relaxed);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let processor = processor.clone();
                let audio_level_clone = audio_level.clone();
                let call_count = call_count.clone();
                let bypass_active_clone = bypass_active_clone.clone();
                let bypass_buf = bypass_buffer.clone();
                let stop_flag = stop_flag.clone();
                device.build_input_stream(
                    config,
                    move |data: &[i16], _| {
                        if stop_flag.load(Ordering::Relaxed) {
                            return;
                        }
                        let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                        let is_bypassed = bypass_active_clone.as_ref().map(|f| f.load(Ordering::Relaxed)).unwrap_or(false);
                        if is_bypassed {
                            let mut buf = bypass_buf.lock();
                            if buf.len() < 16384 {
                                buf.extend(&f32_data);
                            }
                        } else {
                            let mut proc = processor.lock();
                            proc.feed_samples(&f32_data);
                        }

                        let sum_sq: f32 = f32_data.iter().map(|&s| s * s).sum();
                        let rms = (sum_sq / f32_data.len() as f32).sqrt();
                        let c = call_count.fetch_add(1, Ordering::Relaxed);
                        if c % 100 == 0 {
                            log::info!("Input callback I16 #{} - RMS: {:.6}", c, rms);
                        }

                        if let Some(ref level) = audio_level_clone {
                            let val = (rms * 1000.0).min(u32::MAX as f32) as u32;
                            level.store(val, Ordering::Relaxed);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let processor = processor.clone();
                let audio_level_clone = audio_level.clone();
                let call_count = call_count.clone();
                let bypass_active_clone = bypass_active_clone.clone();
                let bypass_buf = bypass_buffer.clone();
                let stop_flag = stop_flag.clone();
                device.build_input_stream(
                    config,
                    move |data: &[u16], _| {
                        if stop_flag.load(Ordering::Relaxed) {
                            return;
                        }
                        let f32_data: Vec<f32> = data.iter().map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0).collect();
                        let is_bypassed = bypass_active_clone.as_ref().map(|f| f.load(Ordering::Relaxed)).unwrap_or(false);
                        if is_bypassed {
                            let mut buf = bypass_buf.lock();
                            if buf.len() < 16384 {
                                buf.extend(&f32_data);
                            }
                        } else {
                            let mut proc = processor.lock();
                            proc.feed_samples(&f32_data);
                        }

                        let sum_sq: f32 = f32_data.iter().map(|&s| s * s).sum();
                        let rms = (sum_sq / f32_data.len() as f32).sqrt();
                        let c = call_count.fetch_add(1, Ordering::Relaxed);
                        if c % 100 == 0 {
                            log::info!("Input callback U16 #{} - RMS: {:.6}", c, rms);
                        }

                        if let Some(ref level) = audio_level_clone {
                            let val = (rms * 1000.0).min(u32::MAX as f32) as u32;
                            level.store(val, Ordering::Relaxed);
                        }
                    },
                    err_fn,
                    None,
                )
            }
            _ => return Err(format!("Unsupported sample format: {:?}", sample_format)),
        }
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

        Ok(stream)
    }

    fn build_output_stream(
        &self,
        device: &Device,
        config: &StreamConfig,
        processor: Arc<Mutex<PitchProcessor>>,
        bypass_buffer: Arc<Mutex<VecDeque<f32>>>,
        preview_enabled: Arc<AtomicBool>,
        preview_buffer: Arc<Mutex<VecDeque<f32>>>,
        bypass_active: Option<Arc<AtomicBool>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<cpal::Stream, String> {
        let err_fn = |err| log::error!("Output stream error: {}", err);
        let bypass_active_clone = bypass_active.clone();
        let source_channels = self.source_channels;

        let stream = device.build_output_stream(
            config,
            move |output: &mut [f32], _| {
                if stop_flag.load(Ordering::Relaxed) {
                    output.fill(0.0);
                    return;
                }

                // Gain boost factor: 2.0x (+6.0dB) to compensate for lack of virtual mic hardware pre-amp
                let gain = 2.0f32;
                let is_bypassed = bypass_active_clone.as_ref().map(|f| f.load(Ordering::Relaxed)).unwrap_or(false);
                let frames_needed = output.len() / 2; // Output is always stereo (2 channels)
                
                if is_bypassed {
                    // Bypass mode: read raw samples from persistent queue
                    let mut buf = bypass_buffer.lock();
                    if source_channels == 1 {
                        let frames_available = buf.len();
                        let frames_to_write = frames_available.min(frames_needed);
                        for i in 0..frames_to_write {
                            let sample = (buf.pop_front().unwrap_or(0.0) * gain).clamp(-1.0, 1.0);
                            output[2 * i] = sample;     // Left
                            output[2 * i + 1] = sample; // Right
                        }
                        if frames_to_write < frames_needed {
                            output[frames_to_write * 2..].fill(0.0);
                        }
                    } else {
                        let samples_available = buf.len();
                        let samples_to_write = samples_available.min(output.len());
                        for i in 0..samples_to_write {
                            output[i] = (buf.pop_front().unwrap_or(0.0) * gain).clamp(-1.0, 1.0);
                        }
                        if samples_to_write < output.len() {
                            output[samples_to_write..].fill(0.0);
                        }
                    }
                } else {
                    // Normal mode: request EXACTLY frames_needed so extra processed samples are never discarded
                    let mut proc = processor.lock();
                    let processed = proc.receive_samples(frames_needed);
                    
                    if processed.is_empty() {
                        output.fill(0.0);
                    } else {
                        if source_channels == 1 {
                            // Upmix Mono to Stereo
                            let frames_to_write = processed.len().min(frames_needed);
                            for i in 0..frames_to_write {
                                let sample = (processed[i] * gain).clamp(-1.0, 1.0);
                                output[2 * i] = sample;     // Left
                                output[2 * i + 1] = sample; // Right
                            }
                            if frames_to_write < frames_needed {
                                output[frames_to_write * 2..].fill(0.0);
                            }
                        } else {
                            // Stereo to Stereo
                            let frames_available = processed.len() / 2;
                            let frames_to_write = frames_available.min(frames_needed);
                            let copy_len = frames_to_write * 2;
                            for i in 0..copy_len {
                                output[i] = (processed[i] * gain).clamp(-1.0, 1.0);
                            }
                            if copy_len < output.len() {
                                output[copy_len..].fill(0.0);
                            }
                        }
                    }
                }

                // Copy output to preview queue if enabled
                if preview_enabled.load(Ordering::Relaxed) {
                    let mut buf = preview_buffer.lock();
                    if buf.len() < 8192 {
                        buf.extend(&output[..]);
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

        Ok(stream)
    }

    fn start_preview_stream(&mut self) -> Result<(), String> {
        if self.preview_stream.is_some() {
            return Ok(());
        }
        
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| "No default system output device found for preview".to_string())?;

        log::info!("Starting preview output stream: {}", device.name().unwrap_or_default());

        let preview_config = device.default_output_config()
            .map_err(|e| format!("Cannot get default preview config: {}", e))?;
        
        let preview_sample_rate = preview_config.sample_rate().0;
        let preview_channels = preview_config.channels();
        
        let stream_config = StreamConfig {
            channels: preview_channels,
            sample_rate: cpal::SampleRate(preview_sample_rate),
            buffer_size: cpal::BufferSize::Fixed(512),
        };

        let preview_buffer = self.preview_buffer.clone();
        let stop_flag = self.stop_flag.clone();
        let err_fn = |err| log::error!("Preview stream error: {}", err);

        let stream = device.build_output_stream(
            &stream_config,
            move |output: &mut [f32], _| {
                if stop_flag.load(Ordering::Relaxed) {
                    output.fill(0.0);
                    return;
                }

                let mut buf = preview_buffer.lock();
                
                // Latency control: if queue accumulates too much (e.g. over 80ms of latency), drain old samples
                let max_latency = 4096;
                if buf.len() > max_latency {
                    let to_drain = buf.len() - max_latency;
                    buf.drain(0..to_drain);
                }

                let len = output.len();
                let available = buf.len();
                
                if preview_channels == 2 {
                    let to_read = len.min(available);
                    for i in 0..to_read {
                        output[i] = buf.pop_front().unwrap_or(0.0);
                    }
                    if to_read < len {
                        output[to_read..].fill(0.0);
                    }
                } else if preview_channels == 1 {
                    let frames_needed = len;
                    let source_frames_available = available / 2;
                    let to_read = frames_needed.min(source_frames_available);
                    for i in 0..to_read {
                        let left = buf.pop_front().unwrap_or(0.0);
                        let right = buf.pop_front().unwrap_or(0.0);
                        output[i] = (left + right) * 0.5;
                    }
                    if to_read < len {
                        output[to_read..].fill(0.0);
                    }
                } else {
                    let to_read = len.min(available);
                    for i in 0..to_read {
                        output[i] = buf.pop_front().unwrap_or(0.0);
                    }
                    if to_read < len {
                        output[to_read..].fill(0.0);
                    }
                }
            },
            err_fn,
            None,
        ).map_err(|e| format!("Failed to build preview output stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start preview stream: {}", e))?;
        self.preview_stream = Some(stream);
        Ok(())
    }

    pub fn set_preview_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.preview_enabled.store(enabled, Ordering::Relaxed);
        if enabled {
            if self.state == EngineState::Running {
                self.start_preview_stream()?;
            }
        } else {
            if let Some(stream) = self.preview_stream.take() {
                let _ = stream.pause();
                drop(stream);
            }
            self.preview_buffer.lock().clear();
        }
        Ok(())
    }

    pub fn set_pitch(&mut self, semitones: f32) {
        *self.pitch_semitones.lock() = semitones;
        if let Some(ref processor) = self.active_processor {
            processor.lock().set_pitch_semitones(semitones);
        }
        log::info!("AudioEngine pitch set to {:.1} semitones", semitones);
    }

    pub fn stop(&mut self) {
        log::info!("Stopping audio engine");
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(stream) = self._input_stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
        if let Some(stream) = self._output_stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
        if let Some(stream) = self.preview_stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
        self.active_processor = None;
        self.state = EngineState::Stopped;
        self.preview_buffer.lock().clear();
        self.bypass_buffer.lock().clear();
        if let Some(ref level) = self.audio_level {
            level.store(0, Ordering::Relaxed);
        }
        log::info!("Audio engine stopped");
    }

    pub fn is_running(&self) -> bool {
        self.state == EngineState::Running
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}
