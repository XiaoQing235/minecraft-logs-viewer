use crate::{
    cache::cancelled,
    platform::{self, FileIdentity},
    source::Revision,
};
use std::{
    fs::{self, File, OpenOptions},
    hash::Hasher,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetStamp {
    pub revision: Revision,
    pub digest: u64,
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn check_protected(destination: &Path, protected: &[(PathBuf, FileIdentity)]) -> io::Result<()> {
    let existing = match File::open(destination) {
        Ok(file) => Some(file),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let target_identity = existing.as_ref().map(platform::identity).transpose()?;
    let absolute = if destination.exists() {
        destination.canonicalize()?
    } else {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        parent.canonicalize()?.join(
            destination
                .file_name()
                .ok_or_else(|| io::Error::other("保存目标没有文件名"))?,
        )
    };
    for (path, identity) in protected {
        if target_identity.as_ref() == Some(identity)
            || std::path::absolute(path)? == absolute
            || path.canonicalize().is_ok_and(|source| source == absolute)
        {
            return Err(io::Error::other(
                "保存目标是日志来源原件或其路径别名，请选择其他位置",
            ));
        }
    }
    Ok(())
}

pub fn target_revision(destination: &Path) -> io::Result<Option<TargetStamp>> {
    match Revision::read(destination) {
        Ok(revision) => {
            let mut file = File::open(destination)?;
            let mut digest = std::collections::hash_map::DefaultHasher::new();
            let mut bytes = [0; 128 * 1024];
            loop {
                let read = file.read(&mut bytes)?;
                if read == 0 {
                    break;
                }
                digest.write(&bytes[..read]);
            }
            if Revision::read(destination)? != revision {
                return Err(io::Error::other("目标在读取状态时变化"));
            }
            Ok(Some(TargetStamp {
                revision,
                digest: digest.finish(),
            }))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub fn write_copy(
    destination: &Path,
    bytes: &[u8],
    protected: &[(PathBuf, FileIdentity)],
    expected: &Option<TargetStamp>,
    cancel: &AtomicBool,
) -> io::Result<TargetStamp> {
    cancelled(cancel)?;
    check_protected(destination, protected)?;
    if target_revision(destination)? != *expected {
        return Err(io::Error::other("目标在保存前发生变化，请重新选择另存位置"));
    }
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = format!(
        ".minecraft-logs-viewer-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let temporary = Temporary(parent.join(name));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary.0)?;
    for chunk in bytes.chunks(128 * 1024) {
        cancelled(cancel)?;
        file.write_all(chunk)?;
    }
    file.sync_all()?;
    drop(file);
    cancelled(cancel)?;
    check_protected(destination, protected)?;
    if target_revision(destination)? != *expected {
        return Err(io::Error::other("目标在保存期间发生变化，已有文件保留"));
    }
    platform::replace(&temporary.0, destination)?;
    target_revision(destination)?.ok_or_else(|| io::Error::other("已保存的文件被外部删除"))
}
