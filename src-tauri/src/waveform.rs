use std::path::Path;
use std::fs::File;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use serde::{Deserialize, Serialize};

/// Waveform data point (min, max amplitude for a segment)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveformPoint {
    pub min: f32,
    pub max: f32,
}

/// Generate waveform data from an audio file
pub fn generate_waveform(file_path: &Path, num_points: usize) -> Result<Vec<WaveformPoint>, String> {
    // Load audio samples
    let samples = load_audio_samples_for_waveform(file_path)?;

    if samples.is_empty() {
        return Err("No audio samples loaded".to_string());
    }

    // Calculate samples per point
    let samples_per_point = samples.len() / num_points;
    if samples_per_point == 0 {
        return Err("Audio too short for requested resolution".to_string());
    }

    let mut waveform: Vec<WaveformPoint> = Vec::with_capacity(num_points);

    for i in 0..num_points {
        let start = i * samples_per_point;
        let end = ((i + 1) * samples_per_point).min(samples.len());
        let segment = &samples[start..end];

        if segment.is_empty() {
            waveform.push(WaveformPoint { min: 0.0, max: 0.0 });
            continue;
        }

        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;

        for &sample in segment {
            if sample < min_val {
                min_val = sample;
            }
            if sample > max_val {
                max_val = sample;
            }
        }

        waveform.push(WaveformPoint {
            min: min_val,
            max: max_val,
        });
    }

    Ok(waveform)
}

/// Load audio samples from a file, converting to mono f32
fn load_audio_samples_for_waveform(file_path: &Path) -> Result<Vec<f32>, String> {
    let file = File::open(file_path)
        .map_err(|e| format!("Failed to open file: {}", e))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = file_path.extension() {
        hint.with_extension(ext.to_str().unwrap_or("mp3"));
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Failed to probe audio: {}", e))?;

    let mut format = probed.format;

    let track = format.tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("No audio track found")?;

    let track_id = track.id;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let mut samples: Vec<f32> = Vec::new();

    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(_) => continue,
        };

        if sample_buf.is_none() {
            let spec = *decoded.spec();
            let duration = decoded.capacity() as u64;
            sample_buf = Some(SampleBuffer::new(duration, spec));
        }

        if let Some(ref mut buf) = sample_buf {
            buf.copy_interleaved_ref(decoded);
            let interleaved = buf.samples();

            // Convert to mono by averaging channels
            for chunk in interleaved.chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                samples.push(mono);
            }
        }
    }

    Ok(samples)
}
