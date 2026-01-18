use id3::TagLike;
use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::mem::MaybeUninit;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use tauri::Emitter;
use std::collections::HashMap;
use uuid::Uuid;

mod db;
mod audio_analysis;
mod waveform;
use db::DownloadHistoryEntry;
use audio_analysis::AudioAnalysis;
use waveform::WaveformPoint;

// Global cancellation flag
static CANCEL_DOWNLOAD: AtomicBool = AtomicBool::new(false);

// Global download queue
lazy_static::lazy_static! {
    static ref DOWNLOAD_QUEUE: Mutex<HashMap<String, QueueItem>> = Mutex::new(HashMap::new());
}

/// Status of a queue item
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum QueueStatus {
    Pending,
    Fetching,
    Ready,
    Downloading,
    Complete,
    Failed,
}

/// An item in the download queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub url: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub thumbnail: Option<String>,
    pub duration: Option<u64>,
    pub status: QueueStatus,
    pub progress: u8,
    pub error: Option<String>,
    #[serde(rename = "outputPath")]
    pub output_path: Option<String>,
}

/// Progress update for a queue item
#[derive(Clone, Serialize)]
pub struct QueueProgress {
    pub id: String,
    pub status: QueueStatus,
    pub progress: u8,
    pub message: String,
}

/// Progress event payload sent to frontend
#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

/// Error types for better error messages
#[derive(Debug, Clone, Serialize)]
pub enum DownloadError {
    NetworkError(String),
    InvalidUrl(String),
    FileError(String),
    ConversionError(String),
    Cancelled,
    Unknown(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::NetworkError(msg) => write!(f, "Network error: {}. Check your internet connection.", msg),
            DownloadError::InvalidUrl(msg) => write!(f, "Invalid URL: {}. Make sure the URL is from a supported site.", msg),
            DownloadError::FileError(msg) => write!(f, "File error: {}. Check disk space and permissions.", msg),
            DownloadError::ConversionError(msg) => write!(f, "Conversion error: {}. The audio format may not be supported.", msg),
            DownloadError::Cancelled => write!(f, "Download cancelled."),
            DownloadError::Unknown(msg) => write!(f, "Error: {}", msg),
        }
    }
}

static PYTHON_INIT: Once = Once::new();

/// Initialize Python environment - looks for bundled Python or falls back to system Python
fn init_python_env() {
    PYTHON_INIT.call_once(|| {
        // Try to find bundled Python in the app resources
        if let Some(resource_dir) = get_resource_dir() {
            let python_dir = resource_dir.join("python");
            if python_dir.exists() {
                // Set environment variables for bundled Python
                std::env::set_var("PYTHONHOME", &python_dir);

                // Find the lib directory (e.g., lib/python3.12)
                let lib_dir = python_dir.join("lib");
                if let Ok(entries) = std::fs::read_dir(&lib_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        if name.to_string_lossy().starts_with("python3") {
                            let python_lib = lib_dir.join(&name);
                            std::env::set_var("PYTHONPATH", &python_lib);
                            break;
                        }
                    }
                }

                println!("Using bundled Python from: {:?}", python_dir);
                return;
            }
        }

        // Fall back to system Python (for development)
        println!("Using system Python");
    });
}

/// Get the app's resource directory
fn get_resource_dir() -> Option<PathBuf> {
    // In production macOS app bundle: App.app/Contents/Resources
    // In development: src-tauri/resources

    let exe_path = std::env::current_exe().ok()?;

    // Check if we're in an app bundle (macOS)
    if exe_path.to_string_lossy().contains(".app/Contents/MacOS") {
        // We're in an app bundle
        let resources = exe_path
            .parent()? // MacOS
            .parent()? // Contents
            .join("Resources");
        if resources.exists() {
            return Some(resources);
        }
    }

    // Development mode: check for resources in src-tauri
    let dev_resources = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources");
    if dev_resources.exists() {
        return Some(dev_resources);
    }

    None
}
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoMetadata {
    #[serde(rename = "videoId")]
    pub video_id: String,
    #[serde(rename = "rawTitle")]
    pub raw_title: String,
    pub title: String,
    pub artist: String,
    pub thumbnail: Option<String>,
    pub duration: Option<u64>,
    #[serde(rename = "channelName")]
    pub channel_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadResult {
    pub success: bool,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub download_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        let download_dir = dirs::download_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
            .unwrap_or_else(|| PathBuf::from("."))
            .to_string_lossy()
            .to_string();

        Self { download_dir }
    }
}

#[derive(Debug, Deserialize)]
struct YtDlpMetadata {
    id: String,
    title: String,
    channel: Option<String>,
    uploader: Option<String>,
    artist: Option<String>,
    track: Option<String>,
    thumbnail: Option<String>,
    duration: Option<f64>,
}

/// Fetch video metadata using yt-dlp via PyO3
fn ytdlp_extract_info(url: &str) -> Result<YtDlpMetadata, String> {
    init_python_env();
    Python::with_gil(|py| {
        let yt_dlp = py.import("yt_dlp").map_err(|e| format!("Failed to import yt_dlp: {}", e))?;

        // Create options dict
        let opts = PyDict::new(py);
        opts.set_item("quiet", true).unwrap();
        opts.set_item("no_warnings", true).unwrap();
        opts.set_item("extract_flat", false).unwrap();
        opts.set_item("noplaylist", true).unwrap();

        // Create YoutubeDL instance
        let ydl_class = yt_dlp.getattr("YoutubeDL").map_err(|e| format!("Failed to get YoutubeDL: {}", e))?;
        let ydl = ydl_class.call1((opts,)).map_err(|e| format!("Failed to create YoutubeDL: {}", e))?;

        // Extract info without downloading
        let info = ydl.call_method1("extract_info", (url, false))
            .map_err(|e| format!("Failed to extract info: {}", e))?;

        // Helper to extract optional string field
        fn get_str(info: &Bound<'_, PyAny>, key: &str) -> Option<String> {
            info.get_item(key).ok().and_then(|v| {
                if v.is_none() { None } else { v.extract().ok() }
            })
        }

        fn get_f64(info: &Bound<'_, PyAny>, key: &str) -> Option<f64> {
            info.get_item(key).ok().and_then(|v| {
                if v.is_none() { None } else { v.extract().ok() }
            })
        }

        // Extract fields from the info dict
        let id: String = get_str(&info, "id").ok_or("No id field")?;
        let title: String = get_str(&info, "title").ok_or("No title field")?;
        let channel: Option<String> = get_str(&info, "channel");
        let uploader: Option<String> = get_str(&info, "uploader");
        let artist: Option<String> = get_str(&info, "artist");
        let track: Option<String> = get_str(&info, "track");
        let thumbnail: Option<String> = get_str(&info, "thumbnail");
        let duration: Option<f64> = get_f64(&info, "duration");

        Ok(YtDlpMetadata {
            id,
            title,
            channel,
            uploader,
            artist,
            track,
            thumbnail,
            duration,
        })
    })
}

