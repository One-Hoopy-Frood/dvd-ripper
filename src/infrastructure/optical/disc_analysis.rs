//! Optical disc analysis: volume label extraction, deterministic fingerprinting, and copy protection inspection.

use crate::domain::entities::disc::DiscCopyProtectionReport;
use std::path::{Path, PathBuf};

/// Normalizes a DVD drive or folder path into a clean `PathBuf`.
pub fn normalize_dvd_path(input: &str) -> PathBuf {
    let clean = input.trim().trim_matches('"').trim_matches('\'');
    #[cfg(target_os = "windows")]
    {
        if clean.len() == 1
            && clean
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
        {
            return PathBuf::from(format!("{}:\\", clean.to_ascii_uppercase()));
        }
        if clean.len() == 2 && clean.ends_with(':') {
            return PathBuf::from(format!("{}\\", clean.to_ascii_uppercase()));
        }
    }
    PathBuf::from(clean)
}

/// Cleans and sanitizes a raw optical volume label string.
pub fn clean_volume_label(label: &str) -> String {
    label
        .trim()
        .trim_matches(char::from(0))
        .replace('_', " ")
        .trim()
        .to_string()
}

/// Automatically detects the active DVD drive containing an inserted disc, or the primary optical drive.
pub fn auto_detect_dvd_drive() -> String {
    let drives = detect_dvd_drives();
    for drive in &drives {
        if get_volume_label(drive).is_some() {
            return drive.clone();
        }
    }
    drives
        .into_iter()
        .next()
        .unwrap_or_else(|| default_dvd_drive_path().to_string())
}

/// Returns the default primary optical DVD drive device path string for the target operating system.
pub fn default_dvd_drive_path() -> &'static str {
    if cfg!(target_os = "windows") {
        "D:\\"
    } else {
        "/dev/sr0"
    }
}

/// Detects all optical DVD/CD drives attached to the operating system.
#[cfg(target_os = "windows")]
pub fn detect_dvd_drives() -> Vec<String> {
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDriveStringsW};
    use windows::core::PCWSTR;

    let mut drives = Vec::new();
    let mut buffer = [0u16; 512];
    unsafe {
        let len = GetLogicalDriveStringsW(Some(&mut buffer));
        if len > 0 {
            let mut i = 0;
            while i < len as usize && buffer[i] != 0 {
                let mut end = i;
                while end < len as usize && buffer[end] != 0 {
                    end += 1;
                }
                let drive_str = String::from_utf16_lossy(&buffer[i..end]);
                let drive_wide: Vec<u16> =
                    drive_str.encode_utf16().chain(std::iter::once(0)).collect();
                if GetDriveTypeW(PCWSTR::from_raw(drive_wide.as_ptr())) == 5 {
                    drives.push(drive_str);
                }
                i = end + 1;
            }
        }
    }
    if drives.is_empty() {
        drives.push("D:\\".to_string());
    }
    drives
}

/// Detects all optical DVD/CD drives attached to Linux system.
#[cfg(target_os = "linux")]
pub fn detect_dvd_drives() -> Vec<String> {
    let mut drives = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("sr") {
                drives.push(format!("/dev/{}", name));
            }
        }
    }
    if drives.is_empty() && std::path::Path::new("/dev/dvd").exists() {
        drives.push("/dev/dvd".to_string());
    }
    if drives.is_empty() && std::path::Path::new("/dev/cdrom").exists() {
        drives.push("/dev/cdrom".to_string());
    }
    if drives.is_empty() {
        drives.push("/dev/sr0".to_string());
    }
    drives
}

/// Detects all optical DVD/CD drives attached to macOS system.
#[cfg(target_os = "macos")]
pub fn detect_dvd_drives() -> Vec<String> {
    let mut drives = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("VIDEO_TS").exists() || path.join("video_ts").exists() {
                drives.push(path.to_string_lossy().to_string());
            }
        }
    }
    if drives.is_empty() && std::path::Path::new("/dev/disk2").exists() {
        drives.push("/dev/disk2".to_string());
    }
    if drives.is_empty() {
        drives.push("/Volumes/DVD".to_string());
    }
    drives
}

