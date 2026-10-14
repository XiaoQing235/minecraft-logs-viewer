use super::{search::SearchUi, theme, viewer};
use fltk::{
    app,
    enums::Damage,
    prelude::*,
    surface::ImageSurface,
    table::{TableContext, TableRow, TableRowSelectFlag},
    window::Window,
};
use minecraft_logs_viewer::{
    cache::Cache,
    search::Hit,
    source::{LogSource, Revision},
    text,
};
use std::{path::Path, sync::atomic::AtomicBool};

fn pixels(surface: &ImageSurface) -> Vec<u8> {
    surface.image().unwrap().to_rgb_data()
}

fn cell(pixels: &[u8], table: &TableRow, row: i32, col: i32) -> Vec<u8> {
    let (x, y, w, h) = table.find_cell(TableContext::Cell, row, col).unwrap();
    let mut result = Vec::new();
    for y in y..y + h {
        let start = (y as usize * 1000 + x as usize) * 3;
        result.extend_from_slice(&pixels[start..start + w.min(200) as usize * 3]);
    }
    result
}

fn check_selection(
    parent: &Window,
    table: &mut TableRow,
    col: i32,
    untouched: [i32; 2],
    selected: &[i32],
) {
    let surface = ImageSurface::new(1000, 600, false);
    surface.draw(table, table.x(), table.y());
    table.clear_damage();
    let before = pixels(&surface);
    for row in untouched {
        let region = cell(&before, table, row, col);
        assert!(
            region
                .iter()
                .enumerate()
                .any(|(index, value)| *value != region[index % 3])
        );
    }
    for &row in selected {
        table.select_row(row, TableRowSelectFlag::Select).unwrap();
    }
    assert!(table.damage_type().contains(Damage::Child));
    assert!(!table.damage_type().contains(Damage::All));
    // ImageSurface::draw forces Damage::All. update_child preserves the actual
    // partial damage produced by selection and draws onto the existing pixels.
    ImageSurface::push_current(&surface);
    parent.update_child(table);
    ImageSurface::pop_current();
    let after = pixels(&surface);
    for row in untouched {
        assert!(
            cell(&before, table, row, col) == cell(&after, table, row, col),
            "selection erased unselected row {row}"
        );
    }
    for &row in selected {
        assert!(cell(&before, table, row, col) != cell(&after, table, row, col));
    }
    for &row in selected {
        table.select_row(row, TableRowSelectFlag::Deselect).unwrap();
    }
    ImageSurface::push_current(&surface);
    parent.update_child(table);
    ImageSurface::pop_current();
    let cleared = pixels(&surface);
    for row in untouched.into_iter().chain(selected.iter().copied()) {
        assert!(cell(&before, table, row, col) == cell(&cleared, table, row, col));
    }
}

#[test]
fn selection_redraw_preserves_unselected_log_and_search_rows() {
    let _app = app::App::default();
    theme::apply(false);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/latest.log");
    let source = LogSource {
        relative: "latest.log".into(),
        revision: Revision::read(&path).unwrap(),
        path,
        member: None,
    };
    let cache = Cache::new().unwrap();
    let log = text::open(
        source.clone(),
        None,
        &cache,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    let parent = Window::default().with_size(1000, 600);
    let (mut table, view) = viewer::build();
    parent.end();
    table.resize(10, 10, 900, 420);
    view.borrow_mut().window.reset(0, log.clone());
    view.borrow_mut().log = Some(log.clone());
    viewer::extent(&mut table, &view.borrow());
    check_selection(&parent, &mut table, 3, [1, 4], &[2]);
    check_selection(&parent, &mut table, 3, [1, 4], &[2, 3]);
    table.resize(10, 10, 700, 320);
    check_selection(&parent, &mut table, 3, [1, 4], &[2, 3]);

    parent.begin();
    let mut search = SearchUi::build();
    parent.end();
    *search.hits.borrow_mut() = log
        .read_range(0, 4)
        .unwrap()
        .into_iter()
        .map(|line| Hit {
            source: source.clone(),
            line: line.number,
            ranges: Vec::new(),
            preview: line.raw,
            before: Vec::new(),
        })
        .collect();
    search.results.set_rows(4);
    search.results.resize(10, 10, 900, 420);
    check_selection(&parent, &mut search.results, 2, [0, 3], &[1]);
}