/// Download audio using yt-dlp via PyO3
fn ytdlp_download(url: &str, output_path: &str) -> Result<String, String> {
    init_python_env();
    Python::with_gil(|py| {
        let yt_dlp = py.import("yt_dlp").map_err(|e| format!("Failed to import yt_dlp: {}", e))?;

        // Create options dict
        let opts = PyDict::new(py);
        opts.set_item("quiet", true).unwrap();
        opts.set_item("no_warnings", true).unwrap();
        opts.set_item("noplaylist", true).unwrap();
        opts.set_item("format", "bestaudio[ext=m4a]/bestaudio/best").unwrap();
        opts.set_item("outtmpl", output_path).unwrap();

        // Post-processors to extract audio
        let pp_dict = PyDict::new(py);
        pp_dict.set_item("key", "FFmpegExtractAudio").unwrap();
        pp_dict.set_item("preferredcodec", "m4a").unwrap();
        let pp_list = PyList::new(py, &[pp_dict]).map_err(|e| format!("Failed to create list: {}", e))?;
        opts.set_item("postprocessors", pp_list).unwrap();

        // Create YoutubeDL instance
        let ydl_class = yt_dlp.getattr("YoutubeDL").map_err(|e| format!("Failed to get YoutubeDL: {}", e))?;
        let ydl = ydl_class.call1((opts,)).map_err(|e| format!("Failed to create YoutubeDL: {}", e))?;

        // Download
        ydl.call_method1("download", (vec![url],))
            .map_err(|e| format!("Failed to download: {}", e))?;

        Ok(output_path.to_string())
    })
}

fn clean_title(title: &str) -> String {
    let mut cleaned = title.to_string();
    let patterns = [
        r"\s*\(Official\s*(Music\s*)?Video\)",
        r"\s*\[Official\s*(Music\s*)?Video\]",
        r"\s*\(Official\s*Audio\)",
        r"\s*\[Official\s*Audio\]",
        r"\s*\(Lyric\s*Video\)",
        r"\s*\[Lyric\s*Video\]",
        r"\s*\(Lyrics\)",
        r"\s*\[Lyrics\]",
        r"\s*\(HD\)",
        r"\s*\[HD\]",
        r"\s*\(HQ\)",
        r"\s*\[HQ\]",
        r"\s*\(4K\)",
        r"\s*\[4K\]",
        r"\s*\|\s*.*$",
    ];

    for pattern in patterns {
        if let Ok(re) = regex::Regex::new(&format!("(?i){}", pattern)) {
            cleaned = re.replace_all(&cleaned, "").to_string();
        }
    }

    cleaned.trim().to_string()
}

fn parse_artist_title(raw_title: &str, channel_name: &str) -> (String, String) {
    let cleaned = clean_title(raw_title);

    // Try "Artist - Title" pattern
    if let Some(pos) = cleaned.find(" - ") {
        let artist = cleaned[..pos].trim().to_string();
        let title = cleaned[pos + 3..].trim().to_string();
        return (artist, title);
    }

    // Try "Artist – Title" (en-dash)
    if let Some(pos) = cleaned.find(" – ") {
        let artist = cleaned[..pos].trim().to_string();
        let title = cleaned[pos + 4..].trim().to_string();
        return (artist, title);
    }

    // Fall back to channel name as artist
    (channel_name.to_string(), cleaned)
}

