# Alternative Ripping & Transcoding Engines Guide

`dvd-ripper` provides a modular, multi-engine architecture designed to extract optical DVD-Video content reliably regardless of disc encryption, copy protection traps, or hardware constraints.

Users can choose between four distinct transcoding/remuxing engines or let the system dynamically select the best available engine through the **Auto-Fallback Composite Engine**.

---

## 1. Engine Comparison & Architecture

```mermaid
graph TD
    Job["TranscodeJob<br/>(Source DVD, Title Number, Output Path)"]
    
    subgraph Auto_Fallback ["Auto-Fallback Orchestrator ('auto')"]
        E1["1. FFmpeg Engine ('ffmpeg')"]
        E2["2. Native IFO Extractor ('native')"]
        E3["3. HandBrakeCLI Engine ('handbrake')"]
        E4["4. MakeMKV Engine ('makemkv')"]
        
        E1 -- "On Bad Sector / Demux Error" --> E2
        E2 -- "If Unsupported / Error" --> E3
        E3 -- "If Encrypted / Error" --> E4
    end

    Job --> Auto_Fallback
```

### Engine Feature Matrix

| Feature / Capability | FFmpeg (`ffmpeg`) | Native IFO (`native`) | HandBrakeCLI (`handbrake`) | MakeMKV (`makemkv`) |
| :--- | :---: | :---: | :---: | :---: |
| **Primary Mechanism** | `dvdvideo` Demuxer | Direct 2048-byte Sector Extractor | `libdvdnav` Navigation | Robot Mode (`makemkvcon`) |
| **External Binary Required** | Yes (`ffmpeg`) | **No (Pure Rust)** | Yes (`HandBrakeCLI`) | Yes (`makemkvcon`) |
| **Lossless Remuxing** | Yes (`--codec copy`) | **Yes (Direct VOB Copy)** | No (Always encodes) | **Yes (Bit-for-bit MKV)** |
| **Video Re-encoding** | H.264, HEVC, AV1 | Optional (via FFmpeg) | x264, x265, SVT-AV1 | No (Remux only) |
| **Hardware Decomb Filtering**| `bwdif`, `yadif`, `w3fdif` | Passthrough | `--comb-detect --decomb` | Passthrough |
| **Bad Sector Trap Resistance**| Moderate | **High (Sector Stepping)** | High (`libdvdnav`) | **Exceptional** |
| **Corrupt Root IFO Recovery** | Low (`libdvdread` aborts) | **High (.BUP Auto-Fallback)** | Moderate | High |
| **Hardware Bus Decryption** | Via `libdvdcss` | Via `libdvdcss` / OS | Via `libdvdcss` | **Native AACS/CSS/BD+ Engine** |

---

## 2. Detailed Engine Specifications

### 2.1. FFmpeg Engine (`--engine ffmpeg`)
- **Adapter**: `crate::infrastructure::ffmpeg::FfmpegTranscoder`
- **How it works**: Invokes `ffmpeg` with `-f dvdvideo -title <N> -i <DRIVE>` along with the project's flexible filter pipelines.
- **Strengths**: 
  - Rich filter graph support (motion-adaptive deinterlacing, 3D denoising, crop dimension detection, audio normalization).
  - Broad codec support including SVT-AV1, NVENC, QSV, and VAAPI hardware acceleration.
- **When to use**: Standard non-damaged DVDs, home video discs, or when specialized encoding filters (e.g. `--denoise`, `--deinterlace-algo bwdif`) are needed.

### 2.2. Native Rust IFO Cell-Sector Extractor (`--engine native`)
- **Adapter**: `crate::infrastructure::engines::NativeIfoTranscoder`
- **How it works**: Directly parses binary `VIDEO_TS.IFO` / `VTS_xx_0.IFO` Program Chain Tables (`VTS_PGCITI`) and Cell Playback Information Tables (`C_PBIT`). It resolves the exact `(first_sector..=last_sector)` 2048-byte logical sector spans belonging to the chosen title track and reads them directly from `VTS_xx_1..9.VOB`.
- **Strengths**:
  - **Zero External Dependencies**: Operates completely in pure Rust.
  - **Demuxer Immunity**: Bypasses all `ffmpeg -f dvdvideo` and `libdvdread` demuxer parsing bugs.
  - **`.BUP` Backup Recovery**: Seamlessly reads `VIDEO_TS.BUP` or `VTS_xx_0.BUP` if the primary `.IFO` has CRC read errors or bad sectors.
  - **Direct Title Set Bypass**: If the root `VIDEO_TS.IFO` is unreadable due to Sony ARccOS protection, it directly discovers and extracts `VTS_01_0.IFO` through `VTS_99_0.IFO`.
  - **Sector-by-Sector Stepping & Instant Cancellation**: Steps down from 64 KiB block reads to individual 2048-byte sector reads when hitting damaged sectors, safely skipping unreferenced gap traps. Every sector read checks cancellation atomically so the Cancel button remains immediately responsive.
  - **Dual Circuit Breakers**:
    1. *Sector Short-Circuit*: If 2 consecutive single-sector read retries fail inside a 64 KB block, stops retrying the remaining 30 sectors and pads with MPEG-2 zero filler, saving minutes of drive laser thrashing.
    2. *Consecutive Block Bailout*: Trips after 8 consecutive damaged blocks (~512 KB), aborting cleanly so `AutoFallbackTranscoder` can advance to MakeMKV.
