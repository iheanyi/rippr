use std::path::Path;
use std::fs::File;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use serde::{Deserialize, Serialize};

/// Audio analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAnalysis {
    pub bpm: Option<f64>,
    #[serde(rename = "bpmConfidence")]
    pub bpm_confidence: Option<f64>,
    pub key: Option<String>,
    #[serde(rename = "keyConfidence")]
    pub key_confidence: Option<f64>,
}

/// Analyze audio file for BPM and Key
pub fn analyze_audio(file_path: &Path) -> Result<AudioAnalysis, String> {
    // Load audio samples
    let samples = load_audio_samples(file_path)?;

    if samples.is_empty() {
        return Err("No audio samples loaded".to_string());
    }

    // Detect BPM using energy-based beat detection
    let (bpm, bpm_confidence) = detect_bpm(&samples, 44100)?;

    // Detect key using chromagram analysis
    let (key, key_confidence) = detect_key(&samples, 44100)?;

    Ok(AudioAnalysis {
        bpm: Some(bpm),
        bpm_confidence: Some(bpm_confidence),
        key: Some(key),
        key_confidence: Some(key_confidence),
    })
}

/// Load audio samples from a file, converting to mono f32
fn load_audio_samples(file_path: &Path) -> Result<Vec<f32>, String> {
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
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    let mut samples: Vec<f32> = Vec::new();
    let max_samples = (sample_rate as usize) * 60; // Max 60 seconds for analysis

    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        if samples.len() >= max_samples {
            break;
        }

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

/// Detect BPM using energy-based beat detection
fn detect_bpm(samples: &[f32], sample_rate: u32) -> Result<(f64, f64), String> {
    if samples.len() < sample_rate as usize * 2 {
        return Err("Audio too short for BPM detection".to_string());
    }

    // Parameters
    let hop_size = sample_rate as usize / 100; // 10ms hops
    let frame_size = sample_rate as usize / 10; // 100ms frames

    // Calculate energy for each frame
    let mut energies: Vec<f32> = Vec::new();
    let mut i = 0;
    while i + frame_size < samples.len() {
        let frame = &samples[i..i + frame_size];
        let energy: f32 = frame.iter().map(|s| s * s).sum();
        energies.push(energy);
        i += hop_size;
    }

    if energies.len() < 10 {
        return Err("Not enough data for BPM detection".to_string());
    }

    // Onset detection using energy difference
    let mut onsets: Vec<f32> = vec![0.0];
    for i in 1..energies.len() {
        let diff = (energies[i] - energies[i - 1]).max(0.0);
        onsets.push(diff);
    }

    // Normalize onsets
    let max_onset = onsets.iter().cloned().fold(0.0_f32, f32::max);
    if max_onset > 0.0 {
        for onset in &mut onsets {
            *onset /= max_onset;
        }
    }

    // Autocorrelation to find periodicity
    let min_bpm = 60.0;
    let max_bpm = 200.0;
    let frames_per_second = sample_rate as f64 / hop_size as f64;

    let min_lag = (frames_per_second * 60.0 / max_bpm) as usize;
    let max_lag = (frames_per_second * 60.0 / min_bpm) as usize;
    let max_lag = max_lag.min(onsets.len() / 2);

    let mut best_bpm = 120.0;
    let mut best_correlation = 0.0_f64;

    for lag in min_lag..max_lag {
        let mut correlation = 0.0_f64;
        let count = onsets.len() - lag;

        for i in 0..count {
            correlation += (onsets[i] * onsets[i + lag]) as f64;
        }
        correlation /= count as f64;

        if correlation > best_correlation {
            best_correlation = correlation;
            let bpm = frames_per_second * 60.0 / lag as f64;
            best_bpm = bpm;
        }
    }

    // Normalize BPM to common range (60-180)
    while best_bpm < 60.0 {
        best_bpm *= 2.0;
    }
    while best_bpm > 180.0 {
        best_bpm /= 2.0;
    }

    // Confidence based on correlation strength
    let confidence = (best_correlation * 100.0).min(100.0);

    // Round to nearest integer
    let bpm = (best_bpm).round();

    Ok((bpm, confidence))
}

/// Key profiles for Krumhansl-Schmuckler algorithm
const MAJOR_PROFILE: [f64; 12] = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE: [f64; 12] = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];
const KEY_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Detect musical key using chromagram analysis
fn detect_key(samples: &[f32], sample_rate: u32) -> Result<(String, f64), String> {
    if samples.len() < sample_rate as usize {
        return Err("Audio too short for key detection".to_string());
    }

    // Calculate chromagram (12-bin representation of pitch classes)
    let chromagram = compute_chromagram(samples, sample_rate);

    // Normalize chromagram
    let sum: f64 = chromagram.iter().sum();
    let normalized: Vec<f64> = if sum > 0.0 {
        chromagram.iter().map(|c| c / sum).collect()
    } else {
        chromagram.clone()
    };

    // Find best matching key using correlation with profiles
    let mut best_key = "C".to_string();
    let mut best_correlation = f64::NEG_INFINITY;

    for root in 0..12 {
        // Rotate chromagram to align with root
        let mut rotated: Vec<f64> = vec![0.0; 12];
        for i in 0..12 {
            rotated[i] = normalized[(i + root) % 12];
        }

        // Correlate with major profile
        let major_corr = pearson_correlation(&rotated, &MAJOR_PROFILE.to_vec());
        if major_corr > best_correlation {
            best_correlation = major_corr;
            best_key = format!("{} Major", KEY_NAMES[root]);
        }

        // Correlate with minor profile
        let minor_corr = pearson_correlation(&rotated, &MINOR_PROFILE.to_vec());
        if minor_corr > best_correlation {
            best_correlation = minor_corr;
            best_key = format!("{}m", KEY_NAMES[root]);
        }
    }

    // Convert correlation to confidence (0-100%)
    let confidence = ((best_correlation + 1.0) / 2.0 * 100.0).max(0.0).min(100.0);

    Ok((best_key, confidence))
}

