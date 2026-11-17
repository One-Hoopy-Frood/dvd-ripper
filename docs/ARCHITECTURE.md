# System Architecture Guide - `dvd-ripper`

## 1. Overview

`dvd-ripper` is an enterprise-grade DVD backup and media processing appliance written in pure Rust. It features a modern **Clean Architecture** (Hexagonal / Ports & Adapters) design governed by the **SOLID Principles**, providing:
- A native desktop immediate-mode GUI powered by `eframe`/`egui`.
- A command-line interface (CLI) powered by `clap`.
- An embedded HTTP REST API, OpenAPI specification, and Server-Sent Events (SSE) server (`api`).
- A multi-drive parallel watcher daemon (`daemon`).
- Home Assistant MQTT smart home integration (`mqtt`) and multi-service HTTP webhooks.
- Disc fingerprinting and local database caching (`dvd`, `imdb`).
- Direct media server scan triggers (Plex, Jellyfin, Emby) and post-processing script hooks (`utils`).
- Pure domain heuristics for PAL 25fps speedup and multi-title decoy loop detection.

For an in-depth breakdown of the 5 SOLID principles and design patterns applied in this codebase, see the dedicated companion specification:  
👉 **[`docs/SOLID_ARCHITECTURE.md`](SOLID_ARCHITECTURE.md)**

---

## 2. Clean Architecture Layer Hierarchy

The codebase is organized into four concentric architectural layers:

```mermaid
graph TD
    subgraph UI_and_Presentation ["1. Presentation & Driving Layer"]
        CLI["CLI: src/cli.rs & src/main.rs"]
        GUI["Desktop GUI: src/gui/ (Modular)"]
        API["REST API, WS & SSE: src/api.rs"]
        Daemon["Watcher Daemon: src/daemon.rs"]
    end

    subgraph Composition_Root ["Composition Root"]
        AppInit["AppInitializer: src/main.rs"]
        AppContainer["AppContainer: src/main.rs"]
    end

    subgraph Application_Layer ["2. Application Use Cases"]
        ProbeDisc["ProbeDiscUseCase"]
        RipSingle["RipSingleTitleUseCase"]
        RipBatch["RipTvBatchUseCase"]
        DualOutput["DualOutputRipUseCase (StreamTee)"]
        DrivePool["DrivePool"]
    end

    subgraph Domain_Layer ["3. Core Domain Layer (Pure Rust)"]
        Entities["Entities: Disc, Metadata, Rip, Drive"]
        Heuristics["Heuristics: PAL Speedup, Cluster Ranking, TV Detection, Telecine IVTC"]
        Ports["Ports (Traits): Transcoder, Optical, Storage, Metadata, AcousticSupervisor, Rosetta, etc."]
    end

    subgraph Infrastructure_Layer ["4. Infrastructure Adapters (Driven)"]
        EnginesInfra["Transcoder Engines: Native IFO, HandBrake, MakeMKV, Fallback"]
        PipelineInfra["Pipeline: StreamTee 128KB Dual-Output Multiplexer"]
        FFmpegInfra["FFmpeg Builder & Process Runner"]
        OpticalInfra["Win32 / Linux ISO-9660 Reader & SCSI Acoustic Controller"]
        MetadataInfra["OMDb / IMDb Client & Local Rosetta Stone Registry"]
        StorageInfra["Disk Space Guard & JSON History"]
        NotifyInfra["MQTT 3.1.1 & Rich Webhooks (Discord/Telegram)"]
        OCRInfra["Tesseract OCR & CLUT Palette Normalizer"]
        SystemInfra["Autonomous Polling Media Monitor & Dialogs"]
    end

    UI_and_Presentation --> AppContainer
    AppContainer --> Application_Layer
    Application_Layer --> Domain_Layer
    Infrastructure_Layer -.->|Implements Ports| Domain_Layer
    Application_Layer -->|Dispatches via Ports| Domain_Layer
```

---

## 3. Module Responsibilities

