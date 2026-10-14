use super::{controls, theme, viewer::truncate};
use fltk::{
    button::{Button, CheckButton},
    draw,
    enums::{Align, FrameType},
    frame::Frame,
    group::Flex,
    input::Input,
    menu::Choice,
    prelude::*,
    table::{TableContext, TableRow, TableRowSelectMode},
};
use minecraft_logs_viewer::{
    format::StyledLine,
    search::{Hit, Options},
};
use std::{cell::RefCell, rc::Rc};

pub struct SearchUi {
    pub layout: Flex,
    pub query: Input,
    pub scope: Choice,
    pub start: Button,
    pub regex: CheckButton,
    pub case: CheckButton,
    pub raw: CheckButton,
    pub level: Choice,
    pub thread: Input,
    pub logger: Input,
    pub from: Input,
    pub to: Input,
    pub summary: Frame,
    pub results: TableRow,
    pub hits: Rc<RefCell<Vec<Hit>>>,
}

impl SearchUi {
    pub fn build() -> Self {
        let mut layout = Flex::default_fill().column();
        layout.set_margin(12);
        layout.set_pad(12);
        let mut row = Flex::default().row();
        row.set_pad(theme::SPACE);
        let mut query = controls::input();
        query.set_tooltip("文字或逐行正则；支持在隐藏 § 控制字符后匹配");
        let mut scope = controls::choice();
        scope.add_choice("当前日志|选中来源|整个目录");
        scope.set_value(0);
        let start = controls::button("搜索", true);
        row.end();
        row.fixed(&scope, 120);
        row.fixed(&start, 80);
        layout.fixed(&row, 38);
        let mut settings = Flex::default().row();
        settings.set_pad(theme::SPACE);
        let regex = controls::toggle("正则表达式");
        let case = controls::toggle("区分大小写");
        let raw = controls::toggle("搜索原文");
        let summary = controls::label("请输入文字或字段条件", false);
        settings.end();
        settings.fixed(&regex, 130);
        settings.fixed(&case, 125);
        settings.fixed(&raw, 120);
        layout.fixed(&settings, 30);
        let mut filters = Flex::default().row();
        filters.set_pad(theme::SPACE);
        filters.set_margins(0, 22, 0, 0);
        let mut level = controls::choice();
        level.set_label("级别");
        level.set_align(Align::TopLeft);
        level.add_choice("全部|TRACE|DEBUG|INFO|WARN|ERROR|FATAL");
        level.set_value(0);
        let mut thread = controls::input();
        thread.set_label("线程包含");
        thread.set_align(Align::TopLeft);
        let mut logger = controls::input();
        logger.set_label("Logger 包含");
        logger.set_align(Align::TopLeft);
        let mut from = controls::input();
        from.set_label("开始时间");
        from.set_align(Align::TopLeft);
        from.set_tooltip("HH:MM:SS，留空表示不限");
        let mut to = controls::input();
        to.set_label("结束时间");
        to.set_align(Align::TopLeft);
        to.set_tooltip("HH:MM:SS，留空表示不限");
        filters.end();
        filters.fixed(&level, 90);
        filters.fixed(&from, 110);
        filters.fixed(&to, 110);
        layout.fixed(&filters, 56);
        let hits = Rc::new(RefCell::new(Vec::<Hit>::new()));
        let mut results = TableRow::default();
        results.set_frame(FrameType::NoBox);
        // Keep untouched cells visible during selection's partial redraw.
        results.set_table_frame(FrameType::NoBox);
        results.set_color(theme::BACKGROUND);
        results.set_cols(3);
        results.set_col_width(0, 280);
        results.set_col_width(1, 65);
        results.set_col_width(2, 1800);
        results.set_col_header(true);
        results.set_col_header_height(32);
        results.set_row_header(false);
        results.set_row_height_all(theme::ROW_HEIGHT);
        results.set_type(TableRowSelectMode::Single);
        results.end();
        let rows = hits.clone();
        results.draw_cell(move |table, context, row, col, x, y, w, h| {
            if context == TableContext::ColHeader {
                draw::set_draw_color(theme::SURFACE);
                draw::draw_rectf(x, y, w, h);
                draw::set_font(theme::UI_FONT, theme::SMALL);
                draw::set_draw_color(theme::MUTED);
                draw::draw_text2(
                    ["来源", "行", "命中内容"][col as usize],
                    x + 12,
                    y,
                    w - 24,
                    h,
                    Align::Left | Align::Inside,
                );
                draw::set_draw_color(theme::RULE);
                draw::draw_line(x, y + h - 1, x + w, y + h - 1);
                return;
            }
            if context != TableContext::Cell || row < 0 {
                return;
            }
            let rows = rows.borrow();
            let Some(hit) = rows.get(row as usize) else {
                return;
            };
            draw::push_clip(x, y, w, h);
            draw::set_draw_color(if table.row_selected(row) {
                theme::SELECTION
            } else {
                theme::BACKGROUND
            });
            draw::draw_rectf(x, y, w, h);
            draw::set_draw_color(if col == 2 { theme::TEXT } else { theme::MUTED });
            draw::set_font(
                if col == 2 {
                    theme::MONO_FONT
                } else {
                    theme::UI_FONT
                },
                theme::SMALL,
            );
            let text = match col {
                0 => hit.source.label(),
                1 => (hit.line + 1).to_string(),
                _ => StyledLine::parse(&hit.preview).text,
            };
            draw::draw_text2(
                truncate(&text, 1500),
                x + 12,
                y,
                w - 18,
                h,
                Align::Left | Align::Inside,
            );
            draw::pop_clip();
        });
        layout.end();
        Self {
            layout,
            query,
            scope,
            start,
            regex,
            case,
            raw,
            level,
            thread,
            logger,
            from,
            to,
            summary,
            results,
            hits,
        }
    }
    pub fn options(&self) -> Options {
        Options {
            query: self.query.value(),
            regex: self.regex.is_checked(),
            case_sensitive: self.case.is_checked(),
            raw: self.raw.is_checked(),
            level: if self.level.value() > 0 {
                self.level.choice().unwrap_or_default()
            } else {
                String::new()
            },
            thread: self.thread.value(),
            logger: self.logger.value(),
            time_from: self.from.value(),
            time_to: self.to.value(),
            ..Options::default()
        }
    }
    pub fn clear(&mut self) {
        self.hits.borrow_mut().clear();
        self.results.set_rows(0);
        self.summary.set_label("正在搜索…");
        self.summary.set_label_color(theme::WARNING);
    }
    pub fn add(&mut self, hits: Vec<Hit>) {
        self.hits.borrow_mut().extend(hits);
        self.results.set_rows(self.hits.borrow().len() as i32);
        self.results.redraw();
        self.summary
            .set_label(&format!("{} 条命中行", self.hits.borrow().len()));
    }
}
