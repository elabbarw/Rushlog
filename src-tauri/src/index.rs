use crate::ffmpeg;
use crate::sidecar;
use std::path::{Path, PathBuf};

/// Generate INDEX.md for a folder by reading all sidecar .md files.
pub fn generate_index(folder_path: &str) -> Result<PathBuf, String> {
    let folder = Path::new(folder_path);
    let folder_name = folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Clips");

    // Find all video files that have a corresponding sidecar
    let entries = std::fs::read_dir(folder).map_err(|e| format!("Cannot read: {e}"))?;
    let mut described_clips: Vec<DescribedClip> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !ffmpeg::is_video_file(&path) {
            continue;
        }
        let sidecar_path = sidecar::sidecar_path_for(&path);
        if !sidecar_path.exists() {
            continue;
        }
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let content = std::fs::read_to_string(&sidecar_path).unwrap_or_default();
        let (title, tags, description, duration) = parse_sidecar(&content);

        // If duration not in sidecar, probe the video
        let duration = if duration > 0.0 {
            duration
        } else {
            ffmpeg::probe(&path).map(|p| p.duration_seconds).unwrap_or(0.0)
        };

        described_clips.push(DescribedClip {
            filename,
            title,
            tags,
            description,
            duration,
        });
    }

    described_clips.sort_by(|a, b| a.filename.cmp(&b.filename));

    if described_clips.is_empty() {
        return Err("No described clips to index".into());
    }

    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let total_seconds: f64 = described_clips.iter().map(|c| c.duration).sum();
    let total_duration = ffmpeg::format_duration(total_seconds);
    let model = crate::settings::load().model;

    let mut output = String::new();

    // YAML frontmatter
    output.push_str(&format!(
        "---\nfolder: {}\ngenerated: {}\nclip_count: {}\ntotal_duration: {}\nmodel: {}\n---\n\n",
        folder_path,
        now,
        described_clips.len(),
        total_duration,
        model,
    ));

    output.push_str(&format!(
        "# {} — Clip Index\n\nThis folder contains {} described clips totalling {}.\nEach entry links to a full markdown sidecar with extended description.\n\n## All clips\n\n",
        folder_name,
        described_clips.len(),
        format_duration_human(total_seconds),
    ));

    let mut tag_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut longest: Option<(&str, f64)> = None;
    let mut shortest: Option<(&str, f64)> = None;

    for clip in &described_clips {
        let duration_str = ffmpeg::format_duration(clip.duration);

        if clip.duration > 0.0 {
            match longest {
                None => longest = Some((&clip.filename, clip.duration)),
                Some((_, d)) if clip.duration > d => longest = Some((&clip.filename, clip.duration)),
                _ => {}
            }
            match shortest {
                None => shortest = Some((&clip.filename, clip.duration)),
                Some((_, d)) if clip.duration < d => shortest = Some((&clip.filename, clip.duration)),
                _ => {}
            }
        }

        for tag in &clip.tags {
            tag_map
                .entry(tag.clone())
                .or_default()
                .push(clip.filename.clone());
        }

        let tags_str = clip.tags.join(", ");
        let sidecar_name = Path::new(&clip.filename)
            .with_extension("md")
            .display()
            .to_string();
        let sidecar_link = format!("./{}", sidecar_name);

        let summary = if clip.description.chars().count() > 120 {
            let truncated: String = clip.description.chars().take(120).collect();
            match truncated.rfind(". ") {
                Some(pos) => truncated[..=pos].to_string(),
                None => format!("{}...", truncated),
            }
        } else {
            clip.description
                .split(". ")
                .next()
                .unwrap_or(&clip.description)
                .to_string()
        };

        output.push_str(&format!(
            "### {}\n**File:** `{}` · **Duration:** {}\n**Tags:** {}\n{}\n[Full description →]({})\n\n",
            clip.title, clip.filename, duration_str, tags_str, summary, sidecar_link,
        ));
    }

    // Tag index
    output.push_str("## Tag index\n\n");
    let mut sorted_tags: Vec<_> = tag_map.iter().collect();
    sorted_tags.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for (tag, files) in sorted_tags {
        let file_list: Vec<&str> = files
            .iter()
            .map(|f| {
                Path::new(f)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(f)
            })
            .collect();
        let display = if file_list.len() > 4 {
            format!("{}...", file_list[..4].join(", "))
        } else {
            file_list.join(", ")
        };
        output.push_str(&format!("**{}** ({} clips): {}\n", tag, files.len(), display));
    }

    // Duration summary
    output.push_str(&format!(
        "\n## Duration summary\n\nTotal: {} across {} clips\n",
        format_duration_human(total_seconds),
        described_clips.len(),
    ));

    if !described_clips.is_empty() {
        let avg = total_seconds / described_clips.len() as f64;
        output.push_str(&format!("Average clip length: {}\n", format_duration_human(avg)));
    }
    if let Some((name, dur)) = longest {
        output.push_str(&format!("Longest: {} — {}\n", name, format_duration_human(dur)));
    }
    if let Some((name, dur)) = shortest {
        output.push_str(&format!("Shortest: {} — {}\n", name, format_duration_human(dur)));
    }

    // Write atomically
    let index_path = determine_index_path(folder);
    let tmp_path = index_path.with_extension("md.tmp");
    std::fs::write(&tmp_path, &output).map_err(|e| format!("Failed to write index: {e}"))?;
    std::fs::rename(&tmp_path, &index_path).map_err(|e| format!("Failed to rename index: {e}"))?;

    Ok(index_path)
}

