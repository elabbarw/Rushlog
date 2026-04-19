use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_media_resolution")]
    pub media_resolution: String,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default = "default_overwrite_policy")]
    pub overwrite_policy: String,
    #[serde(default)]
    pub custom_prompt: String,
    #[serde(default = "default_split_threshold_gb")]
    pub split_threshold_gb: f64,
    #[serde(default = "default_segment_duration_min")]
    pub segment_duration_min: u64,
    #[serde(default)]
    pub include_transcript: bool,
}

fn default_model() -> String { "gemini-2.5-flash-lite".into() }
fn default_media_resolution() -> String { "low".into() }
fn default_concurrency() -> u32 { 3 }
fn default_overwrite_policy() -> String { "skip".into() }
fn default_split_threshold_gb() -> f64 { 2.0 }
fn default_segment_duration_min() -> u64 { 15 }

impl Default for Settings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_model(),
            media_resolution: default_media_resolution(),
            concurrency: default_concurrency(),
            overwrite_policy: default_overwrite_policy(),
            custom_prompt: String::new(),
            split_threshold_gb: default_split_threshold_gb(),
            segment_duration_min: default_segment_duration_min(),
            include_transcript: false,
        }
    }
}

const VALID_MODELS: &[&str] = &[
    "gemini-2.5-flash-lite",
    "gemini-3.1-flash-lite-preview",
    "gemini-3-flash-preview",
];

/// Validate settings before saving. Prevents injection via model field,
/// infinite loops via zero duration, and other invalid configurations.
pub fn validate(settings: &Settings) -> Result<(), String> {
    // Model must be from the allowed list (prevents URL path traversal)
    if !VALID_MODELS.contains(&settings.model.as_str()) {
        return Err(format!(
            "Invalid model '{}'. Must be one of: {}",
            settings.model,
            VALID_MODELS.join(", ")
        ));
    }

    if !["low", "high"].contains(&settings.media_resolution.as_str()) {
        return Err("Media resolution must be 'low' or 'high'".into());
    }

    if !["skip", "overwrite"].contains(&settings.overwrite_policy.as_str()) {
        return Err("Overwrite policy must be 'skip' or 'overwrite'".into());
    }

    if settings.concurrency < 1 || settings.concurrency > 10 {
        return Err("Concurrency must be between 1 and 10".into());
    }

    if settings.split_threshold_gb < 0.1 {
        return Err("Split threshold must be at least 0.1 GB".into());
    }

    // Prevent infinite loop in ffmpeg::split_video
    if settings.segment_duration_min < 1 || settings.segment_duration_min > 120 {
        return Err("Segment duration must be between 1 and 120 minutes".into());
    }

    if settings.custom_prompt.len() > 2000 {
        return Err("Custom prompt must be under 2000 characters".into());
    }

    Ok(())
}

fn settings_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("rushlog");
    std::fs::create_dir_all(&dir).ok();
    dir.join("settings.toml")
}

pub fn load() -> Settings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> Result<(), String> {
    let path = settings_path();
    let contents = toml::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())?;

    // Restrict file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    }

    Ok(())
}
