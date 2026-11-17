//! Domain port abstraction for post-transcoding pipeline processing.

use anyhow::Result;
use std::path::PathBuf;

/// Contextual metadata and configuration passed to post-processors upon rip completion.
#[derive(Debug, Clone, Default)]
pub struct PostProcessContext {
    pub output_file: Option<PathBuf>,
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub media_type: String,
    pub poster_bytes: Option<Vec<u8>>,
    pub dvd_path: PathBuf,
    pub nfo_enabled: bool,
    pub nfo_tags: Option<String>,
    pub checksum_enabled: bool,
    pub audit_log_path: Option<PathBuf>,
    pub auto_cleanup_days: Option<u64>,
    pub post_script: Option<String>,
    pub media_servers_enabled: bool,
    pub jellyfin_url: Option<String>,
    pub jellyfin_key: Option<String>,
    pub plex_url: Option<String>,
    pub plex_token: Option<String>,
    pub emby_url: Option<String>,
    pub emby_key: Option<String>,
    pub no_eject: bool,
    pub eject_autoclose_secs: Option<u64>,
    pub spindown: bool,
}

impl PostProcessContext {
    pub fn new(
        output_file: Option<PathBuf>,
        title: impl Into<String>,
        dvd_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            output_file,
            title: title.into(),
            dvd_path: dvd_path.into(),
            media_type: "Movie".to_string(),
            ..Default::default()
        }
    }

    pub fn from_args(
        args: &crate::cli::Args,
        output_file: Option<PathBuf>,
        title: &str,
        year: Option<u32>,
        dvd_path: PathBuf,
        poster_bytes: Option<Vec<u8>>,
    ) -> Self {
        let media_type = if args.tv { "TV Series" } else { "Movie" };
        Self {
            output_file,
            title: title.to_string(),
            year,
            season: if args.tv { Some(args.season) } else { None },
            episode: if args.tv {
                Some(args.start_episode)
            } else {
                None
            },
            media_type: media_type.to_string(),
            poster_bytes,
            dvd_path,
            nfo_enabled: args.nfo,
            nfo_tags: args.tags.clone(),
            checksum_enabled: args.checksum,
            audit_log_path: args.audit_log.as_ref().map(PathBuf::from),
            auto_cleanup_days: args.auto_cleanup_days,
            post_script: args.post_script.clone(),
            media_servers_enabled: args.jellyfin_url.is_some()
                || args.plex_url.is_some()
                || args.emby_url.is_some(),
            jellyfin_url: args.jellyfin_url.clone(),
            jellyfin_key: args.jellyfin_key.clone(),
            plex_url: args.plex_url.clone(),
            plex_token: args.plex_token.clone(),
            emby_url: args.emby_url.clone(),
            emby_key: args.emby_key.clone(),
            no_eject: args.no_eject,
            eject_autoclose_secs: args.eject_autoclose,
            spindown: args.spindown,
        }
    }

    pub fn with_year(mut self, year: Option<u32>) -> Self {
        self.year = year;
        self
    }

    pub fn with_tv_season_episode(mut self, season: u32, episode: u32) -> Self {
        self.season = Some(season);
        self.episode = Some(episode);
        self.media_type = "TV Series".to_string();
        self
    }

    pub fn with_nfo(mut self, enabled: bool, tags: Option<String>) -> Self {
        self.nfo_enabled = enabled;
        self.nfo_tags = tags;
        self
    }

    pub fn with_checksum(mut self, enabled: bool) -> Self {
        self.checksum_enabled = enabled;
        self
    }

    pub fn with_audit_log(mut self, path: Option<PathBuf>) -> Self {
        self.audit_log_path = path;
        self
    }

    pub fn with_poster_bytes(mut self, poster_bytes: Option<Vec<u8>>) -> Self {
        self.poster_bytes = poster_bytes;
        self
    }

    pub fn with_eject_options(mut self, no_eject: bool, spindown: bool, autoclose_secs: Option<u64>) -> Self {
        self.no_eject = no_eject;
        self.spindown = spindown;
        self.eject_autoclose_secs = autoclose_secs;
        self
    }
}

/// Open/Closed Principle (OCP/SOLID): Pluggable post-processing action executed after rip completion.
pub trait PostProcessor: Send + Sync {
    /// Executes the post-processing action against the completed rip context.
    ///
    /// # Errors
    /// Returns an error if the post-processing action fails critically.
    fn process(&self, ctx: &PostProcessContext) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockPostProcessor {
        called: std::sync::atomic::AtomicBool,
    }