/// Compute chromagram from audio samples using simple FFT-based approach
fn compute_chromagram(samples: &[f32], sample_rate: u32) -> Vec<f64> {
    let mut chromagram = vec![0.0_f64; 12];

    // Use a simple approach: analyze frequency content via autocorrelation
    // Map detected frequencies to pitch classes

    let frame_size = 4096;
    let hop_size = 2048;

    let mut frame_count = 0;
    let mut i = 0;

    while i + frame_size < samples.len() {
        let frame = &samples[i..i + frame_size];

        // Apply window
        let windowed: Vec<f64> = frame
            .iter()
            .enumerate()
            .map(|(n, &s)| {
                let window = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * n as f64 / (frame_size - 1) as f64).cos());
                s as f64 * window
            })
            .collect();

        // Simple pitch detection via autocorrelation
        for note in 0..12 {
            // Calculate frequency for each note across several octaves
            for octave in 2..7 {
                let midi_note = note + (octave * 12);
                let freq = 440.0 * 2.0_f64.powf((midi_note as f64 - 69.0) / 12.0);

                // Calculate lag for this frequency
                let lag = (sample_rate as f64 / freq) as usize;
                if lag > 0 && lag < frame_size / 2 {
                    // Autocorrelation at this lag
                    let mut corr = 0.0_f64;
                    for j in 0..frame_size - lag {
                        corr += windowed[j] * windowed[j + lag];
                    }
                    corr /= (frame_size - lag) as f64;

                    // Add to chromagram (map to pitch class)
                    if corr > 0.0 {
                        chromagram[note] += corr;
                    }
                }
            }
        }

        i += hop_size;
        frame_count += 1;
    }

    // Normalize by frame count
    if frame_count > 0 {
        for c in &mut chromagram {
            *c /= frame_count as f64;
        }
    }

    chromagram
}

/// Calculate Pearson correlation coefficient
fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(a, b)| a * b).sum();
    let sum_x2: f64 = x.iter().map(|a| a * a).sum();
    let sum_y2: f64 = y.iter().map(|a| a * a).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denominator = ((n * sum_x2 - sum_x * sum_x) * (n * sum_y2 - sum_y * sum_y)).sqrt();

    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}
