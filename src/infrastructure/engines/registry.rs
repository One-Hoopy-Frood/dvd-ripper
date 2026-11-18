//! Open/Closed Principle (OCP/SOLID): Dynamic Transcoder Registry and Factory SPI.
//!
//! Allows registering new transcoding engines without modifying core dispatchers or GUI controllers.

use super::{HandBrakeTranscoder, MakeMkvFfmpegTranscoder, MakeMkvTranscoder, NativeIfoTranscoder};
use crate::domain::ports::transcoder::{TranscodeOptions, Transcoder};
use crate::infrastructure::ffmpeg::FfmpegTranscoder;
use anyhow::{Result, anyhow};
use std::sync::Arc;

/// Configuration paths and binary identifiers for transcoding engines.
#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    pub ffmpeg_bin: String,
    pub makemkv_bin: String,
    pub handbrake_bin: String,
}

impl EngineConfig {
    pub fn new(
        ffmpeg_bin: impl Into<String>,
        makemkv_bin: impl Into<String>,
        handbrake_bin: impl Into<String>,
    ) -> Self {
        Self {
            ffmpeg_bin: ffmpeg_bin.into(),
            makemkv_bin: makemkv_bin.into(),
            handbrake_bin: handbrake_bin.into(),
        }
    }
}

/// Factory interface for dynamic instantiation and capability query of transcoding engines (OCP).
pub trait TranscoderFactory: Send + Sync {
    /// Canonical identifier for the engine (e.g., "ffmpeg", "handbrake").
    fn engine_id(&self) -> &'static str;

    /// Aliases or alternative names for the engine.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Checks if a requested engine name matches this factory.
    fn matches(&self, engine_name: &str) -> bool {
        let name_lower = engine_name.to_lowercase();
        self.engine_id() == name_lower || self.aliases().iter().any(|a| *a == name_lower)
    }

    /// Checks whether the engine binary is installed and executable.
    fn is_available(&self, config: &EngineConfig) -> bool;

    /// Instantiates the engine wrapped in a thread-safe reference counted pointer.
    fn create(
        &self,
        config: &EngineConfig,
        options: TranscodeOptions,
    ) -> Result<Arc<dyn Transcoder>>;
}

/// Factory for the default [`FfmpegTranscoder`].
pub struct FfmpegTranscoderFactory;

impl TranscoderFactory for FfmpegTranscoderFactory {
    fn engine_id(&self) -> &'static str {
        "ffmpeg"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["default", "ffmpeg-default"]
    }

    fn is_available(&self, config: &EngineConfig) -> bool {
        let bin = if config.ffmpeg_bin.trim().is_empty() {
            "ffmpeg"
        } else {
            &config.ffmpeg_bin
        };
        crate::utils::command_exists(bin)
    }

    fn create(
        &self,
        config: &EngineConfig,
        options: TranscodeOptions,
    ) -> Result<Arc<dyn Transcoder>> {
        let bin = if config.ffmpeg_bin.trim().is_empty() {
            "ffmpeg"
        } else {
            &config.ffmpeg_bin
        };
        Ok(Arc::new(FfmpegTranscoder::with_options(bin, options)))
    }
}

/// Factory for the pure Rust [`NativeIfoTranscoder`].
pub struct NativeIfoTranscoderFactory;

impl TranscoderFactory for NativeIfoTranscoderFactory {
    fn engine_id(&self) -> &'static str {
        "native"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["ifo", "cell", "native-ifo"]
    }

    fn is_available(&self, config: &EngineConfig) -> bool {
        let bin = if config.ffmpeg_bin.trim().is_empty() {
            "ffmpeg"
        } else {
            &config.ffmpeg_bin
        };
        crate::utils::command_exists(bin)
    }

    fn create(
        &self,
        config: &EngineConfig,
        options: TranscodeOptions,
    ) -> Result<Arc<dyn Transcoder>> {
        let bin = if config.ffmpeg_bin.trim().is_empty() {
            "ffmpeg"
        } else {
            &config.ffmpeg_bin
        };
        Ok(Arc::new(NativeIfoTranscoder::new(bin, options)))
    }
}

