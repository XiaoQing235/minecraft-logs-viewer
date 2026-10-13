//! A reproducible core-path measurement; it does not measure window rendering.
use minecraft_logs_viewer::{
    cache::Cache,
    search,
    source::{LogSource, Revision},
    text,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

fn main() -> std::io::Result<()> {
    let path = PathBuf::from(std::env::args_os().nth(1).expect("provide a generated log"));
    let source = LogSource {
        relative: path.file_name().unwrap().to_string_lossy().into_owned(),
        revision: Revision::read(&path)?,
        path,
        member: None,
    };
    let cancel = AtomicBool::new(false);
    let cache = Cache::new()?;
    let start = Instant::now();
    let mut first = None;
    let log = text::open(source.clone(), None, &cache, &cancel, |_| {
        if first.is_none() {
            first = Some(start.elapsed());
        }
    })?;
    let indexing = start.elapsed();
    let start = Instant::now();
    for index in 0..1000 {
        let line = index * log.rows() / 1000;
        assert_eq!(log.read_range(line, 1)?[0].number, line);
    }
    let positioning = start.elapsed();
    let start = Instant::now();
    let report = search::run(
        vec![source.clone()],
        search::Options {
            query: "NO_MATCH_SENTINEL".into(),
            ..Default::default()
        },
        &cache,
        &cancel,
        |_| {},
    )?;
    let searching = start.elapsed();
    assert_eq!(report.completion, search::Completion::Complete);
    assert_eq!(report.hit_lines, 0);
    cancel.store(true, Ordering::Relaxed);
    let start = Instant::now();
    assert!(text::open(source, None, &cache, &cancel, |_| {}).is_err());
    let cancelled = start.elapsed();
    println!(
        "{}",
        serde_json::json!({
            "bytes": log.index.read().unwrap().bytes,
            "rows": log.rows(),
            "first_batch_ms": first.unwrap().as_secs_f64() * 1000.0,
            "index_seconds": indexing.as_secs_f64(),
            "seek_1000_ms": positioning.as_secs_f64() * 1000.0,
            "search_seconds": searching.as_secs_f64(),
            "search_mib_per_second": log.index.read().unwrap().bytes as f64 / 1048576.0 / searching.as_secs_f64(),
            "pre_cancelled_ms": cancelled.as_secs_f64() * 1000.0,
            "cache_bytes": cache.used()
        })
    );
    Ok(())
}