struct DescribedClip {
    filename: String,
    title: String,
    tags: Vec<String>,
    description: String,
    duration: f64,
}

fn determine_index_path(folder: &Path) -> PathBuf {
    let default_path = folder.join("INDEX.md");
    if default_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&default_path) {
            if !content.contains("Clip Index") {
                return folder.join("INDEX-rush.md");
            }
        }
    }
    default_path
}

/// Parse a sidecar markdown file to extract title, tags, description, and duration.
fn parse_sidecar(content: &str) -> (String, Vec<String>, String, f64) {
    let mut title = String::new();
    let mut tags = Vec::new();
    let mut description = String::new();
    let mut duration = 0.0f64;
    let mut in_frontmatter = false;
    let mut past_frontmatter = false;

    for line in content.lines() {
        if line.trim() == "---" {
            if !in_frontmatter && !past_frontmatter {
                in_frontmatter = true;
                continue;
            } else if in_frontmatter {
                in_frontmatter = false;
                past_frontmatter = true;
                continue;
            }
        }

        if in_frontmatter {
            if let Some(val) = line.strip_prefix("title: ") {
                title = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("tags: [") {
                let tag_str = val.trim_end_matches(']');
                tags = tag_str
                    .split(", ")
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
            } else if let Some(val) = line.strip_prefix("duration: ") {
                duration = parse_duration_str(val.trim());
            }
        } else if past_frontmatter
            && !line.starts_with('#')
            && !line.starts_with("## ")
            && !line.trim().is_empty()
        {
            if description.is_empty() {
                description = line.trim().to_string();
            } else if description.split_whitespace().count() < 80 {
                description.push(' ');
                description.push_str(line.trim());
            }
        }
    }

    (title, tags, description, duration)
}

/// Parse "HH:MM:SS" duration string to seconds.
fn parse_duration_str(s: &str) -> f64 {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        3 => {
            let h: f64 = parts[0].parse().unwrap_or(0.0);
            let m: f64 = parts[1].parse().unwrap_or(0.0);
            let s: f64 = parts[2].parse().unwrap_or(0.0);
            h * 3600.0 + m * 60.0 + s
        }
        2 => {
            let m: f64 = parts[0].parse().unwrap_or(0.0);
            let s: f64 = parts[1].parse().unwrap_or(0.0);
            m * 60.0 + s
        }
        _ => 0.0,
    }
}

fn format_duration_human(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}