/// Factory for [`HandBrakeTranscoder`].
pub struct HandBrakeTranscoderFactory;

impl TranscoderFactory for HandBrakeTranscoderFactory {
    fn engine_id(&self) -> &'static str {
        "handbrake"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["handbrakecli", "hb"]
    }

    fn is_available(&self, config: &EngineConfig) -> bool {
        let bin = if config.handbrake_bin.trim().is_empty() {
            "HandBrakeCLI"
        } else {
            &config.handbrake_bin
        };
        HandBrakeTranscoder::is_available(bin)
    }

    fn create(
        &self,
        config: &EngineConfig,
        options: TranscodeOptions,
    ) -> Result<Arc<dyn Transcoder>> {
        let bin = if config.handbrake_bin.trim().is_empty() {
            "HandBrakeCLI"
        } else {
            &config.handbrake_bin
        };
        Ok(Arc::new(HandBrakeTranscoder::new(bin, options)))
    }
}

/// Factory for [`MakeMkvTranscoder`].
pub struct MakeMkvTranscoderFactory;

impl TranscoderFactory for MakeMkvTranscoderFactory {
    fn engine_id(&self) -> &'static str {
        "makemkv"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["makemkvcon", "mkv"]
    }

    fn is_available(&self, config: &EngineConfig) -> bool {
        let bin = if config.makemkv_bin.trim().is_empty() {
            "makemkvcon"
        } else {
            &config.makemkv_bin
        };
        MakeMkvTranscoder::is_available(bin)
    }

    fn create(
        &self,
        config: &EngineConfig,
        _options: TranscodeOptions,
    ) -> Result<Arc<dyn Transcoder>> {
        let bin = if config.makemkv_bin.trim().is_empty() {
            "makemkvcon"
        } else {
            &config.makemkv_bin
        };
        Ok(Arc::new(MakeMkvTranscoder::new(bin)))
    }
}

/// Factory for [`MakeMkvFfmpegTranscoder`].
pub struct MakeMkvFfmpegTranscoderFactory;

impl TranscoderFactory for MakeMkvFfmpegTranscoderFactory {
    fn engine_id(&self) -> &'static str {
        "makemkv+ffmpeg"
    }

    fn aliases(&self) -> &'static [&'static str] {
        &["makemkv-ffmpeg", "makemkv_ffmpeg", "makemkv-compress"]
    }

    fn is_available(&self, config: &EngineConfig) -> bool {
        let mkv_bin = if config.makemkv_bin.trim().is_empty() {
            "makemkvcon"
        } else {
            &config.makemkv_bin
        };
        let ffmpeg_bin = if config.ffmpeg_bin.trim().is_empty() {
            "ffmpeg"
        } else {
            &config.ffmpeg_bin
        };
        MakeMkvTranscoder::is_available(mkv_bin) && crate::utils::command_exists(ffmpeg_bin)
    }

    fn create(
        &self,
        config: &EngineConfig,
        options: TranscodeOptions,
    ) -> Result<Arc<dyn Transcoder>> {
        let mkv_bin = if config.makemkv_bin.trim().is_empty() {
            "makemkvcon"
        } else {
            &config.makemkv_bin
        };
        let ffmpeg_bin = if config.ffmpeg_bin.trim().is_empty() {
            "ffmpeg"
        } else {
            &config.ffmpeg_bin
        };
        Ok(Arc::new(MakeMkvFfmpegTranscoder::new(
            mkv_bin, ffmpeg_bin, options,
        )))
    }
}

/// Extensible Registry of Transcoder Factories (OCP).
pub struct TranscoderRegistry {
    factories: Vec<Box<dyn TranscoderFactory>>,
}

