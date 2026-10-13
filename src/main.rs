#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod ui;

fn main() {
    if let Err(error) = app::run() {
        if std::env::args().any(|argument| argument == "--smoke") {
            eprintln!("smoke failed: {error}");
            std::process::exit(1);
        }
        fltk::dialog::alert_default(&format!("启动失败：{error}"));
        std::process::exit(1);
    }
}
