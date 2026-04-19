use crate::ffmpeg;
use crate::gemini;
use crate::sidecar::{self, SegmentDescription};
use crate::settings::Settings;
use reqwest::Client;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

type ProgressTx = tokio::sync::mpsc::Sender<ProgressEvent>;

pub async fn process_folder(
    folder_path: String,
    settings: Settings,
    progress_tx: ProgressTx,
    cancel_token: CancellationToken,
) -> Result<(), String> {
    let folder = Path::new(&folder_path);
    let entries = std::fs::read_dir(folder).map_err(|e| format!("Cannot read directory: {e}"))?;
    let mut clips: Vec<(String, String)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !ffmpeg::is_video_file(&path) {
            continue;
        }
        let sidecar = sidecar::sidecar_path_for(&path);
        if sidecar.exists() && settings.overwrite_policy == "skip" {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        clips.push((path.to_string_lossy().to_string(), filename));
    }

    clips.sort_by(|a, b| a.1.cmp(&b.1));

    if clips.is_empty() {
        let _ = progress_tx.send(ProgressEvent::Complete).await;
        return Ok(());
    }

    let concurrency = settings.concurrency.max(1).min(10) as usize;
    let semaphore = Arc::new(Semaphore::new(concurrency));

    // Configure HTTP client with timeouts
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600)) // 10 min max per request
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let settings = Arc::new(settings);
    let mut handles = vec![];

    for (clip_path, filename) in clips {
        // Check cancellation before starting each clip
        if cancel_token.is_cancelled() {
            info!("Processing cancelled by user");
            break;
        }

        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| e.to_string())?;
        let client = client.clone();
        let settings = settings.clone();
        let progress_tx = progress_tx.clone();
        let cancel_token = cancel_token.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit;

            if cancel_token.is_cancelled() {
                return Ok(());
            }

            let result =
                process_single_clip(&client, &settings, &clip_path, &filename, &progress_tx, &cancel_token)
                    .await;

            match &result {
                Ok(_) => {
                    let _ = progress_tx
                        .send(ProgressEvent::ClipDone {
                            filename: filename.clone(),
                        })
                        .await;
                }
                Err(e) if e == "Cancelled" => {
                    // Don't report cancellation as an error
                }
                Err(e) => {
                    let _ = progress_tx
                        .send(ProgressEvent::ClipError {
                            filename: filename.clone(),
                            error: e.clone(),
                        })
                        .await;
                }
            }

            result
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    // Generate index at end of run (unless cancelled)
    if !cancel_token.is_cancelled() {
        let folder_path_clone = folder_path.clone();
        tokio::task::spawn_blocking(move || {
            match crate::index::generate_index(&folder_path_clone) {
                Ok(path) => info!("Index written to {:?}", path),
                Err(e) => error!("Failed to generate index: {}", e),
            }
        })
        .await
        .map_err(|e| e.to_string())?;
    }

    let _ = progress_tx.send(ProgressEvent::Complete).await;
    Ok(())
}

async fn process_single_clip(
    client: &Client,
    settings: &Settings,
    clip_path: &str,
    filename: &str,
    progress_tx: &ProgressTx,
    cancel_token: &CancellationToken,
) -> Result<(), String> {
    let video_path = Path::new(clip_path);

    if !video_path.exists() {
        return Err(format!("File not found: {}", clip_path));
    }

    let threshold_bytes = (settings.split_threshold_gb * 1_073_741_824.0) as u64;
    let needs_split = ffmpeg::needs_splitting(video_path, threshold_bytes);

    let markdown = if needs_split {
        let _ = progress_tx
            .send(ProgressEvent::ClipStatus {
                filename: filename.to_string(),
                status: "splitting".into(),
            })
            .await;

        process_large_clip(client, settings, video_path, filename, progress_tx, cancel_token).await?
    } else {
        let _ = progress_tx
            .send(ProgressEvent::ClipStatus {
                filename: filename.to_string(),
                status: "uploading".into(),
            })
            .await;

        process_normal_clip(client, settings, video_path, filename, progress_tx, cancel_token).await?
    };

    let sidecar_path = sidecar::sidecar_path_for(video_path);
    std::fs::write(&sidecar_path, &markdown)
        .map_err(|e| format!("Failed to write sidecar: {e}"))?;

    info!("Done: {} -> {:?}", filename, sidecar_path);
    Ok(())
}

async fn process_normal_clip(
    client: &Client,
    settings: &Settings,
    video_path: &Path,
    filename: &str,
    progress_tx: &ProgressTx,
    cancel_token: &CancellationToken,
) -> Result<String, String> {
    if cancel_token.is_cancelled() {
        return Err("Cancelled".into());
    }

    let mime = gemini::mime_for_video(video_path);
    let file_uri = gemini::upload_file(client, &settings.api_key, video_path).await?;

    if cancel_token.is_cancelled() {
        return Err("Cancelled".into());
    }

    let _ = progress_tx
        .send(ProgressEvent::ClipStatus {
            filename: filename.to_string(),
            status: "describing".into(),
        })
        .await;

    let desc_result = gemini::describe_clip(
        client,
        &settings.api_key,
        &settings.model,
        &file_uri,
        mime,
        &settings.media_resolution,
        &settings.custom_prompt,
        settings.include_transcript,
    )
    .await?;

    let duration = ffmpeg::probe(video_path)
        .map(|p| p.duration_seconds)
        .unwrap_or(0.0);

    Ok(sidecar::render_sidecar(
        &desc_result.clip,
        filename,
        duration,
        &settings.model,
    ))
}

