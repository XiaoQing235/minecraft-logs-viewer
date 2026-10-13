use crate::{
    archive::{self, Prepared},
    cache::{Cache, CacheFile, LimitedWriter, cancelled},
    source::LogSource,
};
use encoding_rs::{CoderResult, Encoding, UTF_8, UTF_16BE, UTF_16LE};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{Arc, RwLock, atomic::AtomicBool},
};

pub const SAMPLE_SIZE: usize = 128 * 1024;
pub const CHECKPOINT: usize = 256;
pub const MAX_LINE: usize = 32 * 1024 * 1024;
pub const EDIT_LIMIT: u64 = 20 * 1024 * 1024;
pub const ENCODINGS: &[&str] = &[
    "自动",
    "UTF-8",
    "UTF-16LE",
    "UTF-16BE",
    "GBK",
    "GB18030",
    "Big5",
    "Shift_JIS",
    "windows-1252",
];

#[derive(Clone, Debug)]
pub struct EncodingInfo {
    pub encoding: &'static Encoding,
    pub bom: bool,
    pub skip: usize,
    pub basis: String,
}

pub fn detect(sample: &[u8], override_name: Option<&str>) -> io::Result<EncodingInfo> {
    let bom = Encoding::for_bom(sample);
    if let Some(name) = override_name {
        let encoding =
            Encoding::for_label(name.as_bytes()).ok_or_else(|| io::Error::other("不支持的编码"))?;
        let skip = bom
            .filter(|(enc, _)| *enc == encoding)
            .map(|(_, skip)| skip)
            .unwrap_or(0);
        return Ok(EncodingInfo {
            encoding,
            bom: skip > 0,
            skip,
            basis: "手动选择".into(),
        });
    }
    if let Some((encoding, skip)) = bom {
        return Ok(EncodingInfo {
            encoding,
            bom: true,
            skip,
            basis: "BOM".into(),
        });
    }
    let utf8 = match std::str::from_utf8(sample) {
        Ok(_) => true,
        Err(error) => error.error_len().is_none(),
    };
    let encoding;
    let basis;
    let pairs = sample.len() / 2;
    let even_zeros = sample.chunks_exact(2).filter(|pair| pair[0] == 0).count();
    let odd_zeros = sample.chunks_exact(2).filter(|pair| pair[1] == 0).count();
    if pairs >= 4 && odd_zeros * 3 > pairs && even_zeros * 8 < pairs {
        encoding = UTF_16LE;
        basis = "可能的 UTF-16LE（字节分布）";
    } else if pairs >= 4 && even_zeros * 3 > pairs && odd_zeros * 8 < pairs {
        encoding = UTF_16BE;
        basis = "可能的 UTF-16BE（字节分布）";
    } else if utf8 {
        encoding = UTF_8;
        basis = if sample.is_ascii() {
            "ASCII，兼容 UTF-8；继续校验"
        } else {
            "UTF-8 样本校验；继续校验"
        };
    } else {
        let mut detector = chardetng::EncodingDetector::new();
        detector.feed(sample, false);
        encoding = detector.guess(None, true);
        basis = "可能的编码（自动检测）";
    }
    Ok(EncodingInfo {
        encoding,
        bom: false,
        skip: 0,
        basis: basis.into(),
    })
}

#[derive(Clone, Debug, Default)]
pub struct Index {
    pub checkpoints: Vec<u64>,
    pub lines: usize,
    pub bytes: u64,
    pub complete: bool,
    pub errors: bool,
    pub crlf: usize,
    pub lf: usize,
    pub final_newline: bool,
}

pub struct IndexedLog {
    pub source: LogSource,
    pub path: PathBuf,
    pub backing: Arc<CacheFile>,
    pub encoding: EncodingInfo,
    pub live: bool,
    pub index: RwLock<Index>,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub number: usize,
    pub raw: String,
    pub ending: String,
}

