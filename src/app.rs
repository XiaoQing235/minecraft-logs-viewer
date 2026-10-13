use crate::ui::{controls, theme, viewer};
use fltk::{
    app,
    browser::HoldBrowser,
    dialog::{self, FileDialogType, NativeFileChooser},
    enums::{Align, CallbackTrigger, FrameType},
    frame::Frame,
    group::{Flex, Group, Tabs},
    prelude::*,
    tree::{TreeItem, TreeSelect},
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
    let application = app::App::default();
    theme::apply();
    let (commands, actions) = app::channel::<Action>();
    let mut window = Window::new(80, 80, 1280, 820, "Minecraft Logs Viewer");
    window.set_color(theme::BACKGROUND);
    window.size_range(1040, 620, 0, 0);
    let mut layout = Flex::default_fill().column();
    layout.set_margin(20);
    layout.set_pad(12);
    let mut top = Flex::default().row();
    top.set_pad(theme::SPACE);
    let brand = controls::brand();
    let mut directory = controls::input();
    directory.set_readonly(true);
    directory.set_tooltip("当前日志目录");
    let mut open = controls::button("打开文件夹", true);
    controls::shortcut_open(&mut open);
    let mut refresh = controls::button("刷新", false);
    let mut cancel = controls::button("取消", false);
    cancel.deactivate();
    top.end();
    top.fixed(&brand, 218);
    top.fixed(&open, 112);
    top.fixed(&refresh, 60);
    top.fixed(&cancel, 60);
    layout.fixed(&top, 44);
    let divider = controls::header_rule();
    layout.fixed(&divider, 1);
    let mut body = Flex::default().row();
    body.set_pad(16);
    let mut sidebar = Flex::default().column();
    sidebar.set_frame(FrameType::DownBox);
    sidebar.set_color(theme::SURFACE);
    sidebar.set_margin(12);
    sidebar.set_pad(theme::SPACE);
    let mut sidebar_title = controls::label("日志来源", true);
    sidebar_title.set_color(theme::SURFACE);
    sidebar.fixed(&sidebar_title, 28);
    let mut sources_count = controls::label("打开文件夹以发现日志", false);
    sources_count.set_color(theme::SURFACE);
    sidebar.fixed(&sources_count, 22);
    let recursive = controls::toggle("包含子目录");
    sidebar.fixed(&recursive, 30);
    let mut tree = controls::source_tree();
    tree.set_show_root(false);
    tree.set_select_mode(TreeSelect::Multi);
    let mut scope_hint = controls::label("归档根目录及 logs/ 中的日志", false);
    scope_hint.set_color(theme::SURFACE);
    sidebar.fixed(&scope_hint, 24);
    sidebar.end();
    let mut tabs = Tabs::default();
    tabs.set_color(theme::SURFACE);
    tabs.set_selection_color(theme::CONTROL);
    tabs.set_frame(FrameType::FlatBox);
    tabs.set_label_color(theme::TEXT);
    let mut log_tab = Group::default().with_label("日志");
    log_tab.set_color(theme::SURFACE);
    log_tab.set_label_color(theme::TEXT);
    log_tab.set_label_font(theme::UI_FONT);
    log_tab.set_label_size(theme::FONT_SIZE);
    let mut log_layout = Flex::default_fill().column();
    log_layout.set_margin(12);
    log_layout.set_pad(12);
    let mut heading = Flex::default().row();
    let mut file_heading = controls::label("选择一个日志来源", true);
    file_heading.set_color(theme::SURFACE);
    let mut source_metrics = controls::label("", false);
    source_metrics.set_color(theme::SURFACE);
    source_metrics.set_align(Align::Right | Align::Inside);
    heading.end();
    heading.fixed(&source_metrics, 230);
    log_layout.fixed(&heading, 36);
    let mut tools = Flex::default().row();
    tools.set_pad(theme::SPACE);
    let mut encoding_label = controls::label("编码", false);
    encoding_label.set_color(theme::SURFACE);
    let mut encoding = controls::choice();
    encoding.add_choice(&ENCODINGS.join("|"));
    encoding.set_value(0);
    let mut raw = controls::toggle("原文");
    raw.set_tooltip("显示包含 § 控制字符的原始文本");
    let mut obfuscate = controls::toggle("混淆");
    obfuscate.set_tooltip("预览 §k 效果，复制保留真实字符");
    let spacer = Frame::default();
    let mut jump_label = controls::label("行号", false);
    jump_label.set_color(theme::SURFACE);
    let mut jump = controls::number();
    jump.set_value("1");
    jump.set_tooltip("跳转行号");
    let mut go = controls::button("跳转", false);
    let mut copy = controls::button("复制选中行", false);
    copy.deactivate();
    go.deactivate();
    tools.end();
    tools.fixed(&encoding_label, 32);
    tools.fixed(&encoding, 100);
    tools.fixed(&raw, 60);
    tools.fixed(&obfuscate, 60);
    tools.fixed(&jump_label, 32);
    tools.fixed(&jump, 72);
    tools.fixed(&go, 52);
    tools.fixed(&copy, 104);
    let _ = spacer;
    log_layout.fixed(&tools, 34);
    let (mut table, view) = viewer::build();
    log_layout.end();
    let mut log_resize = log_layout.clone();
    log_tab.resize_callback(move |_, x, y, w, h| {
        log_resize.resize(x, y, w, h);
    });
    log_tab.end();
    let mut errors_tab = Group::default().with_label("读取信息");
    errors_tab.set_color(theme::SURFACE);
    errors_tab.set_label_color(theme::TEXT);
    errors_tab.set_label_font(theme::UI_FONT);
    let mut error_layout = Flex::default_fill().column();
    error_layout.set_margin(5);
    let mut errors = HoldBrowser::default();
    errors.set_frame(FrameType::FlatBox);
    errors.set_color(theme::SURFACE);
    errors.set_text_size(theme::FONT_SIZE);
    error_layout.end();
    let mut error_resize = error_layout.clone();
    errors_tab.resize_callback(move |_, x, y, w, h| {
        error_resize.resize(x, y, w, h);
    });
    errors_tab.end();
    tabs.end();
    tabs.auto_layout();
    body.end();
    body.fixed(&sidebar, 270);
    let mut status = controls::label("就绪  ·  打开文件夹后选择日志", false);
    layout.end();
    layout.fixed(&status, 24);
    window.end();
    window.make_resizable(true);
    layout.recalc();
    tabs.auto_layout();
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
    let mut source_items = Vec::<Option<TreeItem>>::new();
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
        } else if argument == "--window-size" {
            if let Some(size) = arguments
                .next()
                .and_then(|value| value.to_str().map(str::to_owned))
                && let Some((width, height)) = size.split_once('x')
                && let (Ok(width), Ok(height)) = (width.parse::<i32>(), height.parse::<i32>())
                && width >= 1040
                && height >= 620
            {
                window.resize(window.x(), window.y(), width, height);
                layout.recalc();
                tabs.auto_layout();
            }
        } else if argument != "--smoke" {
            initial = Some(PathBuf::from(argument));
        }
    }
    if let Some(path) = initial {
        directory.set_value(&path.to_string_lossy());
        root = Some(path.clone());
        scan_id = workers.scan(path, recursive.is_checked());
        cancel.activate();
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
                        source_items.clear();
                        tree.clear();
                        errors.clear();
                        active = None;
                        read_id = 0;
                        view.borrow_mut().log = None;
                        view.borrow_mut().rows.clear();
                        table.set_rows(0);
                        copy.deactivate();
                        go.deactivate();
                        file_heading.set_label("选择一个日志来源");
                        source_metrics.set_label("");
                        sources_count.set_label("正在发现日志…");
                        directory.set_value(&path.to_string_lossy());
                        root = Some(path.clone());
                        scan_id = workers.scan(path, recursive.is_checked());
                        status.set_label("扫描中…");
                        status.set_label_color(theme::WARNING);
                        cancel.activate();
                    }
                }
                Action::Selected | Action::Encoding => {
                    let selected = if matches!(action, Action::Encoding) {
                        active
                    } else {
                        tree.callback_item().and_then(|item| {
                            source_items
                                .iter()
                                .position(|candidate| candidate.as_ref() == Some(&item))
                        })
                    };
                    if let Some(index) = selected.filter(|index| *index < sources.len()) {
                        active = Some(index);
                        view.borrow_mut().log = None;
                        view.borrow_mut().rows.clear();
                        table.set_rows(0);
                        copy.deactivate();
                        go.deactivate();
                        source_metrics.set_label("读取中…");
                        let encoding_name = (encoding.value() > 0)
                            .then(|| ENCODINGS[encoding.value() as usize].to_owned());
                        read_id = workers.open(sources[index].clone(), encoding_name);
                        status.set_label("读取与建立索引…");
                        status.set_label_color(theme::WARNING);
                        cancel.activate();
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
                    status.set_label_color(theme::WARNING);
                    cancel.deactivate();
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
                    let display = source
                        .member
                        .as_ref()
                        .map(|m| format!("{} [#{}]", m.name.replace('/', "∕"), m.ordinal + 1))
                        .unwrap_or_else(|| source.relative.replace(['/', '\\'], "∕"));
                    let leaf = format!("[{index}] {display}");
                    let branch = if source.member.is_some() {
                        format!("归档/{}", source.relative.replace(['/', '\\'], "∕"))
                    } else {
                        "普通日志".into()
                    };
                    let mut item = tree.add(&format!("{branch}/{leaf}"));
                    if let Some(item) = &mut item {
                        item.set_label(&display);
                    }
                    if let Some(mut parent) = tree.find_item(&branch) {
                        parent.set_label_fgcolor(theme::MUTED);
                        parent.set_label_size(theme::SMALL);
                    }
                    source_items.push(item);
                    sources.push(source);
                    sources_count.set_label(&format!("{} 个日志来源", sources.len()));
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
                        if let Some(Some(item)) = source_items.first_mut() {
                            item.select(1);
                            tree.redraw();
                        }
                        read_id = workers.open(sources[0].clone(), None);
                    } else if sources.is_empty() {
                        cancel.deactivate();
                    }
                }
                Event::Loaded(id, log) if id == read_id => {
                    let index = log.index.read().unwrap().clone();
                    table.set_rows(index.lines.min(i32::MAX as usize) as i32);
                    view.borrow_mut().log = Some(log.clone());
                    view.borrow_mut().rows.clear();
                    table.redraw();
                    let title = log
                        .source
                        .member
                        .as_ref()
                        .map(|m| m.name.clone())
                        .unwrap_or_else(|| {
                            log.source
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned()
                        });
                    file_heading.set_label(&title);
                    file_heading.set_tooltip(&log.source.label());
                    source_metrics.set_label(&format!(
                        "{}  ·  {} 行  ·  只读",
                        log.encoding.encoding.name(),
                        index.lines
                    ));
                    source_metrics.set_label_color(if index.errors {
                        theme::ERROR
                    } else {
                        theme::MUTED
                    });
                    copy.activate();
                    go.activate();
                    if index.complete {
                        cancel.deactivate();
                    }
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
                    status.set_label_color(if index.errors {
                        theme::WARNING
                    } else if index.complete {
                        theme::SUCCESS
                    } else {
                        theme::MUTED
                    });
                }
                Event::Failed(id, failure) if id == read_id => {
                    errors.add(&failure);
                    status.set_label(&failure);
                    status.set_label_color(theme::ERROR);
                    source_metrics.set_label("读取失败");
                    source_metrics.set_label_color(theme::ERROR);
                    copy.deactivate();
                    go.deactivate();
                    cancel.deactivate();
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
