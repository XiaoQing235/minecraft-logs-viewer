use crate::ui::{controls, editor::EditorUi, search::SearchUi, theme, viewer};
use fltk::{
    app,
    browser::HoldBrowser,
    dialog::{self, FileDialogOptions, FileDialogType, NativeFileChooser},
    enums::{Align, CallbackTrigger, FrameType},
    frame::Frame,
    group::{Flex, Group, Tabs},
    prelude::*,
    tree::{TreeItem, TreeSelect},
    window::Window,
};
use minecraft_logs_viewer::{
    document::Document,
    save,
    search::Completion,
    source::LogSource,
    text::ENCODINGS,
    worker::{Event, SaveRequest, Workers},
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
    Search,
    Result,
    CreateCopy(bool),
    Save(usize),
    CloseEditor(usize),
    Undo(usize),
    Redo(usize),
    Find(usize),
    Replace(usize),
    Preview(usize),
    CloseWindow,
}

fn create_editor(document: Document, index: usize, commands: app::Sender<Action>) -> EditorUi {
    let mut editor = EditorUi::build(document, index + 1);
    editor
        .save
        .set_callback(move |_| commands.send(Action::Save(index)));
    editor
        .close
        .set_callback(move |_| commands.send(Action::CloseEditor(index)));
    editor
        .undo
        .set_callback(move |_| commands.send(Action::Undo(index)));
    editor
        .redo
        .set_callback(move |_| commands.send(Action::Redo(index)));
    editor
        .find_button
        .set_callback(move |_| commands.send(Action::Find(index)));
    editor
        .replace_button
        .set_callback(move |_| commands.send(Action::Replace(index)));
    editor
        .preview_button
        .set_callback(move |_| commands.send(Action::Preview(index)));
    editor
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
    let mut edit_full = controls::button("编辑副本", false);
    let mut edit_fragment = controls::button("片段副本", false);
    heading.end();
    heading.fixed(&source_metrics, 230);
    heading.fixed(&edit_full, 104);
    heading.fixed(&edit_fragment, 104);
    heading.set_pad(theme::SPACE);
    edit_full.deactivate();
    edit_fragment.deactivate();
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
    let mut search_tab = Group::default().with_label("搜索");
    search_tab.set_color(theme::SURFACE);
    search_tab.set_label_color(theme::TEXT);
    search_tab.set_label_font(theme::UI_FONT);
    let mut search_ui = SearchUi::build();
    let mut search_resize = search_ui.layout.clone();
    search_tab.resize_callback(move |_, x, y, w, h| search_resize.resize(x, y, w, h));
    search_tab.end();
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
    edit_full.set_callback(move |_| commands.send(Action::CreateCopy(true)));
    edit_fragment.set_callback(move |_| commands.send(Action::CreateCopy(false)));
    window.set_callback(move |_| commands.send(Action::CloseWindow));
    search_ui
        .start
        .set_callback(move |_| commands.send(Action::Search));
    search_ui.query.set_trigger(CallbackTrigger::EnterKey);
    search_ui
        .query
        .set_callback(move |_| commands.send(Action::Search));
    search_ui
        .results
        .set_callback(move |_| commands.send(Action::Result));
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
    let mut search_id = 0;
    let mut scan_pending = false;
    let mut scan_failures = Vec::<String>::new();
    let mut search_scan_failures = Vec::<String>::new();
    let mut encoding_overrides = std::collections::BTreeMap::<String, String>::new();
    let mut pending_jump = None::<(usize, std::ops::Range<usize>)>;
    let mut editors = Vec::<Option<EditorUi>>::new();
    let mut preview_input = None::<std::sync::Arc<minecraft_logs_viewer::cache::CacheFile>>;
    let mut protected_sources =
        Vec::<(PathBuf, minecraft_logs_viewer::platform::FileIdentity)>::new();
    let mut saving = None::<(u64, usize)>;
    let mut close_after_save = None::<usize>;
    let mut close_window_after_save = false;
    let smoke = std::env::args().any(|arg| arg == "--smoke");
    let mut arguments = std::env::args_os().skip(1);
    let mut initial = None;
    let mut screenshot = None;
    let mut smoke_query = None;
    let mut smoke_copy = false;
    let mut smoke_save = None;
    let mut smoke_started = false;
    let mut smoke_error = None::<String>;
    while let Some(argument) = arguments.next() {
        if argument == "--screenshot" {
            screenshot = arguments.next().map(PathBuf::from);
        } else if argument == "--smoke-query" {
            smoke_query = arguments
                .next()
                .map(|value| value.to_string_lossy().into_owned());
        } else if argument == "--smoke-copy" {
            smoke_copy = true;
        } else if argument == "--smoke-save" {
            smoke_save = arguments.next().map(PathBuf::from);
            smoke_copy = true;
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
        scan_pending = true;
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
                        preview_input = None;
                        encoding.activate();
                        scan_pending = true;
                        scan_failures.clear();
                        encoding_overrides.clear();
                        pending_jump = None;
                        search_id = 0;
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
                        preview_input = None;
                        encoding.activate();
                        if matches!(action, Action::Encoding) {
                            let name = (encoding.value() > 0)
                                .then(|| ENCODINGS[encoding.value() as usize].to_owned());
                            if let Some(name) = name {
                                encoding_overrides.insert(sources[index].id(), name);
                            } else {
                                encoding_overrides.remove(&sources[index].id());
                            }
                        } else {
                            let value = encoding_overrides
                                .get(&sources[index].id())
                                .and_then(|name| {
                                    ENCODINGS.iter().position(|candidate| *candidate == name)
                                })
                                .unwrap_or(0);
                            encoding.set_value(value as i32);
                            pending_jump = None;
                        }
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
                    if search_id != 0 {
                        search_ui.summary.set_label("已取消 · 当前结果不完整");
                        search_ui.summary.set_label_color(theme::WARNING);
                        search_id = 0;
                    }
                    if scan_pending {
                        scan_failures.push("目录扫描被取消，来源清单不完整".into());
                    }
                    scan_pending = false;
                    scan_id = 0;
                    read_id = 0;
                }
                Action::Search => {
                    let mut options = search_ui.options();
                    options.encoding_overrides = encoding_overrides.clone();
                    if let Err(error) = options.compile() {
                        search_ui.summary.set_label(&error.to_string());
                        search_ui.summary.set_label_color(theme::ERROR);
                        continue;
                    }
                    if search_ui.scope.value() == 2 && scan_pending {
                        search_ui.summary.set_label("目录仍在扫描，请等待扫描完成");
                        continue;
                    }
                    let targets = match search_ui.scope.value() {
                        0 => view
                            .borrow()
                            .log
                            .as_ref()
                            .map(|log| vec![log.source.clone()])
                            .or_else(|| active.map(|index| vec![sources[index].clone()]))
                            .unwrap_or_default(),
                        1 => sources
                            .iter()
                            .zip(&source_items)
                            .filter(|(_, item)| {
                                item.as_ref().is_some_and(|item| item.is_selected())
                            })
                            .map(|(source, _)| source.clone())
                            .collect(),
                        _ => sources.clone(),
                    };
                    if targets.is_empty() {
                        search_ui.summary.set_label("没有可搜索的日志，请选择来源");
                        continue;
                    }
                    search_scan_failures = if search_ui.scope.value() == 2 {
                        scan_failures.clone()
                    } else {
                        Vec::new()
                    };
                    search_ui.clear();
                    search_id = workers.search(targets, options);
                    cancel.activate();
                    let _ = tabs.set_value(&search_tab);
                }
                Action::Result => {
                    let row = search_ui.results.callback_row();
                    let hit = if row >= 0 {
                        search_ui.hits.borrow().get(row as usize).cloned()
                    } else {
                        None
                    };
                    if let Some(hit) = hit {
                        if let Err(error) = hit.source.check_revision() {
                            dialog::alert_default(&error.to_string());
                            continue;
                        }
                        active = sources
                            .iter()
                            .position(|source| source.id() == hit.source.id());
                        pending_jump =
                            Some((hit.line, hit.ranges.first().cloned().unwrap_or(0..0)));
                        let override_name = encoding_overrides.get(&hit.source.id()).cloned();
                        read_id = workers.open(hit.source, override_name);
                        view.borrow_mut().log = None;
                        view.borrow_mut().rows.clear();
                        table.set_rows(0);
                        let _ = tabs.set_value(&log_tab);
                        cancel.activate();
                    }
                }
                Action::CreateCopy(full) => {
                    let log = view.borrow().log.clone();
                    let Some(log) = log else {
                        continue;
                    };
                    let result = if full {
                        log.full_text()
                    } else {
                        let (first, _, last, _) = table.get_selection();
                        let default = if first >= 0 {
                            format!("{}-{}", first + 1, last + 1)
                        } else {
                            format!(
                                "{}-{}",
                                table.row_position() + 1,
                                (table.row_position() + 5).min(table.rows())
                            )
                        };
                        let Some(range) =
                            dialog::input_default("创建片段副本的行号范围（例如 12-20）", &default)
                        else {
                            continue;
                        };
                        match range.split_once('-').and_then(|(from, to)| {
                            Some((
                                from.trim().parse::<usize>().ok()?,
                                to.trim().parse::<usize>().ok()?,
                            ))
                        }) {
                            Some((from, to)) if from > 0 && to >= from && to <= log.rows() => {
                                log.copy_range(from - 1, to - from + 1)
                            }
                            _ => Err(std::io::Error::other("行号范围无效或尚未读取")),
                        }
                    };
                    match result.and_then(|text| Document::from_log(&log, text)) {
                        Ok(mut document) => {
                            document.protected.extend(protected_sources.clone());
                            let index = editors.len();
                            tabs.begin();
                            let editor = create_editor(document, index, commands);
                            tabs.end();
                            let group = editor.group.clone();
                            editors.push(Some(editor));
                            tabs.auto_layout();
                            let _ = tabs.set_value(&group);
                            if smoke && smoke_save.is_some() {
                                commands.send(Action::Save(index));
                            }
                        }
                        Err(error) => dialog::alert_default(&error.to_string()),
                    }
                }
                Action::Save(index) => {
                    if saving.is_some() {
                        status.set_label("已有保存任务进行中，请等待完成");
                        continue;
                    }
                    let Some(editor) = editors.get_mut(index).and_then(Option::as_mut) else {
                        continue;
                    };
                    let target = if smoke && smoke_save.is_some() {
                        smoke_save.clone().unwrap()
                    } else {
                        let mut chooser = NativeFileChooser::new(FileDialogType::BrowseSaveFile);
                        chooser.set_title("另存日志副本");
                        chooser.set_option(FileDialogOptions::SaveAsConfirm);
                        chooser.set_preset_file("log-copy.log");
                        chooser.show();
                        chooser.filename()
                    };
                    if target.as_os_str().is_empty() {
                        close_after_save = None;
                        close_window_after_save = false;
                        continue;
                    }
                    let document = editor.document.borrow().clone();
                    let expected = if document.target.as_ref() == Some(&target) {
                        Ok(document.saved_revision.clone())
                    } else {
                        save::target_revision(&target)
                    };
                    match expected {
                        Ok(expected) => {
                            let request = SaveRequest {
                                editor: index,
                                document,
                                text: editor.buffer.text(),
                                output: editor.output(),
                                target,
                                expected,
                            };
                            let id = workers.save(request);
                            saving = Some((id, index));
                            editor.summary.set_label("保存中…");
                            editor.summary.set_label_color(theme::WARNING);
                            editor.save.deactivate();
                        }
                        Err(error) => dialog::alert_default(&error.to_string()),
                    }
                }
                Action::CloseEditor(index) => {
                    if saving.is_some_and(|(_, saved)| saved == index) {
                        status.set_label("副本正在保存，请等待完成");
                        continue;
                    }
                    let Some(editor) = editors.get(index).and_then(Option::as_ref) else {
                        continue;
                    };
                    if editor.is_modified() {
                        match dialog::choice2_default(
                            "副本有未保存的修改",
                            "取消",
                            "不保存关闭",
                            "保存副本",
                        ) {
                            Some(1) => {}
                            Some(2) => {
                                close_after_save = Some(index);
                                commands.send(Action::Save(index));
                                continue;
                            }
                            _ => continue,
                        }
                    }
                    if let Some(editor) = editors[index].take() {
                        tabs.remove(&editor.group);
                        Group::delete(editor.group);
                        tabs.auto_layout();
                        let _ = tabs.set_value(&log_tab);
                    }
                }
                Action::Undo(index)
                | Action::Redo(index)
                | Action::Find(index)
                | Action::Replace(index)
                | Action::Preview(index) => {
                    let Some(editor) = editors.get_mut(index).and_then(Option::as_mut) else {
                        continue;
                    };
                    match action {
                        Action::Undo(_) => {
                            let _ = editor.buffer.undo();
                        }
                        Action::Redo(_) => {
                            let _ = editor.buffer.redo();
                        }
                        Action::Find(_) => {
                            let needle = editor.needle.value();
                            if needle.is_empty() {
                                continue;
                            }
                            let text = editor.buffer.text();
                            let start = editor.editor.insert_position().max(0) as usize;
                            let found = text
                                .get(start..)
                                .and_then(|tail| tail.find(&needle).map(|offset| start + offset))
                                .or_else(|| text.find(&needle));
                            if let Some(start) = found {
                                editor
                                    .buffer
                                    .select(start as i32, (start + needle.len()) as i32);
                                editor
                                    .editor
                                    .set_insert_position((start + needle.len()) as i32);
                                editor.editor.show_insert_position();
                            } else {
                                editor.summary.set_label("未找到文字");
                            }
                        }
                        Action::Replace(_) => {
                            let needle = editor.needle.value();
                            if needle.is_empty() {
                                continue;
                            }
                            let text = editor
                                .buffer
                                .text()
                                .replace(&needle, &editor.replacement.value());
                            if text.len() as u64 > minecraft_logs_viewer::text::EDIT_LIMIT {
                                editor.summary.set_label("替换后的副本超过 20 MiB，未执行");
                            } else {
                                editor.buffer.replace(0, editor.buffer.length(), &text);
                            }
                        }
                        Action::Preview(_) => {
                            let (owner, file) = workers.cache.file()?;
                            use std::io::Write;
                            let cancel = std::sync::atomic::AtomicBool::new(false);
                            let mut writer = minecraft_logs_viewer::cache::LimitedWriter {
                                file,
                                owner: owner.clone(),
                                cancel: &cancel,
                            };
                            writer.write_all(editor.buffer.text().as_bytes())?;
                            writer.flush()?;
                            drop(writer);
                            let path = owner.path().to_owned();
                            let source = LogSource {
                                relative: "副本格式化预览".into(),
                                revision: minecraft_logs_viewer::source::Revision::read(&path)?,
                                path,
                                member: None,
                            };
                            read_id = workers.open(source, Some("UTF-8".into()));
                            // The preview remains a readable snapshot while displayed.
                            preview_input = Some(owner);
                            active = None;
                            encoding.set_value(1);
                            encoding.deactivate();
                            pending_jump = None;
                            let _ = tabs.set_value(&log_tab);
                        }
                        _ => {}
                    }
                }
                Action::CloseWindow => {
                    if saving.is_some() {
                        status.set_label("副本正在保存，请等待完成后退出");
                        continue;
                    }
                    if let Some(index) = editors
                        .iter()
                        .position(|editor| editor.as_ref().is_some_and(EditorUi::is_modified))
                    {
                        match dialog::choice2_default(
                            "存在未保存的编辑副本",
                            "取消",
                            "不保存退出",
                            "保存副本",
                        ) {
                            Some(1) => window.hide(),
                            Some(2) => {
                                close_window_after_save = true;
                                commands.send(Action::Save(index));
                            }
                            _ => {}
                        }
                    } else {
                        window.hide();
                    }
                }
            }
        }
        for _ in 0..128 {
            let Ok(event) = workers.events.try_recv() else {
                break;
            };
            match event {
                Event::Source(id, source) if id == scan_id => {
                    let protection = (
                        std::path::absolute(&source.path)?,
                        source.revision.identity.clone(),
                    );
                    if !protected_sources.contains(&protection) {
                        protected_sources.push(protection);
                    }
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
                    scan_pending = false;
                    scan_failures = failures.clone();
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
                    if smoke && let Some(query) = &smoke_query {
                        search_ui.query.set_value(query);
                        search_ui.scope.set_value(2);
                        commands.send(Action::Search);
                        smoke_started = true;
                    }
                }
                Event::Loaded(id, log) if id == read_id => {
                    let index = log.index.read().unwrap().clone();
                    table.set_rows(index.lines.min(i32::MAX as usize) as i32);
                    view.borrow_mut().log = Some(log.clone());
                    view.borrow_mut().rows.clear();
                    table.redraw();
                    if let Some((line, range)) = &pending_jump
                        && *line < index.lines
                    {
                        table.set_row_position(*line as i32);
                        view.borrow_mut().focus = Some((*line, range.clone()));
                        pending_jump = None;
                    }
                    let title = if preview_input
                        .as_ref()
                        .is_some_and(|owner| owner.path() == log.source.path)
                    {
                        log.source.relative.clone()
                    } else {
                        log.source
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
                            })
                    };
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
                    edit_full.activate();
                    edit_fragment.activate();
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
                    if smoke && smoke_copy && !smoke_started && index.complete {
                        smoke_started = true;
                        commands.send(Action::CreateCopy(true));
                    }
                }
                Event::Failed(id, failure) if id == read_id => {
                    if smoke {
                        smoke_error = Some(failure.clone());
                    }
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
                Event::Hits(id, hits) if id == search_id => search_ui.add(hits),
                Event::Searched(id, mut report) if id == search_id => {
                    report.failures.extend(search_scan_failures.clone());
                    if report.completion == Completion::Complete && !report.failures.is_empty() {
                        report.completion = Completion::Partial;
                    }
                    let state = match report.completion {
                        Completion::Complete => "完成",
                        Completion::Partial => "部分失败",
                        Completion::Truncated => "已截断（上限 10,000 条）",
                        Completion::Cancelled => "已取消",
                    };
                    search_ui.summary.set_label(&format!(
                        "{state} · {} 条命中行 · {} 个来源 · {} 个失败项",
                        report.hit_lines,
                        report.scanned,
                        report.failures.len()
                    ));
                    search_ui.summary.set_label_color(
                        if report.completion == Completion::Complete {
                            theme::SUCCESS
                        } else {
                            theme::WARNING
                        },
                    );
                    for failure in report.failures {
                        errors.add(&failure);
                    }
                    search_id = 0;
                    cancel.deactivate();
                }
                Event::Failed(id, failure) if id == search_id => {
                    if smoke {
                        smoke_error = Some(failure.clone());
                    }
                    search_ui.summary.set_label(&failure);
                    search_ui.summary.set_label_color(theme::ERROR);
                    search_id = 0;
                    cancel.deactivate();
                }
                Event::Saved(id, index, target, stamp, text, original, output)
                    if saving == Some((id, index)) =>
                {
                    saving = None;
                    if let Some(editor) = editors.get_mut(index).and_then(Option::as_mut) {
                        let current = editor.buffer.text();
                        let mut document = editor.document.borrow_mut();
                        document.base = text;
                        document.original = original;
                        document.target = Some(target);
                        document.saved_revision = Some(stamp);
                        document.output = output;
                        document.decode_errors = false;
                        let modified = current != document.base;
                        drop(document);
                        editor.dirty.set(modified);
                        editor.group.set_label(&format!(
                            "副本 {}{}",
                            index + 1,
                            if modified { " *" } else { "" }
                        ));
                        editor.summary.set_label(if modified {
                            "已保存；保存期间产生的新修改尚未保存"
                        } else {
                            "已保存副本，原始来源保留"
                        });
                        editor.summary.set_label_color(theme::SUCCESS);
                        editor.save.activate();
                        if close_after_save == Some(index) {
                            close_after_save = None;
                            if !modified {
                                commands.send(Action::CloseEditor(index));
                            }
                        }
                        if close_window_after_save {
                            close_window_after_save = false;
                            commands.send(Action::CloseWindow);
                        }
                    }
                }
                Event::Failed(id, failure) if saving.is_some_and(|(saved, _)| saved == id) => {
                    if smoke {
                        smoke_error = Some(failure.clone());
                    }
                    let (_, index) = saving.take().unwrap();
                    if let Some(editor) = editors.get_mut(index).and_then(Option::as_mut) {
                        editor.summary.set_label(&failure);
                        editor.summary.set_label_color(theme::ERROR);
                        editor.save.activate();
                    }
                    close_after_save = None;
                    close_window_after_save = false;
                }
                _ => {}
            }
        }
        if smoke && started.elapsed() > Duration::from_secs(3) {
            if let Some(destination) = &screenshot {
                crate::ui::capture::window(&mut window, destination)?;
            }
            eprintln!(
                "smoke: sources={}, rows={}, hits={}, copies={}",
                sources.len(),
                table.rows(),
                search_ui.hits.borrow().len(),
                editors.iter().flatten().count()
            );
            window.hide();
            if let Some(error) = smoke_error {
                return Err(std::io::Error::other(error));
            }
            if saving.is_some() || search_id != 0 {
                return Err(std::io::Error::other("启动检查的后台任务未完成"));
            }
            break;
        }
    }
    workers.cancel();
    Ok(())
}
