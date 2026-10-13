use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const SINGLE_LIMIT: u64 = 1 << 30;
pub const CACHE_LIMIT: u64 = 2 << 30;

#[derive(Clone)]
pub struct Cache(Arc<CacheRoot>);
struct CacheRoot {
    path: PathBuf,
    used: AtomicU64,
    next: AtomicU64,
}
impl Drop for CacheRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

pub struct CacheFile {
    path: PathBuf,
    bytes: AtomicU64,
    root: Arc<CacheRoot>,
}
impl CacheFile {
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl Drop for CacheFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        self.root
            .used
            .fetch_sub(self.bytes.load(Ordering::Relaxed), Ordering::Relaxed);
    }
}

impl Cache {
    pub fn new() -> io::Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "minecraft-logs-viewer-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path)?;
        Ok(Self(Arc::new(CacheRoot {
            path,
            used: AtomicU64::new(0),
            next: AtomicU64::new(0),
        })))
    }
    pub fn file(&self) -> io::Result<(Arc<CacheFile>, File)> {
        let path = self
            .0
            .path
            .join(self.0.next.fetch_add(1, Ordering::Relaxed).to_string());
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)?;
        Ok((
            Arc::new(CacheFile {
                path,
                bytes: AtomicU64::new(0),
                root: self.0.clone(),
            }),
            file,
        ))
    }
    pub fn used(&self) -> u64 {
        self.0.used.load(Ordering::Relaxed)
    }
}

pub struct LimitedWriter<'a> {
    pub file: File,
    pub owner: Arc<CacheFile>,
    pub cancel: &'a AtomicBool,
}
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        cancelled(self.cancel)?;
        let size = bytes.len() as u64;
        if self.owner.bytes.load(Ordering::Relaxed) + size > SINGLE_LIMIT {
            return Err(io::Error::other("单次缓存超过 1 GiB，已停止读取"));
        }
        self.owner
            .root
            .used
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                (used + size <= CACHE_LIMIT).then_some(used + size)
            })
            .map_err(|_| io::Error::other("缓存超过 2 GiB，请关闭来源或清理缓存"))?;
        match self.file.write(bytes) {
            Ok(written) => {
                self.owner
                    .root
                    .used
                    .fetch_sub(size - written as u64, Ordering::Relaxed);
                self.owner
                    .bytes
                    .fetch_add(written as u64, Ordering::Relaxed);
                Ok(written)
            }
            Err(error) => {
                self.owner.root.used.fetch_sub(size, Ordering::Relaxed);
                Err(error)
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

pub fn cancelled(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "任务已取消"))
    } else {
        Ok(())
    }
}