pub fn read_line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    let read = reader
        .take((MAX_LINE + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if read == 0 {
        return Ok(None);
    }
    if read > MAX_LINE {
        return Err(io::Error::other(
            "单行超过 32 MiB；请使用外部工具处理这一行",
        ));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| io::Error::other("UTF-8 缓存无效"))
}

impl IndexedLog {
    pub fn rows(&self) -> usize {
        self.index.read().unwrap().lines
    }
    pub fn read_range(&self, start: usize, count: usize) -> io::Result<Vec<Line>> {
        let index = self.index.read().unwrap();
        if start >= index.lines {
            return Ok(Vec::new());
        }
        let block = start / CHECKPOINT;
        let offset = index.checkpoints[block];
        let bytes = index.bytes;
        let end = start.saturating_add(count).min(index.lines);
        drop(index);
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut reader = BufReader::new(file.take(bytes.saturating_sub(offset)));
        let mut rows = Vec::new();
        for number in (block * CHECKPOINT)..end {
            let Some(mut raw) = read_line(&mut reader)? else {
                break;
            };
            if number < start {
                continue;
            }
            let ending = if raw.ends_with("\r\n") {
                raw.truncate(raw.len() - 2);
                "\r\n"
            } else if raw.ends_with('\n') {
                raw.pop();
                "\n"
            } else {
                ""
            };
            rows.push(Line {
                number,
                raw,
                ending: ending.into(),
            });
        }
        Ok(rows)
    }
    pub fn copy_range(&self, start: usize, count: usize) -> io::Result<String> {
        let mut output = String::new();
        let end = start.saturating_add(count).min(self.rows());
        for batch in (start..end).step_by(256) {
            for line in self.read_range(batch, (end - batch).min(256))? {
                if output.len() + line.raw.len() + line.ending.len() > EDIT_LIMIT as usize {
                    return Err(io::Error::other("复制片段超过 20 MiB，请缩小范围"));
                }
                output.push_str(&line.raw);
                output.push_str(&line.ending);
            }
        }
        Ok(output)
    }
    pub fn full_text(&self) -> io::Result<String> {
        let index = self.index.read().unwrap();
        if !index.complete && !self.live {
            return Err(io::Error::other("请等待日志读取完成"));
        }
        if index.bytes > EDIT_LIMIT {
            return Err(io::Error::other("全文超过 20 MiB，请选择片段创建副本"));
        }
        let bytes = index.bytes;
        drop(index);
        let mut text = String::new();
        File::open(&self.path)?
            .take(bytes)
            .read_to_string(&mut text)?;
        Ok(text)
    }
}

pub fn open(
    source: LogSource,
    override_name: Option<&str>,
    cache: &Cache,
    cancel: &AtomicBool,
    progress: impl FnMut(Arc<IndexedLog>),
) -> io::Result<Arc<IndexedLog>> {
    let prepared = archive::prepare(&source, cache, cancel)?;
    decode(source, prepared, override_name, cache, cancel, progress)
}

pub fn decode(
    source: LogSource,
    prepared: Prepared,
    override_name: Option<&str>,
    cache: &Cache,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Arc<IndexedLog>),
) -> io::Result<Arc<IndexedLog>> {
    let _keep_prepared = &prepared.owner;
    let mut input = File::open(&prepared.path)?.take(prepared.len);
    let mut sample = vec![0; SAMPLE_SIZE];
    let sampled = input.read(&mut sample)?;
    sample.truncate(sampled);
    let info = detect(&sample, override_name)?;
    let (backing, file) = cache.file()?;
    let log = Arc::new(IndexedLog {
        source,
        path: backing.path().to_owned(),
        backing: backing.clone(),
        encoding: info.clone(),
        live: false,
        index: RwLock::new(Index {
            checkpoints: vec![0],
            ..Index::default()
        }),
    });
    let mut writer = LimitedWriter {
        file,
        owner: backing,
        cancel,
    };
    let mut decoder = info.encoding.new_decoder_without_bom_handling();
    let mut input_bytes = sample[info.skip..].to_vec();
    let mut output = String::with_capacity(SAMPLE_SIZE * 4);
    let mut offset = 0u64;
    let mut line_start = 0u64;
    let mut complete_lines = 0usize;
    let mut previous = 0u8;
    let mut report_at = 512usize;
    let mut eof = false;
    loop {
        cancelled(cancel)?;
        let mut consumed = 0;
        loop {
            output.clear();
            let (result, read, errors) =
                decoder.decode_to_string(&input_bytes[consumed..], &mut output, eof);
            consumed += read;
            writer.write_all(output.as_bytes())?;
            let mut index = log.index.write().unwrap();
            index.errors |= errors;
            for byte in output.bytes() {
                offset += 1;
                if offset - line_start > MAX_LINE as u64 {
                    return Err(io::Error::other("单行超过 32 MiB，已停止读取"));
                }
                if byte == b'\n' {
                    line_start = offset;
                    complete_lines += 1;
                    if previous == b'\r' {
                        index.crlf += 1;
                    } else {
                        index.lf += 1;
                    }
                    if complete_lines.is_multiple_of(CHECKPOINT) {
                        index.checkpoints.push(offset);
                    }
                }
                previous = byte;
            }
            index.bytes = offset;
            index.lines = complete_lines;
            drop(index);
            if result == CoderResult::InputEmpty {
                break;
            }
        }
        writer.flush()?;
        if complete_lines >= report_at {
            progress(log.clone());
            report_at = complete_lines.saturating_add(8192);
        }
        if eof {
            break;
        }
        input_bytes.resize(SAMPLE_SIZE, 0);
        let read = input.read(&mut input_bytes)?;
        input_bytes.truncate(read);
        eof = read == 0;
    }
    log.source.check_revision()?;
    let mut index = log.index.write().unwrap();
    index.final_newline = offset > 0 && previous == b'\n';
    index.lines = complete_lines + usize::from(offset > 0 && previous != b'\n');
    index.complete = true;
    drop(index);
    progress(log.clone());
    Ok(log)
}

