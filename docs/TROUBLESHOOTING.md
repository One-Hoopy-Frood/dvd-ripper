# DVD Ripper Troubleshooting & Diagnostic Guide

This guide details common diagnostic workflows, hardware throughput bottlenecks, copy protection errors, disk space safeguards, and video quality tuning for `dvd-ripper`.

---

## 1. Optical Drive Throughput & RipLock Diagnostics

### Symptom: Slow Ripping Read Speed (< 4 MB/s or < 2x Speed)
Commercial DVD drives often ship with firmware **RipLock** enabled, restricting optical disc read speeds to 2x (approx. 2.7 MB/s) during video playback.

### Diagnostic Command
Run the built-in Optical Drive Read Speed Benchmark:

```bash
dvd-ripper --input D:\ --benchmark
```

### Expected Output
- **High Performance (16x+ Speed)**: `>= 15.0 MB/s` (Ideal for fast batch ripping).
- **Standard Read Speed (8x Speed)**: `8.0 – 15.0 MB/s`.
- **RipLocked / Bottlenecked (< 4x Speed)**: `< 4.0 MB/s`.

### Solutions
1. **Flash Unlocked Firmware**: Flash your optical drive with LibreDrive or MCSE unlocked firmware to remove RipLock read limits.
2. **Check USB/SATA Cables**: Ensure external USB drives use USB 3.0 ports (blue ports) rather than USB 2.0 hubs.

---

## 2. Copy Protection (CSS/CPPM) & libdvdcss Errors

### Symptom: FFmpeg cannot rip encrypted DVD ("CSS support is unavailable")
Commercial encrypted DVDs require `libdvdcss` to decrypt scrambled sector keys during extraction.

### Solution:
Install `libdvdcss` using your operating system package manager:
- **macOS**: `brew install libdvdcss`
- **Linux (Debian/Ubuntu)**: `sudo apt install libdvd-pkg && sudo dpkg-reconfigure libdvd-pkg`
- **Windows**: `choco install libdvdcss` or place `libdvdcss-2.dll` in your system `PATH`.

For full cross-platform instructions, see **[FFmpeg & libdvdcss Installation Guide](DEPENDENCIES_INSTALLATION.md)**.

---

### Symptom: FFmpeg exits with `Invalid IFO for title 0 (VIDEO_TS.IFO)`
On commercial discs with Sony ARccOS or corrupted root tables, FFmpeg's `libdvdread` demuxer aborts with:
```text
[in#0] libdvdread: Invalid IFO for title 0 (VIDEO_TS.IFO).
[in#0] libdvdread: Invalid IFO for title 0 (VIDEO_TS.BUP).
[in#0] Error opening input: Generic error in an external library
```

### Solutions:
1. **Rely on Auto-Fallback (`--engine auto`)**:
   `dvd-ripper` automatically catches FFmpeg demuxer errors and triggers fallback to the pure Rust **Native IFO Extractor** (`native_ifo`), which reads `.BUP` backup tables and directly parses title sets without touching the corrupted root table:
   ```text
   [Auto Engine Fallback] Primary engine encountered an error. Falling back to engine 'native_ifo' (2/2)...
   ```
2. **Force Native IFO Extractor via CLI or GUI**:
   ```bash
   dvd-ripper --cli D: --engine native --search "Movie Title"
   ```
3. **Use MakeMKV for Heavily Encrypted Discs**:
   ```bash
   dvd-ripper --cli D: --engine makemkv
   ```

---

### Symptom: Ripping hangs or produces I/O Errors at specific chapters (e.g. stops at 62.8%)
Commercial DVDs use deliberate bad sector structures (Sony ARccOS / Macrovision RipGuard) positioned in unreferenced gaps between chapters to cause digital read head locks.

### Solutions:
1. **Switch to Native IFO Engine (`--engine native`)**:
   The native extractor only extracts logical sectors explicitly referenced by the Program Chain (`VTS_PGCITI`), completely jumping over unreferenced bad sector traps.
2. **Enable Scratched Disc Fault Tolerance**:
   ```bash
   dvd-ripper --cli D: --ignore-errors
   ```
3. See **[Copy Protection, Bad Sectors, and Disc Recovery Guide](COPY_PROTECTION_AND_BAD_SECTORS.md)** for in-depth diagnostic details.

---

## 3. Enterprise Disk Space Guard Alerts

### Symptom: Ripping aborts with `Disk Space Guard Safeguard Triggered`
Before initiating ripping jobs, `dvd-ripper` inspects available free disk space on the target destination partition.

### Error Message
```text
Disk Space Guard Safeguard Triggered: Only 4.50 GB free space available in 'C:\Films', but minimum threshold is 10 GB.
```

### Solutions
1. **Adjust Minimum Threshold**: Lower `--min-free-gb` if operating on small partitions:
   ```bash
   dvd-ripper --input D:\ --search "Aliens" --min-free-gb 5
   ```