/// Convert M4A/AAC audio file to MP3 using symphonia (decoder) and mp3lame-encoder
fn convert_to_mp3(input_path: &Path, output_path: &Path, bitrate_kbps: u32) -> Result<(), String> {
    // Open the input file
    let file = File::open(input_path).map_err(|e| format!("Failed to open input file: {}", e))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Create a hint to help the format registry guess the format
    let mut hint = Hint::new();
    if let Some(ext) = input_path.extension() {
        hint.with_extension(ext.to_str().unwrap_or("m4a"));
    }

    // Probe the media source
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Failed to probe audio format: {}", e))?;

    let mut format = probed.format;

    // Find the first audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("No audio track found")?;

    let track_id = track.id;

    // Create decoder
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    // Get audio parameters
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or("Unknown sample rate")?;
    // Default to stereo if channel count not available (common for YouTube audio)
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(2);

    // Create MP3 encoder
    let mut mp3_encoder = Builder::new().ok_or("Failed to create MP3 encoder")?;
    mp3_encoder
        .set_num_channels(channels as u8)
        .map_err(|e| format!("Failed to set channels: {:?}", e))?;
    mp3_encoder
        .set_sample_rate(sample_rate)
        .map_err(|e| format!("Failed to set sample rate: {:?}", e))?;
    mp3_encoder
        .set_brate(match bitrate_kbps {
            128 => mp3lame_encoder::Bitrate::Kbps128,
            192 => mp3lame_encoder::Bitrate::Kbps192,
            256 => mp3lame_encoder::Bitrate::Kbps256,
            320 => mp3lame_encoder::Bitrate::Kbps320,
            _ => mp3lame_encoder::Bitrate::Kbps192,
        })
        .map_err(|e| format!("Failed to set bitrate: {:?}", e))?;
    mp3_encoder
        .set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| format!("Failed to set quality: {:?}", e))?;

    let mut mp3_encoder = mp3_encoder
        .build()
        .map_err(|e| format!("Failed to build MP3 encoder: {:?}", e))?;

    // Create output file
    let mut output_file =
        File::create(output_path).map_err(|e| format!("Failed to create output file: {}", e))?;

    // Decode and encode
    let mut sample_buf: Option<SampleBuffer<i16>> = None;

    loop {
        // Get next packet
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(format!("Failed to read packet: {}", e)),
        };

        // Skip packets from other tracks
        if packet.track_id() != track_id {
            continue;
        }

        // Decode packet
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("Failed to decode: {}", e)),
        };

        // Convert to interleaved i16 samples
        if sample_buf.is_none() {
            let spec = *decoded.spec();
            let duration = decoded.capacity() as u64;
            sample_buf = Some(SampleBuffer::new(duration, spec));
        }

        if let Some(ref mut buf) = sample_buf {
            buf.copy_interleaved_ref(decoded);
            let samples = buf.samples();

            // Encode to MP3
            let input = InterleavedPcm(samples);
            let buf_size = mp3lame_encoder::max_required_buffer_size(samples.len());
            let mut mp3_out: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); buf_size];
            let encoded_size = mp3_encoder
                .encode(input, &mut mp3_out)
                .map_err(|e| format!("Failed to encode MP3: {:?}", e))?;

            // Safety: mp3lame-encoder initializes the bytes it writes
            let mp3_bytes: &[u8] =
                unsafe { std::slice::from_raw_parts(mp3_out.as_ptr() as *const u8, encoded_size) };
            output_file
                .write_all(mp3_bytes)
                .map_err(|e| format!("Failed to write MP3 data: {}", e))?;
        }
    }

    // Flush the encoder
    let mut mp3_out: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 7200];
    let encoded_size = mp3_encoder
        .flush::<FlushNoGap>(&mut mp3_out)
        .map_err(|e| format!("Failed to flush MP3 encoder: {:?}", e))?;
    // Safety: mp3lame-encoder initializes the bytes it writes
    let mp3_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(mp3_out.as_ptr() as *const u8, encoded_size) };
    output_file
        .write_all(mp3_bytes)
        .map_err(|e| format!("Failed to write final MP3 data: {}", e))?;

    Ok(())
}

fn get_settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sample-downloader")
        .join("settings.json")
}

#[tauri::command]
async fn fetch_metadata(url: String) -> Result<VideoMetadata, String> {
    // Use PyO3 to call yt-dlp
    let metadata = ytdlp_extract_info(&url)?;

    // Get artist and title - prefer track/artist fields if available (YouTube Music)
    let (artist, title) = if metadata.artist.is_some() && metadata.track.is_some() {
        (
            metadata.artist.clone().unwrap(),
            metadata.track.clone().unwrap(),
        )
    } else {
        let channel = metadata
            .channel
            .clone()
            .or(metadata.uploader.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string());
        parse_artist_title(&metadata.title, &channel)
    };

    Ok(VideoMetadata {
        video_id: metadata.id,
        raw_title: metadata.title.clone(),
        title,
        artist,
        thumbnail: metadata.thumbnail,
        duration: metadata.duration.map(|d| d as u64),
        channel_name: metadata.channel.or(metadata.uploader),
    })
}

/// Helper to emit progress events
fn emit_progress(app: &tauri::AppHandle, stage: &str, percent: u8, message: &str) {
    let _ = app.emit("download-progress", DownloadProgress {
        stage: stage.to_string(),
        percent,
        message: message.to_string(),
    });
}

/// Check if download was cancelled
fn is_cancelled() -> bool {
    CANCEL_DOWNLOAD.load(Ordering::SeqCst)
}

#[tauri::command]
async fn cancel_download() -> Result<(), String> {
    CANCEL_DOWNLOAD.store(true, Ordering::SeqCst);
    Ok(())
}

/// Helper to emit queue progress events
fn emit_queue_progress(app: &tauri::AppHandle, id: &str, status: QueueStatus, progress: u8, message: &str) {
    let _ = app.emit("queue-progress", QueueProgress {
        id: id.to_string(),
        status,
        progress,
        message: message.to_string(),
    });
}

/// Add a URL to the download queue
#[tauri::command]
async fn add_to_queue(app: tauri::AppHandle, url: String) -> Result<QueueItem, String> {
    let id = Uuid::new_v4().to_string()[..8].to_string();

    let item = QueueItem {
        id: id.clone(),
        url: url.clone(),
        title: None,
        artist: None,
        thumbnail: None,
        duration: None,
        status: QueueStatus::Pending,
        progress: 0,
        error: None,
        output_path: None,
    };

    {
        let mut queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
        queue.insert(id.clone(), item.clone());
    }

    // Emit event to notify frontend
    let _ = app.emit("queue-updated", get_queue_items()?);

    Ok(item)
}

/// Add multiple URLs to the queue at once
#[tauri::command]
async fn add_urls_to_queue(app: tauri::AppHandle, urls: Vec<String>) -> Result<Vec<QueueItem>, String> {
    let mut items = Vec::new();

    for url in urls {
        let url = url.trim().to_string();
        if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
            continue;
        }

        let id = Uuid::new_v4().to_string()[..8].to_string();

        let item = QueueItem {
            id: id.clone(),
            url: url.clone(),
            title: None,
            artist: None,
            thumbnail: None,
            duration: None,
            status: QueueStatus::Pending,
            progress: 0,
            error: None,
            output_path: None,
        };

        {
            let mut queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
            queue.insert(id.clone(), item.clone());
        }

        items.push(item);
    }

    // Emit event to notify frontend
    let _ = app.emit("queue-updated", get_queue_items()?);

    Ok(items)
}