    impl PostProcessor for MockPostProcessor {
        fn process(&self, _ctx: &PostProcessContext) -> Result<()> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn test_mock_post_processor() {
        let proc = MockPostProcessor {
            called: std::sync::atomic::AtomicBool::new(false),
        };
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("test.mp4")),
            "Inception",
            PathBuf::from("D:\\"),
        );
        proc.process(&ctx).unwrap();
        assert!(proc.called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_post_process_context_from_args_movie() {
        let mut args = crate::cli::Args::default();
        args.tv = false;
        args.nfo = true;
        args.tags = Some("Sci-Fi,Classic".to_string());
        args.checksum = true;
        args.audit_log = Some("audit.log".to_string());
        args.auto_cleanup_days = Some(30);
        args.post_script = Some("script.bat".to_string());
        args.plex_url = Some("http://localhost:32400".to_string());
        args.plex_token = Some("plextoken".to_string());
        args.no_eject = true;
        args.eject_autoclose = Some(15);
        args.spindown = true;

        let ctx = PostProcessContext::from_args(
            &args,
            Some(PathBuf::from("out.mp4")),
            "The Matrix",
            Some(1999),
            PathBuf::from("D:\\"),
            Some(vec![1, 2, 3]),
        );

        assert_eq!(ctx.media_type, "Movie");
        assert_eq!(ctx.title, "The Matrix");
        assert_eq!(ctx.year, Some(1999));
        assert_eq!(ctx.season, None);
        assert_eq!(ctx.episode, None);
        assert!(ctx.nfo_enabled);
        assert_eq!(ctx.nfo_tags.as_deref(), Some("Sci-Fi,Classic"));
        assert!(ctx.checksum_enabled);
        assert_eq!(ctx.audit_log_path, Some(PathBuf::from("audit.log")));
        assert_eq!(ctx.auto_cleanup_days, Some(30));
        assert_eq!(ctx.post_script.as_deref(), Some("script.bat"));
        assert!(ctx.media_servers_enabled);
        assert_eq!(ctx.plex_url.as_deref(), Some("http://localhost:32400"));
        assert_eq!(ctx.plex_token.as_deref(), Some("plextoken"));
        assert!(ctx.no_eject);
        assert_eq!(ctx.eject_autoclose_secs, Some(15));
        assert!(ctx.spindown);

        let cloned = ctx.clone();
        assert_eq!(cloned.title, ctx.title);
    }

    #[test]
    fn test_post_process_context_from_args_tv() {
        let mut args = crate::cli::Args::default();
        args.tv = true;
        args.season = 3;
        args.start_episode = 4;
        args.jellyfin_url = Some("http://localhost:8096".to_string());
        args.jellyfin_key = Some("jfkey".to_string());
        args.emby_url = Some("http://localhost:8096".to_string());
        args.emby_key = Some("embykey".to_string());

        let ctx = PostProcessContext::from_args(
            &args,
            Some(PathBuf::from("tv.mp4")),
            "Lost",
            Some(2004),
            PathBuf::from("E:\\"),
            None,
        );

        assert_eq!(ctx.media_type, "TV Series");
        assert_eq!(ctx.season, Some(3));
        assert_eq!(ctx.episode, Some(4));
        assert!(ctx.media_servers_enabled);
        assert_eq!(ctx.jellyfin_url.as_deref(), Some("http://localhost:8096"));
        assert_eq!(ctx.emby_key.as_deref(), Some("embykey"));
    }

    #[test]
    fn test_post_process_context_fluent_builder() {
        let ctx = PostProcessContext::new(
            Some(PathBuf::from("S01E01.mp4")),
            "Breaking Bad",
            PathBuf::from("D:\\"),
        )
        .with_year(Some(2008))
        .with_tv_season_episode(1, 1)
        .with_nfo(true, Some("Drama,Crime".to_string()))
        .with_checksum(true)
        .with_audit_log(Some(PathBuf::from("tv_audit.log")))
        .with_poster_bytes(Some(vec![4, 5, 6]))
        .with_eject_options(true, true, Some(10));

        assert_eq!(ctx.media_type, "TV Series");
        assert_eq!(ctx.title, "Breaking Bad");
        assert_eq!(ctx.year, Some(2008));
        assert_eq!(ctx.season, Some(1));
        assert_eq!(ctx.episode, Some(1));
        assert!(ctx.nfo_enabled);
        assert_eq!(ctx.nfo_tags.as_deref(), Some("Drama,Crime"));
        assert!(ctx.checksum_enabled);
        assert_eq!(ctx.audit_log_path, Some(PathBuf::from("tv_audit.log")));
        assert!(ctx.no_eject);
        assert!(ctx.spindown);
        assert_eq!(ctx.eject_autoclose_secs, Some(10));
        assert_eq!(ctx.poster_bytes, Some(vec![4, 5, 6]));
    }
}