2. **Specify External Output Directory**: Point `--out-dir` to an external hard drive or NAS share with ample free space:
   ```bash
   dvd-ripper --input D:\ --search "Aliens" --out-dir "E:\Films"
   ```

---

## 4. Video Interlacing Comb Artifacts

### Symptom: Horizontal jagged lines or comb artifacts on fast motion
Standard-definition DVDs (NTSC 480i / PAL 576i) store video using interlaced fields.

### Solution
Enable motion-adaptive deinterlacing (`--deinterlace`) using the `bwdif` algorithm:

```bash
dvd-ripper --input D:\ --search "Dr Who" --transcode --deinterlace --deinterlace-algo bwdif
```

---

## 5. Network Notifications & Webhook Failures

### Discord / Slack / Webhook HTTP 400 Errors
- Ensure `--webhook-url` contains the full HTTPS webhook endpoint URL (e.g. `https://discord.com/api/webhooks/...`).

### Home Assistant MQTT Disconnections
- Ensure your MQTT broker URI includes the protocol scheme: `--mqtt-broker mqtt://192.168.1.50:1883`.

---

## 6. Optical Drive Freezes on Damaged / Corrupted Sectors

### Symptom: Ripping stalls for minutes at a specific point on scratched discs
When optical drive lasers encounter physical scratches or oxidized disc sectors:
- FFmpeg's `dvdnav` demuxer outputs:
  ```text
  [in#0/dvdvideo] Unable to read next block of PGC (title=2 pgc=2 pg=4 cell=4)
  [in#0/dvdvideo] dvdnav error: Error reading from DVD.
  ```
- Windows `cdrom.sys` blocks synchronously for 10–30 seconds per sector during hardware retry attempts.

### Built-in Protections & Solutions:
1. **Instant Cancellation**:
   - The sector reader checks atomic cancellation flags before every single 2 KB sector read.
   - Click **⏹ Cancel Ripping** in the GUI or press `Ctrl+C` in the CLI to abort immediately without waiting for hardware drive timeouts.
2. **Automatic Circuit Breakers**:
   - **Sector Short-Circuit**: If 2 consecutive single-sector reads fail in a 64 KB block, the engine skips the remaining 30 sectors and pads with zeroed filler to avoid minutes of optical drive thrashing.
   - **8-Block Bailout**: If 8 consecutive 64 KB blocks fail (~512 KB), the Native IFO engine aborts with a clear message and passes control to **MakeMKV** in the automatic cascade.
3. **Throttling Drive Speed**:
   - Set spindle speed to **Silent (4x)** in GUI settings or pass `--acoustic-profile silent`. Lower rotational speed substantially improves tracking over warped or scratched discs.
4. For full details on scratch recovery, see **[Optical Disc Scratch & Error Recovery Guide](OPTICAL_DISC_SCRATCH_AND_ERROR_RECOVERY.md)**.

---

## 7. External Engine Binary Discovery & Execution

### Symptom: `Failed to spawn makemkvcon` or `Failed to spawn HandBrakeCLI`
Occurs if third-party binaries are selected or required by the fallback cascade but are not located in standard search paths.

### Solutions:
1. **Automatic Discovery Paths**:
   - `dvd-ripper` automatically searches standard system locations:
     - **Windows**: `C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`, `C:\Program Files\MakeMKV\makemkvcon64.exe`, `C:\Program Files\HandBrake\HandBrakeCLI.exe`.
     - **macOS**: `/Applications/MakeMKV.app/Contents/MacOS/makemkvcon`, `/opt/homebrew/bin/makemkvcon`.
     - **Linux**: `/usr/bin/makemkvcon`, `/usr/local/bin/makemkvcon`.
2. **Set Custom Binary Location via GUI**:
   - Switch to the **⚙️ Settings** tab.
   - Under **Engine & Binary Overrides**, click **`📂 Browse...`** next to **HandBrakeCLI Path** or **MakeMKV Path** to locate your executable. Changes are auto-saved to `config.toml`.
3. **Specify via CLI**:
   - Pass `--makemkv "C:\Path\To\makemkvcon64.exe"` or `--handbrake "C:\Path\To\HandBrakeCLI.exe"`.

---

## 8. Diagnostic File Logging & Real-Time Log Viewer

### Symptom: Diagnosing unattended crashes or inspecting transcoder command flags
To view all FFmpeg stdout/stderr diagnostics, sector retry logs, and queue dispatch events:

1. **Open via GUI**:
   - Navigate to the **⚙️ Settings** tab.
   - Scroll to **📝 Application Logging**.
   - Verify **Enable Logging to File** is checked.
   - Click **`📄 Open Log File`** to immediately open the runtime log file in your system's default text editor.
2. **Access Directly on Disk**:
   - Windows: `%USERPROFILE%\.dvd-ripper\dvd-ripper.log`
   - Linux / macOS: `~/.dvd-ripper/dvd-ripper.log`
3. See **[Persistent Ripping History & Logging Guide](RIPPING_HISTORY_AND_LOGGING.md)** for full logging configuration options.
