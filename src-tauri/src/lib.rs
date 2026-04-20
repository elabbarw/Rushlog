mod ffmpeg;
mod gemini;
mod index;
mod queue;
mod settings;
mod sidecar;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tauri::State;
use tokio_util::sync::CancellationToken;
use tracing::info;

struct AppState {
    processing: Arc<AtomicBool>,
    cancel_token: std::sync::Mutex<CancellationToken>,
}

// ── Tauri Commands ──

#[tauri::command]
fn check_ffmpeg() -> Result<String, String> {
    ffmpeg::check_available()
}

#[tauri::command]
fn get_settings() -> SettingsView {
    let s = settings::load();
    SettingsView {
        api_key_set: !s.api_key.is_empty(),
        // Only show last 4 chars — prefix "AIza" is already known
        api_key_preview: if s.api_key.len() > 4 {
            format!("********{}", &s.api_key[s.api_key.len() - 4..])
        } else if !s.api_key.is_empty() {
            "*".repeat(s.api_key.len())
        } else {
            String::new()
        },
        model: s.model,
        media_resolution: s.media_resolution,
        concurrency: s.concurrency,
        overwrite_policy: s.overwrite_policy,
        custom_prompt: s.custom_prompt,
        split_threshold_gb: s.split_threshold_gb,
        segment_duration_min: s.segment_duration_min,
        include_transcript: s.include_transcript,
    }
}

#[tauri::command]
fn save_settings(mut new_settings: settings::Settings) -> Result<(), String> {
    if new_settings.api_key.is_empty() {
        let existing = settings::load();
        new_settings.api_key = existing.api_key;
    }
    settings::validate(&new_settings)?;
    settings::save(&new_settings)
}

/// Scan a folder for video files. Single-pass: cleans stale segments and collects clips.
#[tauri::command]
fn scan_folder(folder_path: String) -> Result<ScanResult, String> {
    let folder = Path::new(&folder_path);
    if !folder.is_dir() {
        return Err("Not a valid directory".into());
    }

    let entries =
        std::fs::read_dir(folder).map_err(|e| format!("Cannot read directory: {e}"))?;

    let mut clips = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();

        // Clean stale segment directories (symlink-safe: check metadata, not path)
        if let Some(name_str) = name.to_str() {
            if name_str.starts_with(".rush-segments-") {
                // Use symlink_metadata to detect symlinks — refuse to delete symlinks
                if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.is_dir() && !meta.file_type().is_symlink() {
                        std::fs::remove_dir_all(&path).ok();
                    }
                }
                continue;
            }
        }

        if !path.is_file() || !ffmpeg::is_video_file(&path) {
            continue;
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        let sidecar = sidecar::sidecar_path_for(&path);
        let status = if sidecar.exists() {
            "done".to_string()
        } else {
            "queued".to_string()
        };

        clips.push(ClipInfo {
            path: path.to_string_lossy().to_string(),
            filename,
            file_size,
            status,
        });
    }

    clips.sort_by(|a, b| a.filename.cmp(&b.filename));

    let queued = clips.iter().filter(|c| c.status == "queued").count();
    let already_done = clips.iter().filter(|c| c.status == "done").count();

    Ok(ScanResult {
        folder_path,
        total_clips: clips.len(),
        queued,
        already_done,
        clips,
    })
}

#[tauri::command]
async fn start_processing(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    folder_path: String,
) -> Result<(), String> {
    if state
        .processing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("Already processing".into());
    }

    let settings = settings::load();
    if settings.api_key.is_empty() {
        state.processing.store(false, Ordering::SeqCst);
        return Err("No API key configured. Add your Gemini API key in Settings.".into());
    }

    // Create a fresh cancellation token for this run
    let cancel_token = CancellationToken::new();
    {
        let mut stored = state.cancel_token.lock().unwrap();
        *stored = cancel_token.clone();
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let app_handle = app.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = app_handle.emit("processing-progress", &event);
        }
    });

    let processing_flag = state.processing.clone();
    tokio::spawn(async move {
        let result =
            queue::process_folder(folder_path, settings, tx, cancel_token).await;
        if let Err(e) = &result {
            tracing::error!("Processing failed: {}", e);
        }
        processing_flag.store(false, Ordering::SeqCst);
        let _ = app.emit("processing-complete", &result.is_ok());
    });

    Ok(())
}

#[tauri::command]
fn cancel_processing(state: State<'_, AppState>) -> Result<(), String> {
    let token = state.cancel_token.lock().unwrap();
    token.cancel();
    info!("Processing cancellation requested");
    Ok(())
}

#[tauri::command]
fn regenerate_index(folder_path: String) -> Result<String, String> {
    let path = index::generate_index(&folder_path)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn read_index_file(folder_path: String) -> Result<String, String> {
    let folder = Path::new(&folder_path)
        .canonicalize()
        .map_err(|e| format!("Invalid path: {e}"))?;

    for name in &["INDEX.md", "INDEX-rush.md"] {
        let index_path = folder.join(name);
        if let Ok(resolved) = index_path.canonicalize() {
            if resolved.starts_with(&folder) {
                return std::fs::read_to_string(&resolved)
                    .map_err(|e| format!("Failed to read index: {e}"));
            }
        }
    }
    Err("No index file found".into())
}

#[tauri::command]
fn get_folder_stats(folder_path: String) -> Result<FolderStats, String> {
    let folder = Path::new(&folder_path);
    let entries = std::fs::read_dir(folder).map_err(|e| format!("Cannot read: {e}"))?;

    let mut total = 0u64;
    let mut done = 0u64;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && ffmpeg::is_video_file(&path) {
            total += 1;
            if sidecar::sidecar_path_for(&path).exists() {
                done += 1;
            }
        }
    }

    Ok(FolderStats {
        total,
        done,
        errors: 0,
        queued: total - done,
    })
}

// ── Types ──

#[derive(Debug, Clone, serde::Serialize)]
struct ClipInfo {
    path: String,
    filename: String,
    file_size: u64,
    status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ScanResult {
    folder_path: String,
    total_clips: usize,
    queued: usize,
    already_done: usize,
    clips: Vec<ClipInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct FolderStats {
    total: u64,
    done: u64,
    errors: u64,
    queued: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SettingsView {
    api_key_set: bool,
    api_key_preview: String,
    model: String,
    media_resolution: String,
    concurrency: u32,
    overwrite_policy: String,
    custom_prompt: String,
    split_threshold_gb: f64,
    segment_duration_min: u64,
    include_transcript: bool,
}

// ── Entry point ──

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Rushlog starting up");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            processing: Arc::new(AtomicBool::new(false)),
            cancel_token: std::sync::Mutex::new(CancellationToken::new()),
        })
        .invoke_handler(tauri::generate_handler![
            check_ffmpeg,
            get_settings,
            save_settings,
            scan_folder,
            start_processing,
            cancel_processing,
            get_folder_stats,
            regenerate_index,
            read_index_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