- **When to use**: Discs where FFmpeg outputs `Invalid IFO for title 0` or halts at arbitrary percentages due to ARccOS/Macrovision bad sector traps or physical scratches.

### 2.3. HandBrakeCLI Engine (`--engine handbrake`)
- **Adapter**: `crate::infrastructure::engines::HandBrakeTranscoder`
- **How it works**: Spawns `HandBrakeCLI` in headless mode, mapping the job specification to HandBrake's native flags (`-t <title>`, `-c <chapters>`, `-e <encoder>`, `--encoder-preset`, `--all-audio`, `--all-subtitles`, `--comb-detect --decomb`). Real-time progress is scraped from stdout (`Encoding: task 1 of 1, X.XX % (Y.YY fps, avg Z.ZZ fps, ETA HHhMMmSSs)`).
- **Auto-Discovery**: Automatically finds `HandBrakeCLI` in system `PATH` or standard installation locations (`C:\Program Files\HandBrake\HandBrakeCLI.exe`, `/Applications`, `/usr/bin`).
- **Strengths**:
  - Mature `libdvdnav` DVD virtual machine emulation.
  - Exceptional motion-adaptive decombing filter (`--comb-detect --decomb`).
- **When to use**: Discs with complex multi-angle camera angles, interlaced live-action concert discs, or when HandBrake's encoding presets are preferred.

### 2.4. MakeMKV Engine (`--engine makemkv`)
- **Adapter**: `crate::infrastructure::engines::MakeMkvTranscoder`
- **How it works**: Spawns `makemkvcon` in dedicated robot mode:
  ```bash
  makemkvcon -r --messages=-stdout --progress=-stdout mkv dev:<drive> <title_idx> <out_dir>
  ```
  Routing `--messages=-stdout --progress=-stdout` resolves Windows standard I/O buffer deadlocks. The engine parses real-time progress (`PRGV:current,total,max`) using zero-allocation split-iterators, inspects live CSS key decryption phases (`MSG:3007`, `MSG:3028`, `MSG:5014`), and automatically renames output files to standard Plex/Jellyfin naming conventions (e.g. `Game of Thrones - S02E09 - Blackwater.mkv`).
- **Auto-Discovery**: Automatically scans standard paths on startup:
  - Windows: `C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`, `C:\Program Files\MakeMKV\makemkvcon64.exe`, `makemkvcon.exe`.
  - macOS: `/Applications/MakeMKV.app/Contents/MacOS/makemkvcon`, `/opt/homebrew/bin/makemkvcon`.
  - Linux: `/usr/bin/makemkvcon`, `/usr/local/bin/makemkvcon`.
- **Strengths**:
  - **Hardware Optical Decryption & Low-Level SCSI**: Directly negotiates drive bus authentication for AACS, BD+, and CSS without external wrappers. Communicates with drives via SCSI pass-through for far better scratch error recovery.
  - **Lossless Extraction**: Zero video or audio recompression; 100% original MPEG-2 and Dolby Digital / DTS bitstreams are preserved in a modern Matroska container.
  - **Decoy Title Elimination**: Automatically resolves real feature titles from complex 99-title decoy playlists.
  - **Multi-Episode TV Batch Ripping**: Supports sequential multi-title extraction for TV show discs with clean timestamp discovery and stale temp file cleanup.
- **When to use**: Heavily encrypted discs, Criterion/Disney protected releases, badly scratched media, multi-episode TV box sets, or when bit-for-bit lossless archival remuxing is desired.
- **Detailed Guide**: See [`docs/MAKEMKV_INTEGRATION_GUIDE.md`](MAKEMKV_INTEGRATION_GUIDE.md) for complete IPC protocol specifications and troubleshooting.

