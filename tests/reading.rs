use minecraft_logs_viewer::{
    archive,
    cache::Cache,
    source::{LogSource, Revision},
    text,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn source(path: PathBuf) -> LogSource {
    LogSource {
        relative: path.file_name().unwrap().to_string_lossy().into(),
        revision: Revision::read(&path).unwrap(),
        path,
        member: None,
    }
}

#[test]
fn ordinary_gzip_utf16_and_manual_chinese_round_trip() {
    let cache = Cache::new().unwrap();
    let cancel = AtomicBool::new(false);
    let expected = fs::read_to_string(fixture("latest.log")).unwrap();
    for (name, encoding) in [
        ("latest.log", None),
        ("sample.log.gz", None),
        ("utf16.log", None),
        ("gb18030.log", Some("GB18030")),
    ] {
        let log = text::open(source(fixture(name)), encoding, &cache, &cancel, |_| {}).unwrap();
        assert_eq!(log.full_text().unwrap(), expected, "{name}");
        assert_eq!(log.rows(), 4);
        assert_eq!(log.read_range(3, 2).unwrap()[0].raw, "last line");
        assert!(!log.index.read().unwrap().final_newline);
    }
    let empty = text::open(source(fixture("empty.log")), None, &cache, &cancel, |_| {}).unwrap();
    assert_eq!(empty.rows(), 0);
}

#[test]
fn archive_scope_duplicates_and_inner_gzip() {
    let cache = Cache::new().unwrap();
    let cancel = AtomicBool::new(false);
    let expected = fs::read_to_string(fixture("latest.log")).unwrap();
    for name in [
        "stored.zip",
        "deflate.zip",
        "sample.tar",
        "sample.tar.gz",
        "lzma.7z",
        "lzma2.7z",
    ] {
        let members = archive::list(&fixture(name), &cancel).unwrap();
        assert_eq!(
            members.len(),
            if name.ends_with(".zip") { 4 } else { 3 },
            "{name}"
        );
        for member in members {
            let mut log_source = source(fixture(name));
            log_source.member = Some(member.clone());
            let log = text::open(log_source, None, &cache, &cancel, |_| {}).unwrap();
            assert_eq!(
                log.full_text().unwrap(),
                if member.ordinal == 6 {
                    "second occurrence\n"
                } else {
                    &expected
                },
                "{name}:{}",
                member.name
            );
        }
    }
}

#[test]
fn rar4_and_rar5_stored_compressed_and_solid_decoders() {
    use minecraft_logs_viewer::source::Member;
    let cache = Cache::new().unwrap();
    let cancel = AtomicBool::new(false);
    let bytes = |archive_name: &str, name: &str, ordinal, size| {
        let mut input = source(fixture(archive_name));
        input.member = Some(Member {
            name: name.into(),
            ordinal,
            size,
        });
        let prepared = archive::prepare(&input, &cache, &cancel).unwrap();
        fs::read(&prepared.path).unwrap()
    };
    assert_eq!(
        bytes("rar4.rar", "test.txt", 0, 20),
        b"test text document\r\n"
    );
    assert_eq!(
        bytes("rar5-stored.rar", "helloworld.txt", 0, 28),
        b"hello libarchive test suite!\n"
    );
    let expected = |magic: i32, size| -> Vec<u8> {
        (1..=size / 4)
            .flat_map(|k| (k * k - 3 * k + 1 + magic).max(0).to_le_bytes())
            .collect()
    };
    assert_eq!(
        bytes("rar5-compressed.rar", "test.bin", 0, 1200),
        expected(0, 1200)
    );
    // The final member requires decoding the preceding solid dictionary.
    assert_eq!(
        bytes("rar5-solid.rar", "test4.bin", 3, 4096),
        expected(4, 4096)
    );
}

#[test]
fn sparse_index_across_decoding_chunks_and_final_line() {
    let cache = Cache::new().unwrap();
    let cancel = AtomicBool::new(false);
    let (file, mut writer) = cache.file().unwrap();
    use std::io::Write;
    for number in 0..12000 {
        writeln!(writer, "{number}:你好§a文档").unwrap();
    }
    write!(writer, "tail").unwrap();
    writer.flush().unwrap();
    let log = text::open(
        source(file.path().to_owned()),
        None,
        &cache,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(log.rows(), 12001);
    let rows = log.read_range(8191, 3).unwrap();
    assert_eq!(rows[0].raw, "8191:你好§a文档");
    assert_eq!(rows[2].number, 8193);
    assert_eq!(log.read_range(12000, 1).unwrap()[0].raw, "tail");
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(text::open(source(fixture("latest.log")), None, &cache, &cancel, |_| {}).is_err());
}
