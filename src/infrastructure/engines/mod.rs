//! Optical ripping and transcoding engine implementations.
//!
//! Exposes alternative backends implementing `domain::ports::Transcoder`:
//! - [`FfmpegTranscoder`]: Default FFmpeg transcoding & lossless remuxing engine
//! - [`NativeIfoTranscoder`]: Pure Rust IFO Cell-Sector extraction bypassing FFmpeg demuxer bugs
//! - [`HandBrakeTranscoder`]: HandBrakeCLI adapter for robust optical transcode & deinterlacing
//! - [`MakeMkvTranscoder`]: MakeMKV CLI adapter for AACS/BD+/CSS decryption & lossless MKV remuxing
//! - [`MakeMkvFfmpegTranscoder`]: Two-stage pipeline adapter combining MakeMKV decryption with FFmpeg compression
//! - [`AutoFallbackTranscoder`]: Composite fallback orchestrator across multiple engines

pub mod fallback;
pub mod handbrake;
pub mod makemkv;
pub mod makemkv_ffmpeg;
pub mod native_cell_extractor;

pub use fallback::AutoFallbackTranscoder;
pub use handbrake::HandBrakeTranscoder;
pub use makemkv::MakeMkvTranscoder;
pub use makemkv_ffmpeg::MakeMkvFfmpegTranscoder;
pub use native_cell_extractor::NativeIfoTranscoder;
