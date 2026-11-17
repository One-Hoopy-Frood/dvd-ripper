# Rust Library Crate Developer Guide - `dvd-ripper`

`dvd-ripper` is designed as a modular, production-grade Rust library adhering to **Clean Architecture** and **SOLID Principles**. It allows external Rust applications, background services, and automated media pipelines to interact with physical optical media, execute transcoding engines, query metadata, and manage batch ripping workflows programmatically.

---

## 1. Adding `dvd-ripper` as a Dependency

Add `dvd-ripper` to your `Cargo.toml`:

```toml
[dependencies]
# Headless CLI & daemon usage (recommended for servers and services)
dvd-ripper = { version = "0.4.0", default-features = false, features = ["cli"] }

# Minimal core library (pure domain entities, ports, and heuristics only)
# dvd-ripper = { version = "0.4.0", default-features = false, features = ["core"] }

# Full desktop installation including native immediate-mode GUI
# dvd-ripper = { version = "0.4.0", features = ["gui"] }
```

### Cargo Feature Flags

| Feature | Description | Default |
| :--- | :--- | :---: |
| `cli` | Enables CLI parsing (`clap`), ANSI terminal formatting, and CLI runner use cases. | **Yes** |
| `core` | Pure domain models, port interfaces, heuristics, and in-memory test doubles with zero GUI dependencies. | No |
| `gui` | Compiles the native desktop GUI powered by `eframe`, `egui`, `rfd`, and `image`. | No |

---

## 2. The Library Prelude

