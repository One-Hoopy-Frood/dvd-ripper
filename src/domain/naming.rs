//! Pure domain naming conventions for Plex/Jellyfin/Emby media library organization (SRP).

use crate::domain::entities::options::OutputNamingSpec;
use crate::domain::value_objects::sanitize_filename;
use std::path::{Path, PathBuf};

/// Formats a sanitized media folder/file stem `<Title> (<Year>)` or `<Title>`.
pub fn format_media_stem(title: Option<&str>, year: Option<u32>, fallback: &str) -> String {
    let raw = title
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback);
    let clean = sanitize_filename(raw);
    let base = if clean.is_empty() {
        fallback.to_string()
    } else {
        clean
    };
    match year {
        Some(y) if y >= 1888 && !base.contains(&format!("({})", y)) => format!("{} ({})", base, y),
        _ => base,
    }
}

/// Resolves the output file extension (`iso`, `mkv`, or `mp4`) from a segregated [`OutputNamingSpec`].
pub fn resolve_container_extension(spec: &OutputNamingSpec) -> &'static str {
    if spec.iso {
        "iso"
    } else if spec.mkv {
        "mkv"
    } else {
        "mp4"
    }
}

/// Helper: Checks if the path component matches a given folder name (case-insensitive).
pub fn is_named_folder(path: &Path, name: &str) -> bool {
    let clean = name.trim();
    if clean.is_empty() {
        return false;
    }
    if let Some(file_name) = path.file_name() {
        let name_str = file_name.to_string_lossy();
        let last_comp = name_str.rsplit(['/', '\\']).next().unwrap_or(&name_str);
        if last_comp.eq_ignore_ascii_case(clean) {
            return true;
        }
    }
    let full = path.to_string_lossy();
    let last = full
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("");
    last.eq_ignore_ascii_case(clean)
}

/// Helper: Checks if the path component is named "films" (case-insensitive).
pub fn is_named_films(path: &Path) -> bool {
    is_named_films_custom(path, "Films")
}

/// Helper: Checks if the path component is named "films" or matches a custom films folder name (case-insensitive).
pub fn is_named_films_custom(path: &Path, custom_name: &str) -> bool {
    is_named_folder(path, "films") || is_named_folder(path, custom_name)
}

/// Helper: Checks if the path component is named "tv" or "tv shows" (case-insensitive).
pub fn is_named_tv(path: &Path) -> bool {
    is_named_tv_custom(path, "TV")
}

/// Helper: Checks if the path component is named "tv", "tv shows", or matches a custom tv folder name (case-insensitive).
pub fn is_named_tv_custom(path: &Path, custom_name: &str) -> bool {
    is_named_folder(path, "tv")
        || is_named_folder(path, "tv shows")
        || is_named_folder(path, custom_name)
}

/// Resolves the root destination directory for Film/Movie rips with custom films and tv subfolder names.
/// Defaults to `<out_dir>/<films_dir>`. If `out_dir` already ends in the films folder, it is kept as-is.
/// If `out_dir` ends in the tv folder, its parent sibling `/<films_dir>` is used.
pub fn resolve_films_root_with_tv(out_dir: &str, films_dir: &str, tv_dir: &str) -> PathBuf {
    let folder_name = if films_dir.trim().is_empty() {
        "Films"
    } else {
        films_dir.trim()
    };
    let trimmed = out_dir.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." {
        return PathBuf::from(folder_name);
    }
    let p = Path::new(trimmed);
    if is_named_films_custom(p, folder_name) {
        p.to_path_buf()
    } else if is_named_tv_custom(p, tv_dir) {
        let parent = p
            .parent()
            .filter(|par| !par.as_os_str().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                if let Some(idx) = trimmed.rfind(['/', '\\']) {
                    let parent_str = &trimmed[..idx];
                    if !parent_str.is_empty() {
                        return Some(PathBuf::from(parent_str));
                    }
                }
                None
            });
        match parent {
            Some(parent) => parent.join(folder_name),
            _ => PathBuf::from(folder_name),
        }
    } else {
        p.join(folder_name)
    }
}

