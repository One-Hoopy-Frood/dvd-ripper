//! Interface Segregation Principle (ISP/SOLID): Segregated option value objects replacing monolithic `Args` dependencies.

use serde::{Deserialize, Serialize};

/// Segregated video codec and rate-control specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoEncodingSpec {
    pub codec: String,
    pub profile: String,
    pub preset: String,
    pub crf: Option<u8>,
    pub target_size_mb: Option<u32>,
    pub hwaccel: String,
    pub hwaccel_fallback: bool,
    pub tonemap: Option<String>,
}

impl Default for VideoEncodingSpec {
    fn default() -> Self {
        Self {
            codec: "h264".to_string(),
            profile: "standard".to_string(),
            preset: "slow".to_string(),
            crf: None,
            target_size_mb: None,
            hwaccel: "none".to_string(),
            hwaccel_fallback: true,
            tonemap: None,
        }
    }
}

/// Segregated video filter pipeline specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VideoFilterSpec {
    pub deinterlace: bool,
    pub deinterlace_algo: Option<String>,
    pub denoise: bool,
    pub auto_crop: bool,
    pub detelecine: bool,
}

/// Segregated audio stream selection and loudness normalization specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AudioEncodingSpec {
    pub all_audio: bool,
    pub normalize_audio: bool,
    pub norm_target: Option<i32>,
    pub dual_audio: bool,
    pub audio_track: Option<u32>,
    pub audio_lang: Option<String>,
    pub auto_audio_pref: Option<String>,
    pub audio_title: Option<String>,
    pub audio_downmix: Option<String>,
}

/// Segregated subtitle extraction, burn-in, and OCR specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SubtitleSpec {
    pub subtitles: bool,
    pub sub_lang: Option<String>,
    pub sub_format: Option<String>,
    pub sub_forced_only: bool,
    pub sub_default: bool,
    pub sub_burnin: bool,
    pub sub_external_srt: bool,
    pub ocr: bool,
    pub ocr_lang: String,
}

/// Segregated optical disc title, chapter, angle, and error-resilience specification.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiscSelectionSpec {
    pub title_number: u32,
    pub start_chapter: Option<u32>,
    pub end_chapter: Option<u32>,
    pub split_chapters: bool,
    pub angle: Option<u32>,
    pub rip_extras: bool,
    pub sector_retries: u32,
}

/// Segregated output file and container naming specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputNamingSpec {
    pub out_dir: String,
    pub films_dir: String,
    pub tv_dir: String,
    pub explicit_output: Option<String>,
    pub mkv: bool,
    pub iso: bool,
    pub no_overwrite: bool,
}

impl Default for OutputNamingSpec {
    fn default() -> Self {
        Self {
            out_dir: "Films".to_string(),
            films_dir: "Films".to_string(),
            tv_dir: "TV".to_string(),
            explicit_output: None,
            mkv: false,
            iso: false,
            no_overwrite: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segregated_specs_defaults() {
        let video = VideoEncodingSpec::default();
        assert_eq!(video.codec, "h264");
        assert_eq!(video.preset, "slow");
        assert_eq!(video.crf, None);

        let filters = VideoFilterSpec::default();
        assert!(!filters.auto_crop);
        assert!(!filters.detelecine);

        let audio = AudioEncodingSpec::default();
        assert!(!audio.all_audio);

        let subs = SubtitleSpec::default();
        assert!(!subs.subtitles);

        let disc = DiscSelectionSpec::default();
        assert_eq!(disc.title_number, 0);
        assert_eq!(disc.start_chapter, None);

        let naming = OutputNamingSpec::default();
        assert_eq!(naming.out_dir, "Films");
        assert!(!naming.mkv);
        assert!(!naming.iso);
    }
}
