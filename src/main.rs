#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod ui;

fn main() {
    if let Err(error) = app::run() {
        fltk::dialog::alert_default(&format!("启动失败：{error}"));
        std::process::exit(1);
    }
}