/// Resolves the root destination directory for Film/Movie rips with a custom films subfolder name.
pub fn resolve_films_root_custom(out_dir: &str, films_dir: &str) -> PathBuf {
    resolve_films_root_with_tv(out_dir, films_dir, "TV")
}

/// Resolves the root destination directory for Film/Movie rips using standard "Films" default.
pub fn resolve_films_root(out_dir: &str) -> PathBuf {
    resolve_films_root_custom(out_dir, "Films")
}

/// Resolves the root destination directory for TV series rips with custom tv and films subfolder names.
/// Defaults to `<out_dir>/<tv_dir>`. If `out_dir` already ends in the tv folder, it is kept as-is.
/// If `out_dir` ends in the films folder, its parent sibling `/<tv_dir>` is used.
pub fn resolve_tv_root_with_films(out_dir: &str, tv_dir: &str, films_dir: &str) -> PathBuf {
    let folder_name = if tv_dir.trim().is_empty() {
        "TV"
    } else {
        tv_dir.trim()
    };
    let trimmed = out_dir.trim().trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed == "." {
        return PathBuf::from(folder_name);
    }
    let p = Path::new(trimmed);
    if is_named_tv_custom(p, folder_name) {
        p.to_path_buf()
    } else if is_named_films_custom(p, films_dir) {
        let parent = p
            .parent()
            .filter(|par| !par.as_os_str().is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                if let Some(idx) = trimmed.rfind(['/', '\\']) {
                    let parent_str = &trimmed[..idx];
                    if !parent_str.is_empty() {
                        return Some(PathBuf::from(parent_str));
                    }
                }
                None
            });
        match parent {
            Some(parent) => parent.join(folder_name),
            _ => PathBuf::from(folder_name),
        }
    } else {
        p.join(folder_name)
    }
}

/// Resolves the root destination directory for TV series rips with a custom tv subfolder name.
pub fn resolve_tv_root_custom(out_dir: &str, tv_dir: &str) -> PathBuf {
    resolve_tv_root_with_films(out_dir, tv_dir, "Films")
}

/// Resolves the root destination directory for TV series rips using standard "TV" default.
pub fn resolve_tv_root(out_dir: &str) -> PathBuf {
    resolve_tv_root_custom(out_dir, "TV")
}

/// Resolves a movie output path purely from [`OutputNamingSpec`], title, and optional release year.
/// Always places film rips into the configured films subfolder of the base directory:
/// `<base_folder>/<films_dir>/<Title (Year)>/<Title (Year)>.<ext>`.
pub fn resolve_movie_output_path(
    spec: &OutputNamingSpec,
    title: Option<&str>,
    year: Option<u32>,
) -> PathBuf {
    if let Some(ref explicit) = spec.explicit_output
        && !explicit.trim().is_empty()
    {
        return PathBuf::from(explicit.trim());
    }
    let ext = resolve_container_extension(spec);
    let stem = format_media_stem(title, year, "DVD_Rip");
    let films_dir = if spec.films_dir.trim().is_empty() {
        "Films"
    } else {
        &spec.films_dir
    };
    let tv_dir = if spec.tv_dir.trim().is_empty() {
        "TV"
    } else {
        &spec.tv_dir
    };
    let films_root = resolve_films_root_with_tv(&spec.out_dir, films_dir, tv_dir);
    films_root.join(&stem).join(format!("{}.{}", stem, ext))
}

