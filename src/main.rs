use fltk::{app, group::Flex, prelude::*, table::TableRow, text::TextEditor, window::Window};

fn main() {
    let app = app::App::default().with_scheme(app::Scheme::Gtk);
    let mut window = Window::new(100, 100, 1100, 720, "Minecraft Logs Viewer");
    let mut layout = Flex::default_fill().column();
    let mut table = TableRow::default();
    table.set_rows(1);
    table.set_cols(2);
    table.set_col_width_all(350);
    table.end();
    let mut editor = TextEditor::default();
    let mut buffer = fltk::text::TextBuffer::default();
    buffer.set_text("Minecraft Logs Viewer — 日志副本编辑\n");
    editor.set_buffer(buffer);
    layout.end();
    layout.fixed(&editor, 160);
    window.end();
    window.make_resizable(true);
    window.show();
    if std::env::args().any(|arg| arg == "--smoke") {
        app::add_timeout3(0.5, |_| app::quit());
    }
    let _ = regex::Regex::new(r"\[(INFO|WARN|ERROR)\]").unwrap();
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(b"Minecraft", true);
    let encoding = detector.guess(None, true);
    let _ = encoding.decode(b"Minecraft");
    let _ = compress_tools::list_archive_files(&mut std::io::Cursor::new(Vec::<u8>::new()));
    app.run().expect("desktop event loop failed");
}
