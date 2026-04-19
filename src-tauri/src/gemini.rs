use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing::{debug, info, warn};

const FILES_API_BASE: &str = "https://generativelanguage.googleapis.com/upload/v1beta/files";
const GENERATE_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const FILES_STATUS_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/files";

// ── Retry config ──

const MAX_RETRIES: u32 = 5;
const INITIAL_BACKOFF_MS: u64 = 1000;
const MAX_BACKOFF_MS: u64 = 60_000;
const BACKOFF_MULTIPLIER: f64 = 2.0;

/// Determines if an HTTP status code is retryable.
fn is_retryable(status: reqwest::StatusCode) -> bool {
    matches!(
        status.as_u16(),
        429 | 500 | 502 | 503 | 504
    )
}

/// Calculate backoff duration with jitter for a given attempt.
fn backoff_duration(attempt: u32) -> Duration {
    let base_ms = (INITIAL_BACKOFF_MS as f64) * BACKOFF_MULTIPLIER.powi(attempt as i32);
    let capped_ms = base_ms.min(MAX_BACKOFF_MS as f64);
    // Add ~25% jitter
    let jitter = capped_ms * 0.25 * (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as f64
        / u32::MAX as f64);
    Duration::from_millis((capped_ms + jitter) as u64)
}

/// Extract Retry-After header value in seconds, if present.
fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
}

// ── Types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipDescription {
    pub title: String,
    pub tags: Vec<String>,
    pub description: String,
    #[serde(default)]
    pub transcript: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeResult {
    pub clip: ClipDescription,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}

// ── Upload ──

/// Upload a video file via the Gemini Files API using streaming upload.
pub async fn upload_file(
    client: &Client,
    api_key: &str,
    file_path: &Path,
) -> Result<String, String> {
    let original_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("video.mp4");

    // Sanitize filename — replace spaces and special chars for API compatibility
    let file_name: String = original_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let mime = mime_for_video(file_path);

    let file_size = std::fs::metadata(file_path)
        .map_err(|e| format!("Cannot read file: {e}"))?
        .len();

    info!("Uploading {} ({} bytes)", original_name, file_size);

    let mut last_error = String::new();

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let delay = backoff_duration(attempt);
            warn!(
                "Upload retry {}/{} after {:?}",
                attempt + 1,
                MAX_RETRIES,
                delay
            );
            tokio::time::sleep(delay).await;
        }

        // Open file fresh each attempt (stream is consumed on send)
        let file = File::open(file_path)
            .await
            .map_err(|e| format!("Failed to open file: {e}"))?;
        let stream = FramedRead::new(file, BytesCodec::new());
        let body = reqwest::Body::wrap_stream(stream);

        let metadata = serde_json::json!({
            "file": { "displayName": file_name }
        });

        let form = reqwest::multipart::Form::new()
            .part(
                "metadata",
                reqwest::multipart::Part::text(metadata.to_string())
                    .mime_str("application/json")
                    .unwrap(),
            )
            .part(
                "file",
                reqwest::multipart::Part::stream_with_length(body, file_size)
                    .file_name(file_name.clone())
                    .mime_str(mime)
                    .unwrap(),
            );

        let resp = match client
            .post(FILES_API_BASE)
            .header("x-goog-api-key", api_key)
            .multipart(form)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("Network error: {e}");
                warn!("{}", last_error);
                continue;
            }
        };

        let status = resp.status();

        if status.is_success() {
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse upload response: {e}"))?;

            let uri = body["file"]["uri"]
                .as_str()
                .ok_or_else(|| "No file URI in upload response".to_string())?
                .to_string();

            info!("Upload complete: {}", uri);
            wait_for_file_active(client, api_key, &uri).await?;
            return Ok(uri);
        }

        // Not retryable (e.g. 400 Bad Request) — fail immediately
        if !is_retryable(status) {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            return Err(format!(
                "Upload failed ({}): {}",
                status,
                extract_error_message(&body)
            ));
        }

        // Retryable error
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        last_error = format!("Upload error ({}): {}", status, extract_error_message(&body));
        warn!("{}", last_error);
    }

    Err(format!(
        "Upload failed after {} retries. Last error: {}",
        MAX_RETRIES, last_error
    ))
}

/// Poll file status with exponential backoff until ACTIVE.
async fn wait_for_file_active(
    client: &Client,
    api_key: &str,
    file_uri: &str,
) -> Result<(), String> {
    let file_name = file_uri
        .rsplit('/')
        .next()
        .ok_or_else(|| "Invalid file URI".to_string())?;

    let url = format!("{}/{}", FILES_STATUS_BASE, file_name);

    for attempt in 0..30u32 {
        let delay = backoff_duration(attempt.min(6));

        let resp = match client
            .get(&url)
            .header("x-goog-api-key", api_key)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("File status check failed: {e}");
                tokio::time::sleep(delay).await;
                continue;
            }
        };

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse file status: {e}"))?;

        let state = body["state"].as_str().unwrap_or("UNKNOWN");
        match state {
            "ACTIVE" => return Ok(()),
            "FAILED" => return Err("File processing failed on Gemini's side".into()),
            _ => {
                if attempt % 5 == 0 {
                    info!("File state: {}, waiting...", state);
                }
                tokio::time::sleep(delay).await;
            }
        }
    }

    Err("Timeout waiting for file to become active".into())
}