/// Get all items in the queue
fn get_queue_items() -> Result<Vec<QueueItem>, String> {
    let queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
    Ok(queue.values().cloned().collect())
}

#[tauri::command]
async fn get_queue() -> Result<Vec<QueueItem>, String> {
    get_queue_items()
}

/// Remove an item from the queue
#[tauri::command]
async fn remove_from_queue(app: tauri::AppHandle, id: String) -> Result<(), String> {
    {
        let mut queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
        queue.remove(&id);
    }

    let _ = app.emit("queue-updated", get_queue_items()?);
    Ok(())
}

/// Clear all completed/failed items from the queue
#[tauri::command]
async fn clear_completed(app: tauri::AppHandle) -> Result<(), String> {
    {
        let mut queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
        queue.retain(|_, item| {
            item.status != QueueStatus::Complete && item.status != QueueStatus::Failed
        });
    }

    let _ = app.emit("queue-updated", get_queue_items()?);
    Ok(())
}

/// Update a queue item's metadata after fetching
fn update_queue_item_metadata(id: &str, title: String, artist: String, thumbnail: Option<String>, duration: Option<u64>) -> Result<(), String> {
    let mut queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
    if let Some(item) = queue.get_mut(id) {
        item.title = Some(title);
        item.artist = Some(artist);
        item.thumbnail = thumbnail;
        item.duration = duration;
        item.status = QueueStatus::Ready;
    }
    Ok(())
}

/// Update a queue item's status
fn update_queue_item_status(id: &str, status: QueueStatus, progress: u8, error: Option<String>, output_path: Option<String>) -> Result<(), String> {
    let mut queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
    if let Some(item) = queue.get_mut(id) {
        item.status = status;
        item.progress = progress;
        item.error = error;
        if output_path.is_some() {
            item.output_path = output_path;
        }
    }
    Ok(())
}

/// Process a single queue item (fetch metadata + download)
#[tauri::command]
async fn process_queue_item(
    app: tauri::AppHandle,
    id: String,
    output_dir: String,
) -> Result<DownloadResult, String> {
    // Get the queue item
    let item = {
        let queue = DOWNLOAD_QUEUE.lock().map_err(|e| format!("Queue lock error: {}", e))?;
        queue.get(&id).cloned().ok_or("Item not found in queue")?
    };

    // Update status to fetching
    update_queue_item_status(&id, QueueStatus::Fetching, 0, None, None)?;
    emit_queue_progress(&app, &id, QueueStatus::Fetching, 0, "Fetching metadata...");
    let _ = app.emit("queue-updated", get_queue_items()?);

    // Fetch metadata if not already fetched
    let (title, artist, thumbnail) = if item.title.is_some() && item.artist.is_some() {
        (item.title.unwrap(), item.artist.unwrap(), item.thumbnail)
    } else {
        let metadata = ytdlp_extract_info(&item.url).map_err(|e| {
            let _ = update_queue_item_status(&id, QueueStatus::Failed, 0, Some(e.clone()), None);
            let _ = app.emit("queue-updated", get_queue_items().unwrap_or_default());
            e
        })?;

        let (artist, title) = if metadata.artist.is_some() && metadata.track.is_some() {
            (metadata.artist.clone().unwrap(), metadata.track.clone().unwrap())
        } else {
            let channel = metadata.channel.clone()
                .or(metadata.uploader.clone())
                .unwrap_or_else(|| "Unknown Artist".to_string());
            parse_artist_title(&metadata.title, &channel)
        };

        update_queue_item_metadata(&id, title.clone(), artist.clone(), metadata.thumbnail.clone(), metadata.duration.map(|d| d as u64))?;
        let _ = app.emit("queue-updated", get_queue_items()?);

        (title, artist, metadata.thumbnail)
    };

    // Update status to downloading
    update_queue_item_status(&id, QueueStatus::Downloading, 10, None, None)?;
    emit_queue_progress(&app, &id, QueueStatus::Downloading, 10, "Starting download...");
    let _ = app.emit("queue-updated", get_queue_items()?);

    // Download the audio (using the existing download logic)
    let result = download_audio_internal(&app, &id, &item.url, &title, &artist, &output_dir, thumbnail.as_deref()).await;

    match result {
        Ok(download_result) => {
            update_queue_item_status(&id, QueueStatus::Complete, 100, None, Some(download_result.path.clone()))?;
            emit_queue_progress(&app, &id, QueueStatus::Complete, 100, "Complete!");
            let _ = app.emit("queue-updated", get_queue_items()?);
            Ok(download_result)
        }
        Err(e) => {
            update_queue_item_status(&id, QueueStatus::Failed, 0, Some(e.clone()), None)?;
            emit_queue_progress(&app, &id, QueueStatus::Failed, 0, &e);
            let _ = app.emit("queue-updated", get_queue_items()?);
            Err(e)
        }
    }
}

