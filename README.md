# Rushlog

You had an eventful day — or week — and recorded hundreds of videos. You got back exhausted and now you have to ingest the rushes and figure out what's useful and what's junk. Imagine instead you just point an app at your folder of rushes and leave it to work for a few minutes. When you come back, every clip has a markdown file describing what's in it — title, tags, description, even a full transcript if you want one. Fire up Claude (or ChatGPT, or any LLM) and drop in the index file. Ask it to group your best shots, draft a storyboard, or tell you which clips to use for your edit. Done.

Rushlog is a cross-platform desktop tool that batch-describes video rushes into searchable markdown sidecars. It uses Google's Gemini API because it's the only model today that can natively ingest full video files — audio, visuals, and temporal context — rather than relying on extracted frames or transcripts alone. The output is a markdown file per clip plus a single `INDEX.md` summarising everything — portable, plain-text, no lock-in.

## Features

- Direct video upload to Gemini with structured JSON output (enforced schema)
- Markdown sidecars with YAML frontmatter next to each video file
- `INDEX.md` with tag index and duration summary for the whole folder
- Optional verbatim transcript extraction via Gemini
- Large file splitting (FFmpeg stream-copy, no re-encode) for videos over 2 GB
- Configurable model, resolution, concurrency, and custom prompts
- Light/dark mode following system appearance

## Requirements

- **FFmpeg** installed on your system (the app will detect if it's missing and guide you through installation)
  - **macOS:** `brew install ffmpeg`
  - **Windows:** `winget install Gyan.FFmpeg` or download from [ffmpeg.org](https://ffmpeg.org/download.html)
- **Gemini API key** from [Google AI Studio](https://aistudio.google.com/apikey) (free tier, no credit card)

## Install

Download the latest `.dmg` (macOS) or `.msi` (Windows) from [Releases](../../releases).

### macOS — unsigned app warning

Rushlog is open source and not code-signed. macOS will block it by default. After installing, run:

```bash
xattr -cr "/Applications/Rushlog.app"
```

Then open normally. Alternatively, right-click the app and choose **Open** — macOS will show an "Open Anyway" dialog.

## Build from source

```bash
# Prerequisites: Rust, Node.js, FFmpeg
git clone https://github.com/user/rushlog.git
cd rushlog
npm install
npm run tauri build
```

The built app will be at `src-tauri/target/release/bundle/`.

For development:

```bash
npm run tauri dev
```

## Supported formats

MP4, MOV, MKV, AVI, M4V, WebM, MXF, BRAW (Blackmagic RAW), R3D (RED RAW), MTS, M2TS, TS — anything FFmpeg can probe.

## How it works

1. Select a folder of video files
2. The app probes each clip with `ffprobe` and skips any that already have a sidecar
3. Each clip is streamed to Google's Gemini Files API (not loaded into memory)
4. Gemini returns structured JSON (title, tags, description, optional transcript) via enforced schema
5. A markdown sidecar is written next to each video
6. An `INDEX.md` is generated summarising all clips with a tag index

No database, no state files. The filesystem is the truth — if `clip.md` exists, that clip is done. Move the folder to another machine and everything still works.

### Sidecar format

```markdown
---
title: Fishing boats returning at dusk
source: GX010234.MP4
duration: 00:01:47
tags: [boats, harbour, golden-hour, handheld, seagulls, wide-shot]
generated: 2026-04-19T14:22:08Z
model: gemini-3.1-flash-lite-preview
has_transcript: true
---

# Fishing boats returning at dusk

Three small fishing boats motor into a stone harbour under low orange light.
The camera is handheld on the quayside; gulls cross frame twice. Ambient
audio: engine idle, distant voices, water against the hull.

## Transcript

Hello, we're coming in now. Can you grab the line?
```

## Settings

| Setting | Default | Notes |
|---|---|---|
| API key | — | Stored in OS data directory |
| Model | `gemini-2.5-flash-lite` | Also: `gemini-3.1-flash-lite-preview`, `gemini-3-flash-preview` |
| Media resolution | Low | Low recommended; High for text-heavy footage |
| Concurrency | 3 | 1-10 parallel workers |
| Include transcript | Off | Verbatim speech transcription via Gemini |
| Split threshold | 2 GB | Files larger than this are split into segments |
| Segment duration | 15 min | Length of each segment when splitting |
| Overwrite policy | Skip | Skip or overwrite existing sidecars |

## Stack

- **Tauri 2** (Rust + system webview)
- **React + TypeScript** with Tailwind CSS
- **FFmpeg** for probing and large-file splitting
- **Gemini API** with structured output (enforced JSON schema)
- **No database** — filesystem is the only state

## License

[MIT](LICENSE) — Wanis Elabbar
