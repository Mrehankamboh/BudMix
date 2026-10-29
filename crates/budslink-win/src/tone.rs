//! Plays the test beep through WASAPI in shared mode, so it mixes with
//! whatever else is playing and never takes the device away from other apps.

use std::thread;
use std::time::Duration;

use budslink_core::tone::Tone;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, eConsole, eRender,
};
use windows::Win32::Media::KernelStreaming::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::core::{Error, PCWSTR, Result};

use crate::com::{self, wide};

const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
/// 200 ms buffer, in 100-ns units.
const BUFFER_DURATION: i64 = 2_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SampleFormat {
    F32,
    I16,
}

/// Plays the tone on `endpoint_id`, or on the default output when `None`,
/// and returns once it has been heard.
pub(crate) fn play(endpoint_id: Option<&str>) -> Result<()> {
    com::ensure_initialized();
    // SAFETY: COM calls on live interfaces. The mix format is freed below and
    // the render buffer is only written within the size WASAPI granted.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device: IMMDevice = match endpoint_id {
            Some(id) => enumerator.GetDevice(PCWSTR(wide(id).as_ptr()))?,
            None => enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?,
        };
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let format = client.GetMixFormat()?;
        let result = render(&client, format);
        CoTaskMemFree(Some(format as *const _));
        result
    }
}

/// # Safety
/// `format` must be the mix format returned by `client.GetMixFormat()`.
unsafe fn render(client: &IAudioClient, format: *const WAVEFORMATEX) -> Result<()> {
    // SAFETY: guaranteed by the caller; read unaligned because the structs
    // are packed.
    let (header, sample_format) = unsafe {
        let header = std::ptr::read_unaligned(format);
        (header, sample_format(format, &header)?)
    };
    let channels = usize::from(header.nChannels);
    let tone = Tone::new(header.nSamplesPerSec);

    // SAFETY: COM calls on a live client with its own mix format.
    unsafe {
        client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, BUFFER_DURATION, 0, format, None)?;
        let buffer_frames = client.GetBufferSize()?;
        let render: IAudioRenderClient = client.GetService()?;
        client.Start()?;

        let mut samples = vec![0.0_f32; buffer_frames as usize * channels];
        let mut frame = 0_u64;
        while frame < tone.total_frames() {
            let free = buffer_frames - client.GetCurrentPadding()?;
            if free > 0 {
                let written = tone.fill(frame, channels, &mut samples[..free as usize * channels]);
                let data = render.GetBuffer(written as u32)?;
                write(data, &samples[..written * channels], sample_format);
                render.ReleaseBuffer(written as u32, 0)?;
                frame += written as u64;
            }
            thread::sleep(Duration::from_millis(10));
        }
        // Let the buffered end of the tone play out before stopping.
        while client.GetCurrentPadding()? > 0 {
            thread::sleep(Duration::from_millis(10));
        }
        client.Stop()?;
    }
    Ok(())
}

/// # Safety
/// `format` must point to a valid mix format whose header is `header`.
unsafe fn sample_format(format: *const WAVEFORMATEX, header: &WAVEFORMATEX) -> Result<SampleFormat> {
    let tag = header.wFormatTag;
    let bits = header.wBitsPerSample;
    let float = match tag {
        WAVE_FORMAT_IEEE_FLOAT => true,
        WAVE_FORMAT_EXTENSIBLE => {
            // SAFETY: an extensible format is a WAVEFORMATEXTENSIBLE.
            let extensible =
                unsafe { std::ptr::read_unaligned(format.cast::<WAVEFORMATEXTENSIBLE>()) };
            let sub_format = extensible.SubFormat;
            sub_format == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
        }
        WAVE_FORMAT_PCM => false,
        _ => return Err(unsupported(tag, bits)),
    };
    match (float, bits) {
        (true, 32) => Ok(SampleFormat::F32),
        (false, 16) => Ok(SampleFormat::I16),
        _ => Err(unsupported(tag, bits)),
    }
}

fn unsupported(tag: u16, bits: u16) -> Error {
    Error::new(
        windows::Win32::Foundation::E_NOTIMPL,
        format!("unsupported audio format (tag {tag:#06x}, {bits} bits)"),
    )
}

/// # Safety
/// `data` must have room for `samples.len()` samples of `format`.
unsafe fn write(data: *mut u8, samples: &[f32], format: SampleFormat) {
    match format {
        SampleFormat::F32 => {
            // SAFETY: guaranteed by the caller; WASAPI buffers are aligned.
            let out = unsafe { std::slice::from_raw_parts_mut(data.cast::<f32>(), samples.len()) };
            out.copy_from_slice(samples);
        }
        SampleFormat::I16 => {
            // SAFETY: as above.
            let out = unsafe { std::slice::from_raw_parts_mut(data.cast::<i16>(), samples.len()) };
            for (out, sample) in out.iter_mut().zip(samples) {
                *out = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            }
        }
    }
}