/// Internal download function for queue processing
async fn download_audio_internal(
    app: &tauri::AppHandle,
    queue_id: &str,
    url: &str,
    title: &str,
    artist: &str,
    output_dir: &str,
    thumbnail_url: Option<&str>,
) -> Result<DownloadResult, String> {
    // Sanitize filename
    let safe_title: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    let safe_artist: String = artist
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    // Final MP3 path
    let final_filename = format!("{} - {}.mp3", safe_artist, safe_title);
    let final_path = PathBuf::from(output_dir).join(&final_filename);

    // If the file already exists, return it immediately
    if final_path.exists() {
        return Ok(DownloadResult {
            success: true,
            path: final_path.to_string_lossy().to_string(),
        });
    }

    // Check for cancellation
    if is_cancelled() {
        return Err(DownloadError::Cancelled.to_string());
    }

    // Stage 1: Downloading (10-60%)
    emit_queue_progress(app, queue_id, QueueStatus::Downloading, 20, "Fetching audio stream...");
    let _ = update_queue_item_status(queue_id, QueueStatus::Downloading, 20, None, None);

    // Download as m4a first
    let temp_filename = format!("{} - {}.m4a", safe_artist, safe_title);
    let temp_path = PathBuf::from(output_dir).join(&temp_filename);

    ytdlp_download(url, temp_path.to_str().unwrap())
        .map_err(|e| {
            if e.contains("URL") || e.contains("Unsupported") {
                DownloadError::InvalidUrl(e).to_string()
            } else if e.contains("network") || e.contains("connection") || e.contains("timeout") {
                DownloadError::NetworkError(e).to_string()
            } else {
                DownloadError::Unknown(e).to_string()
            }
        })?;

    emit_queue_progress(app, queue_id, QueueStatus::Downloading, 60, "Download complete");
    let _ = update_queue_item_status(queue_id, QueueStatus::Downloading, 60, None, None);

    // Check for cancellation
    if is_cancelled() {
        let _ = std::fs::remove_file(&temp_path);
        return Err(DownloadError::Cancelled.to_string());
    }

    // Find the downloaded file
    let actual_temp_path = if temp_path.exists() {
        temp_path.clone()
    } else {
        let with_ext = PathBuf::from(format!("{}.m4a", temp_path.display()));
        if with_ext.exists() {
            with_ext
        } else {
            let entries: Vec<_> = std::fs::read_dir(output_dir)
                .map_err(|e| DownloadError::FileError(format!("Failed to read output dir: {}", e)).to_string())?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(&format!("{} - {}", safe_artist, safe_title))
                })
                .collect();

            entries.first()
                .map(|e| e.path())
                .ok_or_else(|| DownloadError::FileError("Downloaded file not found".to_string()).to_string())?
        }
    };

    // Stage 2: Converting (60-90%)
    emit_queue_progress(app, queue_id, QueueStatus::Downloading, 65, "Converting to MP3...");
    let _ = update_queue_item_status(queue_id, QueueStatus::Downloading, 65, None, None);

    convert_to_mp3(&actual_temp_path, &final_path, 192)
        .map_err(|e| DownloadError::ConversionError(e).to_string())?;

    emit_queue_progress(app, queue_id, QueueStatus::Downloading, 90, "Conversion complete");
    let _ = update_queue_item_status(queue_id, QueueStatus::Downloading, 90, None, None);

    // Stage 3: Tagging (90-100%)
    emit_queue_progress(app, queue_id, QueueStatus::Downloading, 92, "Writing metadata...");

    let mut tag = id3::Tag::new();
    tag.set_title(title);
    tag.set_artist(artist);

    // Embed album art if thumbnail URL provided
    if let Some(thumb_url) = thumbnail_url {
        emit_queue_progress(app, queue_id, QueueStatus::Downloading, 94, "Downloading album art...");
        if let Ok(image_data) = download_thumbnail(thumb_url) {
            let mime_type = if thumb_url.contains(".png") { "image/png" } else { "image/jpeg" };
            let picture = id3::frame::Picture {
                mime_type: mime_type.to_string(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: "Cover".to_string(),
                data: image_data,
            };
            tag.add_frame(picture);
        }
    }

    tag.write_to_path(&final_path, id3::Version::Id3v24)
        .map_err(|e| DownloadError::FileError(format!("Failed to write ID3 tags: {}", e)).to_string())?;

    emit_queue_progress(app, queue_id, QueueStatus::Downloading, 98, "Cleaning up...");

    // Remove the temporary m4a file
    let _ = std::fs::remove_file(&actual_temp_path);

    // Save to download history
    let final_path_str = final_path.to_string_lossy().to_string();
    if let Err(e) = db::save_download(
        url,
        title,
        artist,
        thumbnail_url,
        None, // duration could be passed in but kept simple
        &final_path_str,
    ) {
        eprintln!("Warning: Failed to save to history: {}", e);
    }

    Ok(DownloadResult {
        success: true,
        path: final_path_str,
    })
}

/// Download thumbnail image from URL
fn download_thumbnail(url: &str) -> Result<Vec<u8>, String> {
    let response = reqwest::blocking::get(url)
        .map_err(|e| format!("Failed to download thumbnail: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Failed to download thumbnail: HTTP {}", response.status()));
    }

    response.bytes()
        .map(|b| b.to_vec())
        .map_err(|e| format!("Failed to read thumbnail bytes: {}", e))
}

