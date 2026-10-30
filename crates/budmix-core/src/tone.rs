//! The test beep played into the buds by the setup check and the CLI: a
//! 440 Hz sine with short fades, so it starts and stops without a click.

use std::f64::consts::TAU;

pub const TONE_HZ: f64 = 440.0;
pub const TONE_MS: u64 = 700;
const FADE_MS: u64 = 15;
/// About -12 dBFS: clearly audible, never harsh.
const AMPLITUDE: f64 = 0.25;

/// A fixed-length tone at one sample rate.
#[derive(Debug, Clone, Copy)]
pub struct Tone {
    sample_rate: u32,
    total_frames: u64,
    fade_frames: u64,
}

impl Tone {
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate = sample_rate.max(1);
        let frames_for = |ms: u64| u64::from(sample_rate) * ms / 1000;
        Self {
            sample_rate,
            total_frames: frames_for(TONE_MS),
            fade_frames: frames_for(FADE_MS).max(1),
        }
    }

    pub fn total_frames(&self) -> u64 {
        self.total_frames
    }

    /// The sample at `frame`; silence after the end.
    pub fn sample(&self, frame: u64) -> f32 {
        if frame >= self.total_frames {
            return 0.0;
        }
        let remaining = self.total_frames - frame;
        let envelope = (frame.min(remaining) as f64 / self.fade_frames as f64).min(1.0);
        let phase = TAU * TONE_HZ * frame as f64 / f64::from(self.sample_rate);
        (AMPLITUDE * envelope * phase.sin()) as f32
    }

    /// Writes interleaved samples starting at `start_frame`, the same value on
    /// every channel. Returns the number of frames written, at most what
    /// fits in `out` and never past the end of the tone.
    pub fn fill(&self, start_frame: u64, channels: usize, out: &mut [f32]) -> usize {
        let channels = channels.max(1);
        let left = self.total_frames.saturating_sub(start_frame);
        let frames = (out.len() / channels).min(usize::try_from(left).unwrap_or(usize::MAX));
        for (index, frame) in out.chunks_exact_mut(channels).take(frames).enumerate() {
            frame.fill(self.sample(start_frame + index as u64));
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_follows_the_sample_rate() {
        assert_eq!(Tone::new(48_000).total_frames(), 33_600);
        assert_eq!(Tone::new(44_100).total_frames(), 30_870);
    }

    #[test]
    fn starts_and_ends_silent() {
        let tone = Tone::new(48_000);
        assert_eq!(tone.sample(0), 0.0);
        assert!(tone.sample(tone.total_frames() - 1).abs() < 0.001);
        assert_eq!(tone.sample(tone.total_frames()), 0.0);
        assert_eq!(tone.sample(u64::MAX), 0.0);
    }

    #[test]
    fn never_exceeds_the_amplitude() {
        let tone = Tone::new(48_000);
        let peak = (0..tone.total_frames())
            .map(|frame| tone.sample(frame).abs())
            .fold(0.0_f32, f32::max);
        assert!(peak <= AMPLITUDE as f32 + f32::EPSILON);
        assert!(peak > AMPLITUDE as f32 * 0.99);
    }

    #[test]
    fn fade_in_is_gradual() {
        let tone = Tone::new(48_000);
        // Within the first millisecond the tone is still far below full level.
        let early = (0..48)
            .map(|frame| tone.sample(frame).abs())
            .fold(0.0_f32, f32::max);
        assert!(early < AMPLITUDE as f32 * 0.1);
    }

    #[test]
    fn chunked_fill_matches_one_pass() {
        let tone = Tone::new(44_100);
        let channels = 2;
        let mut whole = vec![0.0; tone.total_frames() as usize * channels];
        assert_eq!(
            tone.fill(0, channels, &mut whole),
            tone.total_frames() as usize
        );

        let mut chunked = Vec::new();
        let mut frame = 0;
        loop {
            let mut buffer = [1.0_f32; 2 * 441];
            let written = tone.fill(frame, channels, &mut buffer);
            if written == 0 {
                break;
            }
            chunked.extend_from_slice(&buffer[..written * channels]);
            frame += written as u64;
        }
        assert_eq!(chunked, whole);
    }

    #[test]
    fn every_channel_gets_the_same_sample() {
        let tone = Tone::new(48_000);
        let mut buffer = [0.0_f32; 6 * 100];
        tone.fill(1_000, 6, &mut buffer);
        for frame in buffer.chunks_exact(6) {
            assert!(frame.iter().all(|sample| *sample == frame[0]));
        }
    }

    #[test]
    fn fill_stops_at_the_end() {
        let tone = Tone::new(48_000);
        let mut buffer = [9.0_f32; 2 * 100];
        let written = tone.fill(tone.total_frames() - 10, 2, &mut buffer);
        assert_eq!(written, 10);
        assert_eq!(buffer[20], 9.0, "nothing written past the end");
    }
}