async fn process_large_clip(
    client: &Client,
    settings: &Settings,
    video_path: &Path,
    filename: &str,
    progress_tx: &ProgressTx,
    cancel_token: &CancellationToken,
) -> Result<String, String> {
    let segment_duration_secs = settings.segment_duration_min * 60;

    let path_owned = video_path.to_path_buf();
    let (segments_dir, plans) = tokio::task::spawn_blocking(move || {
        ffmpeg::plan_segments(&path_owned, segment_duration_secs)
    })
    .await
    .map_err(|e| format!("Plan task panicked: {e}"))??;

    let total = plans.len();
    info!("Planned {} segments for {}", total, filename);

    let mut segment_descriptions: Vec<SegmentDescription> = Vec::new();

    for (i, plan) in plans.iter().enumerate() {
        if cancel_token.is_cancelled() {
            std::fs::remove_dir_all(&segments_dir).ok();
            return Err("Cancelled".into());
        }

        // Phase 1: Compress
        let _ = progress_tx
            .send(ProgressEvent::ClipStatus {
                filename: filename.to_string(),
                status: format!("compressing {}/{}", i + 1, total),
            })
            .await;

        let source = video_path.to_path_buf();
        let plan_clone = plan.clone();
        let segment = match tokio::task::spawn_blocking(move || {
            ffmpeg::encode_segment(&source, &plan_clone)
        })
        .await
        .map_err(|e| format!("Encode panicked: {e}"))?
        {
            Ok(seg) => seg,
            Err(e) => {
                warn!("Failed to encode segment {}: {}. Skipping.", i, e);
                continue;
            }
        };

        if cancel_token.is_cancelled() {
            std::fs::remove_file(&segment.path).ok();
            std::fs::remove_dir_all(&segments_dir).ok();
            return Err("Cancelled".into());
        }

        // Phase 2: Upload
        let _ = progress_tx
            .send(ProgressEvent::ClipStatus {
                filename: filename.to_string(),
                status: format!("uploading {}/{}", i + 1, total),
            })
            .await;

        let file_uri = match gemini::upload_file(client, &settings.api_key, &segment.path).await {
            Ok(uri) => {
                std::fs::remove_file(&segment.path).ok();
                uri
            }
            Err(e) => {
                std::fs::remove_file(&segment.path).ok();
                warn!("Failed to upload segment {}: {}. Skipping.", i, e);
                continue;
            }
        };

        if cancel_token.is_cancelled() {
            std::fs::remove_dir_all(&segments_dir).ok();
            return Err("Cancelled".into());
        }

        // Phase 3: Describe
        let _ = progress_tx
            .send(ProgressEvent::ClipStatus {
                filename: filename.to_string(),
                status: format!("describing {}/{}", i + 1, total),
            })
            .await;

        let segment_prompt = format!(
            "This is segment {} of {} from a longer video. \
             This segment starts at {} and covers {} of footage.{}",
            i + 1,
            total,
            ffmpeg::format_duration(segment.start_seconds),
            ffmpeg::format_duration(segment.duration_seconds),
            if settings.custom_prompt.is_empty() {
                String::new()
            } else {
                format!(" {}", settings.custom_prompt)
            }
        );

        match gemini::describe_clip(
            client,
            &settings.api_key,
            &settings.model,
            &file_uri,
            "video/mp4",
            &settings.media_resolution,
            &segment_prompt,
            settings.include_transcript,
        )
        .await
        {
            Ok(result) => {
                segment_descriptions.push(SegmentDescription {
                    start_seconds: segment.start_seconds,
                    duration_seconds: segment.duration_seconds,
                    description: result.clip,
                    input_tokens: result.input_tokens,
                    output_tokens: result.output_tokens,
                });
            }
            Err(e) => {
                warn!("Failed to describe segment {}: {}. Skipping.", i, e);
            }
        }
    }

    // Clean up segments directory
    std::fs::remove_dir_all(&segments_dir).ok();

    if segment_descriptions.is_empty() {
        return Err("All segments failed to process".into());
    }

    // Warn if partial
    if segment_descriptions.len() < total {
        warn!(
            "{}: only {}/{} segments succeeded — sidecar will be partial",
            filename,
            segment_descriptions.len(),
            total
        );
    }

    let total_duration = ffmpeg::probe(video_path)
        .map(|p| p.duration_seconds)
        .unwrap_or(0.0);

    Ok(sidecar::render_segmented_sidecar(
        &segment_descriptions,
        filename,
        total_duration,
        &settings.model,
        total, // pass total planned segments for partial detection
    ))
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum ProgressEvent {
    ClipDone { filename: String },
    ClipError { filename: String, error: String },
    ClipStatus { filename: String, status: String },
    Complete,
}
