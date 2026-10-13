use super::{controls, theme};
use fltk::{
    button::{Button, CheckButton},
    enums::FrameType,
    frame::Frame,
    group::{Flex, Group},
    input::Input,
    menu::Choice,
    prelude::*,
    text::{TextBuffer, TextEditor},
};
use minecraft_logs_viewer::{
    document::{Document, FinalNewline, Newline, Output},
    text::ENCODINGS,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub struct EditorUi {
    pub group: Group,
    pub editor: TextEditor,
    pub buffer: TextBuffer,
    pub document: Rc<RefCell<Document>>,
    pub dirty: Rc<Cell<bool>>,
    pub save: Button,
    pub close: Button,
    pub undo: Button,
    pub redo: Button,
    pub find_button: Button,
    pub replace_button: Button,
    pub preview_button: Button,
    pub needle: Input,
    pub replacement: Input,
    pub encoding: Choice,
    pub newline: Choice,
    pub ending: Choice,
    pub bom: CheckButton,
    pub convert: CheckButton,
    pub summary: Frame,
}

impl EditorUi {
    pub fn build(document: Document, number: usize) -> Self {
        let mut group = Group::default().with_label(&format!("副本 {number}"));
        group.set_color(theme::SURFACE);
        group.set_label_color(theme::TEXT);
        group.set_label_font(theme::UI_FONT);
        let mut layout = Flex::default_fill().column();
        layout.set_margin(12);
        layout.set_pad(12);
        let mut heading = Flex::default().row();
        heading.set_pad(theme::SPACE);
        let mut title = controls::label(&document.title, true);
        title.set_color(theme::SURFACE);
        let save = controls::button("另存副本", true);
        let close = controls::button("关闭", false);
        heading.end();
        heading.fixed(&save, 104);
        heading.fixed(&close, 60);
        layout.fixed(&heading, 36);
        let mut settings = Flex::default().row();
        settings.set_pad(theme::SPACE);
        let mut encoding = controls::choice();
        encoding.add_choice(&ENCODINGS[1..].join("|"));
        let selected = ENCODINGS[1..].iter().position(|name| {
            encoding_rs::Encoding::for_label(name.as_bytes()) == Some(document.output.encoding)
        });
        if let Some(selected) = selected {
            encoding.set_value(selected as i32);
        } else {
            encoding.add_choice(document.output.encoding.name());
            encoding.set_value(ENCODINGS.len() as i32 - 1);
        }
        let mut newline = controls::choice();
        newline.add_choice("保持换行|LF|CRLF");
        newline.set_value(0);
        let mut ending = controls::choice();
        ending.add_choice("保持末尾|确保末尾换行|去除末尾换行");
        ending.set_value(0);
        let bom = controls::toggle("BOM");
        bom.set_checked(document.output.bom);
        let convert = controls::toggle("允许转换错误文字");
        settings.end();
        settings.fixed(&encoding, 115);
        settings.fixed(&newline, 115);
        settings.fixed(&ending, 140);
        settings.fixed(&bom, 70);
        layout.fixed(&settings, 34);
        let mut find = Flex::default().row();
        find.set_pad(theme::SPACE);
        let undo = controls::button("撤销", false);
        let redo = controls::button("重做", false);
        let mut needle = controls::input();
        needle.set_tooltip("查找文字，区分大小写");
        let find_button = controls::button("查找", false);
        let mut replacement = controls::input();
        replacement.set_tooltip("替换成");
        let replace_button = controls::button("全部替换", false);
        let preview_button = controls::button("格式预览", false);
        find.end();
        find.fixed(&undo, 52);
        find.fixed(&redo, 52);
        find.fixed(&find_button, 52);
        find.fixed(&replace_button, 92);
        find.fixed(&preview_button, 92);
        layout.fixed(&find, 34);
        let mut editor = TextEditor::default();
        editor.set_frame(FrameType::FlatBox);
        editor.set_color(theme::BACKGROUND);
        editor.set_text_color(theme::TEXT);
        editor.set_text_font(theme::MONO_FONT);
        editor.set_text_size(theme::FONT_SIZE);
        editor.set_selection_color(theme::SELECTION);
        editor.set_cursor_color(theme::FOCUS);
        editor.set_linenumber_width(52);
        editor.set_linenumber_font(theme::MONO_FONT);
        editor.set_linenumber_size(theme::SMALL);
        editor.set_linenumber_fgcolor(theme::MUTED);
        editor.set_linenumber_bgcolor(theme::BACKGROUND);
        let mut buffer = TextBuffer::default();
        buffer.set_text(&document.base);
        buffer.can_undo(true);
        editor.set_buffer(buffer.clone());
        let mut summary = controls::label(
            if document.decode_errors {
                "来源有解码错误，保存前请确认转换"
            } else {
                "独立副本 · § 控制字符在编辑器中保留"
            },
            false,
        );
        summary.set_color(theme::SURFACE);
        layout.fixed(&summary, 24);
        layout.end();
        let mut resize = layout.clone();
        group.resize_callback(move |_, x, y, w, h| resize.resize(x, y, w, h));
        group.end();
        let dirty = Rc::new(Cell::new(false));
        let changed = dirty.clone();
        let mut tab = group.clone();
        buffer.add_modify_callback(move |_, inserted, deleted, _, _| {
            if inserted > 0 || deleted > 0 {
                changed.set(true);
                tab.set_label(&format!("副本 {number} *"));
            }
        });
        Self {
            group,
            editor,
            buffer,
            document: Rc::new(RefCell::new(document)),
            dirty,
            save,
            close,
            undo,
            redo,
            find_button,
            replace_button,
            preview_button,
            needle,
            replacement,
            encoding,
            newline,
            ending,
            bom,
            convert,
            summary,
        }
    }
    pub fn output(&self) -> Output {
        Output {
            encoding: encoding_rs::Encoding::for_label(self.encoding.choice().unwrap().as_bytes())
                .unwrap(),
            bom: self.bom.is_checked(),
            newline: match self.newline.value() {
                1 => Newline::Lf,
                2 => Newline::Crlf,
                _ => Newline::Preserve,
            },
            final_newline: match self.ending.value() {
                1 => FinalNewline::Ensure,
                2 => FinalNewline::Remove,
                _ => FinalNewline::Preserve,
            },
            allow_errors: self.convert.is_checked(),
        }
    }
    pub fn is_modified(&self) -> bool {
        let document = self.document.borrow();
        (self.dirty.get() && self.buffer.text() != document.base)
            || self.output() != document.output
    }
}
