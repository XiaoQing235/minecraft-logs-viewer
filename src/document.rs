use crate::{
    platform::FileIdentity,
    text::{EDIT_LIMIT, IndexedLog},
};
use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE};
use std::{io, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Newline {
    Preserve,
    Lf,
    Crlf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalNewline {
    Preserve,
    Ensure,
    Remove,
}

#[derive(Clone, Debug)]
pub struct Output {
    pub encoding: &'static Encoding,
    pub bom: bool,
    pub newline: Newline,
    pub final_newline: FinalNewline,
    pub allow_errors: bool,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub title: String,
    pub base: String,
    pub original: String,
    pub output: Output,
    pub decode_errors: bool,
    pub protected: Vec<(PathBuf, FileIdentity)>,
    pub target: Option<PathBuf>,
    pub saved_revision: Option<crate::save::TargetStamp>,
}

impl Document {
    pub fn from_log(log: &IndexedLog, text: String) -> io::Result<Self> {
        if text.len() as u64 > EDIT_LIMIT {
            return Err(io::Error::other("副本超过 20 MiB，请缩小范围"));
        }
        let index = log.index.read().unwrap();
        Ok(Self {
            title: format!("{} 的副本", log.source.label()),
            base: text.replace("\r\n", "\n"),
            original: text,
            output: Output {
                encoding: log.encoding.encoding,
                bom: log.encoding.bom,
                newline: Newline::Preserve,
                final_newline: FinalNewline::Preserve,
                allow_errors: false,
            },
            decode_errors: index.errors,
            protected: vec![(
                log.source
                    .path
                    .canonicalize()
                    .or_else(|_| std::path::absolute(&log.source.path))?,
                log.source.revision.identity.clone(),
            )],
            target: None,
            saved_revision: None,
        })
    }
    pub fn encode(&self, edited: &str, output: &Output) -> io::Result<Vec<u8>> {
        if self.decode_errors && !output.allow_errors {
            return Err(io::Error::other(
                "来源包含解码错误。请先切换编码，或明确允许转换错误文字后另存",
            ));
        }
        let normalized = edited.replace("\r\n", "\n");
        let mut text = match output.newline {
            Newline::Lf => normalized,
            Newline::Crlf => normalized.replace('\n', "\r\n"),
            Newline::Preserve => {
                if normalized == self.base {
                    self.original.clone()
                } else {
                    let crlf = self.original.matches("\r\n").count();
                    let lf = self.original.matches('\n').count();
                    if crlf > 0 && crlf < lf {
                        return Err(io::Error::other(
                            "混合换行文档已修改，请明确选择 LF 或 CRLF",
                        ));
                    }
                    if crlf > 0 {
                        normalized.replace('\n', "\r\n")
                    } else {
                        normalized
                    }
                }
            }
        };
        match output.final_newline {
            FinalNewline::Preserve => {}
            FinalNewline::Ensure if !text.ends_with('\n') => text.push_str(
                if output.newline == Newline::Crlf
                    || output.newline == Newline::Preserve && self.original.contains("\r\n")
                {
                    "\r\n"
                } else {
                    "\n"
                },
            ),
            FinalNewline::Remove => {
                while text.ends_with('\n') {
                    text.pop();
                    if text.ends_with('\r') {
                        text.pop();
                    }
                }
            }
            _ => {}
        }
        let mut bytes = Vec::new();
        if output.encoding == UTF_16LE || output.encoding == UTF_16BE {
            if output.bom {
                bytes.extend_from_slice(if output.encoding == UTF_16LE {
                    &[0xff, 0xfe]
                } else {
                    &[0xfe, 0xff]
                });
            }
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&if output.encoding == UTF_16LE {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                });
            }
        } else {
            if output.bom {
                if output.encoding != UTF_8 {
                    return Err(io::Error::other("所选编码不支持 BOM"));
                }
                bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
            }
            let (encoded, _, errors) = output.encoding.encode(&text);
            if errors {
                return Err(io::Error::other(
                    "所选编码无法表示新增字符，请选择 UTF-8 或 UTF-16",
                ));
            }
            bytes.extend_from_slice(&encoded);
        }
        Ok(bytes)
    }
}