#[tauri::command]
async fn download_audio(
    app: tauri::AppHandle,
    url: String,
    title: String,
    artist: String,
    output_dir: String,
    thumbnail_url: Option<String>,
) -> Result<DownloadResult, String> {
    // Reset cancellation flag at start
    CANCEL_DOWNLOAD.store(false, Ordering::SeqCst);

    // Sanitize filename
    let safe_title: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    let safe_artist: String = artist
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    // Final MP3 path
    let final_filename = format!("{} - {}.mp3", safe_artist, safe_title);
    let final_path = PathBuf::from(&output_dir).join(&final_filename);

    // If the file already exists, return it immediately (handle duplicates gracefully)
    if final_path.exists() {
        emit_progress(&app, "complete", 100, "File already exists");
        return Ok(DownloadResult {
            success: true,
            path: final_path.to_string_lossy().to_string(),
        });
    }

    // Check for cancellation
    if is_cancelled() {
        return Err(DownloadError::Cancelled.to_string());
    }

    // Stage 1: Downloading (0-60%)
    emit_progress(&app, "downloading", 0, "Starting download...");

    // Download as m4a first (best audio quality)
    let temp_filename = format!("{} - {}.m4a", safe_artist, safe_title);
    let temp_path = PathBuf::from(&output_dir).join(&temp_filename);

    emit_progress(&app, "downloading", 10, "Fetching audio stream...");

    // Download audio using yt-dlp via PyO3
    ytdlp_download(&url, temp_path.to_str().unwrap())
        .map_err(|e| {
            if e.contains("URL") || e.contains("Unsupported") {
                DownloadError::InvalidUrl(e).to_string()
            } else if e.contains("network") || e.contains("connection") || e.contains("timeout") {
                DownloadError::NetworkError(e).to_string()
            } else {
                DownloadError::Unknown(e).to_string()
            }
        })?;

    emit_progress(&app, "downloading", 60, "Download complete");

    // Check for cancellation
    if is_cancelled() {
        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);
        return Err(DownloadError::Cancelled.to_string());
    }

    // Find the downloaded file
    let actual_temp_path = if temp_path.exists() {
        temp_path.clone()
    } else {
        let with_ext = PathBuf::from(format!("{}.m4a", temp_path.display()));
        if with_ext.exists() {
            with_ext
        } else {
            // Look for any file matching the pattern
            let entries: Vec<_> = std::fs::read_dir(&output_dir)
                .map_err(|e| DownloadError::FileError(format!("Failed to read output dir: {}", e)).to_string())?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(&format!("{} - {}", safe_artist, safe_title))
                })
                .collect();

            entries.first()
                .map(|e| e.path())
                .ok_or_else(|| DownloadError::FileError("Downloaded file not found".to_string()).to_string())?
        }
    };

    // Stage 2: Converting (60-90%)
    emit_progress(&app, "converting", 65, "Converting to MP3...");

    // Check for cancellation
    if is_cancelled() {
        let _ = std::fs::remove_file(&actual_temp_path);
        return Err(DownloadError::Cancelled.to_string());
    }

    convert_to_mp3(&actual_temp_path, &final_path, 192)
        .map_err(|e| DownloadError::ConversionError(e).to_string())?;

    emit_progress(&app, "converting", 90, "Conversion complete");

    // Stage 3: Tagging (90-100%)
    emit_progress(&app, "tagging", 92, "Writing metadata...");

    // Write ID3 tags
    let mut tag = id3::Tag::new();
    tag.set_title(&title);
    tag.set_artist(&artist);

    // Embed album art if thumbnail URL provided
    if let Some(ref thumb_url) = thumbnail_url {
        emit_progress(&app, "tagging", 94, "Downloading album art...");
        match download_thumbnail(thumb_url) {
            Ok(image_data) => {
                // Determine MIME type based on URL or default to JPEG
                let mime_type = if thumb_url.contains(".png") {
                    "image/png"
                } else {
                    "image/jpeg"
                };

                let picture = id3::frame::Picture {
                    mime_type: mime_type.to_string(),
                    picture_type: id3::frame::PictureType::CoverFront,
                    description: "Cover".to_string(),
                    data: image_data,
                };
                tag.add_frame(picture);
                emit_progress(&app, "tagging", 96, "Album art embedded");
            }
            Err(e) => {
                // Log but don't fail - album art is optional
                eprintln!("Warning: Failed to embed album art: {}", e);
            }
        }
    }

    tag.write_to_path(&final_path, id3::Version::Id3v24)
        .map_err(|e| DownloadError::FileError(format!("Failed to write ID3 tags: {}", e)).to_string())?;

    emit_progress(&app, "tagging", 98, "Cleaning up...");

    // Remove the temporary m4a file
    let _ = std::fs::remove_file(&actual_temp_path);

    // Save to download history
    let final_path_str = final_path.to_string_lossy().to_string();
    if let Err(e) = db::save_download(
        &url,
        &title,
        &artist,
        thumbnail_url.as_deref(),
        None, // duration not available here
        &final_path_str,
    ) {
        eprintln!("Warning: Failed to save to history: {}", e);
    }

    emit_progress(&app, "complete", 100, "Done!");

    Ok(DownloadResult {
        success: true,
        path: final_path_str,
    })
}

#[tauri::command]
async fn get_settings() -> Result<Settings, String> {
    let path = get_settings_path();

    if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str(&content).map_err(|e| e.to_string())
    } else {
        Ok(Settings::default())
    }
}