### 3.1 Presentation & Driving Adapters
- **`src/main.rs`**: CLI composition root, CLI flag dispatch, and default runtime parameter merging.
- **`src/lib.rs`**: Core library entry point, Cargo feature flag gating (`cli`, `gui`, `core`), Clean Architecture module definitions, and pruned public surface.
- **`src/prelude.rs`**: Convenient library prelude for external crate consumers (`use dvd_ripper::prelude::*;`).
- **`src/cli.rs`**: Schema definitions for command-line arguments (`Args`), segregated configuration traits (`TranscodeConfig`, `AudioConfig`, `SubtitleConfig`).
- **`src/gui/`**: Immediate-mode desktop GUI built with `eframe`/`egui`. Includes disc search modals, live progress gauges, output folder management (`📂 Browse...`, `↗ Open`, `↺ Reset`), and cooperative rip cancellation.
- **`src/api.rs`**: Embedded HTTP REST API server with connection rate-limiting (`MAX_CONCURRENT_API_CONNECTIONS = 32`), Slowloris timeouts (`10s`), Bearer token authentication (`--api-key`), OpenAPI v3 JSON (`/api/openapi.json`), Server-Sent Events live progress stream (`/api/events`), Prometheus metrics (`/metrics`), and Web UI dashboard.
- **`src/daemon.rs`**: Headless multi-drive watcher daemon that monitors optical drives, automatically detects inserted discs, fetches metadata, and coordinates ripping pipelines.

### 3.2 Application Use Cases (`src/application/`)
- **`probe_disc.rs`**: Coordinates optical drive inspection, volume label extraction, and title probing.
- **`rip_single_title.rs`**: Orchestrates single movie rip pipelines, including disk space verification, output folder routing, transcoding, subtitle extraction, and history recording.
- **`rip_tv_batch.rs`**: Orchestrates multi-episode TV series batch ripping, cumulative episode numbering, and cooperative cancellation.
- **`dump_iso.rs`**: Bit-exact 1:1 sector-by-sector archival optical disc imaging.
- **`dual_output_rip.rs`**: Single-pass dual output multiplexer (StreamTee) simultaneously writing lossless archival master and streaming mp4.
- **`drive_pool.rs`**: Manages concurrent multi-drive ripping jobs and thread-safe drive status updates.

### 3.3 Domain Layer (`src/domain/`)
- **`entities/`**: Core data models (`DvdSource`, `DvdTitleInfo`, `TvEpisodeInfo`, `FilmMetadata`, `RipRecord`, `DiscInfo`, `DiscOptions`, `DriveStatusInfo`, `JobItem`, `BoxSetRecord`, `ProgressEvent`).
- **`naming.rs`**: Pure domain Plex/Jellyfin naming taxonomy, sibling folder routing math, and filename sanitization.
- **`heuristics/`**:
  - `speedup.rs`: PAL 25fps vs 23.976fps runtime adjustment (4% duration discrepancy resolution).
  - `cluster_detector.rs`: Title duration clustering and deterministic ranking to defeat anti-copying decoy loops (e.g. John Wick 3).
  - `tv_detector.rs`: Filter out studio intros and composite "Play All" tracks.
- **`ports/`**: Abstract interfaces (`Transcoder`, `DriveDetector`, `DiscInspector`, `TrayController`, `MetadataProvider`, `HistoryRepository`, `DiskSpaceGuard`, `Notifier`, `FileDialogService`, and in-memory test doubles).

### 3.4 Infrastructure Adapters (`src/infrastructure/`)
- **`engines/`**:
  - `native_cell_extractor.rs`: Pure Rust 2048-byte sector Program Chain (`VTS_PGCITI`) extractor with `.BUP` backup table support, dual sector/block circuit breakers, and bad sector fault recovery.
  - `handbrake.rs`: HandBrakeCLI adapter leveraging `libdvdnav` virtual machine navigation, hardware decombing, and auto-path discovery.
  - `makemkv.rs`: MakeMKV robot-mode adapter (`makemkvcon -r --messages=-stdout --progress=-stdout`) with drive bus key extraction (AACS/CSS/BD+), zero-deadlock pipes, TV batch episode ripping, and auto-discovery.
  - `fallback.rs`: Composite priority cascade orchestrator (`AutoFallbackTranscoder`).
