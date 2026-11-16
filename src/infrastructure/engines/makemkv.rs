//! MakeMKV CLI (`makemkvcon`) Transcoder adapter implementing the domain `Transcoder` port.
//!
//! Executes `makemkvcon` in robot mode (`-r --progress=-same`) for lossless `.mkv` remuxing
//! with hardware-level optical decryption (AACS, BD+, CSS, structural obfuscation).

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow};

use crate::domain::ports::transcoder::{
    ProgressObserver, TranscodeJob, Transcoder,
};
use crate::infrastructure::ffmpeg::ChildGuard;

/// Transcoder implementation powered by MakeMKV's `makemkvcon`.
pub struct MakeMkvTranscoder {
    makemkv_path: String,
}

impl MakeMkvTranscoder {
    /// Creates a new `MakeMkvTranscoder`, automatically resolving default paths if necessary.
    pub fn new(makemkv_path: impl Into<String>) -> Self {
        let raw = makemkv_path.into();
        let resolved = Self::resolve_path(&raw);
        Self {
            makemkv_path: resolved,
        }
    }

    /// Resolves the MakeMKV CLI executable path.
    ///
    /// If the configured path is explicitly set to an existing file, it is returned.
    /// If the default executable name ("makemkvcon" / "makemkv" or empty) is used,
    /// it checks whether it is available in system PATH; if not, it automatically searches
    /// well-known standard installation paths (e.g. `C:\Program Files (x86)\MakeMKV\makemkvcon64.exe`).
    pub fn resolve_path(configured_path: &str) -> String {
        let trimmed = configured_path.trim();
        if trimmed.is_empty()
            || trimmed.eq_ignore_ascii_case("makemkvcon")
            || trimmed.eq_ignore_ascii_case("makemkv")
        {
            if Self::check_binary(trimmed) {
                return trimmed.to_string();
            }
            if let Some(found) = Self::find_installed_makemkv() {
                return found;
            }
            return trimmed.to_string();
        }

        let p = Path::new(trimmed);
        if p.is_file() || Self::check_binary(trimmed) {
            return trimmed.to_string();
        }

        trimmed.to_string()
    }

    /// Searches well-known operating system default installation directories for MakeMKV.
    pub fn find_installed_makemkv() -> Option<String> {
        let mut candidates = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
                candidates.push(format!(r"{}\MakeMKV\makemkvcon64.exe", pf86));
                candidates.push(format!(r"{}\MakeMKV\makemkvcon.exe", pf86));
            }
            if let Ok(pf) = std::env::var("ProgramFiles") {
                candidates.push(format!(r"{}\MakeMKV\makemkvcon64.exe", pf));
                candidates.push(format!(r"{}\MakeMKV\makemkvcon.exe", pf));
            }
            candidates.push(r"C:\Program Files (x86)\MakeMKV\makemkvcon64.exe".to_string());
            candidates.push(r"C:\Program Files (x86)\MakeMKV\makemkvcon.exe".to_string());
            candidates.push(r"C:\Program Files\MakeMKV\makemkvcon64.exe".to_string());
            candidates.push(r"C:\Program Files\MakeMKV\makemkvcon.exe".to_string());
        }

        #[cfg(target_os = "macos")]
        {
            candidates.push("/Applications/MakeMKV.app/Contents/MacOS/makemkvcon".to_string());
            candidates.push("/usr/local/bin/makemkvcon".to_string());
            candidates.push("/opt/homebrew/bin/makemkvcon".to_string());
        }

        #[cfg(not(target_os = "windows"))]
        {
            candidates.push("/usr/bin/makemkvcon".to_string());
            candidates.push("/usr/local/bin/makemkvcon".to_string());
        }

        for candidate in candidates {
            if Path::new(&candidate).is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Internal probe verifying whether a binary path can be executed and responds.
    fn check_binary(path: &str) -> bool {
        let mut cmd = Command::new(path);
        crate::utils::configure_silent_command(&mut cmd);
        cmd.arg("-r")
            .arg("info")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success() || s.code() == Some(1))
            .unwrap_or(false)
    }

    /// Checks whether `makemkvcon` is available in PATH or at the specified executable path.
    pub fn is_available(path: &str) -> bool {
        let resolved = Self::resolve_path(path);
        Self::check_binary(&resolved)
    }

    /// Parses MakeMKV robot progress line:
    /// `PRGV:current,total,max` e.g. `PRGV:32768,65536,65536` -> 50.0%
    /// MakeMKV reports current sub-task progress and overall batch total against a constant
    /// maximum scale (typically 65536). Dividing by `max` produces smooth linear progress,
    /// preventing erratic jumps across operation phases.
    pub fn parse_progress_line(line: &str) -> Option<f64> {
        if !line.starts_with("PRGV:") {
            return None;
        }
        let parts: Vec<&str> = line[5..].split(',').collect();
        if parts.len() >= 3 {
            let current = parts[0].trim().parse::<f64>().ok()?;
            let total = parts[1].trim().parse::<f64>().ok()?;
            let max = parts[2].trim().parse::<f64>().ok()?;
            if max > 0.0 {
                // When ripping a single title, `current` tracks progress of the active title
                // while `total` tracks overall batch progress against `max`.
                let val = if current > 0.0 { current } else { total };
                return Some(((val / max) * 100.0).clamp(0.0, 99.9));
            }
        } else if parts.len() == 2 {
            let current = parts[0].trim().parse::<f64>().ok()?;
            let max = parts[1].trim().parse::<f64>().ok()?;
            if max > 0.0 {
                return Some(((current / max) * 100.0).clamp(0.0, 99.9));
            }
        }
        None
    }

    /// Parses MakeMKV robot operation title lines:
    /// `PRGC:code,id,"name"` or `PRGT:code,id,"name"` e.g. `PRGC:5014,0,"Saving 1 titles"`
    pub fn parse_operation_title(line: &str) -> Option<String> {
        if line.starts_with("PRGC:") || line.starts_with("PRGT:") {
            if let Some(start) = line.find('"') {
                if let Some(end) = line.rfind('"') {
                    if end > start {
                        let text = &line[start + 1..end];
                        if !text.is_empty() {
                            return Some(text.to_string());
                        }
                    }
                }
            }
        }
        None
    }
}

