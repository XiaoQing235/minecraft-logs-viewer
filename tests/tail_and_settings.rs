use minecraft_logs_viewer::{
    cache::Cache,
    settings::Settings,
    source::{LogSource, Revision},
    tail,
    text::IndexedLog,
};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

fn source(path: PathBuf) -> LogSource {
    LogSource {
        relative: "latest.log".into(),
        revision: Revision::read(&path).unwrap(),
        path,
        member: None,
    }
}
fn wait_for(
    receiver: &mpsc::Receiver<Arc<IndexedLog>>,
    mut predicate: impl FnMut(&IndexedLog) -> bool,
) -> Arc<IndexedLog> {
    let started = std::time::Instant::now();
    loop {
        let log = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        if predicate(&log) {
            return log;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "tail did not reach expected state"
        );
    }
}
#[test]
fn tail_buffers_utf8_and_resets_on_truncate_and_rotation() {
    let cache = Cache::new().unwrap();
    let (owner, file) = cache.file().unwrap();
    drop(file);
    let path = owner.path().with_file_name("latest.log");
    fs::write(&path, [b"first\n".as_slice(), &[0xe4, 0xbd]].concat()).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_cache = cache.clone();
    let reader = source(path.clone());
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        tail::follow(
            reader,
            None,
            &worker_cache,
            &worker_cancel,
            |log| {
                sender.send(log).unwrap();
            },
            || {},
        )
    });
    let old = wait_for(&receiver, |log| log.rows() == 1);
    assert!(!old.index.read().unwrap().errors);
    assert_eq!(old.full_text().unwrap(), "first\n");
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[0xa0, b'\n'])
        .unwrap();
    let appended = wait_for(&receiver, |log| log.rows() == 2);
    assert_eq!(appended.read_range(1, 1).unwrap()[0].raw, "你");
    assert_eq!(old.rows(), 1);
    fs::write(&path, b"fresh\n").unwrap();
    wait_for(&receiver, |log| {
        log.rows() == 1 && log.read_range(0, 1).unwrap()[0].raw == "fresh"
    });
    let rotated = path.with_extension("old");
    fs::rename(&path, &rotated).unwrap();
    fs::write(&path, b"rotated\n").unwrap();
    wait_for(&receiver, |log| {
        log.rows() == 1 && log.read_range(0, 1).unwrap()[0].raw == "rotated"
    });
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(
        worker.join().unwrap().unwrap_err().kind(),
        std::io::ErrorKind::Interrupted
    );
    fs::remove_file(path).unwrap();
    fs::remove_file(rotated).unwrap();
}
#[test]
fn tail_holds_split_utf16_surrogates() {
    let cache = Cache::new().unwrap();
    let (owner, file) = cache.file().unwrap();
    drop(file);
    let path = owner.path().with_file_name("latest.log");
    fs::write(&path, [0xff, 0xfe, 0x3d, 0xd8]).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let worker_cache = cache.clone();
    let reader = source(path.clone());
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        tail::follow(
            reader,
            None,
            &worker_cache,
            &worker_cancel,
            |log| {
                sender.send(log).unwrap();
            },
            || {},
        )
    });
    let empty = wait_for(&receiver, |log| log.rows() == 0);
    assert!(!empty.index.read().unwrap().errors);
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[0x00, 0xde, 0x0a, 0x00])
        .unwrap();
    let appended = wait_for(&receiver, |log| log.rows() == 1);
    assert_eq!(appended.read_range(0, 1).unwrap()[0].raw, "😀");
    assert!(!appended.index.read().unwrap().errors);
    cancel.store(true, Ordering::Relaxed);
    assert_eq!(
        worker.join().unwrap().unwrap_err().kind(),
        std::io::ErrorKind::Interrupted
    );
    fs::remove_file(path).unwrap();
}
#[test]
fn settings_defaults_and_unicode_bookmark_serialization() {
    let settings: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(settings.window, [1280, 820]);
    let mut settings = Settings::default();
    settings.position("中文\0来源".into(), 123);
    let json = serde_json::to_string(&settings).unwrap();
    let restored: Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.positions["中文\0来源"], 123);
}