#[derive(Clone, Debug, Default)]
pub struct Record {
    pub time: Option<String>,
    pub thread: Option<String>,
    pub level: Option<String>,
    pub logger: Option<String>,
    pub message_offset: usize,
    pub exception_continuation: bool,
}

pub fn parse_record(raw: &str) -> Record {
    use std::sync::OnceLock;
    static PREFIX: OnceLock<regex::Regex> = OnceLock::new();
    let prefix = PREFIX.get_or_init(|| regex::Regex::new(r"^\[([^\]]+)\]\s+\[([^\]]+?)/(TRACE|DEBUG|INFO|WARN|ERROR|FATAL)\](?:\s+\[([^\]]+)\])?:?").unwrap());
    match prefix.captures(raw) {
        Some(captures) => Record {
            time: Some(captures[1].into()),
            thread: Some(captures[2].into()),
            level: Some(captures[3].into()),
            logger: captures.get(4).map(|m| m.as_str().into()),
            message_offset: captures.get(0).unwrap().end(),
            exception_continuation: false,
        },
        None => Record {
            exception_continuation: raw.trim_start().starts_with("at ")
                || raw.trim_start().starts_with("Caused by:")
                || raw.trim_start().starts_with("... "),
            ..Record::default()
        },
    }
}

pub fn stream_lines(
    prepared: Prepared,
    override_name: Option<&str>,
    cancel: &AtomicBool,
    mut visit: impl FnMut(usize, &str) -> io::Result<bool>,
) -> io::Result<bool> {
    let _keep_prepared = &prepared.owner;
    let mut input = File::open(&prepared.path)?.take(prepared.len);
    let mut sample = vec![0; SAMPLE_SIZE];
    let read = input.read(&mut sample)?;
    sample.truncate(read);
    let info = detect(&sample, override_name)?;
    let mut decoder = info.encoding.new_decoder_without_bom_handling();
    let mut bytes = sample[info.skip..].to_vec();
    let mut output = String::with_capacity(SAMPLE_SIZE * 4);
    let mut line = String::new();
    let mut number = 0;
    let mut had_errors = false;
    let mut eof = false;
    loop {
        cancelled(cancel)?;
        let mut consumed = 0;
        loop {
            output.clear();
            let (result, read, errors) =
                decoder.decode_to_string(&bytes[consumed..], &mut output, eof);
            consumed += read;
            had_errors |= errors;
            for part in output.split_inclusive('\n') {
                line.push_str(part);
                if line.len() > MAX_LINE {
                    return Err(io::Error::other("单行超过 32 MiB，搜索未完成"));
                }
                if line.ends_with('\n') {
                    let raw = line
                        .strip_suffix("\r\n")
                        .or_else(|| line.strip_suffix('\n'))
                        .unwrap_or(&line);
                    if !visit(number, raw)? {
                        return Ok(had_errors);
                    }
                    line.clear();
                    number += 1;
                }
            }
            if result == CoderResult::InputEmpty {
                break;
            }
        }
        if eof {
            break;
        }
        bytes.resize(SAMPLE_SIZE, 0);
        let read = input.read(&mut bytes)?;
        bytes.truncate(read);
        eof = read == 0;
    }
    if !line.is_empty() {
        visit(number, &line)?;
    }
    Ok(had_errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoding_detection_and_manual_bom() {
        assert_eq!(detect(b"\xff\xfeA\0B\0", None).unwrap().encoding, UTF_16LE);
        assert_eq!(detect(b"A\0B\0C\0D\0", None).unwrap().encoding, UTF_16LE);
        assert_eq!(detect("你好".as_bytes(), None).unwrap().encoding, UTF_8);
        assert_eq!(detect(b"\xef\xbb\xbfx", Some("UTF-8")).unwrap().skip, 3);
        assert!(detect(b"", Some("unknown")).is_err());
    }
    #[test]
    fn prefix_keeps_unknown_lines() {
        let record = parse_record("[12:34:56] [Server thread/ERROR] [minecraft/Main]: test");
        assert_eq!(record.level.as_deref(), Some("ERROR"));
        assert_eq!(record.logger.as_deref(), Some("minecraft/Main"));
        assert!(parse_record("\tat java.lang.Thread.run(Thread.java:1)").exception_continuation);
        assert_eq!(parse_record("unknown").level, None);
    }
}
