use crate::{
    cache::{Cache, CacheFile, LimitedWriter, cancelled},
    source::{LogSource, Member, allowed_member},
};
use compress_tools::{ArchiveContents, ArchiveIteratorBuilder};
use std::{
    cell::Cell,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

fn error(error: compress_tools::Error) -> io::Error {
    io::Error::other(error.to_string())
}
fn regular(stat: &compress_tools::stat) -> bool {
    (stat.st_mode as u32 & 0o170000) == 0o100000
}

pub fn list(path: &Path, cancel: &AtomicBool) -> io::Result<Vec<Member>> {
    let mut iterator = ArchiveIteratorBuilder::new(File::open(path)?)
        .mtree_format(false)
        .build()
        .map_err(error)?;
    let mut output = Vec::new();
    let mut ordinal = 0;
    while let Some(entry) = iterator.next_header() {
        cancelled(cancel)?;
        match entry {
            ArchiveContents::StartOfEntry(name, stat) => {
                if regular(&stat) && allowed_member(&name) {
                    output.push(Member {
                        name,
                        ordinal,
                        size: stat.st_size.max(0) as u64,
                    });
                }
                ordinal += 1;
            }
            ArchiveContents::Err(failure) => return Err(error(failure)),
            _ => {}
        }
    }
    Ok(output)
}

pub struct Prepared {
    pub path: std::path::PathBuf,
    pub owner: Option<Arc<CacheFile>>,
    pub len: u64,
}

pub fn visit_group(
    sources: &[LogSource],
    cache: &Cache,
    cancel: &AtomicBool,
    mut visit: impl FnMut(&LogSource, Prepared) -> io::Result<bool>,
) -> io::Result<()> {
    if sources.is_empty() {
        return Ok(());
    }
    sources[0].check_revision()?;
    let selected: std::collections::HashMap<_, _> = sources
        .iter()
        .filter_map(|source| {
            source
                .member
                .as_ref()
                .map(|member| (member.ordinal, source))
        })
        .collect();
    let ordinals: std::collections::HashSet<_> = selected.keys().copied().collect();
    let counter = Rc::new(Cell::new(0));
    let filter_counter = counter.clone();
    let iterator = ArchiveIteratorBuilder::new(File::open(&sources[0].path)?)
        .mtree_format(false)
        .filter(move |_, stat| {
            let ordinal = filter_counter.get();
            filter_counter.set(ordinal + 1);
            regular(stat) && ordinals.contains(&ordinal)
        })
        .build()
        .map_err(error)?;
    let mut current = None;
    let mut writer = None;
    let mut visited = std::collections::HashSet::new();
    for entry in iterator {
        cancelled(cancel)?;
        match entry {
            ArchiveContents::StartOfEntry(name, _) => {
                let ordinal = counter.get() - 1;
                let source = selected[&ordinal];
                if name != source.member.as_ref().unwrap().name {
                    return Err(io::Error::other("归档成员已变化"));
                }
                let (owner, file) = cache.file()?;
                current = Some(source);
                writer = Some(LimitedWriter {
                    file,
                    owner,
                    cancel,
                });
                visited.insert(ordinal);
            }
            ArchiveContents::DataChunk(bytes) => writer.as_mut().unwrap().write_all(&bytes)?,
            ArchiveContents::EndOfEntry => {
                let source = current.take().unwrap();
                let mut stored = writer.take().unwrap();
                stored.flush()?;
                let mut path = stored.owner.path().to_owned();
                let mut owner = stored.owner.clone();
                drop(stored);
                if source
                    .member
                    .as_ref()
                    .unwrap()
                    .name
                    .to_ascii_lowercase()
                    .ends_with(".log.gz")
                {
                    let (decoded, file) = cache.file()?;
                    let mut output = LimitedWriter {
                        file,
                        owner: decoded.clone(),
                        cancel,
                    };
                    let mut input = File::open(&path)?;
                    let mut magic = [0; 2];
                    input.read_exact(&mut magic)?;
                    if magic != [0x1f, 0x8b] {
                        return Err(io::Error::other("归档日志不包含有效 gzip 头"));
                    }
                    compress_tools::uncompress_data(File::open(&path)?, &mut output)
                        .map_err(error)?;
                    output.flush()?;
                    path = decoded.path().to_owned();
                    owner = decoded;
                }
                let len = File::open(&path)?.metadata()?.len();
                if !visit(
                    source,
                    Prepared {
                        path,
                        owner: Some(owner),
                        len,
                    },
                )? {
                    return Ok(());
                }
            }
            ArchiveContents::Err(failure) => return Err(error(failure)),
        }
    }
    if visited.len() != selected.len() {
        return Err(io::Error::other("部分归档成员不存在"));
    }
    sources[0].check_revision()
}

pub fn prepare(source: &LogSource, cache: &Cache, cancel: &AtomicBool) -> io::Result<Prepared> {
    source.check_revision()?;
    let mut path = source.path.clone();
    let mut owner = None;
    if let Some(member) = &source.member {
        let (stored, file) = cache.file()?;
        let mut writer = LimitedWriter {
            file,
            owner: stored.clone(),
            cancel,
        };
        let counter = Rc::new(Cell::new(0usize));
        let filter_counter = counter.clone();
        let wanted = member.ordinal;
        let iterator = ArchiveIteratorBuilder::new(File::open(&source.path)?)
            .mtree_format(false)
            .filter(move |_, stat| {
                let ordinal = filter_counter.get();
                filter_counter.set(ordinal + 1);
                ordinal == wanted && regular(stat)
            })
            .build()
            .map_err(error)?;
        let mut found = false;
        for entry in iterator {
            cancelled(cancel)?;
            match entry {
                ArchiveContents::StartOfEntry(name, _) => {
                    if name != member.name {
                        return Err(io::Error::other("归档成员已变化"));
                    }
                    found = true;
                }
                ArchiveContents::DataChunk(bytes) => writer.write_all(&bytes)?,
                ArchiveContents::EndOfEntry if found => break,
                ArchiveContents::Err(failure) => return Err(error(failure)),
                _ => {}
            }
        }
        if !found {
            return Err(io::Error::other("归档成员不存在"));
        }
        writer.flush()?;
        path = stored.path().to_owned();
        owner = Some(stored);
    }
    let gz = source
        .member
        .as_ref()
        .map(|m| m.name.to_ascii_lowercase().ends_with(".log.gz"))
        .unwrap_or_else(|| {
            source
                .path
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".log.gz")
        });
    if gz {
        let (stored, file) = cache.file()?;
        let input = File::open(&path)?;
        let mut magic = [0; 2];
        (&input).read_exact(&mut magic)?;
        if magic != [0x1f, 0x8b] {
            return Err(io::Error::other("日志不包含有效 gzip 头"));
        }
        let input = File::open(&path)?;
        let mut writer = LimitedWriter {
            file,
            owner: stored.clone(),
            cancel,
        };
        compress_tools::uncompress_data(input, &mut writer).map_err(error)?;
        writer.flush()?;
        path = stored.path().to_owned();
        owner = Some(stored);
    }
    cancelled(cancel)?;
    source.check_revision()?;
    let len = File::open(&path)?.metadata()?.len();
    Ok(Prepared { path, owner, len })
}
