// audio/processor.rs — Real-time pitch shifting using SoundTouch
//
// SoundTouch is a mature C++ audio processing library that supports:
//   - Pitch shifting with formant preservation (voice sounds natural, not chipmunk)
//   - Low-latency streaming mode for real-time use
//   - Independent pitch & tempo control

use soundtouch::{Setting, SoundTouch};

/// Wraps the SoundTouch engine for real-time pitch processing.
/// Processes f32 mono or stereo audio samples and returns pitch-shifted samples.
pub struct PitchProcessor {
    engine: SoundTouch,
    sample_rate: u32,
    channels: u16,
    /// Current pitch shift in semitones (negative = lower pitch)
    pitch_semitones: f32,
}

impl PitchProcessor {
    /// Creates a new pitch processor.
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate (e.g. 44100 or 48000)
    /// * `channels` - Number of audio channels (1 = mono, 2 = stereo)
    /// * `pitch_semitones` - Initial pitch shift in semitones (e.g. -3.0 for 3 semitones lower)
    pub fn new(sample_rate: u32, channels: u16, pitch_semitones: f32) -> Self {
        let mut engine = SoundTouch::new();

        // Configure for real-time streaming (minimize latency)
        engine.set_sample_rate(sample_rate);
        engine.set_channels(channels as u32);

        // UseAaFilter = 0 means disable anti-alias filter (lower latency)
        engine.set_setting(Setting::UseAaFilter, 0);

        // UseQuickseek = 1 means enable fast seeking (lower CPU, slight quality tradeoff)
        engine.set_setting(Setting::UseQuickseek, 1);

        // Tune for minimal latency in real-time mode
        engine.set_setting(Setting::SequenceMs, 30);
        engine.set_setting(Setting::SeekwindowMs, 15);
        engine.set_setting(Setting::OverlapMs, 8);

        // Set initial pitch
        let pitch_factor = semitones_to_factor(pitch_semitones);
        engine.set_pitch(pitch_factor);

        // Keep tempo unchanged — we're only shifting pitch, not speed
        engine.set_tempo(1.0);
        engine.set_rate(1.0);

        log::info!(
            "PitchProcessor initialized: {}Hz, {}ch, pitch={:.1} semitones (factor={:.4})",
            sample_rate,
            channels,
            pitch_semitones,
            pitch_factor
        );

        Self {
            engine,
            sample_rate,
            channels,
            pitch_semitones,
        }
    }

    /// Updates the pitch shift in real-time without restarting the stream.
    pub fn set_pitch_semitones(&mut self, semitones: f32) {
        let clamped = semitones.clamp(-12.0, 12.0);
        self.pitch_semitones = clamped;
        let factor = semitones_to_factor(clamped);
        self.engine.set_pitch(factor);
        log::info!("SoundTouch pitch updated: {:.1} semitones (factor={:.4})", clamped, factor);
    }

    /// Gets the current pitch shift in semitones.
    pub fn pitch_semitones(&self) -> f32 {
        self.pitch_semitones
    }

    /// Feeds raw input samples into the pitch processor.
    /// Call this from the audio capture callback.
    /// `samples` contains interleaved channel data (e.g. L,R,L,R for stereo).
    pub fn feed_samples(&mut self, samples: &[f32]) {
        // SoundTouch expects num_samples = number of sample FRAMES (not total samples)
        // For stereo: num_frames = samples.len() / channels
        let num_frames = samples.len() / self.channels as usize;
        self.engine.put_samples(samples, num_frames);
    }

    /// Retrieves processed (pitch-shifted) samples.
    /// Returns however many samples are available (may be fewer than requested on first call).
    /// `max_frames` is the maximum number of FRAMES (not total samples) to read.
    pub fn receive_samples(&mut self, max_frames: usize) -> Vec<f32> {
        let available = self.available_frames();
        if available == 0 {
            return vec![];
        }
        let frames_to_read = available.min(max_frames);
        let sample_count = frames_to_read * self.channels as usize;
        let mut output = vec![0.0f32; sample_count];
        let frames_read = self.engine.receive_samples(&mut output, frames_to_read);
        let actual_samples = frames_read * self.channels as usize;
        output.truncate(actual_samples);
        output
    }

    /// Flushes any buffered samples (call when stopping).
    pub fn flush(&mut self) {
        self.engine.flush();
    }

    /// Returns how many processed frames are ready to read.
    pub fn available_frames(&mut self) -> usize {
        // num_samples() returns i32 in this version of the crate
        let n = self.engine.num_samples();
        if n < 0 { 0 } else { n as usize }
    }
}

/// Converts semitones to a pitch multiplication factor.
///
/// Formula: factor = 2^(semitones/12)
/// Examples:
///   -3 semitones → 0.8409 (lower pitch)
///   -6 semitones → 0.7071 (much lower)
///    0 semitones → 1.0000 (unchanged)
fn semitones_to_factor(semitones: f32) -> f64 {
    (2.0_f64).powf(semitones as f64 / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semitones_to_factor() {
        let factor = semitones_to_factor(0.0);
        assert!((factor - 1.0).abs() < 0.001, "0 semitones should be factor 1.0");

        let factor_down = semitones_to_factor(-12.0);
        assert!((factor_down - 0.5).abs() < 0.001, "-12 semitones should be factor 0.5 (one octave down)");

        let factor_up = semitones_to_factor(12.0);
        assert!((factor_up - 2.0).abs() < 0.001, "+12 semitones should be factor 2.0 (one octave up)");
    }
}
