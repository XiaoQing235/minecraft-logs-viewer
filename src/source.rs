use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use crate::archive;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    pub len: u64,
    pub modified: Option<SystemTime>,
    pub identity: crate::platform::FileIdentity,
}

impl Revision {
    pub fn read(path: &Path) -> std::io::Result<Self> {
        let file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        Ok(Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            identity: crate::platform::identity(&file)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    pub ordinal: usize,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct LogSource {
    pub path: PathBuf,
    pub relative: String,
    pub member: Option<Member>,
    pub revision: Revision,
}

impl LogSource {
    pub fn label(&self) -> String {
        match &self.member {
            Some(member) => format!(
                "{}/{} [#{}]",
                self.relative,
                member.name,
                member.ordinal + 1
            ),
            None => self.relative.clone(),
        }
    }
    pub fn id(&self) -> String {
        format!(
            "{}\0{:?}",
            self.path.to_string_lossy(),
            self.member.as_ref().map(|m| (&m.name, m.ordinal))
        )
    }
    pub fn is_live(&self) -> bool {
        self.member.is_none()
            && self
                .path
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("latest.log"))
    }
    pub fn check_revision(&self) -> std::io::Result<()> {
        if Revision::read(&self.path)? != self.revision {
            return Err(std::io::Error::other("来源已变化，请刷新目录后重试"));
        }
        Ok(())
    }
}

pub fn is_log(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".log") || lower.ends_with(".log.gz")
}

pub fn is_archive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".zip", ".rar", ".7z", ".tar", ".tar.gz", ".tgz"]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

pub fn allowed_member(name: &str) -> bool {
    let path = name.replace('\\', "/");
    if path.starts_with('/') || path.contains(':') || path.contains('\0') {
        return false;
    }
    let parts: Vec<_> = path.split('/').filter(|part| *part != ".").collect();
    if parts.iter().any(|part| part.is_empty() || *part == "..") {
        return false;
    }
    match parts.as_slice() {
        [file] => is_log(file),
        [directory, file] => directory.eq_ignore_ascii_case("logs") && is_log(file),
        _ => false,
    }
}

pub fn scan(
    root: &Path,
    recursive: bool,
    cancel: &AtomicBool,
    mut found: impl FnMut(LogSource),
    mut failed: impl FnMut(String),
) {
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                failed(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    failed(error.to_string());
                    continue;
                }
            };
            let kind = match entry.file_type() {
                Ok(kind) => kind,
                Err(error) => {
                    failed(error.to_string());
                    continue;
                }
            };
            if kind.is_symlink() {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                if recursive {
                    pending.push(path);
                }
                continue;
            }
            if !kind.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_log(&name) && !is_archive(&name) {
                continue;
            }
            let result = (|| -> std::io::Result<()> {
                let revision = Revision::read(&path)?;
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                if is_log(&name) {
                    found(LogSource {
                        path,
                        relative,
                        member: None,
                        revision,
                    });
                } else {
                    for member in archive::list(&path, cancel)? {
                        found(LogSource {
                            path: path.clone(),
                            relative: relative.clone(),
                            member: Some(member),
                            revision: revision.clone(),
                        });
                    }
                    if Revision::read(&path)? != revision {
                        return Err(std::io::Error::other("归档在扫描期间变化"));
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                failed(format!("{name}: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_scope_rejects_deeper_paths_and_traversal() {
        for name in [
            "latest.log",
            "logs/a.LOG.GZ",
            "./logs/./latest.log",
            "logs\\a.log",
        ] {
            assert!(allowed_member(name), "{name}");
        }
        for name in [
            "a/logs/b.log",
            "logs/a/b.log",
            "../a.log",
            "/a.log",
            "C:a.log",
            "logs//a.log",
            "logs.zip",
            "logs/",
        ] {
            assert!(!allowed_member(name), "{name}");
        }
    }
}