impl Transcoder for MakeMkvTranscoder {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let out_dir = job.output_path.parent().unwrap_or(Path::new("."));
        if !out_dir.exists() {
            std::fs::create_dir_all(out_dir)?;
        }

        // Snapshot existing .mkv files in out_dir before running MakeMKV
        let existing_mkvs: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(out_dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("mkv"))
                        .unwrap_or(false)
            })
            .collect();

        // makemkvcon mkv disc:<id> or dev:<dev> or file:<path> <title_index> <dest_dir>
        let mut cmd = Command::new(&self.makemkv_path);
        crate::utils::configure_silent_command(&mut cmd);

        cmd.arg("-r")
            .arg("--messages=-stdout")
            .arg("--progress=-stdout")
            .arg("mkv");

        let src_str = job.source_dvd.to_string_lossy();
        let trimmed_src = src_str.trim();
        if trimmed_src.ends_with(".iso") || trimmed_src.ends_with(".ISO") {
            cmd.arg(format!("iso:{}", trimmed_src));
        } else if trimmed_src.contains("VIDEO_TS") || trimmed_src.contains("video_ts") {
            cmd.arg(format!("file:{}", trimmed_src));
        } else {
            let clean = trimmed_src.trim_end_matches('\\').trim_end_matches('/');
            if clean.len() == 2 && clean.ends_with(':') {
                cmd.arg(format!("dev:{}", clean));
            } else if clean.starts_with("/dev/") {
                cmd.arg(format!("dev:{}", clean));
            } else if clean.starts_with("disc:")
                || clean.starts_with("dev:")
                || clean.starts_with("file:")
                || clean.starts_with("iso:")
            {
                cmd.arg(clean);
            } else {
                cmd.arg("disc:0");
            }
        }

        // Title index in makemkv is 0-based for specific title or "all"
        let title_arg = if job.title_number > 0 {
            (job.title_number - 1).to_string()
        } else {
            "all".to_string()
        };
        cmd.arg(title_arg);
        cmd.arg(out_dir);

        if let Some(obs) = observer {
            obs.on_log(&format!(
                "[MakeMKV Engine] Ripping title to directory '{}'...",
                out_dir.display()
            ));
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let raw_child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn makemkvcon ('{}'). Ensure MakeMKV is installed.",
                self.makemkv_path
            )
        })?;
        let mut child = ChildGuard::new(raw_child);

        let stdout = child
            .as_mut()
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture makemkvcon stdout"))?;
        let reader = BufReader::new(stdout);

        let mut current_phase = "Initializing MakeMKV...".to_string();
        let mut is_saving = false;
        let mut max_saving_pct: f64 = 0.0;

        if let Some(obs) = observer {
            obs.on_progress(0.0, "N/A", &current_phase);
        }

        for line_res in reader.lines() {
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                let _ = child.as_mut().kill();
                let _ = child.as_mut().wait();
                return Err(anyhow!("MakeMKV operation cancelled by user."));
            }

            if let Ok(line) = line_res {
                if let Some(phase) = Self::parse_operation_title(&line) {
                    if phase.to_lowercase().contains("saving") {
                        is_saving = true;
                    }
                    current_phase = phase;
                    if !is_saving {
                        if let Some(obs) = observer {
                            obs.on_progress(0.0, "N/A", &current_phase);
                        }
                    }
                } else if line.starts_with("MSG:3007") {
                    current_phase = "Scanning disc structure & decrypting CSS keys...".to_string();
                    if let Some(obs) = observer {
                        obs.on_progress(0.0, "N/A", &current_phase);
                    }
                } else if line.starts_with("MSG:3028") {
                    current_phase = "Analyzing DVD titles...".to_string();
                    if let Some(obs) = observer {
                        obs.on_progress(0.0, "N/A", &current_phase);
                    }
                } else if line.starts_with("MSG:5014") {
                    current_phase = "Saving title to MKV...".to_string();
                    is_saving = true;
                    if let Some(obs) = observer {
                        obs.on_progress(0.0, "N/A", &current_phase);
                    }
                }

                if let Some(pct) = Self::parse_progress_line(&line) {
                    if is_saving {
                        max_saving_pct = max_saving_pct.max(pct);
                        if let Some(obs) = observer {
                            obs.on_progress(max_saving_pct, "N/A", &current_phase);
                        }
                    }
                } else if line.starts_with("MSG:")
                    && let Some(obs) = observer
                {
                    obs.on_log(&format!("[MakeMKV] {}", line));
                }
            }
        }

        let status = child
            .as_mut()
            .wait()
            .context("Failed to wait on makemkvcon process")?;
        child.disarm();

        if !status.success() {
            return Err(anyhow!(
                "makemkvcon exited with non-zero code: {:?}",
                status.code()
            ));
        }

        // MakeMKV outputs title00.mkv, title_t00.mkv, or <DISCLABEL>_t00.mkv into out_dir.
        // Identify newly created .mkv and rename to match job.output_path if specified.
        let dest = if job.output_path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("mkv")).unwrap_or(false) {
            job.output_path.to_path_buf()
        } else {
            job.output_path.with_extension("mkv")
        };

        if let Ok(entries) = std::fs::read_dir(out_dir) {
            let mut candidates: Vec<std::path::PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("mkv"))
                            .unwrap_or(false)
                        && p != &dest
                        && p != job.output_path
                        && !existing_mkvs.contains(p)
                })
                .collect();

            // Sort candidates by modification time descending to pick the freshest file
            candidates.sort_by_key(|p| p.metadata().and_then(|m| m.modified()).ok());

            let chosen = candidates.pop().or_else(|| {
                // Secondary fallback scan by typical MakeMKV title prefixes
                if let Ok(fallback_entries) = std::fs::read_dir(out_dir) {
                    for entry in fallback_entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("mkv")).unwrap_or(false)
                            && path != dest
                            && path != job.output_path
                            && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                            && (file_name.starts_with("title") || file_name.starts_with("t0") || file_name.contains("_t"))
                        {
                            return Some(path);
                        }
                    }
                }
                None
            });

            if let Some(target) = chosen {
                // Remove pre-existing dest if present to avoid Windows ERROR_ALREADY_EXISTS
                if dest.exists() {
                    let _ = std::fs::remove_file(&dest);
                }
                // Clean up any stale partial mp4 / temporary file from previous failed attempts
                if job.output_path.exists() && job.output_path != dest {
                    let _ = std::fs::remove_file(job.output_path);
                }
                let tmp_vob = job.output_path.with_extension("native_cells.tmp.vob");
                if tmp_vob.exists() {
                    let _ = std::fs::remove_file(&tmp_vob);
                }

                if let Err(e) = std::fs::rename(&target, &dest) {
                    // Fallback to copy + remove for cross-filesystem moves
                    if let Err(copy_err) = std::fs::copy(&target, &dest).and_then(|_| std::fs::remove_file(&target)) {
                        if let Some(obs) = observer {
                            obs.on_log(&format!(
                                "[MakeMKV] Warning: could not move '{}' to '{}': {} / {}",
                                target.display(), dest.display(), e, copy_err
                            ));
                        }
                    } else if let Some(obs) = observer {
                        obs.on_log(&format!(
                            "[MakeMKV] Output saved as '{}'",
                            dest.display()
                        ));
                    }
                } else if let Some(obs) = observer {
                    obs.on_log(&format!(
                        "[MakeMKV] Output saved as '{}'",
                        dest.display()
                    ));
                }
            }
        }

        if let Some(obs) = observer {
            obs.on_progress(100.0, "N/A", "Done");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_makemkv_progress() {
        let line = "PRGV:32768,65536,65536";
        let pct = MakeMkvTranscoder::parse_progress_line(line);
        assert!(pct.is_some());
        assert!((pct.unwrap() - 50.0).abs() < 0.1);

        // MakeMKV sub-step vs overall progress: dividing by max (65536) yields correct pct
        let sub_step = "PRGV:3744,313,65536";
        let pct_sub = MakeMkvTranscoder::parse_progress_line(sub_step);
        assert!(pct_sub.is_some());
        assert!((pct_sub.unwrap() - (3744.0 / 65536.0 * 100.0)).abs() < 0.01);

        let finished = "PRGV:65536,65536,65536";
        let pct_fin = MakeMkvTranscoder::parse_progress_line(finished);
        assert!(pct_fin.is_some());
        assert!((pct_fin.unwrap() - 99.9).abs() < 0.1);

        // Invalid cases
        assert!(MakeMkvTranscoder::parse_progress_line("MSG:5004,0,0").is_none());
        assert!(MakeMkvTranscoder::parse_progress_line("PRGV:").is_none());
        assert!(MakeMkvTranscoder::parse_progress_line("PRGV:abc,def").is_none());
        assert!(MakeMkvTranscoder::parse_progress_line("PRGV:10,0").is_none());
        assert!(MakeMkvTranscoder::parse_progress_line("PRGV:10,0,0").is_none());
    }

    #[test]
    fn test_parse_makemkv_operation_title() {
        assert_eq!(
            MakeMkvTranscoder::parse_operation_title("PRGC:5018,0,\"Scanning CD-ROM devices\""),
            Some("Scanning CD-ROM devices".to_string())
        );
        assert_eq!(
            MakeMkvTranscoder::parse_operation_title("PRGT:3404,0,\"Opening Blu-ray disc\""),
            Some("Opening Blu-ray disc".to_string())
        );
        assert_eq!(
            MakeMkvTranscoder::parse_operation_title("PRGC:5014,0,\"Saving 1 titles\""),
            Some("Saving 1 titles".to_string())
        );
        assert_eq!(
            MakeMkvTranscoder::parse_operation_title("MSG:1005,0,1,\"MakeMKV v2.0.0 started\""),
            None
        );
    }

    #[test]
    fn test_makemkv_is_available_nonexistent() {
        assert!(!MakeMkvTranscoder::is_available("nonexistent_makemkvcon_xyz_1234"));
    }

    #[test]
    fn test_makemkv_transcode_nonexistent_binary() {
        let transcoder = MakeMkvTranscoder::new("nonexistent_makemkvcon_xyz_1234");
        let tmp = std::env::temp_dir().join(format!("mmkv_test_{}", std::process::id()));
        let out_file = tmp.join("out.mkv");
        let job = TranscodeJob {
            source_dvd: Path::new("D:\\"),
            output_path: &out_file,
            title_number: 1,
            display_title: "Title 1",
            expected_duration_secs: Some(3600.0),
        };

        let res = transcoder.transcode(&job, None, None);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Failed to spawn makemkvcon"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_makemkv_transcode_iso_source_arg() {
        let transcoder = MakeMkvTranscoder::new("nonexistent_makemkvcon_xyz_1234");
        let tmp = std::env::temp_dir().join(format!("mmkv_iso_test_{}", std::process::id()));
        let out_file = tmp.join("out.mkv");
        let job = TranscodeJob {
            source_dvd: Path::new("C:\\movies\\sample.iso"),
            output_path: &out_file,
            title_number: 0,
            display_title: "Main",
            expected_duration_secs: None,
        };

        let res = transcoder.transcode(&job, None, None);
        assert!(res.is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