/// Resolves a TV episode output path purely from [`OutputNamingSpec`], show name, optional release year, season, episode, and optional episode title.
/// Follows Plex/Jellyfin convention: `<tv_root>/<Show Name (Year)>/Season <XX>/<Show Name> - S<XX>E<YY> - <Episode Title>.<ext>`
pub fn resolve_tv_episode_output_path(
    spec: &OutputNamingSpec,
    show_name: &str,
    year: Option<u32>,
    season: u32,
    episode: u32,
    ep_title: Option<&str>,
) -> PathBuf {
    if let Some(ref explicit) = spec.explicit_output
        && !explicit.trim().is_empty()
    {
        return PathBuf::from(explicit.trim());
    }
    let ext = resolve_container_extension(spec);
    let tv_dir = if spec.tv_dir.trim().is_empty() {
        "TV"
    } else {
        &spec.tv_dir
    };
    let films_dir = if spec.films_dir.trim().is_empty() {
        "Films"
    } else {
        &spec.films_dir
    };
    let tv_root = resolve_tv_root_with_films(&spec.out_dir, tv_dir, films_dir);
    let show_stem = format_media_stem(Some(show_name), year, "TV_Show");
    let season_folder = format!("Season {:02}", season);
    let ep_clean_name = match ep_title {
        Some(t) if !t.trim().is_empty() => format!(
            "{} - S{:02}E{:02} - {}.{}",
            show_name,
            season,
            episode,
            sanitize_filename(t.trim()),
            ext
        ),
        _ => format!("{} - S{:02}E{:02}.{}", show_name, season, episode, ext),
    };
    tv_root
        .join(show_stem)
        .join(season_folder)
        .join(ep_clean_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_naming_conventions() {
        let spec = OutputNamingSpec {
            out_dir: "/movies".to_string(),
            films_dir: "Films".to_string(),
            tv_dir: "TV".to_string(),
            explicit_output: None,
            mkv: true,
            iso: false,
            no_overwrite: false,
        };
        let p = resolve_movie_output_path(&spec, Some("The Matrix"), Some(1999));
        assert!(p.to_string_lossy().ends_with("The Matrix (1999).mkv"));
        assert!(p.to_string_lossy().contains("Films"));
    }

    #[test]
    fn test_domain_naming_films_root_handling() {
        let base = PathBuf::from("C:\\Media\\DVD Rips");
        assert_eq!(
            resolve_films_root(&base.to_string_lossy()),
            base.join("Films")
        );
        let base_films = base.join("Films");
        assert_eq!(
            resolve_films_root(&base_films.to_string_lossy()),
            base.join("Films")
        );
        let base_tv = base.join("TV");
        assert_eq!(
            resolve_films_root(&base_tv.to_string_lossy()),
            base.join("Films")
        );
    }

    #[test]
    fn test_domain_naming_custom_folders() {
        let base = Path::new("Media");
        let movies = base.join("Movies");
        let series = base.join("Series");

        let spec = OutputNamingSpec {
            out_dir: base.to_string_lossy().to_string(),
            films_dir: "Movies".to_string(),
            tv_dir: "Series".to_string(),
            explicit_output: None,
            mkv: true,
            iso: false,
            no_overwrite: false,
        };
        let p = resolve_movie_output_path(&spec, Some("Inception"), Some(2010));
        assert!(p.to_string_lossy().contains("Movies"));
        assert!(p.to_string_lossy().ends_with("Inception (2010).mkv"));

        assert_eq!(
            resolve_films_root_custom(&base.to_string_lossy(), "Movies"),
            movies
        );
        assert_eq!(
            resolve_tv_root_custom(&base.to_string_lossy(), "Series"),
            series
        );
        assert_eq!(
            resolve_films_root_with_tv(&series.to_string_lossy(), "Movies", "Series"),
            movies
        );
        assert_eq!(
            resolve_tv_root_with_films(&movies.to_string_lossy(), "Series", "Movies"),
            series
        );
    }

    #[test]
    fn test_domain_naming_tv_episode_output_path() {
        let spec = OutputNamingSpec {
            out_dir: "C:\\Videos".to_string(),
            films_dir: "Films".to_string(),
            tv_dir: "TV".to_string(),
            explicit_output: None,
            mkv: false,
            iso: false,
            no_overwrite: false,
        };
        let p =
            resolve_tv_episode_output_path(&spec, "Breaking Bad", Some(2008), 1, 1, Some("Pilot"));
        let path_str = p.to_string_lossy();
        assert!(path_str.contains("TV"));
        assert!(path_str.contains("Breaking Bad (2008)"));
        assert!(path_str.contains("Season 01"));
        assert!(path_str.ends_with("Breaking Bad - S01E01 - Pilot.mp4"));
    }
}