/// Fallback drive detection for unhandled target OS platforms.
#[cfg(all(
    not(target_os = "windows"),
    not(target_os = "linux"),
    not(target_os = "macos")
))]
pub fn detect_dvd_drives() -> Vec<String> {
    vec!["/dev/sr0".to_string()]
}

/// Retrieves the local volume label of a DVD drive using platform-native calls.
#[cfg(target_os = "windows")]
pub fn get_volume_label(root_path: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::GetVolumeInformationW;
    use windows::core::PCWSTR;

    let target_path = if root_path.is_empty() || root_path.eq_ignore_ascii_case("auto") {
        auto_detect_dvd_drive()
    } else {
        root_path.to_string()
    };

    if (target_path.to_lowercase().ends_with(".iso")
        || target_path.to_lowercase().ends_with(".img"))
        && let Some(stem) = std::path::Path::new(&target_path).file_stem()
    {
        return Some(clean_volume_label(&stem.to_string_lossy()));
    }

    let mut path_wide = [0u16; 260];
    let mut len = 0;
    for c in target_path.encode_utf16() {
        if len < 258 {
            path_wide[len] = c;
            len += 1;
        }
    }
    if len > 0 && path_wide[len - 1] != b'\\' as u16 {
        path_wide[len] = b'\\' as u16;
        len += 1;
    }
    path_wide[len] = 0;

    let mut volume_name = [0u16; 260];

    unsafe {
        let result = GetVolumeInformationW(
            PCWSTR::from_raw(path_wide.as_ptr()),
            Some(&mut volume_name),
            None,
            None,
            None,
            None,
        );
        if result.is_ok() {
            let name = String::from_utf16_lossy(&volume_name);
            let trimmed = name.trim_matches(char::from(0)).trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

/// Retrieves the local volume label of a DVD drive using platform-native calls.
#[cfg(not(target_os = "windows"))]
pub fn get_volume_label(root_path: &str) -> Option<String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let dev_path = if root_path.is_empty() || root_path.eq_ignore_ascii_case("auto") {
        auto_detect_dvd_drive()
    } else {
        root_path.to_string()
    };

    if (dev_path.to_lowercase().ends_with(".iso") || dev_path.to_lowercase().ends_with(".img"))
        && let Some(stem) = std::path::Path::new(&dev_path).file_stem()
    {
        return Some(clean_volume_label(&stem.to_string_lossy()));
    }

    let target_file = Path::new(&dev_path);
    if target_file.is_file() || target_file.exists() {
        if let Ok(mut f) = File::open(target_file) {
            let _ = f.seek(SeekFrom::Start(0x8000));
            let mut buf = [0u8; 2048];
            if f.read_exact(&mut buf).is_ok() && buf[0] == 0x01 && &buf[1..6] == b"CD001" {
                let raw_label = String::from_utf8_lossy(&buf[40..72]);
                let trimmed = raw_label.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(clean_volume_label(&trimmed));
                }
            }
        }
    }

    None
}

/// Computes a deterministic disc fingerprint hash based on volume descriptors or VTS header metadata.
pub fn compute_disc_fingerprint(root_path: &str) -> String {
    let resolved = normalize_dvd_path(root_path);
    let mut hasher_data = Vec::new();

    // 1. Try reading Sector 16 Primary Volume Descriptor (if raw device or ISO)
    if let Ok(mut f) = std::fs::File::open(&resolved) {
        use std::io::{Read, Seek, SeekFrom};
        let _ = f.seek(SeekFrom::Start(0x8000));
        let mut sector_buf = [0u8; 2048];
        if f.read_exact(&mut sector_buf).is_ok() {
            hasher_data.extend_from_slice(&sector_buf);
        }
    }

    // 2. Try inspecting VIDEO_TS/VTS_01_0.IFO if mounted directory
    let video_ts = if resolved.join("VIDEO_TS").exists() {
        resolved.join("VIDEO_TS")
    } else if resolved.join("video_ts").exists() {
        resolved.join("video_ts")
    } else {
        resolved.clone()
    };

    if let Ok(entries) = std::fs::read_dir(&video_ts) {
        let mut ifo_files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("ifo"))
            })
            .collect();
        ifo_files.sort();
        for ifo in ifo_files {
            if let Ok(metadata) = std::fs::metadata(&ifo) {
                hasher_data.extend_from_slice(&metadata.len().to_le_bytes());
            }
        }
    }

    if hasher_data.is_empty() {
        let label = get_volume_label(root_path).unwrap_or_else(|| "UNKNOWN".to_string());
        hasher_data.extend_from_slice(label.as_bytes());
    }

    // Simple FNV-1a 64-bit hash
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in hasher_data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    format!("disc_{:016x}", hash)
}

