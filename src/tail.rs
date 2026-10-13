use crate::{
    cache::{Cache, LimitedWriter, cancelled},
    source::{LogSource, Revision},
    text::{self, CHECKPOINT, Index, IndexedLog, SAMPLE_SIZE},
};
use encoding_rs::CoderResult;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{Arc, RwLock, atomic::AtomicBool},
    thread,
    time::Duration,
};

fn snapshot(
    source: &LogSource,
    writer: &LimitedWriter<'_>,
    encoding: &text::EncodingInfo,
    index: &Index,
) -> Arc<IndexedLog> {
    Arc::new(IndexedLog {
        source: source.clone(),
        path: writer.owner.path().to_owned(),
        backing: writer.owner.clone(),
        encoding: encoding.clone(),
        live: true,
        index: RwLock::new(index.clone()),
    })
}

fn anchor(file: &mut File, position: u64) -> io::Result<Vec<u8>> {
    let start = position.saturating_sub(64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = vec![0; (position - start) as usize];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub fn follow(
    mut source: LogSource,
    override_name: Option<&str>,
    cache: &Cache,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Arc<IndexedLog>),
    mut waiting: impl FnMut(),
) -> io::Result<()> {
    if !source.is_live() {
        return Err(io::Error::other("实时跟随仅支持普通 latest.log"));
    }
    'reset: loop {
        cancelled(cancel)?;
        let mut file = match File::open(&source.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                waiting();
                thread::sleep(Duration::from_millis(250));
                continue;
            }
            Err(error) => return Err(error),
        };
        source.revision = Revision::read(&source.path)?;
        let mut sample = vec![0; SAMPLE_SIZE];
        let read = file.read(&mut sample)?;
        sample.truncate(read);
        let encoding = text::detect(&sample, override_name)?;
        file.seek(SeekFrom::Start(encoding.skip as u64))?;
        let (owner, output) = cache.file()?;
        let mut writer = LimitedWriter {
            file: output,
            owner,
            cancel,
        };
        let mut decoder = encoding.encoding.new_decoder_without_bom_handling();
        let mut index = Index {
            checkpoints: vec![0],
            ..Index::default()
        };
        let mut raw_position = encoding.skip as u64;
        let mut offset = 0u64;
        let mut last_newline = 0u64;
        let mut previous = 0;
        let mut expected_anchor = anchor(&mut file, raw_position)?;
        let mut bytes = vec![0; SAMPLE_SIZE];
        let mut decoded = String::with_capacity(SAMPLE_SIZE * 4);
        let mut reported = 0usize;
        loop {
            cancelled(cancel)?;
            let revision = match Revision::read(&source.path) {
                Ok(revision) => revision,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    waiting();
                    thread::sleep(Duration::from_millis(250));
                    continue;
                }
                Err(error) => return Err(error),
            };
            if revision.identity != source.revision.identity
                || revision.len < raw_position
                || anchor(&mut file, raw_position)? != expected_anchor
            {
                continue 'reset;
            }
            source.revision = revision.clone();
            file.seek(SeekFrom::Start(raw_position))?;
            while raw_position < revision.len {
                cancelled(cancel)?;
                let wanted = (revision.len - raw_position).min(bytes.len() as u64) as usize;
                let read = file.read(&mut bytes[..wanted])?;
                if read == 0 {
                    continue 'reset;
                }
                raw_position += read as u64;
                let mut consumed = 0;
                loop {
                    decoded.clear();
                    let (result, read, errors) =
                        decoder.decode_to_string(&bytes[consumed..read], &mut decoded, false);
                    consumed += read;
                    index.errors |= errors;
                    writer.write_all(decoded.as_bytes())?;
                    for byte in decoded.bytes() {
                        offset += 1;
                        if offset - last_newline > text::MAX_LINE as u64 {
                            return Err(io::Error::other("单行超过 32 MiB，已停止跟随"));
                        }
                        if byte == b'\n' {
                            index.lines += 1;
                            last_newline = offset;
                            if previous == b'\r' {
                                index.crlf += 1;
                            } else {
                                index.lf += 1;
                            }
                            if index.lines.is_multiple_of(CHECKPOINT) {
                                index.checkpoints.push(offset);
                            }
                        }
                        previous = byte;
                    }
                    index.bytes = last_newline;
                    index.final_newline = last_newline > 0;
                    if result == CoderResult::InputEmpty {
                        break;
                    }
                }
                if index.lines >= reported + 8192 {
                    writer.flush()?;
                    progress(snapshot(&source, &writer, &encoding, &index));
                    reported = index.lines;
                }
            }
            expected_anchor = anchor(&mut file, raw_position)?;
            writer.flush()?;
            if reported != index.lines || reported == 0 {
                progress(snapshot(&source, &writer, &encoding, &index));
                reported = index.lines;
            }
            thread::sleep(Duration::from_millis(250));
        }
    }
}
