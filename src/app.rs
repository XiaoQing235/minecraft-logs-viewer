use crate::ui::viewer;
use fltk::{
    app,
    browser::HoldBrowser,
    button::{Button, CheckButton},
    dialog::{self, FileDialogType, NativeFileChooser},
    enums::CallbackTrigger,
    frame::Frame,
    group::{Flex, Group, Tabs},
    input::{Input, IntInput},
    menu::Choice,
    prelude::*,
    tree::{Tree, TreeSelect},
    window::Window,
};
use minecraft_logs_viewer::{
    source::LogSource,
    text::ENCODINGS,
    worker::{Event, Workers},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Action {
    Open,
    Refresh,
    Selected,
    Encoding,
    Jump,
    Copy,
    Cancel,
}

fn browse_directory() -> Option<PathBuf> {
    let mut chooser = NativeFileChooser::new(FileDialogType::BrowseDir);
    chooser.set_title("打开 Minecraft 日志目录");
    chooser.show();
    let path = chooser.filename();
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

pub fn run() -> std::io::Result<()> {
    let application = app::App::default().with_scheme(app::Scheme::Gtk);
    app::set_visible_focus(true);
    let (commands, actions) = app::channel::<Action>();
    let mut window = Window::new(80, 80, 1200, 780, "Minecraft Logs Viewer");
    window.size_range(820, 520, 0, 0);
    let mut layout = Flex::default_fill().column();
    layout.set_margin(10);
    layout.set_pad(8);
    let mut top = Flex::default().row();
    let mut open = Button::default().with_label("打开目录");
    let mut directory = Input::default();
    directory.set_readonly(true);
    let mut refresh = Button::default().with_label("刷新");
    let recursive = CheckButton::default().with_label("子目录");
    let mut cancel = Button::default().with_label("取消任务");
    top.end();
    top.fixed(&open, 100);
    top.fixed(&refresh, 70);
    top.fixed(&recursive, 90);
    top.fixed(&cancel, 90);
    layout.fixed(&top, 34);
    let mut tools = Flex::default().row();
    let mut encoding = Choice::default();
    encoding.add_choice(&ENCODINGS.join("|"));
    encoding.set_value(0);
    let mut raw = CheckButton::default().with_label("显示原文");
    let mut obfuscate = CheckButton::default().with_label("混淆预览");
    let mut jump = IntInput::default();
    jump.set_value("1");
    jump.set_tooltip("跳转行号");
    let mut go = Button::default().with_label("跳转");
    let mut copy = Button::default().with_label("复制选中行");
    tools.end();
    tools.fixed(&encoding, 130);
    tools.fixed(&raw, 115);
    tools.fixed(&obfuscate, 110);
    tools.fixed(&jump, 120);
    tools.fixed(&go, 60);
    tools.fixed(&copy, 115);
    layout.fixed(&tools, 32);
    let mut body = Flex::default().row();
    let mut tree = Tree::default();
    tree.set_show_root(false);
    tree.set_select_mode(TreeSelect::Multi);
    let mut tabs = Tabs::default();
    let mut log_tab = Group::default().with_label("日志");
    let mut log_layout = Flex::default_fill().column();
    log_layout.set_margin(4);
    let (mut table, view) = viewer::build();
    log_layout.end();
    log_tab.resizable(&log_layout);
    log_tab.end();
    let errors_tab = Group::default().with_label("读取信息");
    let mut error_layout = Flex::default_fill().column();
    error_layout.set_margin(5);
    let mut errors = HoldBrowser::default();
    error_layout.end();
    errors_tab.resizable(&error_layout);
    errors_tab.end();
    tabs.end();
    tabs.auto_layout();
    body.end();
    body.fixed(&tree, 285);
    let mut status = Frame::default().with_label("打开目录以浏览普通日志和归档成员");
    status.set_align(fltk::enums::Align::Left | fltk::enums::Align::Inside);
    layout.end();
    layout.fixed(&status, 26);
    window.end();
    window.make_resizable(true);
    layout.recalc();
    tabs.auto_layout();
    log_layout.resize(tabs.x() + 4, tabs.y() + 28, tabs.w() - 8, tabs.h() - 32);
    error_layout.resize(tabs.x() + 4, tabs.y() + 28, tabs.w() - 8, tabs.h() - 32);
    open.set_callback(move |_| commands.send(Action::Open));
    refresh.set_callback(move |_| commands.send(Action::Refresh));
    cancel.set_callback(move |_| commands.send(Action::Cancel));
    tree.set_callback(move |_| commands.send(Action::Selected));
    encoding.set_callback(move |_| commands.send(Action::Encoding));
    go.set_callback(move |_| commands.send(Action::Jump));
    copy.set_callback(move |_| commands.send(Action::Copy));
    let raw_view = view.clone();
    let mut raw_table = table.clone();
    raw.set_callback(move |button| {
        raw_view.borrow_mut().raw = button.is_checked();
        raw_table.redraw();
    });
    let obfuscated_view = view.clone();
    let mut obfuscated_table = table.clone();
    obfuscate.set_callback(move |button| {
        obfuscated_view.borrow_mut().obfuscate = button.is_checked();
        obfuscated_table.redraw();
    });
    jump.set_trigger(CallbackTrigger::EnterKey);
    jump.set_callback(move |_| commands.send(Action::Jump));
    let mut workers = Workers::new()?;
    let mut sources = Vec::<LogSource>::new();
    let mut root = None::<PathBuf>;
    let mut scan_id = 0;
    let mut read_id = 0;
    let mut active = None::<usize>;
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    let mut arguments = std::env::args_os().skip(1);
    let mut initial = None;
    let mut screenshot = None;
    while let Some(argument) = arguments.next() {
        if argument == "--screenshot" {
            screenshot = arguments.next().map(PathBuf::from);
        } else if argument != "--smoke" {
            initial = Some(PathBuf::from(argument));
        }
    }
    if let Some(path) = initial {
        directory.set_value(&path.to_string_lossy());
        root = Some(path.clone());
        scan_id = workers.scan(path, recursive.is_checked());
    }
    window.show();
    if smoke {
        eprintln!("smoke: window shown={}", window.shown());
    }
    app::add_timeout3(0.05, |handle| {
        app::awake();
        app::repeat_timeout3(0.05, handle);
    });
    let started = Instant::now();
    while application.wait() {
        while let Some(action) = actions.recv() {
            match action {
                Action::Open | Action::Refresh => {
                    let chosen = match action {
                        Action::Open => browse_directory(),
                        _ => root.clone(),
                    };
                    if let Some(path) = chosen {
                        workers.cancel();
                        sources.clear();
                        tree.clear();
                        errors.clear();
                        active = None;
                        read_id = 0;
                        view.borrow_mut().log = None;
                        view.borrow_mut().rows.clear();
                        table.set_rows(0);
                        directory.set_value(&path.to_string_lossy());
                        root = Some(path.clone());
                        scan_id = workers.scan(path, recursive.is_checked());
                        status.set_label("扫描中…");
                    }
                }
                Action::Selected | Action::Encoding => {
                    let selected = if matches!(action, Action::Encoding) {
                        active
                    } else {
                        tree.callback_item()
                            .and_then(|item| item.label())
                            .and_then(|label| {
                                label
                                    .strip_prefix('[')
                                    .and_then(|s| s.split_once(']'))
                                    .and_then(|(n, _)| n.parse::<usize>().ok())
                            })
                    };
                    if let Some(index) = selected.filter(|index| *index < sources.len()) {
                        active = Some(index);
                        view.borrow_mut().log = None;
                        view.borrow_mut().rows.clear();
                        table.set_rows(0);
                        let encoding_name = (encoding.value() > 0)
                            .then(|| ENCODINGS[encoding.value() as usize].to_owned());
                        read_id = workers.open(sources[index].clone(), encoding_name);
                        status.set_label("读取与建立索引…");
                        let _ = tabs.set_value(&log_tab);
                    }
                }
                Action::Jump => {
                    if let Ok(line) = jump.value().parse::<i32>() {
                        if line > 0 && line <= table.rows() {
                            table.set_row_position(line - 1);
                        } else {
                            status.set_label("行号超出已读取范围");
                        }
                    }
                }
                Action::Copy => {
                    if let Some(log) = view.borrow().log.clone() {
                        let (first, _, last, _) = table.get_selection();
                        if first >= 0 {
                            match log.copy_range(first as usize, (last - first + 1) as usize) {
                                Ok(text) => app::copy(&text),
                                Err(error) => dialog::alert_default(&error.to_string()),
                            }
                        }
                    }
                }
                Action::Cancel => {
                    workers.cancel();
                    status.set_label("任务已取消，当前内容可能不完整");
                }
            }
        }
        for _ in 0..128 {
            let Ok(event) = workers.events.try_recv() else {
                break;
            };
            match event {
                Event::Source(id, source) if id == scan_id => {
                    let index = sources.len();
                    let leaf = format!(
                        "[{index}] {}",
                        source
                            .member
                            .as_ref()
                            .map(|m| format!("{} [#{}]", m.name.replace('/', "∕"), m.ordinal + 1))
                            .unwrap_or_else(|| source.relative.replace(['/', '\\'], "∕"))
                    );
                    let branch = if source.member.is_some() {
                        format!("归档/{}", source.relative.replace(['/', '\\'], "∕"))
                    } else {
                        "普通日志".into()
                    };
                    tree.add(&format!("{branch}/{leaf}"));
                    sources.push(source);
                    status.set_label(&format!("扫描中 · {} 个日志来源", sources.len()));
                }
                Event::Scanned(id, failures) if id == scan_id => {
                    for failure in &failures {
                        errors.add(failure);
                    }
                    status.set_label(&format!(
                        "扫描完成 · {} 个来源 · {} 个失败项",
                        sources.len(),
                        failures.len()
                    ));
                    if active.is_none() && !sources.is_empty() {
                        active = Some(0);
                        read_id = workers.open(sources[0].clone(), None);
                    }
                }
                Event::Loaded(id, log) if id == read_id => {
                    let index = log.index.read().unwrap().clone();
                    table.set_rows(index.lines.min(i32::MAX as usize) as i32);
                    view.borrow_mut().log = Some(log.clone());
                    view.borrow_mut().rows.clear();
                    table.redraw();
                    log_tab.set_label(&format!("日志 · {}", log.source.relative));
                    status.set_label(&format!(
                        "{} · {} · {} 行{}{} · 缓存 {:.1} MiB",
                        log.source.label(),
                        log.encoding.encoding.name(),
                        index.lines,
                        if index.complete {
                            ""
                        } else {
                            "（索引中）"
                        },
                        if index.errors {
                            " · 存在解码错误，请切换编码"
                        } else {
                            ""
                        },
                        workers.cache.used() as f64 / 1048576.0
                    ));
                }
                Event::Failed(id, failure) if id == read_id => {
                    errors.add(&failure);
                    status.set_label(&failure);
                    view.borrow_mut().log = None;
                    view.borrow_mut().rows.clear();
                    table.set_rows(0);
                }
                _ => {}
            }
        }
        if smoke && started.elapsed() > Duration::from_secs(2) {
            if let Some(destination) = &screenshot {
                crate::ui::capture::window(&mut window, destination)?;
            }
            eprintln!("smoke: sources={}, rows={}", sources.len(), table.rows());
            window.hide();
            break;
        }
    }
    workers.cancel();
    Ok(())
}
