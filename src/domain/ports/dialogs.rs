use std::path::{Path, PathBuf};

/// Abstraction for platform-specific file/folder picker dialogs.
pub trait FileDialogService: Send + Sync {
    fn pick_folder(&self, starting_dir: Option<&Path>, title: &str) -> Option<PathBuf>;
    fn pick_file(
        &self,
        starting_dir: Option<&Path>,
        title: &str,
        filter_name: Option<&str>,
        extensions: Option<&[&str]>,
    ) -> Option<PathBuf>;
    fn open_in_explorer(&self, directory: &Path);
    fn open_file(&self, path: &Path);
}
