use crate::{
    archive,
    cache::Cache,
    format::StyledLine,
    source::LogSource,
    text::{self, parse_record},
};
use regex::{Regex, RegexBuilder};
use std::{
    collections::BTreeMap,
    io,
    ops::Range,
    sync::atomic::{AtomicBool, Ordering},
};

pub const HIT_LIMIT: usize = 10000;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub query: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub raw: bool,
    pub level: String,
    pub thread: String,
    pub logger: String,
    pub time_from: String,
    pub time_to: String,
    pub encoding: Option<String>,
    pub encoding_overrides: BTreeMap<String, String>,
}

impl Options {
    pub fn compile(&self) -> io::Result<Regex> {
        if self.query.is_empty()
            && self.level.is_empty()
            && self.thread.is_empty()
            && self.logger.is_empty()
            && self.time_from.is_empty()
            && self.time_to.is_empty()
        {
            return Err(io::Error::other("请输入搜索文字或字段条件"));
        }
        for time in [&self.time_from, &self.time_to] {
            if time.is_empty() {
                continue;
            }
            let parts: Vec<_> = time.split(':').collect();
            if parts.len() != 3
                || parts.iter().any(|part| part.len() != 2)
                || parts.iter().enumerate().any(|(index, part)| {
                    part.parse::<u8>()
                        .map_or(true, |value| value > if index == 0 { 23 } else { 59 })
                })
            {
                return Err(io::Error::other(
                    "时间条件使用 HH:MM:SS（00:00:00 至 23:59:59）",
                ));
            }
        }
        if !self.time_from.is_empty() && !self.time_to.is_empty() && self.time_from > self.time_to {
            return Err(io::Error::other("开始时间不能晚于结束时间"));
        }
        RegexBuilder::new(&if self.regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        })
        .case_insensitive(!self.case_sensitive)
        .build()
        .map_err(|error| io::Error::other(error.to_string()))
    }
    pub fn matches(&self, raw: &str, matcher: &Regex) -> Vec<Range<usize>> {
        let styled = StyledLine::parse(raw);
        let record = parse_record(&styled.text);
        if !self.level.is_empty() && record.level.as_deref() != Some(&self.level) {
            return Vec::new();
        }
        for (filter, field) in [
            (&self.thread, &record.thread),
            (&self.logger, &record.logger),
        ] {
            if !filter.is_empty()
                && !field
                    .as_ref()
                    .is_some_and(|field| field.to_lowercase().contains(&filter.to_lowercase()))
            {
                return Vec::new();
            }
        }
        if !self.time_from.is_empty() || !self.time_to.is_empty() {
            let Some(time) = &record.time else {
                return Vec::new();
            };
            let time = time.rsplit(' ').next().unwrap_or(time);
            if !self.time_from.is_empty() && time < self.time_from.as_str()
                || !self.time_to.is_empty() && time > self.time_to.as_str()
            {
                return Vec::new();
            }
        }
        let haystack = if self.raw { raw } else { &styled.text };
        if self.query.is_empty() {
            return std::iter::once(0..raw.len()).collect();
        }
        matcher
            .find_iter(haystack)
            .take(256)
            .filter_map(|found| {
                if self.raw {
                    Some(found.range())
                } else if found.is_empty() {
                    Some(0..0)
                } else {
                    styled.raw_range(found.range())
                }
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub source: LogSource,
    pub line: usize,
    pub ranges: Vec<Range<usize>>,
    pub preview: String,
    pub before: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Completion {
    Complete,
    Partial,
    Truncated,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub completion: Completion,
    pub hit_lines: usize,
    pub failures: Vec<String>,
    pub scanned: usize,
}

fn preview(text: &str) -> String {
    let mut end = text.len().min(1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

pub fn run(
    sources: Vec<LogSource>,
    options: Options,
    cache: &Cache,
    cancel: &AtomicBool,
    mut batch: impl FnMut(Vec<Hit>),
) -> io::Result<Report> {
    let matcher = options.compile()?;
    let mut report = Report {
        completion: Completion::Complete,
        hit_lines: 0,
        failures: Vec::new(),
        scanned: 0,
    };
    let mut buffered = Vec::new();
    let mut groups = BTreeMap::new();
    for source in sources {
        groups
            .entry(source.path.clone())
            .or_insert_with(Vec::new)
            .push(source);
    }
    'groups: for group in groups.into_values() {
        if cancel.load(Ordering::Relaxed) {
            report.completion = Completion::Cancelled;
            break;
        }
        let mut process = |source: &LogSource, prepared: archive::Prepared| -> io::Result<bool> {
            let mut previous = std::collections::VecDeque::<String>::new();
            let encoding = options
                .encoding_overrides
                .get(&source.id())
                .map(String::as_str)
                .or(options.encoding.as_deref());
            let result = text::stream_lines(prepared, encoding, cancel, |number, raw| {
                let ranges = options.matches(raw, &matcher);
                if !ranges.is_empty() {
                    buffered.push(Hit {
                        source: source.clone(),
                        line: number,
                        ranges,
                        preview: preview(raw),
                        before: previous.iter().cloned().collect(),
                    });
                    report.hit_lines += 1;
                    if buffered.len() >= 128 {
                        batch(std::mem::take(&mut buffered));
                    }
                    if report.hit_lines >= HIT_LIMIT {
                        report.completion = Completion::Truncated;
                        return Ok(false);
                    }
                }
                previous.push_back(preview(raw));
                if previous.len() > 2 {
                    previous.pop_front();
                }
                Ok(true)
            });
            match result {
                Ok(errors) => {
                    if errors {
                        report.failures.push(format!(
                            "{}: 存在解码错误，请指定编码后重新搜索",
                            source.label()
                        ));
                    }
                    if let Err(error) = source.check_revision() {
                        report.failures.push(format!("{}: {error}", source.label()));
                    }
                    report.scanned += 1;
                }
                Err(error) => report.failures.push(format!("{}: {error}", source.label())),
            }
            Ok(report.completion != Completion::Truncated && !cancel.load(Ordering::Relaxed))
        };
        if group[0].member.is_some() {
            if let Err(error) = archive::visit_group(&group, cache, cancel, &mut process) {
                report
                    .failures
                    .push(format!("{}: {error}", group[0].relative));
            }
        } else {
            match archive::prepare(&group[0], cache, cancel) {
                Ok(prepared) => {
                    process(&group[0], prepared)?;
                }
                Err(error) => report
                    .failures
                    .push(format!("{}: {error}", group[0].label())),
            }
        }
        if report.completion == Completion::Truncated {
            break 'groups;
        }
    }
    if !buffered.is_empty() {
        batch(buffered);
    }
    if cancel.load(Ordering::Relaxed) {
        report.completion = Completion::Cancelled;
    } else if report.completion == Completion::Complete && !report.failures.is_empty() {
        report.completion = Completion::Partial;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_unicode_matches_cross_codes() {
        let options = Options {
            query: "你好ABCDEF".into(),
            ..Options::default()
        };
        let raw = "[12:34:56] [Server thread/INFO]: §a你好§lABC§bDEF";
        let ranges = options.matches(raw, &options.compile().unwrap());
        assert_eq!(&raw[ranges[0].clone()], "你好§lABC§bDEF");
    }
    #[test]
    fn invalid_regex_and_time_and_fields() {
        assert!(
            Options {
                query: "[".into(),
                regex: true,
                ..Options::default()
            }
            .compile()
            .is_err()
        );
        let options = Options {
            query: "message".into(),
            level: "WARN".into(),
            time_from: "12:00:00".into(),
            ..Options::default()
        };
        assert!(
            !options
                .matches(
                    "[12:34:56] [Server thread/WARN]: MESSAGE",
                    &options.compile().unwrap()
                )
                .is_empty()
        );
        assert!(
            options
                .matches(
                    "[11:34:56] [Server thread/WARN]: message",
                    &options.compile().unwrap()
                )
                .is_empty()
        );
    }
}
