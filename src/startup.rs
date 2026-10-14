use std::path::{Path, PathBuf};

fn logs_at(directory: &Path) -> Option<PathBuf> {
    if directory.is_dir()
        && directory
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("logs"))
    {
        return Some(directory.to_owned());
    }
    let child = directory.join("logs");
    child.is_dir().then_some(child)
}

/// Launch context takes priority over a previously visited directory.
pub fn detect(working: &Path, executable: &Path) -> Option<PathBuf> {
    logs_at(working).or_else(|| executable.parent().and_then(logs_at))
}
