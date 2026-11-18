//! MakeMKV + FFmpeg Two-Stage Pipeline Transcoder adapter (`MakeMkvFfmpegTranscoder`).
//!
//! Orchestrates a two-stage ripping and compression pipeline:
//! 1. Stage 1 (MakeMKV): Executes `makemkvcon` in robot mode to handle copy protection
//!    (CSS, ARccOS, structural obfuscation) and physical read retries, outputting a temporary
//!    raw lossless `.mkv` bitstream.
//! 2. Stage 2 (FFmpeg): Takes the decrypted intermediate `.mkv` as standard file input and
//!    transcodes it into the final destination container (`.mp4` or `.mkv`) using user-configured
//!    codecs (H.264, HEVC, AV1), CRF quality targets, hardware acceleration, audio downmixing,
//!    and subtitle pass-through.
//! 3. RAII Cleanup: Automatically deletes the multi-gigabyte intermediate temporary file
//!    upon completion, failure, or user cancellation.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, anyhow};

use crate::domain::ports::transcoder::{
    ProgressObserver, TranscodeJob, TranscodeOptions, Transcoder,
};
use crate::infrastructure::engines::MakeMkvTranscoder;
use crate::infrastructure::ffmpeg::ChildGuard;
use crate::infrastructure::ffmpeg::progress_parser::parse_ffmpeg_progress_bytes;
use crate::infrastructure::storage::TempResource;

/// Two-stage pipeline transcoder: MakeMKV optical decryption -> FFmpeg compression pass.
pub struct MakeMkvFfmpegTranscoder {
    pub makemkv: MakeMkvTranscoder,
    pub ffmpeg_path: String,
    pub options: TranscodeOptions,
}

impl MakeMkvFfmpegTranscoder {
    /// Constructs a new `MakeMkvFfmpegTranscoder` with configured executable paths and options.
    pub fn new(
        makemkv_path: impl Into<String>,
        ffmpeg_path: impl Into<String>,
        options: TranscodeOptions,
    ) -> Self {
        Self {
            makemkv: MakeMkvTranscoder::new(makemkv_path),
            ffmpeg_path: ffmpeg_path.into(),
            options,
        }
    }

    /// Constructs a `MakeMkvFfmpegTranscoder` reusing an existing `MakeMkvTranscoder` instance.
    pub fn with_makemkv(
        makemkv: MakeMkvTranscoder,
        ffmpeg_path: impl Into<String>,
        options: TranscodeOptions,
    ) -> Self {
        Self {
            makemkv,
            ffmpeg_path: ffmpeg_path.into(),
            options,
        }
    }

    /// Builds the FFmpeg Command configured to transcode the decrypted intermediate MKV file.
    pub fn build_ffmpeg_command(&self, input_file: &Path, output_path: &Path) -> Command {
        let mut cmd = Command::new(&self.ffmpeg_path);
        crate::utils::configure_silent_command(&mut cmd);
        cmd.stdin(std::process::Stdio::null())
            .arg("-nostdin")
            .arg("-y")
            .arg("-fflags")
            .arg("+genpts+igndts")
            .arg("-i")
            .arg(input_file);

        // Video stream mapping (primary video stream)
        cmd.arg("-map").arg("0:v:0?");

        // Audio stream mapping
        if self.options.all_audio {
            cmd.arg("-map").arg("0:a?");
        } else if let Some(idx) = self.options.audio_track {
            cmd.arg("-map").arg(format!("0:a:{}?", idx));
        } else if let Some(ref lang) = self.options.audio_lang {
            cmd.arg("-map").arg(format!("0:a:m:language:{}?", lang));
        } else {
            cmd.arg("-map").arg("0:a:0?");
        }

        // Subtitle stream mapping
        let is_mkv = output_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("mkv"))
            .unwrap_or(false);

        if self.options.subtitles && is_mkv {
            if let Some(ref lang) = self.options.sub_lang {
                cmd.arg("-map").arg(format!("0:s:m:language:{}?", lang));
            } else {
                cmd.arg("-map").arg("0:s?");
            }
            cmd.arg("-c:s").arg("copy");
        }

        // Video filters (deinterlace, denoise)
        let mut vf_filters = Vec::new();
        if self.options.deinterlace {
            let algo = self.options.deinterlace_algo.as_deref().unwrap_or("yadif");
            vf_filters.push(algo.to_string());
        }
        if self.options.denoise {
            vf_filters.push("hqdn3d".to_string());
        }
        if !vf_filters.is_empty() {
            cmd.arg("-vf").arg(vf_filters.join(","));
        }

