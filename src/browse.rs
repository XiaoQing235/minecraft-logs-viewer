use crate::text::{EDIT_LIMIT, IndexedLog};
use std::{collections::VecDeque, io, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub source: usize,
    pub line: Option<usize>,
}

pub struct FileView {
    pub source: usize,
    pub log: Arc<IndexedLog>,
    pub lines: usize,
    pub complete: bool,
    pub failed: bool,
}

#[derive(Default)]
pub struct Window {
    pub files: VecDeque<FileView>,
}

impl Window {
    pub fn reset(&mut self, source: usize, log: Arc<IndexedLog>) {
        self.files.clear();
        self.update(source, log, false);
    }
    pub fn update(&mut self, source: usize, log: Arc<IndexedLog>, prepend: bool) {
        // Freeze the published extent: the decoder can extend its shared index
        // before the UI receives the next event.
        let index = log.index.read().unwrap().clone();
        let file = FileView {
            source,
            log,
            lines: index.lines,
            complete: index.complete,
            failed: false,
        };
        if let Some(old) = self.files.iter_mut().find(|old| old.source == source) {
            *old = file;
        } else if prepend {
            self.files.push_front(file);
        } else {
            self.files.push_back(file);
        }
    }
    pub fn rows(&self) -> usize {
        self.files.iter().map(|file| file.lines + 1).sum()
    }
    pub fn locate(&self, mut row: usize) -> Option<Anchor> {
        for file in &self.files {
            if row <= file.lines {
                return Some(Anchor {
                    source: file.source,
                    line: row.checked_sub(1),
                });
            }
            row -= file.lines + 1;
        }
        None
    }
    pub fn position(&self, anchor: Anchor) -> Option<usize> {
        let mut offset = 0;
        for file in &self.files {
            if file.source == anchor.source {
                return match anchor.line {
                    Some(line) if line < file.lines => Some(offset + line + 1),
                    None => Some(offset),
                    _ => None,
                };
            }
            offset += file.lines + 1;
        }
        None
    }
    pub fn file(&self, source: usize) -> Option<&FileView> {
        self.files.iter().find(|file| file.source == source)
    }
    /// Keep the visible files and one preceding file for immediate reverse scrolling.
    pub fn trim(&mut self, first_row: usize, visible: usize) -> usize {
        let Some(first) = self.locate(first_row) else {
            return 0;
        };
        let last = self
            .locate((first_row + visible).min(self.rows().saturating_sub(1)))
            .unwrap();
        let first_index = self
            .files
            .iter()
            .position(|file| file.source == first.source)
            .unwrap();
        let last_index = self
            .files
            .iter()
            .position(|file| file.source == last.source)
            .unwrap();
        let mut removed = 0;
        for _ in 0..first_index.saturating_sub(1) {
            removed += self.files.pop_front().unwrap().lines + 1;
        }
        let keep = last_index - first_index.saturating_sub(1) + 2;
        self.files.truncate(keep);
        removed
    }
    pub fn copy(&self, first: usize, last: usize) -> io::Result<String> {
        let mut text = String::new();
        let mut offset = 0;
        for file in &self.files {
            let start = offset + 1;
            let end = start + file.lines;
            let from = first.max(start);
            let to = last.saturating_add(1).min(end);
            if from < to {
                let piece = file.log.copy_range(from - start, to - from)?;
                let separator = !text.is_empty() && !text.ends_with('\n');
                if text.len() + usize::from(separator) + piece.len() > EDIT_LIMIT as usize {
                    return Err(io::Error::other("复制内容超过 20 MiB，请缩小范围"));
                }
                if separator {
                    text.push('\n');
                }
                text.push_str(&piece);
            }
            offset = end;
        }
        Ok(text)
    }
}

/// Preserve wheel distance while the next streaming batch or file is loading.
pub fn scroll(
    position: usize,
    delta: i64,
    rows: usize,
    previous: bool,
    next: bool,
) -> (usize, i64) {
    let wanted = (position as i64).saturating_add(delta);
    let end = rows.saturating_sub(1) as i64;
    let clamped = wanted.clamp(0, end);
    let pending = if wanted < 0 && previous || wanted > end && next {
        wanted - clamped
    } else {
        0
    };
    (clamped as usize, pending)
}
