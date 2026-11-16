# Persistent Ripping History & Logging Guide

`dvd-ripper` maintains a persistent database log of all backup jobs in `ripping_history.json` via [`src/history.rs`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/history.rs).

This document details the structure, schema, management tools, and library auditing capabilities enabled by the ripping history database.

---

## 1. Storage Location & Resolution

The history log is stored as a JSON array file located in:
- **User Home Directory (Default)**: `~/.dvd-ripper/ripping_history.json`
- **Local Working Directory (Fallback)**: `./ripping_history.json`

---

## 2. JSON Schema Specification

Each backup operation appends a structured record (`RipRecord`) to `ripping_history.json`.

```json
[
  {
    "timestamp": "2026-08-21 16:04:12",
    "disc_label": "KILL_BILL_VOL1",
    "title_name": "Kill Bill: Vol. 1",
    "year": 2003,
    "media_type": "Movie",
    "output_path": "Films/Kill Bill Vol. 1 (2003)/Kill Bill Vol. 1 (2003).mpg",
    "duration_secs": 6660.0,
    "file_size_bytes": 4831838208,
    "status": "Completed",
    "video_codec": "copy",
    "audio_lang": "eng",
    "subtitles_enabled": true
  }
]
```

### Field Definitions

| Field Name | Type | Description |
|---|---|---|
| `timestamp` | String | ISO-8601 formatted date and time of job execution. |
| `disc_label` | String | Optical volume label read from optical drive sector descriptors. |
| `title_name` | String | Resolved movie or TV show title name. |
| `year` | Integer | Release year resolved via OMDb/IMDb APIs. |
| `media_type` | String | Classification: `"Movie"` or `"TV Series"`. |
| `output_path` | String | Relative or absolute path to generated output file. |
| `duration_secs` | Float | Total running time of extracted video stream in seconds. |
| `file_size_bytes` | Integer | Final disk size of output file in bytes. |
| `status` | String | Final job status: `"Completed"`, `"Cancelled"`, or `"Failed"`. |
| `video_codec` | String | Video codec choice (`"copy"`, `"h264"`, `"hevc"`, `"av1"`). |
| `audio_lang` | String | Preferred audio language code (e.g. `"eng"`). |
| `subtitles_enabled` | Boolean | `true` if subtitle streams were extracted into container. |

---

## 3. Accessing & Managing History

### 3.1 Via Desktop GUI (History Tab)
1. Open `dvd-ripper` GUI.
2. Select the **📜 History Log** tab.
3. Browse past rip operations sorted chronologically.
4. Click **Clear History Log** to reset history records.

### 3.2 Via REST API
- **List Ripping History**: `GET /api/history`
- **Clear History Log**: `POST /api/history/clear`

### 3.3 Via Command-Line JSON Parsers (`jq`)
Search history database from terminal:

```bash
# View total number of completed rips
jq 'map(select(.status == "Completed")) | length' ~/.dvd-ripper/ripping_history.json

# Calculate total gigabytes extracted
jq '[.[].file_size_bytes] | add / 1073741824' ~/.dvd-ripper/ripping_history.json
```

---

## 5. Diagnostic Runtime File Logging (`dvd-ripper.log`)

While `ripping_history.json` tracks completed transactional outcomes, `dvd-ripper` also includes a full runtime diagnostic file logging subsystem.

### 5.1 Overview & Architecture
All application execution events, drive discovery probes, FFmpeg commands and standard error streams, MakeMKV progress packets, and optical read errors are routed to the diagnostic logger:
- **Default Filepath**: `~/.dvd-ripper/dvd-ripper.log` (Windows: `%USERPROFILE%\.dvd-ripper\dvd-ripper.log`, Linux/macOS: `~/.dvd-ripper/dvd-ripper.log`).
- **Thread-Safe Appending**: Uses a thread-safe mutex and buffering to ensure logs from background transcoder threads, GUI loops, and child process monitors are preserved in strict chronological order.
- **Configurable & Disablable**: File logging can be completely toggled off or redirected to any custom path on the filesystem.

### 5.2 Configuring File Logging

#### Via TOML Configuration (`config.toml`)
```toml
# Enable logging to disk (default: true)
file_log_enabled = true

# Optional custom destination log file path
log_file = "C:\\Users\\User\\.dvd-ripper\\dvd-ripper.log"
```

#### Via Desktop GUI (Settings Tab)
1. Open the **⚙️ Settings** tab.
2. Scroll to the **📝 Application Logging** section.
3. Check/uncheck **Enable Logging to File** to toggle runtime disk logging.
4. Modify the **Log File Path** directly or click **📂 Browse...** to select a new destination.
5. Click **📄 Open Log File** to immediately launch the log in your operating system's default text editor (Notepad, VS Code, TextEdit, etc.) for real-time inspection.

### 5.3 Example Diagnostic Output
```text
[2026-11-16 16:30:29] Running command: ffmpeg ["-nostdin", "-stats", "-i", "D:\\", ...]
[2026-11-16 16:30:29] Ripping Film: Game of Thrones - S02E09 - Blackwater
[2026-11-16 16:30:30] frame=   48 fps= 46 q=27.0 size= 512KiB time=00:00:01.92
[2026-11-16 16:35:12] Unable to read next block of PGC (title=2 pgc=2 pg=4 cell=4)
[2026-11-16 16:35:12] Retrying rip with error-tolerant demux flags enabled...
```