/// Inspects the disc directory structure to detect CSS indicators, IFO/VOB file topologies, and integrity.
pub fn inspect_disc_copy_protection(dvd_path: &Path) -> DiscCopyProtectionReport {
    let video_ts = if dvd_path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("VIDEO_TS"))
    {
        dvd_path.to_path_buf()
    } else {
        dvd_path.join("VIDEO_TS")
    };

    let mut vob_count = 0;
    let mut ifo_count = 0;
    let mut total_bytes = 0u64;
    let mut diagnostic_notes = Vec::new();
    let mut has_css_indicators = false;

    if video_ts.exists() && video_ts.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&video_ts) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext_upper = ext.to_uppercase();
                    if ext_upper == "VOB" {
                        vob_count += 1;
                        if let Ok(meta) = entry.metadata() {
                            total_bytes += meta.len();
                        }
                    } else if ext_upper == "IFO" {
                        ifo_count += 1;
                        if let Ok(meta) = entry.metadata() {
                            total_bytes += meta.len();
                        }
                        if path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .is_some_and(|s| s.eq_ignore_ascii_case("VIDEO_TS.IFO"))
                            && let Ok(mut f) = std::fs::File::open(&path)
                        {
                            use std::io::Read;
                            let mut buf = [0u8; 2048];
                            if f.read(&mut buf).is_ok()
                                && (buf[0x0014] & 0x01 != 0 || buf[0x0015] != 0)
                            {
                                has_css_indicators = true;
                            }
                        }
                    }
                }
            }
        }
    } else {
        diagnostic_notes.push(format!(
            "VIDEO_TS directory not found at path {}",
            dvd_path.display()
        ));
    }

    if vob_count > 0 && total_bytes < 50 * 1024 * 1024 {
        diagnostic_notes.push("Disc total size is suspiciously small for DVD-Video".to_string());
    }

    if has_css_indicators {
        diagnostic_notes.push(
            "Encrypted CSS disc headers detected: decryption keys or MakeMKV/libdvdcss required"
                .to_string(),
        );
    } else {
        diagnostic_notes.push("No explicit CSS indicators found in VIDEO_TS.IFO".to_string());
    }

    DiscCopyProtectionReport {
        has_css_indicators,
        vob_count,
        ifo_count,
        total_bytes,
        diagnostic_notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_dvd_path_windows() {
        if cfg!(target_os = "windows") {
            assert_eq!(normalize_dvd_path("D").to_str().unwrap(), "D:\\");
            assert_eq!(normalize_dvd_path("d:").to_str().unwrap(), "D:\\");
            assert_eq!(normalize_dvd_path("E:\\").to_str().unwrap(), "E:\\");
        }
    }

    #[test]
    fn test_clean_volume_label() {
        assert_eq!(clean_volume_label("THE_MATRIX\0"), "THE MATRIX");
        assert_eq!(clean_volume_label("  PULP_FICTION  "), "PULP FICTION");
    }

    #[test]
    fn test_compute_disc_fingerprint_fallback() {
        let fp = compute_disc_fingerprint("Z:\\NonExistentPath_Test");
        assert!(fp.starts_with("disc_"));
    }

    #[test]
    fn test_inspect_disc_copy_protection_nonexistent() {
        let rep = inspect_disc_copy_protection(Path::new("Z:\\NonExistentPath"));
        assert!(!rep.has_css_indicators);
        assert_eq!(rep.vob_count, 0);
        assert_eq!(rep.ifo_count, 0);
        assert!(!rep.diagnostic_notes.is_empty());
    }
}
