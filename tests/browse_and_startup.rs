use minecraft_logs_viewer::{
    browse::{Anchor, Window, scroll},
    cache::Cache,
    source::{LogSource, Revision},
    startup, text,
};
use std::{fs, path::Path, sync::atomic::AtomicBool};

fn source(name: &str) -> LogSource {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    LogSource {
        relative: name.into(),
        revision: Revision::read(&path).unwrap(),
        path,
        member: None,
    }
}

#[test]
fn stream_maps_source_lines_copies_across_boundaries_and_keeps_scroll_anchor() {
    let cache = Cache::new().unwrap();
    let cancel = AtomicBool::new(false);
    let first = text::open(source("latest.log"), None, &cache, &cancel, |_| {}).unwrap();
    let second = text::open(source("sample.log.gz"), None, &cache, &cancel, |_| {}).unwrap();
    let empty = text::open(source("empty.log"), None, &cache, &cancel, |_| {}).unwrap();
    let mut window = Window::default();
    window.reset(1, first.clone());
    window.update(2, empty, false);
    window.update(3, second, false);
    assert_eq!(window.rows(), 11); // Two four-line files plus three separators.
    assert_eq!(
        window.locate(4),
        Some(Anchor {
            source: 1,
            line: Some(3)
        })
    );
    assert_eq!(
        window.locate(5),
        Some(Anchor {
            source: 2,
            line: None
        })
    );
    assert_eq!(
        window.locate(7),
        Some(Anchor {
            source: 3,
            line: Some(0)
        })
    );
    let expected = format!("last line\n{}", first.read_range(0, 1).unwrap()[0].raw);
    assert_eq!(window.copy(4, 7).unwrap(), format!("{expected}\r\n"));
    let anchor = window.locate(8).unwrap();
    window.update(0, first, true);
    assert_eq!(window.position(anchor), Some(13));
    let removed = window.trim(13, 1);
    assert_eq!(removed, 10);
    assert_eq!(window.position(anchor), Some(3));
    assert_eq!(window.files.len(), 2);
}

#[test]
fn streaming_extent_changes_do_not_move_published_file_boundaries() {
    let cache = Cache::new().unwrap();
    let cancel = AtomicBool::new(false);
    let log = text::open(source("latest.log"), None, &cache, &cancel, |_| {}).unwrap();
    log.index.write().unwrap().lines = 2;
    let mut window = Window::default();
    window.reset(0, log.clone());
    log.index.write().unwrap().lines = 4;
    assert_eq!(window.rows(), 3);
    window.update(0, log, false);
    assert_eq!(window.rows(), 5);
}

#[test]
fn wheel_distance_survives_forward_loading_and_backward_prepend() {
    assert_eq!(scroll(2, 7, 5, false, true), (4, 5));
    assert_eq!(scroll(4, 5, 12, false, true), (9, 0));
    assert_eq!(scroll(0, -4, 5, true, false), (0, -4));
    // The original top gains five rows when the preceding file is inserted.
    assert_eq!(scroll(5, -4, 10, false, false), (1, 0));
    assert_eq!(scroll(0, -50, 10, false, true), (0, 0));
    assert_eq!(scroll(9, 50, 10, true, false), (9, 0));
}

#[test]
fn startup_finds_working_or_executable_logs_before_recent_directories() {
    let directory = std::env::temp_dir().join(format!(
        "mlv-startup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(directory.join("working/logs")).unwrap();
    fs::create_dir_all(directory.join("portable/Logs")).unwrap();
    fs::create_dir_all(directory.join("other")).unwrap();
    let executable = directory.join("portable/Logs/viewer");
    assert_eq!(
        startup::detect(&directory.join("working"), &executable),
        Some(directory.join("working/logs"))
    );
    assert_eq!(
        startup::detect(&directory.join("working/logs"), &executable),
        Some(directory.join("working/logs"))
    );
    assert_eq!(
        startup::detect(&directory.join("other"), &executable),
        Some(directory.join("portable/Logs"))
    );
    assert_eq!(
        startup::detect(&directory.join("other"), &directory.join("working/viewer")),
        Some(directory.join("working/logs"))
    );
    assert_eq!(
        startup::detect(&directory.join("other"), &directory.join("other/viewer")),
        None
    );
    fs::remove_dir_all(directory).unwrap();
}