#[tauri::command]
async fn save_settings(settings: Settings) -> Result<(), String> {
    let path = get_settings_path();

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let content = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
async fn get_default_download_dir() -> Result<String, String> {
    let dir = dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .ok_or("Could not determine download directory")?;

    Ok(dir.to_string_lossy().to_string())
}

// History commands
#[tauri::command]
async fn get_download_history(limit: Option<u32>) -> Result<Vec<DownloadHistoryEntry>, String> {
    db::get_history(limit.unwrap_or(50))
        .map_err(|e| format!("Failed to get history: {}", e))
}

#[tauri::command]
async fn search_download_history(query: String, limit: Option<u32>) -> Result<Vec<DownloadHistoryEntry>, String> {
    db::search_history(&query, limit.unwrap_or(50))
        .map_err(|e| format!("Failed to search history: {}", e))
}

#[tauri::command]
async fn delete_history_entry(id: i64) -> Result<(), String> {
    db::delete_history_entry(id)
        .map_err(|e| format!("Failed to delete history entry: {}", e))
}

#[tauri::command]
async fn clear_download_history() -> Result<(), String> {
    db::clear_history()
        .map_err(|e| format!("Failed to clear history: {}", e))
}

/// Analyze audio file for BPM and Key
#[tauri::command]
async fn analyze_audio_file(file_path: String) -> Result<AudioAnalysis, String> {
    let path = std::path::PathBuf::from(&file_path);
    audio_analysis::analyze_audio(&path)
}

/// Generate waveform data from an audio file
#[tauri::command]
async fn generate_waveform(file_path: String, num_points: Option<usize>) -> Result<Vec<WaveformPoint>, String> {
    let path = std::path::PathBuf::from(&file_path);
    let points = num_points.unwrap_or(200);
    waveform::generate_waveform(&path, points)
}

/// Download audio with trim/clip support
#[tauri::command]
async fn download_audio_trimmed(
    app: tauri::AppHandle,
    url: String,
    title: String,
    artist: String,
    output_dir: String,
    thumbnail_url: Option<String>,
    start_time: f64,
    end_time: f64,
) -> Result<DownloadResult, String> {
    // Reset cancellation flag at start
    CANCEL_DOWNLOAD.store(false, Ordering::SeqCst);

    // Sanitize filename with time range indicator
    let safe_title: String = title
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    let safe_artist: String = artist
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect();

    // Include time range in filename to distinguish clips
    let time_suffix = format!("_{:.0}-{:.0}s", start_time, end_time);
    let final_filename = format!("{} - {}{}.mp3", safe_artist, safe_title, time_suffix);
    let final_path = PathBuf::from(&output_dir).join(&final_filename);

    // If the file already exists, return it immediately
    if final_path.exists() {
        emit_progress(&app, "complete", 100, "File already exists");
        return Ok(DownloadResult {
            success: true,
            path: final_path.to_string_lossy().to_string(),
        });
    }

    // Check for cancellation
    if is_cancelled() {
        return Err(DownloadError::Cancelled.to_string());
    }

    // Stage 1: Downloading (0-50%)
    emit_progress(&app, "downloading", 0, "Starting download...");

    // Download as m4a first
    let temp_filename = format!("{} - {}_temp.m4a", safe_artist, safe_title);
    let temp_path = PathBuf::from(&output_dir).join(&temp_filename);

    emit_progress(&app, "downloading", 10, "Fetching audio stream...");

    ytdlp_download(&url, temp_path.to_str().unwrap())
        .map_err(|e| {
            if e.contains("URL") || e.contains("Unsupported") {
                DownloadError::InvalidUrl(e).to_string()
            } else if e.contains("network") || e.contains("connection") || e.contains("timeout") {
                DownloadError::NetworkError(e).to_string()
            } else {
                DownloadError::Unknown(e).to_string()
            }
        })?;

    emit_progress(&app, "downloading", 50, "Download complete");

    // Check for cancellation
    if is_cancelled() {
        let _ = std::fs::remove_file(&temp_path);
        return Err(DownloadError::Cancelled.to_string());
    }

    // Find the downloaded file
    let actual_temp_path = if temp_path.exists() {
        temp_path.clone()
    } else {
        let with_ext = PathBuf::from(format!("{}.m4a", temp_path.display()));
        if with_ext.exists() {
            with_ext
        } else {
            let entries: Vec<_> = std::fs::read_dir(&output_dir)
                .map_err(|e| DownloadError::FileError(format!("Failed to read output dir: {}", e)).to_string())?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .contains(&format!("{} - {}_temp", safe_artist, safe_title))
                })
                .collect();

            entries.first()
                .map(|e| e.path())
                .ok_or_else(|| DownloadError::FileError("Downloaded file not found".to_string()).to_string())?
        }
    };

    // Stage 2: Converting with trim (50-90%)
    emit_progress(&app, "converting", 55, "Converting and trimming to MP3...");

    convert_to_mp3_trimmed(&actual_temp_path, &final_path, 192, start_time, end_time)
        .map_err(|e| DownloadError::ConversionError(e).to_string())?;

    emit_progress(&app, "converting", 90, "Conversion complete");

    // Stage 3: Tagging (90-100%)
    emit_progress(&app, "tagging", 92, "Writing metadata...");

    let mut tag = id3::Tag::new();
    tag.set_title(&title);
    tag.set_artist(&artist);

    // Embed album art if thumbnail URL provided
    if let Some(ref thumb_url) = thumbnail_url {
        emit_progress(&app, "tagging", 94, "Downloading album art...");
        if let Ok(image_data) = download_thumbnail(thumb_url) {
            let mime_type = if thumb_url.contains(".png") { "image/png" } else { "image/jpeg" };
            let picture = id3::frame::Picture {
                mime_type: mime_type.to_string(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: "Cover".to_string(),
                data: image_data,
            };
            tag.add_frame(picture);
        }
    }

    tag.write_to_path(&final_path, id3::Version::Id3v24)
        .map_err(|e| DownloadError::FileError(format!("Failed to write ID3 tags: {}", e)).to_string())?;

    emit_progress(&app, "tagging", 98, "Cleaning up...");

    // Remove the temporary m4a file
    let _ = std::fs::remove_file(&actual_temp_path);

    // Save to download history
    let final_path_str = final_path.to_string_lossy().to_string();
    let trimmed_title = format!("{} ({}s-{}s)", title, start_time as i32, end_time as i32);
    if let Err(e) = db::save_download(
        &url,
        &trimmed_title,
        &artist,
        thumbnail_url.as_deref(),
        Some((end_time - start_time) as u64),
        &final_path_str,
    ) {
        eprintln!("Warning: Failed to save to history: {}", e);
    }

    emit_progress(&app, "complete", 100, "Done!");

    Ok(DownloadResult {
        success: true,
        path: final_path_str,
    })
}

/// Get current yt-dlp version
#[tauri::command]
async fn get_ytdlp_version() -> Result<String, String> {
    init_python_env();
    Python::with_gil(|py| {
        let yt_dlp = py.import("yt_dlp").map_err(|e| format!("Failed to import yt_dlp: {}", e))?;
        let version = yt_dlp.getattr("version")
            .and_then(|v| v.getattr("__version__"))
            .map_err(|_| "Failed to get version".to_string())?;
        let version_str: String = version.extract().map_err(|e| format!("Failed to extract version: {}", e))?;
        Ok(version_str)
    })
}