- **`ffmpeg/`**: Modular FFmpeg command assembly (`builder.rs`), process management (`process_runner.rs` with `ChildGuard`), and zero-allocation progress parsing (`progress_parser.rs`).
- **`optical/`**: Platform-specific optical drive readers (Windows Win32 API, Linux sysfs, ISO-9660 Sector 16 volume descriptor reader, sub-10ms binary `ifo_parser.rs`, and extracted `disc_analysis.rs`).
- **`metadata/`**: Online metadata fetching via OMDb and TMDb APIs with persistent local fingerprint caching (`fingerprints.json`) and local Rosetta Stone Registry.
- **`storage/`**: Enterprise disk space guard checks (`disk_guard.rs`), atomic file writers, runtime diagnostic file logger (`dvd-ripper.log`), and thread-safe JSON history repository (`ripping_history.json`).
- **`notifications/`**: Home Assistant MQTT 3.1.1 auto-discovery telemetry and HTTP webhooks (Discord, Telegram, Ntfy, Gotify, Slack).
- **`ocr/`**: On-the-fly Tesseract OCR execution converting bitmap `dvdsub` overlays to `.srt` SubRip text sidecars.

---

## 4. Media Output Organization & Domain Naming

Output paths are automatically separated into clean, media-server-compliant hierarchies governed by pure domain naming logic (`src/domain/naming.rs` and re-exported in `src/ffmpeg.rs`):
- **Films**: `<out_dir>/<films_dir>/<Movie Title (Year)>/<Movie Title (Year)>.<ext>` (default subfolder: `Films`)
- **TV Series**: `<out_dir>/<tv_dir>/<Show Name (Year)>/Season <SS>/<Show Name - S<SS>E<EE>>.<ext>` (default subfolder: `TV`)

### Sibling Root Auto-Resolution & Custom Taxonomies
The path resolution engine (`resolve_films_root_with_tv`, `resolve_tv_root_with_films`, `resolve_films_root_custom`, `resolve_tv_root_custom`) intelligently handles custom destination directories and user-defined subfolder names:
- If `out_dir` ends in the TV subfolder (`TV`, `Series`, `TV Shows`), films are automatically routed into the configured films folder (`Films`, `Movies`, etc.) under the common parent directory.
- If `out_dir` ends in the films subfolder (`Films`, `Movies`, `Cinema`), TV series are automatically routed into the configured TV folder under the common parent directory.
- Redundant nesting (such as `Movies/Movies` or `Films/Films`) is systematically prevented.

For full details, see [`docs/MEDIA_ORGANIZATION.md`](MEDIA_ORGANIZATION.md) and [`docs/CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md`](CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md).

---

## 5. Testing & Verification

The codebase contains **467 automated tests** (421 unit tests, 4 installer binary tests, 9 CLI tests, 13 LSP contract tests, and 20 doc-tests) achieving **>83% line coverage** across 34,000+ lines of code:
- Pure domain heuristic validation (PAL speedup calculations, cluster ranking, duration tolerances).
- In-memory mock tests for application use cases (`RipSingleTitleUseCase`, `RipTvBatchUseCase`, `DumpIsoUseCase`).
- Reusable test doubles in `domain::ports::testing` (`VirtualOpticalDrive`, `InMemoryHistoryRepository`, `RecordingNotifier`, `FixedDiskSpaceGuard`, `RecordingProgressObserver`).
- Liskov Substitution Principle (LSP) architectural contract tests in `tests/lsp_contract_tests.rs`.
- Cooperative cancellation propagation and child process teardown (`ChildGuard`).
- Transcoder engine adapters (Native IFO, HandBrake, MakeMKV, Fallback).
- Video filter graph generation (`bwdif`, `yadif`, `hqdn3d`).
- Hardware acceleration encoder selection (NVENC, VAAPI, QSV).
- ISO-9660 disc fingerprinting and volume label parsing.
- Disk space guards, collision prevention, and output path routing.

Execute the test suite locally with:

```bash
cargo test
```

---

## 6. Technical Documentation Index

