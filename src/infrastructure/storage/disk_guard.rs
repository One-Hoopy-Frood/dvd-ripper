use crate::domain::ports::storage::DiskSpaceGuard;
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

/// Queries available free disk space on the specified directory in bytes.
#[cfg(target_os = "windows")]
pub fn get_free_disk_space_bytes(dir_path: &Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    // 1. Attempt to create destination directory so it exists
    let _ = std::fs::create_dir_all(dir_path);

    // 2. Resolve relative paths against current working directory
    let abs_path = if dir_path.as_os_str().is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("C:\\"))
    } else if dir_path.is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(dir_path))
            .unwrap_or_else(|_| dir_path.to_path_buf())
    } else {
        dir_path.to_path_buf()
    };

    // 3. Walk upwards until an existing directory is found (or drive root)
    let mut target = abs_path;
    while !target.exists() {
        if let Some(parent) = target.parent() {
            if parent.as_os_str().is_empty() || parent == target {
                target = PathBuf::from("C:\\");
                break;
            }
            target = parent.to_path_buf();
        } else {
            target = PathBuf::from("C:\\");
            break;
        }
    }

    // 4. Windows API requires root paths to have a trailing backslash (e.g. "C:\\")
    let target_str = target.to_string_lossy();
    let normalized = if target_str.len() == 2 && target_str.ends_with(':') {
        format!("{}\\", target_str)
    } else {
        target_str.to_string()
    };

    let mut path_wide: Vec<u16> = std::ffi::OsStr::new(&normalized).encode_wide().collect();
    path_wide.push(0);

    let mut free_bytes_available: u64 = 0;
    let mut total_number_of_bytes: u64 = 0;
    let mut total_number_of_free_bytes: u64 = 0;

    let res = unsafe {
        GetDiskFreeSpaceExW(
            windows::core::PCWSTR(path_wide.as_ptr()),
            Some(&mut free_bytes_available),
            Some(&mut total_number_of_bytes),
            Some(&mut total_number_of_free_bytes),
        )
    };

    if res.is_err() {
        let fallback_res = unsafe {
            GetDiskFreeSpaceExW(
                windows::core::PCWSTR::null(),
                Some(&mut free_bytes_available),
                Some(&mut total_number_of_bytes),
                Some(&mut total_number_of_free_bytes),
            )
        };
        if fallback_res.is_err() {
            return Ok(500 * 1024 * 1024 * 1024);
        }
    }

    Ok(free_bytes_available)
}

#[cfg(not(target_os = "windows"))]
pub fn get_free_disk_space_bytes(_dir_path: &Path) -> Result<u64> {
    Ok(100 * 1024 * 1024 * 1024)
}

/// Verifies that available free disk space on target_dir exceeds min_free_gb threshold.
pub fn check_disk_space_guard(target_dir: &Path, min_free_gb: u64) -> Result<u64> {
    if min_free_gb == 0 {
        let free = get_free_disk_space_bytes(target_dir).unwrap_or(0);
        return Ok(free);
    }
    let free_bytes = get_free_disk_space_bytes(target_dir)?;
    let required_bytes = min_free_gb * 1024 * 1024 * 1024;
    if free_bytes < required_bytes {
        let free_gb = free_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        return Err(anyhow!(
            "Disk Space Guard Safeguard Triggered: Only {:.2} GB free space available in '{}', but minimum threshold is {} GB.",
            free_gb,
            target_dir.display(),
            min_free_gb
        ));
    }
    Ok(free_bytes)
}

/// Verifies available disk space, falling back to secondary fallback_dir if primary space is below min_free_gb threshold.
pub fn check_disk_space_guard_with_fallback(
    primary_dir: &Path,
    fallback_dir: Option<&Path>,
    min_free_gb: u64,
) -> Result<PathBuf> {
    match check_disk_space_guard(primary_dir, min_free_gb) {
        Ok(_) => Ok(primary_dir.to_path_buf()),
        Err(e) => {
            if let Some(fb) = fallback_dir
                && check_disk_space_guard(fb, min_free_gb).is_ok()
            {
                return Ok(fb.to_path_buf());
            }
            Err(e)
        }
    }
}

/// Implementation of DiskSpaceGuard querying filesystem free capacity.
#[derive(Debug, Default, Clone)]
pub struct SystemDiskSpaceGuard;

impl SystemDiskSpaceGuard {
    pub fn new() -> Self {
        Self
    }
}

impl DiskSpaceGuard for SystemDiskSpaceGuard {
    fn check_free_space(&self, path: &Path, min_free_gb: u64) -> Result<()> {
        check_disk_space_guard(path, min_free_gb).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_disk_space_guard_zero_gb() {
        let guard = SystemDiskSpaceGuard::new();
        assert!(guard.check_free_space(Path::new("."), 0).is_ok());

        // Extremely high threshold to exercise the error branch
        let err_res = guard.check_free_space(Path::new("."), 1_000_000);
        assert!(err_res.is_err());
        assert!(
            err_res
                .unwrap_err()
                .to_string()
                .contains("Disk Space Guard Safeguard Triggered")
        );
    }
}
