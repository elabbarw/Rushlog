use std::path::Path;
use std::process::Command;

/// Resolve a binary name by checking system PATH and common install locations.
fn resolve_bin(name: &str) -> String {
    let candidates: &[&str] = match name {
        "ffprobe" => &["ffprobe", "/opt/homebrew/bin/ffprobe", "/usr/local/bin/ffprobe", "/usr/bin/ffprobe"],
        "ffmpeg" => &["ffmpeg", "/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg", "/usr/bin/ffmpeg"],
        _ => &[],
    };

    for candidate in candidates {
        if Command::new(candidate)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok()
        {
            return candidate.to_string();
        }
    }

    // Fall back to bare name (will fail with a clear error if not found)
    name.to_string()
}

/// Check whether ffmpeg and ffprobe are available on the system.
/// Returns Ok(version_string) or Err(explanation).
pub fn check_available() -> Result<String, String> {
    let ffmpeg = resolve_bin("ffmpeg");
    let output = Command::new(&ffmpeg)
        .arg("-version")
        .output()
        .map_err(|_| "FFmpeg is not installed or not found in PATH.".to_string())?;

    if !output.status.success() {
        return Err("FFmpeg was found but returned an error.".to_string());
    }

    let version_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("ffmpeg (unknown version)")
        .to_string();

    // Also verify ffprobe
    let ffprobe = resolve_bin("ffprobe");
    Command::new(&ffprobe)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| "ffprobe is not installed or not found in PATH.".to_string())?;

    Ok(version_line)
}

fn ffprobe_bin() -> String {
    static BIN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BIN.get_or_init(|| resolve_bin("ffprobe")).clone()
}

fn ffmpeg_bin() -> String {
    static BIN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BIN.get_or_init(|| resolve_bin("ffmpeg")).clone()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProbeResult {
    pub duration_seconds: f64,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
}

pub fn probe(path: &Path) -> Result<ProbeResult, String> {
    let output = Command::new(ffprobe_bin())
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("ffprobe not found: {e}. Install FFmpeg to use Rushlog."))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {stderr}"));
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Failed to parse ffprobe output: {e}"))?;

    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let file_size = json["format"]["size"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));

    let video_stream = json["streams"]
        .as_array()
        .and_then(|streams| streams.iter().find(|s| s["codec_type"] == "video"));

    let (codec, width, height) = match video_stream {
        Some(stream) => (
            stream["codec_name"].as_str().unwrap_or("unknown").to_string(),
            stream["width"].as_u64().unwrap_or(0) as u32,
            stream["height"].as_u64().unwrap_or(0) as u32,
        ),
        None => ("unknown".into(), 0, 0),
    };

    Ok(ProbeResult {
        duration_seconds: duration,
        codec,
        width,
        height,
        file_size,
    })
}

pub fn is_video_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    matches!(
        ext.as_deref(),
        Some("mp4" | "mov" | "mkv" | "avi" | "m4v" | "webm" | "mxf" | "braw" | "r3d" | "mts" | "m2ts" | "ts")
    )
}

pub fn format_duration(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Check if a file exceeds the size threshold for splitting.
pub fn needs_splitting(path: &Path, threshold_bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|m| m.len() > threshold_bytes)
        .unwrap_or(false)
}

/// Plan how to split a video — returns the segment boundaries without encoding anything.
pub fn plan_segments(
    path: &Path,
    segment_duration_secs: u64,
) -> Result<(std::path::PathBuf, Vec<SegmentPlan>), String> {
    let probe_result = probe(path)?;
    let total_duration = probe_result.duration_seconds;

    if total_duration <= 0.0 {
        return Err("Cannot determine video duration for splitting".into());
    }
    if segment_duration_secs == 0 {
        return Err("Segment duration must be greater than zero".into());
    }

    let parent = path.parent().ok_or("Cannot determine parent directory")?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("video");
    let segments_dir = parent.join(format!(".rush-segments-{}", stem));
    std::fs::create_dir_all(&segments_dir)
        .map_err(|e| format!("Failed to create segments directory: {e}"))?;

    let mut plans = Vec::new();
    let mut start = 0.0f64;
    let mut index = 0u32;

    while start < total_duration {
        let remaining = total_duration - start;
        let seg_dur = (segment_duration_secs as f64).min(remaining);
        let output_path = segments_dir.join(format!("seg_{:04}.mp4", index));

        plans.push(SegmentPlan {
            output_path,
            start_seconds: start,
            duration_seconds: seg_dur,
            index,
        });

        start += segment_duration_secs as f64;
        index += 1;
    }

    Ok((segments_dir, plans))
}

/// Encode a single segment to 480p H.264. Called per-segment so the caller can emit progress.
pub fn encode_segment(source: &Path, plan: &SegmentPlan) -> Result<Segment, String> {
    let start_str = format!("{:.3}", plan.start_seconds);
    let dur_str = format!("{:.3}", plan.duration_seconds);

    let output = Command::new(ffmpeg_bin())
        .args(["-y", "-ss", &start_str, "-i"])
        .arg(source)
        .args([
            "-t", &dur_str,
            "-c:v", "libx264",
            "-crf", "28",
            "-preset", "fast",
            "-vf", "scale=-2:480",
            "-c:a", "aac",
            "-b:a", "64k",
            "-movflags", "+faststart",
        ])
        .arg(&plan.output_path)
        .output()
        .map_err(|e| format!("ffmpeg encode failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg encode failed at segment {}: {stderr}", plan.index));
    }

    let actual_dur = probe(&plan.output_path)
        .map(|p| p.duration_seconds)
        .unwrap_or(plan.duration_seconds);

    Ok(Segment {
        path: plan.output_path.clone(),
        start_seconds: plan.start_seconds,
        duration_seconds: actual_dur,
        index: plan.index,
    })
}

#[derive(Debug, Clone)]
pub struct SegmentPlan {
    pub output_path: std::path::PathBuf,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub index: u32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Segment {
    pub path: std::path::PathBuf,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub index: u32,
}
