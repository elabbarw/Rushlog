use crate::gemini::ClipDescription;
use std::path::Path;

pub fn render_sidecar(
    desc: &ClipDescription,
    source_filename: &str,
    duration_seconds: f64,
    model: &str,
) -> String {
    let duration = crate::ffmpeg::format_duration(duration_seconds);
    let tags_str = desc
        .tags
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let has_transcript = desc.transcript.as_ref().is_some_and(|t| !t.is_empty());

    let mut output = format!(
        "---\ntitle: {title}\nsource: {source}\nduration: {duration}\ntags: [{tags}]\ngenerated: {now}\nmodel: {model}\nhas_transcript: {has_transcript}\n---\n\n# {title}\n\n{description}\n",
        title = desc.title,
        source = source_filename,
        duration = duration,
        tags = tags_str,
        now = now,
        model = model,
        has_transcript = has_transcript,
        description = desc.description,
    );

    if let Some(transcript) = &desc.transcript {
        if !transcript.is_empty() {
            output.push_str(&format!("\n## Transcript\n\n{}\n", transcript));
        }
    }

    output
}

/// Render a sidecar for a large video that was split into segments.
pub fn render_segmented_sidecar(
    segments: &[SegmentDescription],
    source_filename: &str,
    total_duration_seconds: f64,
    model: &str,
    total_planned: usize,
) -> String {
    let is_partial = segments.len() < total_planned;
    let duration = crate::ffmpeg::format_duration(total_duration_seconds);
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let mut all_tags: Vec<String> = Vec::new();
    for seg in segments {
        for tag in &seg.description.tags {
            let lower = tag.to_lowercase();
            if !all_tags.contains(&lower) {
                all_tags.push(lower);
            }
        }
    }
    let tags_str = all_tags.join(", ");

    let title = if segments.len() == 1 {
        segments[0].description.title.clone()
    } else {
        format!("{} (full analysis)", segments[0].description.title)
    };

    let has_any_transcript = segments.iter().any(|s| {
        s.description.transcript.as_ref().is_some_and(|t| !t.is_empty())
    });

    let partial_line = if is_partial {
        format!("partial: true\nsegments_failed: {}\n", total_planned - segments.len())
    } else {
        String::new()
    };

    let mut output = format!(
        "---\ntitle: {title}\nsource: {source}\nduration: {duration}\ntags: [{tags}]\ngenerated: {now}\nmodel: {model}\nsegments: {seg_count}\n{partial}has_transcript: {has_transcript}\n---\n\n# {title}\n\n",
        title = title,
        source = source_filename,
        duration = duration,
        tags = tags_str,
        now = now,
        partial = partial_line,
        model = model,
        seg_count = segments.len(),
        has_transcript = has_any_transcript,
    );

    if let Some(first) = segments.first() {
        output.push_str(&first.description.description);
        output.push('\n');
    }

    output.push_str("\n## Timeline\n\n");

    for seg in segments {
        let start = crate::ffmpeg::format_duration(seg.start_seconds);
        let end = crate::ffmpeg::format_duration(seg.start_seconds + seg.duration_seconds);
        let seg_tags = seg.description.tags.join(", ");

        output.push_str(&format!(
            "### {start} — {end}: {title}\n**Tags:** {tags}\n{desc}\n\n",
            start = start,
            end = end,
            title = seg.description.title,
            tags = seg_tags,
            desc = seg.description.description,
        ));
    }

    // Combine all segment transcripts into a full transcript section
    if has_any_transcript {
        output.push_str("## Full Transcript\n\n");
        for seg in segments {
            if let Some(transcript) = &seg.description.transcript {
                if !transcript.is_empty() {
                    let start = crate::ffmpeg::format_duration(seg.start_seconds);
                    output.push_str(&format!("**[{}]** {}\n\n", start, transcript));
                }
            }
        }
    }

    output
}

pub fn sidecar_path_for(video_path: &Path) -> std::path::PathBuf {
    video_path.with_extension("md")
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SegmentDescription {
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub description: ClipDescription,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
}