| Guide | File Path | Focus Area |
|---|---|---|
| **Ripping Engines** | [`docs/RIPPING_ENGINES.md`](RIPPING_ENGINES.md) | FFmpeg, Native IFO, HandBrake, MakeMKV, & Auto-Fallback |
| **MEncoder & Legacy Codecs** | [`docs/MENCODER_AND_LEGACY_CODEC_STUDY.md`](MENCODER_AND_LEGACY_CODEC_STUDY.md) | Technical post-mortem, 3:2 pulldown history, & modern alternatives |
| **Optical I/O & Bus Operations** | [`docs/CROSS_PLATFORM_OPTICAL_IO.md`](CROSS_PLATFORM_OPTICAL_IO.md) | Win32 SCSI Pass-Through, Linux ioctls, RipLock, & tray controls |
| **Copy Protection & Recovery** | [`docs/COPY_PROTECTION_AND_BAD_SECTORS.md`](COPY_PROTECTION_AND_BAD_SECTORS.md) | CSS, ARccOS bad sectors, .BUP recovery, & sector rescue |
| **Software Quality Attributes** | [`docs/SOFTWARE_LIBRARY_ATTRIBUTES.md`](SOFTWARE_LIBRARY_ATTRIBUTES.md) | 10 Library Attributes: Security, Performance, Zero-Alloc |
| **Security & Sanitization** | [`docs/SECURITY_AND_PATH_TRAVERSAL.md`](SECURITY_AND_PATH_TRAVERSAL.md) | Path traversal prevention, CLI injection safety, & safe XML/NFO |
| **SOLID Architecture** | [`docs/SOLID_ARCHITECTURE.md`](SOLID_ARCHITECTURE.md) | Deep dive into SRP, OCP, LSP, ISP, DIP, heuristics, & mocks |
| **Testing & Coverage** | [`docs/TESTING_AND_COVERAGE.md`](TESTING_AND_COVERAGE.md) | 409+ tests, in-memory mocks, doc-tests, & coverage targets |
| **Media Organization** | [`docs/MEDIA_ORGANIZATION.md`](MEDIA_ORGANIZATION.md) | Folder conventions, sibling resolution, Plex/Jellyfin, & sidecars |
| **Custom Media Taxonomy** | [`docs/CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md`](CUSTOM_MEDIA_TAXONOMY_AND_NAMING.md) | Configurable Films/TV directories, sibling math, & API integration |
| **Developer Guide** | [`docs/DEVELOPER_GUIDE.md`](DEVELOPER_GUIDE.md) | Onboarding, coding standards, adding use cases & adapters |
| **CLI User Manual** | [`docs/CLI_GUIDE.md`](CLI_GUIDE.md) | Flags, syntax, video profiles, and automation examples |
| **Desktop GUI Guide** | [`docs/GUI_GUIDE.md`](GUI_GUIDE.md) | UI layout, search modal, progress gauges, & cancel flow |
| **Configuration Guide** | [`docs/CONFIGURATION.md`](CONFIGURATION.md) | `dvd-ripper.toml` parameters, priority order, & defaults |
| **REST API Reference** | [`docs/API_REFERENCE.md`](API_REFERENCE.md) | Endpoints, SSE streaming, OpenAPI v3 spec, & metrics |
| **Daemon Mode** | [`docs/DAEMON_APPLIANCE_MODE.md`](DAEMON_APPLIANCE_MODE.md) | Headless auto-rip, Home Assistant MQTT, & webhooks |
| **Hardware Acceleration** | [`docs/HARDWARE_ACCELERATION.md`](HARDWARE_ACCELERATION.md) | GPU transcoding (NVENC, VAAPI, QSV, V4L2) |
| **Media Server Integration**| [`docs/MEDIA_SERVER_INTEGRATION.md`](MEDIA_SERVER_INTEGRATION.md)| Plex/Jellyfin/Emby webhooks & NFO schemas |
| **TV Box Set Guide** | [`docs/TV_BOXSET_GUIDE.md`](TV_BOXSET_GUIDE.md) | Multi-disc TV season cumulative episode tracking |
| **Installer & Service Guide**| [`docs/INSTALLER_AND_SERVICE_GUIDE.md`](INSTALLER_AND_SERVICE_GUIDE.md)| Cross-platform installer CLI, systemd, udev, & PATH |
| **Troubleshooting Guide** | [`docs/TROUBLESHOOTING.md`](TROUBLESHOOTING.md) | RipLock bottlenecks, disk space guards, & CSS/CPPM |
| **Contributor Guide** | [`CONTRIBUTING.md`](../CONTRIBUTING.md) | Open source contribution workflow, style guide, & PRs |
