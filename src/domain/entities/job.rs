use serde::{Deserialize, Serialize};

/// Execution status of an optical rip job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

/// Dynamic progress metrics emitted during transcoding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RipProgress {
    pub percent: f64,
    pub fps: String,
    pub speed: String,
    pub eta_secs: Option<u64>,
}

/// Historical record of a completed, cancelled, or failed rip.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RipRecord {
    #[serde(alias = "title_name")]
    pub title: String,
    pub media_type: String,
    pub output_path: String,
    pub timestamp: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disc_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_codec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitles_enabled: Option<bool>,
}

impl RipRecord {
    pub fn new(title: &str, media_type: &str, output_path: &str, status: &str) -> Self {
        let now = chrono::Local::now();
        Self {
            title: title.to_string(),
            media_type: media_type.to_string(),
            output_path: output_path.to_string(),
            timestamp: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            status: status.to_string(),
            disc_label: None,
            year: None,
            duration_secs: None,
            file_size_bytes: None,
            video_codec: None,
            audio_lang: None,
            subtitles_enabled: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_details(
        title: &str,
        media_type: &str,
        output_path: &str,
        status: &str,
        disc_label: Option<String>,
        year: Option<u32>,
        duration_secs: Option<f64>,
        file_size_bytes: Option<u64>,
        video_codec: Option<String>,
        audio_lang: Option<String>,
        subtitles_enabled: Option<bool>,
    ) -> Self {
        let now = chrono::Local::now();
        Self {
            title: title.to_string(),
            media_type: media_type.to_string(),
            output_path: output_path.to_string(),
            timestamp: now.format("%Y-%m-%d %H:%M:%S").to_string(),
            status: status.to_string(),
            disc_label,
            year,
            duration_secs,
            file_size_bytes,
            video_codec,
            audio_lang,
            subtitles_enabled,
        }
    }
}

/// Represents a single queued ripping job item in the batch processing engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobItem {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub drive: String,
    pub status: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spool_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub priority: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_number: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
}

/// Represents a tracked TV series season box set state across multi-disc insertions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoxSetRecord {
    pub show_name: String,
    pub season: u32,
    pub last_episode: u32,
    pub total_discs_ripped: u32,
    pub updated_at: String,
}
