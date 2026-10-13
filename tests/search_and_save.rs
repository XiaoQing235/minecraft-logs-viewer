use minecraft_logs_viewer::{
    cache::Cache,
    document::{Document, Newline},
    save,
    search::{self, Completion, Options},
    source::{self, LogSource, Revision},
    text,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn log_source(path: PathBuf) -> LogSource {
    LogSource {
        relative: path.file_name().unwrap().to_string_lossy().into(),
        revision: Revision::read(&path).unwrap(),
        path,
        member: None,
    }
}

#[test]
fn search_across_plain_and_archives_maps_visible_hits() {
    let cancel = AtomicBool::new(false);
    let cache = Cache::new().unwrap();
    let mut sources = vec![log_source(fixture("latest.log"))];
    for name in ["deflate.zip", "sample.tar.gz", "lzma2.7z"] {
        let container = log_source(fixture(name));
        for member in minecraft_logs_viewer::archive::list(&container.path, &cancel).unwrap() {
            let mut source = container.clone();
            source.member = Some(member);
            sources.push(source);
        }
    }
    let mut hits = Vec::new();
    let report = search::run(
        sources,
        Options {
            query: "你好ABCDEF".into(),
            ..Options::default()
        },
        &cache,
        &cancel,
        |batch| hits.extend(batch),
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Complete);
    assert_eq!(hits.len(), 10);
    for hit in hits {
        assert_eq!(hit.line, 0);
        assert_eq!(&hit.preview[hit.ranges[0].clone()], "你好§lABC§bDEF");
    }
}

#[test]
fn truncated_and_failed_searches_are_distinct() {
    let cancel = AtomicBool::new(false);
    let cache = Cache::new().unwrap();
    let (owner, mut file) = cache.file().unwrap();
    for _ in 0..10010 {
        writeln!(file, "hit").unwrap();
    }
    file.flush().unwrap();
    let report = search::run(
        vec![log_source(owner.path().to_owned())],
        Options {
            query: "hit".into(),
            ..Options::default()
        },
        &cache,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Truncated);
    assert_eq!(report.hit_lines, 10000);
    let mut stale = log_source(fixture("latest.log"));
    stale.revision.len += 1;
    let report = search::run(
        vec![stale],
        Options {
            query: "hit".into(),
            ..Options::default()
        },
        &cache,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(report.completion, Completion::Partial);
    assert_eq!(report.failures.len(), 1);
    let mut found = Vec::new();
    source::scan(
        &fixture(""),
        false,
        &cancel,
        |source| found.push(source),
        |_| {},
    );
    assert!(!found.is_empty());
}

#[test]
fn safe_save_rejects_original_hardlinks_and_external_changes() {
    let cancel = AtomicBool::new(false);
    let cache = Cache::new().unwrap();
    let (original, mut file) = cache.file().unwrap();
    file.write_all(b"original\n").unwrap();
    file.flush().unwrap();
    drop(file);
    let log = text::open(
        log_source(original.path().to_owned()),
        None,
        &cache,
        &cancel,
        |_| {},
    )
    .unwrap();
    let document = Document::from_log(&log, log.full_text().unwrap()).unwrap();
    let expected = save::target_revision(original.path()).unwrap();
    assert!(
        save::write_copy(
            original.path(),
            b"changed",
            &document.protected,
            &expected,
            &cancel
        )
        .is_err()
    );
    assert_eq!(fs::read(original.path()).unwrap(), b"original\n");
    let linked = original.path().with_extension("link");
    fs::hard_link(original.path(), &linked).unwrap();
    assert!(
        save::write_copy(
            &linked,
            b"changed",
            &document.protected,
            &save::target_revision(&linked).unwrap(),
            &cancel
        )
        .is_err()
    );
    fs::remove_file(linked).unwrap();
    let copy = original.path().with_extension("copy");
    let mut output = document.output.clone();
    output.newline = Newline::Crlf;
    let bytes = document.encode("edited\n", &output).unwrap();
    let stamp = save::write_copy(&copy, &bytes, &document.protected, &None, &cancel).unwrap();
    assert_eq!(fs::read(&copy).unwrap(), b"edited\r\n");
    fs::write(&copy, b"other!\r\n").unwrap();
    fs::File::options()
        .write(true)
        .open(&copy)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(stamp.revision.modified.unwrap()))
        .unwrap();
    assert!(save::write_copy(&copy, b"new", &document.protected, &Some(stamp), &cancel).is_err());
    assert_eq!(fs::read(&copy).unwrap(), b"other!\r\n");
    fs::remove_file(copy).unwrap();
}

#[test]
fn utf16_output_and_unrepresentable_characters() {
    let cancel = AtomicBool::new(false);
    let cache = Cache::new().unwrap();
    let log = text::open(
        log_source(fixture("utf16.log")),
        None,
        &cache,
        &cancel,
        |_| {},
    )
    .unwrap();
    let document = Document::from_log(&log, log.full_text().unwrap()).unwrap();
    let bytes = document.encode("你好😀\n", &document.output).unwrap();
    assert_eq!(&bytes[..2], &[0xff, 0xfe]);
    let (decoded, _, errors) = encoding_rs::UTF_16LE.decode(&bytes);
    assert_eq!(decoded, "你好😀\r\n");
    assert!(!errors);
    let mut output = document.output.clone();
    output.encoding = encoding_rs::WINDOWS_1252;
    output.bom = false;
    assert!(document.encode("你好😀", &output).is_err());
}