        // Video encoding options
        if !self.options.transcode || self.options.codec.eq_ignore_ascii_case("copy") {
            cmd.arg("-c:v").arg("copy");
        } else {
            if self.options.hwaccel != "none" && !self.options.hwaccel.is_empty() {
                crate::ffmpeg::apply_hwaccel_encoder(
                    &mut cmd,
                    &self.options.hwaccel,
                    &self.options.preset,
                );
            } else {
                let vcodec = crate::ffmpeg::resolve_video_codec_name(&self.options.codec);
                cmd.arg("-c:v").arg(vcodec);
                cmd.arg("-preset").arg(&self.options.preset);
            }
            cmd.arg("-crf")
                .arg(self.options.crf.unwrap_or(20).to_string());
        }

        // Audio encoding options
        if self.options.codec.eq_ignore_ascii_case("copy") {
            cmd.arg("-c:a").arg("copy");
        } else if self.options.normalize_audio {
            cmd.arg("-af").arg("loudnorm=I=-16:TP=-1.5:LRA=11");
            cmd.arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        } else {
            cmd.arg("-c:a").arg("aac").arg("-b:a").arg("192k");
        }

        // Chapters mapping
        cmd.arg("-map_chapters").arg("0");

        cmd.arg(output_path);
        cmd
    }

    /// Executes Stage 2 FFmpeg transcode with real-time progress parsing and cancellation support.
    fn run_ffmpeg_stage2(
        &self,
        input_file: &Path,
        output_path: &Path,
        expected_duration_secs: Option<f64>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        if output_path.exists() {
            let _ = std::fs::remove_file(output_path);
        }

        let mut cmd = self.build_ffmpeg_command(input_file, output_path);
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::null());

        let raw_child = cmd.spawn().with_context(|| {
            format!(
                "Failed to spawn FFmpeg process ('{}'). Is FFmpeg installed?",
                self.ffmpeg_path
            )
        })?;
        let mut child = ChildGuard::new(raw_child);

        let mut stderr = child
            .as_mut()
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture FFmpeg stderr"))?;

        let mut total_duration = expected_duration_secs.unwrap_or(0.0);
        let mut buf = [0u8; 1024];
        let mut line_bytes = Vec::new();

        loop {
            if let Some(ref flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                let _ = child.as_mut().kill();
                let _ = child.as_mut().wait();
                if output_path.exists() {
                    let _ = std::fs::remove_file(output_path);
                }
                return Err(anyhow!("FFmpeg transcode cancelled by user."));
            }

            match stderr.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    for &b in &buf[..n] {
                        if b == b'\r' || b == b'\n' {
                            if !line_bytes.is_empty() {
                                let line_str = String::from_utf8_lossy(&line_bytes);

                                if total_duration <= 0.0
                                    && let Some(pos) = line_str.find("Duration: ")
                                {
                                    let end = (pos + 21).min(line_str.len());
                                    if let Some(dur) =
                                        crate::utils::parse_duration(&line_str[pos + 10..end])
                                    {
                                        total_duration = dur;
                                    }
                                }

                                if let Some(slice) = parse_ffmpeg_progress_bytes(&line_bytes) {
                                    if total_duration > 0.0
                                        && let Some(time) = slice.time_secs
                                    {
                                        let pct = (time / total_duration * 100.0).clamp(0.0, 100.0);
                                        // Stage 2 is 50% to 100% of overall progress
                                        let overall_pct = 50.0 + (pct * 0.5);
                                        let fps_str = slice
                                            .fps
                                            .map(|f| format!("{:.1}", f))
                                            .unwrap_or_else(|| "N/A".to_string());
                                        let speed_str = format!(
                                            "[Stage 2/2: FFmpeg Compress] {}",
                                            slice.speed.unwrap_or("1.0x")
                                        );
                                        if let Some(obs) = observer {
                                            obs.on_progress(overall_pct, &fps_str, &speed_str);
                                        }
                                    }
                                } else if (line_str.starts_with('[')
                                    || line_str.contains("error")
                                    || line_str.contains("Error"))
                                    && let Some(obs) = observer
                                {
                                    obs.on_log(&line_str);
                                }
                                line_bytes.clear();
                            }
                        } else {
                            line_bytes.push(b);
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        let status = child
            .as_mut()
            .wait()
            .context("Failed to wait on FFmpeg process")?;
        child.disarm();

        if !status.success() {
            if output_path.exists() {
                let _ = std::fs::remove_file(output_path);
            }
            return Err(anyhow!(
                "FFmpeg transcoding exited with non-zero code: {:?}",
                status.code()
            ));
        }

        Ok(())
    }
}

impl Transcoder for MakeMkvFfmpegTranscoder {
    fn transcode(
        &self,
        job: &TranscodeJob,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        let out_dir = job.output_path.parent().unwrap_or_else(|| Path::new("."));
        if !out_dir.exists() {
            std::fs::create_dir_all(out_dir)?;
        }

        // Intermediate lossless MKV in destination folder for volume locality
        let temp_res = TempResource::in_directory(out_dir, "makemkv_intermediate", "mkv");
        let temp_mkv: PathBuf = temp_res.path().to_path_buf();

        if let Some(obs) = observer {
            obs.on_log(&format!(
                "[MakeMKV+FFmpeg Pipeline] Stage 1/2: Decrypting optical disc with MakeMKV to intermediate '{}'...",
                temp_mkv.display()
            ));
        }

        // --- STAGE 1: MakeMKV Optical Decryption & Lossless Rip ---
        let stage1_job = TranscodeJob {
            source_dvd: job.source_dvd,
            title_number: job.title_number,
            output_path: &temp_mkv,
            display_title: job.display_title,
            expected_duration_secs: job.expected_duration_secs,
        };

        struct Stage1Observer<'a> {
            inner: Option<&'a dyn ProgressObserver>,
        }
        impl<'a> ProgressObserver for Stage1Observer<'a> {
            fn on_progress(&self, percent: f64, fps: &str, speed: &str) {
                if let Some(obs) = self.inner {
                    // Map 0..100% of Stage 1 to 0..50% overall
                    let scaled = (percent * 0.5).clamp(0.0, 50.0);
                    let phase = format!("[Stage 1/2: MakeMKV Rip] {}", speed);
                    obs.on_progress(scaled, fps, &phase);
                }
            }
            fn on_log(&self, line: &str) {
                if let Some(obs) = self.inner {
                    obs.on_log(line);
                }
            }
        }

        let stage1_obs = Stage1Observer { inner: observer };
        self.makemkv
            .transcode(&stage1_job, Some(&stage1_obs), cancel_flag.clone())?;

        if !temp_mkv.exists() {
            return Err(anyhow!(
                "MakeMKV finished without producing expected intermediate file '{}'",
                temp_mkv.display()
            ));
        }

        if let Some(ref flag) = cancel_flag
            && flag.load(Ordering::SeqCst)
        {
            return Err(anyhow!("Ripping cancelled by user before Stage 2."));
        }

        if let Some(obs) = observer {
            obs.on_log(&format!(
                "[MakeMKV+FFmpeg Pipeline] Stage 2/2: Compressing intermediate file with FFmpeg to '{}'...",
                job.output_path.display()
            ));
        }

        // --- STAGE 2: FFmpeg Transcoding / Compression Pass ---
        self.run_ffmpeg_stage2(
            &temp_mkv,
            job.output_path,
            job.expected_duration_secs,
            observer,
            cancel_flag,
        )?;

        if let Some(obs) = observer {
            obs.on_progress(100.0, "N/A", "Complete");
            obs.on_log("[MakeMKV+FFmpeg Pipeline] Rip and transcode completed successfully.");
        }

        // `temp_res` drops here, automatically unlinking `temp_mkv`
        Ok(())
    }

    fn transcode_chapter_range(
        &self,
        job: &TranscodeJob,
        _chapter_range: Option<(u32, u32)>,
        observer: Option<&dyn ProgressObserver>,
        cancel_flag: Option<Arc<AtomicBool>>,
    ) -> Result<()> {
        self.transcode(job, observer, cancel_flag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_makemkv_ffmpeg_command_building() {
        let options = TranscodeOptions {
            codec: "hevc".to_string(),
            preset: "fast".to_string(),
            crf: Some(22),
            deinterlace: true,
            normalize_audio: true,
            subtitles: true,
            ..Default::default()
        };
        let pipeline = MakeMkvFfmpegTranscoder::new("makemkvcon", "ffmpeg", options);
        let in_file = Path::new("intermediate.mkv");
        let out_file = Path::new("final_movie.mkv");

        let cmd = pipeline.build_ffmpeg_command(in_file, out_file);
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"intermediate.mkv".to_string()));
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx265".to_string()));
        assert!(args.contains(&"-crf".to_string()));
        assert!(args.contains(&"22".to_string()));
        assert!(args.contains(&"-preset".to_string()));
        assert!(args.contains(&"fast".to_string()));
        assert!(args.contains(&"-vf".to_string()));
        assert!(args.contains(&"yadif".to_string()));
        assert!(args.contains(&"-af".to_string()));
        assert!(args.contains(&"loudnorm=I=-16:TP=-1.5:LRA=11".to_string()));
        assert!(args.contains(&"-map_chapters".to_string()));
        assert!(args.contains(&"final_movie.mkv".to_string()));
    }

    #[test]
    fn test_makemkv_ffmpeg_stream_copy_command() {
        let options = TranscodeOptions {
            codec: "copy".to_string(),
            transcode: false,
            ..Default::default()
        };
        let pipeline = MakeMkvFfmpegTranscoder::new("makemkvcon", "ffmpeg", options);
        let in_file = Path::new("intermediate.mkv");
        let out_file = Path::new("final_movie.mp4");

        let cmd = pipeline.build_ffmpeg_command(in_file, out_file);
        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"copy".to_string()));
        assert!(args.contains(&"-c:a".to_string()));
    }
}