/// Check for yt-dlp updates by comparing with PyPI
#[tauri::command]
async fn check_ytdlp_update() -> Result<Option<String>, String> {
    // Get current version
    let current_version = get_ytdlp_version().await?;

    // Fetch latest version from PyPI
    let client = reqwest::blocking::Client::new();
    let response = client.get("https://pypi.org/pypi/yt-dlp/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .map_err(|e| format!("Failed to check for updates: {}", e))?;

    if !response.status().is_success() {
        return Err("Failed to fetch version info from PyPI".to_string());
    }

    let json: serde_json::Value = response.json()
        .map_err(|e| format!("Failed to parse PyPI response: {}", e))?;

    let latest_version = json["info"]["version"]
        .as_str()
        .ok_or("Failed to get latest version from PyPI")?
        .to_string();

    // Compare versions
    if latest_version != current_version {
        Ok(Some(latest_version))
    } else {
        Ok(None)
    }
}

/// Update yt-dlp to latest version
#[tauri::command]
async fn update_ytdlp(app: tauri::AppHandle) -> Result<String, String> {
    use std::process::Command;

    // Emit progress
    let _ = app.emit("ytdlp-update-progress", "Starting update...");

    // Run pip install --upgrade yt-dlp
    let output = Command::new("pip3")
        .args(["install", "--upgrade", "yt-dlp"])
        .output()
        .map_err(|e| format!("Failed to run pip: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Update failed: {}", stderr));
    }

    let _ = app.emit("ytdlp-update-progress", "Update complete!");

    // Get the new version
    get_ytdlp_version().await
}

/// Convert M4A/AAC to MP3 with time trimming
fn convert_to_mp3_trimmed(input_path: &Path, output_path: &Path, bitrate_kbps: u32, start_time: f64, end_time: f64) -> Result<(), String> {
    let file = File::open(input_path).map_err(|e| format!("Failed to open input file: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = input_path.extension() {
        hint.with_extension(ext.to_str().unwrap_or("m4a"));
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| format!("Failed to probe audio format: {}", e))?;

    let mut format = probed.format;

    let track = format.tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or("No audio track found")?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.ok_or("Unknown sample rate")?;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Failed to create decoder: {}", e))?;

    // Calculate sample positions for trim
    let start_sample = (start_time * sample_rate as f64) as u64;
    let end_sample = (end_time * sample_rate as f64) as u64;

    // Create MP3 encoder
    let mut mp3_encoder = Builder::new().ok_or("Failed to create MP3 encoder")?;
    mp3_encoder.set_num_channels(channels as u8).map_err(|e| format!("Failed to set channels: {:?}", e))?;
    mp3_encoder.set_sample_rate(sample_rate).map_err(|e| format!("Failed to set sample rate: {:?}", e))?;
    mp3_encoder.set_brate(match bitrate_kbps {
        128 => mp3lame_encoder::Bitrate::Kbps128,
        192 => mp3lame_encoder::Bitrate::Kbps192,
        256 => mp3lame_encoder::Bitrate::Kbps256,
        320 => mp3lame_encoder::Bitrate::Kbps320,
        _ => mp3lame_encoder::Bitrate::Kbps192,
    }).map_err(|e| format!("Failed to set bitrate: {:?}", e))?;
    mp3_encoder.set_quality(mp3lame_encoder::Quality::Best).map_err(|e| format!("Failed to set quality: {:?}", e))?;

    let mut mp3_encoder = mp3_encoder.build().map_err(|e| format!("Failed to build MP3 encoder: {:?}", e))?;

    let mut output_file = File::create(output_path).map_err(|e| format!("Failed to create output file: {}", e))?;

    let mut sample_buf: Option<SampleBuffer<i16>> = None;
    let mut current_sample: u64 = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("Failed to read packet: {}", e)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("Failed to decode: {}", e)),
        };

        if sample_buf.is_none() {
            let spec = *decoded.spec();
            let duration = decoded.capacity() as u64;
            sample_buf = Some(SampleBuffer::new(duration, spec));
        }

        if let Some(ref mut buf) = sample_buf {
            buf.copy_interleaved_ref(decoded);
            let samples = buf.samples();
            let num_frames = samples.len() / channels;

            // Calculate which samples from this packet fall within our trim range
            let packet_start = current_sample;
            let packet_end = current_sample + num_frames as u64;

            if packet_end > start_sample && packet_start < end_sample {
                // Some or all of this packet falls within our range
                let trim_start = if packet_start < start_sample {
                    ((start_sample - packet_start) as usize) * channels
                } else {
                    0
                };

                let trim_end = if packet_end > end_sample {
                    samples.len() - ((packet_end - end_sample) as usize) * channels
                } else {
                    samples.len()
                };

                if trim_start < trim_end {
                    let trimmed_samples = &samples[trim_start..trim_end];

                    let input = InterleavedPcm(trimmed_samples);
                    let buf_size = mp3lame_encoder::max_required_buffer_size(trimmed_samples.len());
                    let mut mp3_out: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); buf_size];
                    let encoded_size = mp3_encoder.encode(input, &mut mp3_out)
                        .map_err(|e| format!("Failed to encode MP3: {:?}", e))?;

                    let mp3_bytes: &[u8] = unsafe { std::slice::from_raw_parts(mp3_out.as_ptr() as *const u8, encoded_size) };
                    output_file.write_all(mp3_bytes).map_err(|e| format!("Failed to write MP3 data: {}", e))?;
                }
            }

            current_sample = packet_end;

            // Stop if we've passed the end time
            if current_sample >= end_sample {
                break;
            }
        }
    }

    // Flush the encoder
    let mut mp3_out: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); 7200];
    let encoded_size = mp3_encoder.flush::<FlushNoGap>(&mut mp3_out)
        .map_err(|e| format!("Failed to flush MP3 encoder: {:?}", e))?;
    let mp3_bytes: &[u8] = unsafe { std::slice::from_raw_parts(mp3_out.as_ptr() as *const u8, encoded_size) };
    output_file.write_all(mp3_bytes).map_err(|e| format!("Failed to write final MP3 data: {}", e))?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            fetch_metadata,
            download_audio,
            cancel_download,
            get_settings,
            save_settings,
            get_default_download_dir,
            add_to_queue,
            add_urls_to_queue,
            get_queue,
            remove_from_queue,
            clear_completed,
            process_queue_item,
            get_download_history,
            search_download_history,
            delete_history_entry,
            clear_download_history,
            analyze_audio_file,
            generate_waveform,
            download_audio_trimmed,
            get_ytdlp_version,
            check_ytdlp_update,
            update_ytdlp,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
