use crate::platform;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Read, Write},
    path::PathBuf,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    pub source: String,
    pub line: usize,
    pub digest: u64,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub window: [i32; 2],
    pub font_size: i32,
    pub light: bool,
    pub recursive: bool,
    pub recent: Vec<PathBuf>,
    pub positions: BTreeMap<String, usize>,
    pub bookmarks: Vec<Bookmark>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            window: [1280, 820],
            font_size: 14,
            light: false,
            recursive: false,
            recent: Vec::new(),
            positions: BTreeMap::new(),
            bookmarks: Vec::new(),
        }
    }
}
impl Settings {
    pub fn load() -> io::Result<Self> {
        let path = platform::data_dir()?.join("settings.json");
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        if file.metadata()?.len() > 1024 * 1024 {
            return Err(io::Error::other("设置文件超过 1 MiB"));
        }
        let mut json = String::new();
        file.take(1024 * 1024 + 1).read_to_string(&mut json)?;
        let mut settings: Self = serde_json::from_str(&json).map_err(io::Error::other)?;
        settings.window[0] = settings.window[0].clamp(1040, 3840);
        settings.window[1] = settings.window[1].clamp(620, 2160);
        settings.font_size = settings.font_size.clamp(10, 24);
        settings.recent.truncate(12);
        settings.bookmarks.truncate(1000);
        if settings.positions.len() > 1000 {
            settings.positions = settings.positions.into_iter().take(1000).collect();
        }
        Ok(settings)
    }
    pub fn store(&self) -> io::Result<()> {
        let directory = platform::data_dir()?;
        fs::create_dir_all(&directory)?;
        let data = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        if data.len() > 1024 * 1024 {
            return Err(io::Error::other("本地设置超过 1 MiB，请清理书签或阅读记录"));
        }
        let temporary = directory.join(format!("settings-{}.tmp", std::process::id()));
        let mut file = File::create(&temporary)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        platform::replace(&temporary, &directory.join("settings.json"))
    }
    pub fn visited(&mut self, path: PathBuf) {
        self.recent.retain(|old| *old != path);
        self.recent.insert(0, path);
        self.recent.truncate(12);
    }
    pub fn position(&mut self, source: String, line: usize) {
        if self.positions.len() >= 1000 && !self.positions.contains_key(&source) {
            self.positions.pop_first();
        }
        self.positions.insert(source, line);
    }
}
pub fn digest(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325u64, |digest, byte| {
        (digest ^ byte as u64).wrapping_mul(0x100000001b3)
    })
}