impl Default for TranscoderRegistry {
    fn default() -> Self {
        let mut registry = Self {
            factories: Vec::new(),
        };
        registry.register(Box::new(FfmpegTranscoderFactory));
        registry.register(Box::new(NativeIfoTranscoderFactory));
        registry.register(Box::new(HandBrakeTranscoderFactory));
        registry.register(Box::new(MakeMkvTranscoderFactory));
        registry.register(Box::new(MakeMkvFfmpegTranscoderFactory));
        registry
    }
}

impl TranscoderRegistry {
    /// Constructs an empty registry.
    pub fn new() -> Self {
        Self {
            factories: Vec::new(),
        }
    }

    /// Registers a new transcoder factory (Open for Extension).
    pub fn register(&mut self, factory: Box<dyn TranscoderFactory>) {
        self.factories.push(factory);
    }

    /// Finds a factory matching the specified engine name or alias.
    pub fn find_factory(&self, engine_name: &str) -> Option<&dyn TranscoderFactory> {
        self.factories
            .iter()
            .find(|f| f.matches(engine_name))
            .map(|f| f.as_ref())
    }

    /// Checks if a designated engine is installed and available.
    pub fn is_available(&self, engine_name: &str, config: &EngineConfig) -> bool {
        self.find_factory(engine_name)
            .map(|f| f.is_available(config))
            .unwrap_or(false)
    }

    /// Instantiates the designated transcoder engine.
    ///
    /// Falls back to FFmpeg if the engine name is unrecognized or defaults.
    pub fn create_transcoder(
        &self,
        engine_name: &str,
        config: &EngineConfig,
        options: TranscodeOptions,
    ) -> Result<Arc<dyn Transcoder>> {
        if let Some(factory) = self.find_factory(engine_name) {
            factory.create(config, options)
        } else {
            // Unrecognized engine - fallback gracefully to FFmpeg
            if let Some(ffmpeg_factory) = self.find_factory("ffmpeg") {
                ffmpeg_factory.create(config, options)
            } else {
                Err(anyhow!(
                    "No available transcoder engine found for '{}'",
                    engine_name
                ))
            }
        }
    }

    /// Lists all registered canonical engine IDs.
    pub fn list_engines(&self) -> Vec<&'static str> {
        self.factories.iter().map(|f| f.engine_id()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_default_factories_registered() {
        let registry = TranscoderRegistry::default();
        let engines = registry.list_engines();
        assert!(engines.contains(&"ffmpeg"));
        assert!(engines.contains(&"native"));
        assert!(engines.contains(&"handbrake"));
        assert!(engines.contains(&"makemkv"));
        assert!(engines.contains(&"makemkv+ffmpeg"));
    }

    #[test]
    fn test_registry_alias_matching() {
        let registry = TranscoderRegistry::default();
        assert!(registry.find_factory("makemkv-ffmpeg").is_some());
        assert!(registry.find_factory("makemkv_ffmpeg").is_some());
        assert!(registry.find_factory("makemkv-compress").is_some());
        assert!(registry.find_factory("handbrakecli").is_some());
        assert!(registry.find_factory("native-ifo").is_some());
    }

    #[test]
    fn test_registry_extensibility_custom_factory() {
        struct CustomDummyFactory;
        impl TranscoderFactory for CustomDummyFactory {
            fn engine_id(&self) -> &'static str {
                "custom-mock"
            }
            fn is_available(&self, _cfg: &EngineConfig) -> bool {
                true
            }
            fn create(
                &self,
                _cfg: &EngineConfig,
                _opts: TranscodeOptions,
            ) -> Result<Arc<dyn Transcoder>> {
                Ok(Arc::new(NativeIfoTranscoder::new(
                    "ffmpeg",
                    TranscodeOptions::default(),
                )))
            }
        }

        let mut registry = TranscoderRegistry::default();
        registry.register(Box::new(CustomDummyFactory));
        assert!(registry.find_factory("custom-mock").is_some());
    }
}