The [`prelude`](file:///c:/Users/User/.gemini/antigravity-ide/scratch/dvd-ripper/src/prelude.rs) module re-exports the most commonly used domain entities, port traits, application use cases, and testing doubles:

```rust
use dvd_ripper::prelude::*;
```

### Exported Types Overview

- **Domain Entities**: `DvdSource`, `DvdTitleInfo`, `TvEpisodeInfo`, `FilmMetadata`, `RipRecord`, `DiscInfo`, `DiscOptions`, `DriveStatusInfo`, `ProgressEvent`.
- **Domain Ports**: `Transcoder`, `OpticalDrive`, `DiscInspector`, `TitleProber`, `TrayController`, `MetadataProvider`, `HistoryRepository`, `DiskSpaceGuard`, `Notifier`.
- **Composition Root**: `AppContainer`, `AppInitializer`.
- **Test Doubles**: `VirtualOpticalDrive`, `InMemoryHistoryRepository`, `RecordingNotifier`, `FixedDiskSpaceGuard`, `RecordingProgressObserver`.

---

## 3. The Composition Root (`AppContainer`)

The `AppContainer` acts as the dependency injection composition root. It instantiates, configures, and wires concrete infrastructure adapters to abstract domain ports.

### 3.1 Quick Start: Production Container

```rust,no_run
use dvd_ripper::prelude::*;

fn main() -> anyhow::Result<()> {
    // Builds production container with system FFmpeg binary
    let container = AppContainer::build_production("ffmpeg");

    // Retrieve configured use case instances
    let probe_uc = container.probe_disc_use_case();
    
    Ok(())
}
```

### 3.2 Fluent Builder Configuration

For customized pipelines, use `AppContainer::builder()`:

```rust,no_run
use dvd_ripper::prelude::*;
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    let container = AppContainer::builder()
        .with_ffmpeg("/usr/local/bin/ffmpeg")
        .with_disk_guard(Arc::new(FixedDiskSpaceGuard::new(50))) // 50 GB threshold
        .build();

    Ok(())
}
```

---

## 4. Programmatic Use Case Invocations

### 4.1 Probing Optical Discs (`ProbeDiscUseCase`)

Inspect optical disc volume labels, copy protection structures, and title durations:

```rust,no_run
use dvd_ripper::prelude::*;
use dvd_ripper::infrastructure::optical::PhysicalOpticalDrive;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let container = AppContainer::build_production("ffmpeg");
    let probe_uc = container.probe_disc_use_case();

    // Wrap physical drive (Windows "D:\" or Linux "/dev/sr0")
    let drive = PhysicalOpticalDrive::new(
        "D:\\",
        "ffmpeg",
        container.acoustic_supervisor.clone(),
    );

    let probe_result = probe_uc.execute(&drive)?;
    println!("Volume Label: {:?}", probe_result.disc_info.volume_label);
    println!("Primary Title: #{}", probe_result.primary_title_num);
    println!("Total Titles: {}", probe_result.disc_info.titles.len());

    Ok(())
}
```

### 4.2 Single Title Ripping (`RipSingleTitleUseCase`)

Execute single movie or documentary title extraction with progress tracking:

```rust,no_run
use dvd_ripper::prelude::*;
use dvd_ripper::application::rip_single_title::RipSingleTitleRequest;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let container = AppContainer::build_production("ffmpeg");
    let rip_uc = container.rip_single_title_use_case();

    let request = RipSingleTitleRequest::builder("D:\\", PathBuf::from("C:\\Media\\DVD Rips"))
        .title_number(1)
        .film_name("Inception")
        .film_year(2010)
        .mkv(true)
        .build();

    let outcome = rip_uc.execute(&request)?;
    println!("Rip completed to: {}", outcome.output_file.display());
    Ok(())
}
```

### 4.3 Multi-Episode TV Series Batch Ripping (`RipTvBatchUseCase`)

Extract multiple episodes with TVMaze metadata integration:

```rust,no_run
use dvd_ripper::prelude::*;
use dvd_ripper::application::rip_tv_batch::RipTvBatchRequest;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let container = AppContainer::build_production("ffmpeg");
    let batch_uc = container.rip_tv_batch_use_case();

    let request = RipTvBatchRequest::builder("D:\\", PathBuf::from("C:\\Media\\TV"))
        .series_title("Breaking Bad")
        .series_year(2008)
        .season(1)
        .start_episode(1)
        .all_episodes(true)
        .build();

    let results = batch_uc.execute(&request)?;
    println!("Extracted {} TV episodes.", results.len());
    Ok(())
}
```

### 4.4 Bit-Exact Archival ISO Dumping (`DumpIsoUseCase`)

Create 1:1 sector-by-sector archival ISO images:

```rust,no_run
use dvd_ripper::prelude::*;
use dvd_ripper::application::dump_iso::DumpIsoRequest;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let container = AppContainer::build_production("ffmpeg");
    let iso_uc = container.dump_iso_use_case();

    let request = DumpIsoRequest::builder("D:\\", PathBuf::from("C:\\Backups\\Disc.iso"))
        .build();

    iso_uc.execute(&request)?;
    println!("ISO created successfully.");
    Ok(())
}
```

---

## 5. Optical Source Abstraction (`DvdSource`)

The `DvdSource` enum normalizes optical sources across operating systems:

```rust
use dvd_ripper::prelude::*;
use std::path::PathBuf;

// Windows Drive Letter
let src1 = DvdSource::parse("D:");
assert_eq!(src1, DvdSource::DriveLetter('D'));

// Linux Device Path
let src2 = DvdSource::parse("/dev/sr0");
assert_eq!(src2, DvdSource::Device(PathBuf::from("/dev/sr0")));

// Archival Disc Image
let src3 = DvdSource::parse("C:\\Movies\\backup.iso");
assert_eq!(src3, DvdSource::IsoImage(PathBuf::from("C:\\Movies\\backup.iso")));
```

---

## 6. Testing Downstream Applications with Test Doubles

`dvd-ripper` provides thread-safe in-memory test doubles that allow downstream crates and microservices to test ripping workflows in CI environments without physical optical drives, filesystems, or FFmpeg binaries:

```rust
use dvd_ripper::prelude::*;
use std::path::Path;

#[test]
fn test_my_service_with_virtual_drive() {
    // 1. Setup Virtual Optical Drive
    let drive = VirtualOpticalDrive::new("D:\\")
        .with_label("THE_MATRIX")
        .with_fingerprint("matrix_fp_1999")
        .with_titles(vec![
            DvdTitleInfo {
                title_num: 1,
                duration_secs: 8160.0, // 2h 16m
            },
            DvdTitleInfo {
                title_num: 2,
                duration_secs: 300.0,  // Trailer
            },
        ]);

    assert_eq!(drive.get_path(), Path::new("D:\\"));
    assert_eq!(drive.read_volume_label(), Some("THE_MATRIX".to_string()));
    assert_eq!(drive.probe_titles().len(), 2);

    // 2. Setup In-Memory History Ledger
    let history = InMemoryHistoryRepository::new();
    let record = RipRecord::new("The Matrix", "Movie", "C:\\out.mkv", "Success");
    history.append_record(&record, None).unwrap();
    assert_eq!(history.load_history(None).len(), 1);

    // 3. Setup Recording Notifier
    let notifier = RecordingNotifier::new();
    notifier.send(&NotificationEvent::RipSuccess {
        title: "The Matrix".to_string(),
        output_file: "out.mkv".to_string(),
    });
    assert_eq!(notifier.events().len(), 1);
}
```

---

## 7. Error Handling

All use cases return standard `anyhow::Result<T>` with rich contextual errors. Specific domain errors are categorized in `DvdError`:

```rust
use dvd_ripper::prelude::*;

match result {
    Ok(path) => println!("Success: {}", path.display()),
    Err(err) => {
        eprintln!("Operation failed: {:#}", err);
    }
}
```