### 2.5. MakeMKV + FFmpeg Two-Stage Compression Pipeline (`--engine makemkv+ffmpeg`)
- **Adapter**: `crate::infrastructure::engines::MakeMkvFfmpegTranscoder`
- **How it works**: Combines MakeMKV's robust optical decryption with FFmpeg's video compression:
  1. *Stage 1*: MakeMKV rips the title track to a temporary intermediate file in the destination directory, handling all copy protections, CSS keys, and bad sector traps without demuxer crashes.
  2. *Stage 2*: FFmpeg takes the clean intermediate file and compresses it to the final MP4 or MKV according to the user's selected video codec (H.264, HEVC, AV1), CRF, preset, hardware acceleration (NVENC, QSV, VAAPI), audio downmix, and subtitles.
  3. *Cleanup*: RAII [`TempResource`] automatically deletes the multi-gigabyte intermediate file upon completion, failure, or cancellation.
- **When to use**: Encrypted or damaged discs that require MakeMKV to extract safely, but where compact compressed files (~1–1.5 GB) are desired instead of raw 4–8 GB MPEG-2 bitstreams.

### 2.6. Auto-Fallback Engine (`--engine auto`) [Default]
- **Adapter**: `crate::infrastructure::engines::AutoFallbackTranscoder`
- **How it works**: Orchestrates an intelligent priority cascade:
  1. Attempts primary extraction via **FFmpeg**.
  2. If FFmpeg fails with a copy protection, demuxer, or bad-sector error, automatically falls back to **Native IFO Cell Extractor**.
  3. If Native IFO trips its consecutive damaged block circuit breaker, it advances to **MakeMKV** (or **MakeMKV+FFmpeg** when compression is enabled) for low-level SCSI pass-through.
  4. If MakeMKV is unavailable or fails, it calls **HandBrakeCLI**.
- **When to use**: Recommended for all automated, headless daemon, and desktop usage. For a deep dive, see [`docs/OPTICAL_DISC_SCRATCH_AND_ERROR_RECOVERY.md`](OPTICAL_DISC_SCRATCH_AND_ERROR_RECOVERY.md).

---

## 3. Configuration & CLI Usage

### Command-Line Arguments
```bash
# Force pure Rust Native IFO cell extractor (bypasses FFmpeg demuxer entirely)
dvd-ripper --cli D: --engine native --search "The Matrix"

# Use HandBrakeCLI with custom binary location
dvd-ripper --cli D: --engine handbrake --handbrake "C:\Program Files\HandBrake\HandBrakeCLI.exe"

# Use MakeMKV for lossless robot-mode remuxing
dvd-ripper --cli D: --engine makemkv --makemkv "C:\Program Files (x86)\MakeMKV\makemkvcon64.exe"

# Default auto-fallback with all tools available in PATH or auto-discovered
dvd-ripper --cli D: --engine auto
```

### Configuration File (`dvd-ripper.toml`)
```toml
# Default transcoding engine: "auto", "ffmpeg", "native", "handbrake", or "makemkv"
engine = "auto"

# Custom executable paths for third-party engines (optional; auto-discovered by default)
handbrake = "HandBrakeCLI"
makemkv = "makemkvcon"
```

### Desktop GUI
In the desktop GUI:
1. Open the **Rip Disc** tab.
2. In the **Quality & Video Profile** section, open the **Transcoding Engine** dropdown.
3. Select **Auto Fallback**, **Native IFO Extractor**, **HandBrakeCLI**, or **MakeMKV**.
4. Configure or browse binary paths in the **Settings** tab via the **`📂 Browse...`** buttons if executables are installed in custom non-system directories.

---

## 4. Evaluation of Legacy Engines (MEncoder, transcode, dvdbackup)

During the engine architectural evaluation, historical CLI utilities including `mencoder` (MPlayer project), `transcode`, and `dvdbackup` were scrutinized for potential inclusion as fallback backends.

While `mencoder` pioneered early 3:2 pulldown IVTC (`ivtc`, `pullup`), its single-threaded MPEG demuxer, lack of modern container support (Matroska/MP4 with HEVC/AV1), and vulnerability to ARccOS intentionally corrupt sector traps make it obsolete for contemporary workloads.

For a comprehensive historical post-mortem, code comparisons, and technical analysis, see:  
👉 **[`docs/MENCODER_AND_LEGACY_CODEC_STUDY.md`](MENCODER_AND_LEGACY_CODEC_STUDY.md)**.