// ── Describe ──

/// Describe a video clip using Gemini's GenerateContent API with enforced structured output.
pub async fn describe_clip(
    client: &Client,
    api_key: &str,
    model: &str,
    file_uri: &str,
    file_mime: &str,
    media_resolution: &str,
    custom_prompt: &str,
    include_transcript: bool,
) -> Result<DescribeResult, String> {
    let url = format!("{}/{}:generateContent", GENERATE_API_BASE, model);

    let transcript_instruction = if include_transcript {
        " `transcript` (verbatim transcription of all spoken words and dialogue in the clip, \
         preserving speaker turns where distinguishable, in the original language; \
         if no speech is present return an empty string)."
    } else {
        ""
    };

    let system_prompt = format!(
        "You are a video logger. Given a video clip, return a JSON object with exactly these fields: \
        `title` (6 words max, descriptive not clickbait), \
        `tags` (array of 5-10 lowercase single-word or hyphenated tags covering subject, location type, mood, camera style, and notable objects), \
        `description` (2 to 3 sentences, factual, present tense, focused on what is visible and audible, maximum 60 words total).{transcript} \
        Do not invent details. If a field cannot be determined, return an empty string or empty array.{custom}",
        transcript = transcript_instruction,
        custom = if custom_prompt.is_empty() {
            String::new()
        } else {
            format!("\n\nAdditional instructions: {}", custom_prompt)
        }
    );

    let resolution_value = match media_resolution {
        "high" => "MEDIA_RESOLUTION_HIGH",
        _ => "MEDIA_RESOLUTION_LOW",
    };

    let mut properties = serde_json::json!({
        "title": { "type": "STRING", "description": "Descriptive title, 6 words max" },
        "tags": { "type": "ARRAY", "items": {"type": "STRING"}, "description": "5-10 lowercase tags" },
        "description": { "type": "STRING", "description": "2-3 factual sentences, max 60 words" }
    });
    let mut required = vec!["title", "tags", "description"];

    if include_transcript {
        properties.as_object_mut().unwrap().insert(
            "transcript".to_string(),
            serde_json::json!({
                "type": "STRING",
                "description": "Verbatim transcription of all spoken audio, empty string if no speech"
            }),
        );
        required.push("transcript");
    }

    let request_body = serde_json::json!({
        "systemInstruction": { "parts": [{"text": system_prompt}] },
        "contents": [{ "parts": [{ "fileData": { "mimeType": file_mime, "fileUri": file_uri } }] }],
        "generationConfig": {
            "responseMimeType": "application/json",
            "responseSchema": { "type": "OBJECT", "properties": properties, "required": required },
            "mediaResolution": resolution_value
        }
    });

    info!(
        "Sending GenerateContent request (transcript: {})",
        include_transcript
    );

    let mut last_error = String::new();

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let delay = backoff_duration(attempt);
            warn!(
                "Describe retry {}/{} after {:?}",
                attempt + 1,
                MAX_RETRIES,
                delay
            );
            tokio::time::sleep(delay).await;
        }

        let resp = match client
            .post(&url)
            .header("x-goog-api-key", api_key)
            .json(&request_body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("Network error: {e}");
                warn!("{}", last_error);
                continue;
            }
        };

        let status = resp.status();

        // Honour Retry-After on 429
        if status == 429 {
            let wait = retry_after(&resp).unwrap_or_else(|| backoff_duration(attempt).max(Duration::from_secs(10)));
            last_error = format!("Rate limited (429), waiting {:?}", wait);
            warn!("{}", last_error);
            tokio::time::sleep(wait).await;
            continue;
        }

        // Non-retryable client error — fail fast
        if status.is_client_error() && status != 429 {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            return Err(format!(
                "API error ({}): {}",
                status,
                extract_error_message(&body)
            ));
        }

        // Retryable server error
        if status.is_server_error() {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            last_error = format!("Server error ({}): {}", status, extract_error_message(&body));
            warn!("{}", last_error);
            continue;
        }

        // Success — parse response
        let body: serde_json::Value = match resp.json().await {
            Ok(b) => b,
            Err(e) => {
                last_error = format!("Failed to parse response: {e}");
                warn!("{}", last_error);
                continue;
            }
        };

        let usage = &body["usageMetadata"];
        let input_tokens = usage["promptTokenCount"].as_i64();
        let output_tokens = usage["candidatesTokenCount"].as_i64();

        let text = body["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or_else(|| {
                debug!(
                    "Unexpected API response: {}",
                    serde_json::to_string_pretty(&body).unwrap_or_default()
                );
                "Unexpected API response format".to_string()
            })?;

        let clip: ClipDescription =
            serde_json::from_str(text).map_err(|e| format!("Failed to parse JSON: {e}"))?;

        return Ok(DescribeResult {
            clip,
            input_tokens,
            output_tokens,
        });
    }

    Err(format!(
        "All {} retries exhausted. Last error: {}",
        MAX_RETRIES, last_error
    ))
}

// ── Helpers ──

fn extract_error_message(body: &serde_json::Value) -> String {
    body.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("Unknown error")
        .to_string()
}

/// Get the MIME type for a video file.
pub fn mime_for_video(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("webm") => "video/webm",
        Some("mxf") => "application/mxf",
        Some("mts" | "m2ts" | "ts") => "video/mp2t",
        _ => "application/octet-stream",
    }
}
