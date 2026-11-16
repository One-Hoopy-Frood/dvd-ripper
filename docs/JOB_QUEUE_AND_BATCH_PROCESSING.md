# Job Queue & Batch Processing Guide - `dvd-ripper`

`dvd-ripper` features a robust, multi-job asynchronous Queue Management system designed for unattended bulk ripping, multi-title TV series extraction, and disc autoloader processing.

---

## 1. Overview & Architecture

When ripping DVDs containing multiple episodes, bonus features, or multi-angle cuts, processing titles one by one manually is time-consuming. The Job Queue system decouples title extraction configuration from execution:

- **Enqueue Any Title or Disc**: Configure ripping options, audio/subtitle tracks, and destination filenames for each job independently, then append them to the queue.
- **Asynchronous Execution**: The queue runs sequentially in a dedicated background worker thread, ensuring the GUI remains 100% responsive at 60 FPS.
- **Drive Tray Management & Ejection Suppression**: During multi-job queue execution on a single optical disc, automatic drive tray ejection is intelligently **suppressed** between jobs. The tray only ejects after the final queued job completes (unless `--no-eject` is active), preventing aborted jobs due to premature disc ejection.
- **Fault-Tolerant Progress**: If a single job encounters an unrecoverable optical read error or fails, the queue logs the error, flags the item as `Failed`, and automatically progresses to the remaining queued jobs without halting the entire batch.

---

## 2. Using the Job Queue in the GUI

The graphical interface provides a dedicated **📋 Job Queue** tab (Tab 6):

### 2.1 Adding Jobs to the Queue
1. Scan your DVD in the **💿 Disc & Ripping** tab.
2. Select a title track, audio stream, subtitle preferences, and output filename.
3. Click **➕ Add to Queue** (instead of "Rip Now").
4. Repeat for any additional titles or episodes you wish to process from the disc.
5. In the **📋 Job Queue** tab, review the pending jobs.

### 2.2 Managing the Queue
- **▶ Start Queue**: Dispatches the first `Pending` job to the background transcoder engine.
- **⏸ Pause / Resume Queue**: Pauses dispatching of subsequent jobs after the active job finishes, or resumes processing.
- **⏹ Cancel Current Job**: Cancels the actively running rip and moves the queue forward or pauses.
- **❌ Remove Selected**: Removes highlighted pending jobs from the queue list.
- **🗑 Clear Completed**: Cleans up finished or failed jobs from the view.
- **⬆ Move Up / ⬇ Move Down**: Reorders the priority of pending jobs.

### 2.3 Job Status Lifecycle

```
[Pending] ──▶ [Processing] ──┬──▶ [Completed]
                             ├──▶ [Failed]
                             └──▶ [Cancelled]
```

- `Pending`: Waiting in queue for an available processing slot.
- `Processing`: Actively demuxing/transcoding; shows live frame rate, elapsed time, and percentage progress.
- `Completed`: Successfully transcoded, tagged, and finalized with NFO/checksum sidecars.
- `Failed`: Encountered an unrecoverable demux/encoding error; detailed diagnostic log recorded.
- `Cancelled`: Terminated by user interaction before completion.

---

## 3. Disc Tray Ejection Suppression Logic

A common pitfall with automated disc rippers is ejecting the optical disc drive tray after the first title finishes, corrupting or failing subsequent titles in the queue.

`dvd-ripper` implements automated ejection suppression:
1. When starting a job from the queue, `state.is_processing_queue` is flagged as `true`.
2. When an individual job finishes, the engine checks whether additional pending jobs targeting the same physical optical drive remain in the queue:
   - **If more jobs exist**: The completion handler suppresses optical tray ejection, resets the engine state, and dispatches the next job immediately.
   - **If no jobs remain**: The queue processing concludes, and tray ejection proceeds according to the user's `--no-eject` and `eject_autoclose` preferences.

---

## 4. TV Series Box Set Auto-Batching

For multi-disc TV box sets, the queue works seamlessly with the **📺 TV Box Sets** tab:
1. Scan a multi-episode disc.
2. The TV title matcher maps Title 1, 2, 3, 4 to `S01E01`, `S01E02`, `S01E03`, `S01E04`.
3. Click **Enqueue All Detected Episodes**: All detected episodes are parsed and pushed into the Job Queue as discrete jobs with correct Plex/Kodi season/episode filenames.
4. Click **Start Queue** in Tab 6: The entire disc is ripped sequentially into your organized library folder without any manual intervention.

---

## 5. Headless & CLI Batch Execution

In CLI mode, multi-title ripping can be executed via the TV batch mode or by chaining commands:

```bash
# Batch rip all episodes on DVD drive D: matching duration filter (20 to 65 minutes)
dvd-ripper -i D:\ --auto-boxset --min-duration 1200 --max-duration 3900 --no-eject

# Batch rip entire TV series disc with error tolerance and subrip extraction
dvd-ripper -i D:\ --auto-boxset --ignore-errors --subtitles --sub-format subrip --out-dir "C:\Media\TV"
```

---

## 6. Historical Logging & Queue Auditing

Every queued job records execution metrics:
- Logged into the unified application file log (`dvd-ripper.log`).
- Recorded into persistent history (`ripping_history.json`) with start time, completion duration, output file size, and target engine.
- Emits structured JSON-Lines events when configured with `--audit-log audit.jsonl`.
