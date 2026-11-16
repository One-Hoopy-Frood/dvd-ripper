# Optical Disc Scratch & Error Recovery Guide - `dvd-ripper`

This guide explains how `dvd-ripper` handles physically scratched, damaged, oxidized (disc rot), or copy-protected optical media, detailing the internal recovery architecture, circuit breakers, and engine cascade.

---

## 1. The Physics of DVD Read Failures

Optical media discs are read using a focused 650 nm red laser tracking a microscopic spiral groove of pits and lands. Common causes of read interruptions include:
- **Radial Scratches**: Scratches perpendicular to the spiral track. Often correctable via built-in Reed-Solomon Product Code (RS-PC) error correction.
- **Circumferential Scratches**: Scratches that follow the disc rotation curve. These obscure hundreds of consecutive sectors, exhausting hardware error correction.
- **Deliberate Bad Sectors (Copy Protection)**: Technologies such as Sony ARccOS and RipGuard intentionally master unreadable, corrupt sectors in fake navigation cells or dummy tracks to trigger buffer underruns in naive rippers.
- **Disc Rot & Delamination**: Chemical oxidation of the reflective aluminum layer creating microscopic pinholes.

### Why Operating Systems Hang on Bad Sectors
When a damaged sector is encountered, the optical drive firmware attempts multiple re-reads, focusing sweeps, and tracking adjustments. At the operating system layer:
- On Windows, `cdrom.sys` blocks the issuing thread in a synchronous `ReadFile` call for **10 to 30 seconds per unreadable 2 KB sector**.
- If a ripper attempts naive sector-by-sector retries across a damaged 64 KB block (32 sectors), a single damaged block can lock up the I/O bus and freeze the application for **5 to 15 minutes**.

---

## 2. Multi-Engine Auto-Fallback Cascade

To handle varying types of disc damage and copy protections without user intervention, `dvd-ripper` provides the `auto` engine cascade:

```
[Disc Input]
     │
     ▼
┌───────────────────────────────┐
│ 1. FFmpeg dvdvideo Demuxer    │ ──(Success)──▶ Final Transcode / Remux
└───────────────────────────────┘
     │ (Fails on PGC block error / corrupt cell)
     ▼
┌───────────────────────────────┐
│ 2. Native IFO Cell Extractor  │ ──(Success)──▶ Final Transcode / Remux
└───────────────────────────────┘
     │ (Trips circuit breaker after corrupted chunks)
     ▼
┌───────────────────────────────┐
│ 3. MakeMKV (makemkvcon)       │ ──(Success)──▶ Final MKV Remux
└───────────────────────────────┘
     │ (If unavailable or fails)
     ▼
┌───────────────────────────────┐
│ 4. HandBrake (HandBrakeCLI)   │ ──(Success)──▶ Final HandBrake Output
└───────────────────────────────┘
```

---

## 3. Native IFO Cell Extractor Circuit Breakers

When FFmpeg's `dvdnav` demuxer fails with errors like:
```text
[in#0/dvdvideo] Unable to read next block of PGC
[in#0/dvdvideo] dvdnav error: Error reading from DVD.
```
The engine automatically falls back to the pure Rust **Native IFO Cell Extractor**. To prevent infinite hardware hangs on physical media, the extractor employs three protective mechanisms:

### 3.1 Responsive Instant Cancellation
- Optical drive I/O operations normally block standard thread cancellation.
- The sector-by-sector read loop actively checks `cancel_flag.is_cancelled()` before **every single sector read request**.
- Clicking **Cancel** in the GUI or sending `SIGINT` (Ctrl+C) in the CLI interrupts reading immediately without waiting for hardware drive timeouts.

### 3.2 Sector-Level Short-Circuit
- DVD Video VOB sectors are structured in 64 KB chunks (32 sectors of 2048 bytes).
- If a 64 KB read fails, the extractor falls back to reading 2 KB sectors individually.
- **Circuit Breaker**: If **2 consecutive single-sector reads fail** within the same 64 KB block, the engine recognizes an unreadable cluster, stops attempting the remaining 30 sectors, and pads the missing sectors with zeroed MPEG-2 filler packets. This cuts drive laser thrashing time from 10 minutes down to 20 seconds.

### 3.3 Consecutive Damaged Block Threshold (8-Chunk Bailout)
- If an optical disc has a major physical gouge spanning several consecutive blocks, continuing native file I/O will severely wear out the optical drive laser.
- **Circuit Breaker**: If **8 consecutive 64 KB chunks** (~512 KB of data) cannot be read, the native extractor aborts with:
  ```text
  Excessive consecutive read errors encountered (...). Aborting native extraction to allow fallback engine.
  ```
- This triggers the automatic fallback cascade to advance to **MakeMKV** (Engine 3).

---

## 4. MakeMKV Low-Level SCSI Pass-Through

MakeMKV interacts with optical hardware differently than standard demuxers:
- Bypasses filesystem-level abstractions and communicates directly with optical drives via SCSI Pass-Through Direct (SPTI on Windows, SG on Linux).
- Employs intelligent hardware error handling that instructs the drive firmware to minimize retries on unreadable sectors rather than stalling.
- Decrypts CSS and AACS bus encryption on the fly.
- `dvd-ripper` automatically searches standard installation directories (e.g. `C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`) or allows manual binary path configuration in Settings.

---

## 5. Practical Steps for Recovering Badly Scratched Discs

If a DVD rip stalls or reports block read errors:

### 1. Reduce Optical Spindle Speed
High rotational speeds (16x+) cause optical discs to vibrate and wobble, increasing laser tracking errors on scratches.
- In GUI: Set **Acoustic Profile** to **Silent (4x)** in Disc & Ripping settings.
- In CLI: Pass `--acoustic-profile silent`.
- Running at 4x provides significantly higher read torque and vastly improved tracking over surface scratches.

### 2. Enable Fault-Tolerant Demuxing
Pass the `--ignore-errors` flag (or check **Ignore Read Errors** in GUI Settings). This activates:
- FFmpeg: `-fflags +genpts+discardcorrupt -err_detect ignore_err`.
- Prevents FFmpeg from aborting when it encounters non-monotonic presentation timestamps (PTS) caused by dropped sectors.

### 3. Force MakeMKV Engine Directly
If you know a disc has ARccOS protection or heavy scratching:
- In GUI: Change Transcoding Engine dropdown to **MakeMKV (makemkvcon)**.
- In CLI: Pass `--engine makemkv`.

### 4. Physical Cleaning
- Always clean optical discs radially (from the center hole straight outward to the rim), never in a circular motion. Circular scratches follow the data track and cause far more damage.
- Use lukewarm water with mild dish soap or isopropyl alcohol (90%+) and a microfiber cloth.
