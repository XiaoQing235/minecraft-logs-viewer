use super::theme;
use fltk::{
    draw,
    enums::{Align, Color, Font, FrameType},
    prelude::*,
    table::{TableContext, TableRow, TableRowSelectMode},
};
use minecraft_logs_viewer::{
    browse::{Anchor, Window},
    format::StyledLine,
    text::{IndexedLog, Line, parse_record},
};
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Default)]
pub struct View {
    pub log: Option<Arc<IndexedLog>>,
    pub rows: Vec<Line>,
    pub raw: bool,
    pub obfuscate: bool,
    pub font_size: i32,
    pub focus: Option<(usize, std::ops::Range<usize>)>,
    pub window: Window,
    row_source: Option<usize>,
}

impl View {
    pub fn clear(&mut self) {
        self.log = None;
        self.rows.clear();
        self.row_source = None;
        self.window.files.clear();
    }
    pub fn line_at(&self, row: i32) -> Option<(Arc<IndexedLog>, usize)> {
        let anchor = self.window.locate(row.max(0) as usize)?;
        Some((self.window.file(anchor.source)?.log.clone(), anchor.line?))
    }
    pub fn current_line(&self, row: i32) -> usize {
        self.window
            .locate(row.max(0) as usize)
            .and_then(|anchor| anchor.line)
            .unwrap_or(0)
    }
}

pub fn page_rows(table: &TableRow) -> usize {
    let height = if table.rows() > 0 {
        table.row_height(0)
    } else {
        theme::ROW_HEIGHT
    };
    ((table.h() - table.col_header_height() - 14) / height.max(1)).max(1) as usize
}

pub fn extent(table: &mut TableRow, view: &View) {
    // Trailing viewport space lets the final real row reach the top, including
    // short/empty files. It also keeps the scrollbar visible at every file size.
    let rows = view.window.rows();
    let extent = if rows == 0 {
        0
    } else {
        rows.saturating_add(page_rows(table)).min(i32::MAX as usize) as i32
    };
    if table.rows() != extent {
        table.set_rows(extent);
    }
    global_range(table);
}

pub fn global_range(table: &mut TableRow) {
    if table.rows() > 0 {
        let height = table.row_height(0).max(1);
        let viewport = table.h() - table.col_header_height() - 14;
        table.scrollbar().set_bounds(
            0.0,
            (table.rows() as f64 * height as f64 - viewport as f64).max(0.0),
        );
    }
}

pub fn position(table: &mut TableRow, row: usize, offset: f64) {
    if table.rows() == 0 {
        return;
    }
    global_range(table);
    let mut scrollbar = table.scrollbar();
    let pixel = row as f64 * table.row_height(0).max(1) as f64 + offset;
    scrollbar.set_value(pixel.clamp(0.0, scrollbar.maximum()));
    // Updating rows can clamp the native scrollbar into the same row while
    // retaining a different pixel offset. Its callback updates both positions.
    scrollbar.do_callback();
}

pub fn file_range(table: &mut TableRow, view: &View, source: usize) {
    if let Some(file) = view.window.file(source)
        && let Some(start) = view.window.position(Anchor { source, line: None })
    {
        let height = table.row_height(0).max(1) as f64;
        let mut scrollbar = table.scrollbar();
        scrollbar.set_bounds(start as f64 * height, (start + file.lines) as f64 * height);
        scrollbar
            .set_slider_size((page_rows(table) as f64 / file.lines.max(1) as f64).min(1.0) as f32);
        scrollbar.set_tooltip("当前文件范围 · 拖动定位；鼠标滚轮可跨文件浏览");
    }
}

pub fn anchor(table: &TableRow, view: &View) -> Option<Anchor> {
    view.window.locate(table.row_position().max(0) as usize)
}

pub fn build() -> (TableRow, Rc<RefCell<View>>) {
    let view = Rc::new(RefCell::new(View {
        font_size: 14,
        ..View::default()
    }));
    let mut table = TableRow::default();
    table.set_color(theme::BACKGROUND);
    // Fl_Table draws its outer frame after its child scrollbars. A filled box
    // paints over them; use a frame that does not fill the widget.
    table.set_frame(FrameType::NoBox);
    table.set_table_frame(FrameType::FlatBox);
    table.set_cols(4);
    table.set_col_width(0, 55);
    table.set_col_width(1, 94);
    table.set_col_width(2, 78);
    table.set_col_width(3, 2000);
    table.set_row_height_all(theme::ROW_HEIGHT);
    table.set_col_header(true);
    table.set_col_header_height(32);
    table.set_row_header(false);
    table.set_type(TableRowSelectMode::Multi);
    table.super_handle_first(false);
    table.set_scrollbar_size(14);
    let mut scrollbar = table.scrollbar();
    scrollbar.set_color(theme::CONTROL);
    scrollbar.set_selection_color(theme::MUTED);
    scrollbar.set_frame(FrameType::FlatBox);
    scrollbar.set_slider_frame(FrameType::FlatBox);
    scrollbar.set_tooltip("连续日志窗口 · 拖动定位，滚轮跨文件浏览");
    let mut horizontal = table.hscrollbar();
    horizontal.set_color(theme::CONTROL);
    horizontal.set_selection_color(theme::MUTED);
    horizontal.set_frame(FrameType::FlatBox);
    horizontal.set_slider_frame(FrameType::FlatBox);
    table.end();
    let state = view.clone();
    table.draw_cell(move |table, context, row, col, x, y, w, h| {
        if context == TableContext::EndPage && state.borrow().log.is_none() {
            let center_x = table.x() + table.w() / 2;
            let center_y = table.y() + table.h() / 2 - 36;
            draw::set_draw_color(theme::RULE);
            draw::draw_rounded_rect(center_x - 18, center_y - 60, 36, 44, 5);
            draw::draw_line(center_x - 9, center_y - 45, center_x + 9, center_y - 45);
            draw::draw_line(center_x - 9, center_y - 37, center_x + 9, center_y - 37);
            draw::set_font(theme::HEADING_FONT, 18);
            draw::set_draw_color(theme::TEXT);
            draw::draw_text2(
                "从左侧选择日志",
                table.x(),
                center_y,
                table.w(),
                28,
                Align::Center,
            );
            draw::set_font(theme::UI_FONT, theme::FONT_SIZE);
            draw::set_draw_color(theme::MUTED);
            draw::draw_text2(
                "普通日志与归档中的日志都可以在这里浏览",
                table.x(),
                center_y + 34,
                table.w(),
                24,
                Align::Center,
            );
            return;
        }
        if context == TableContext::ColHeader {
            draw::set_draw_color(theme::SURFACE);
            draw::draw_rectf(x, y, w, h);
            draw::set_font(theme::UI_FONT, theme::SMALL);
            draw::set_draw_color(theme::MUTED);
            draw::draw_text2(
                ["行", "时间", "级别", "日志内容"][col as usize],
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
        let mut state = state.borrow_mut();
        let Some(anchor) = state.window.locate(row as usize) else {
            draw::set_draw_color(theme::BACKGROUND);
            draw::draw_rectf(x, y, w, h);
            return;
        };
        let file = state.window.file(anchor.source).unwrap();
        let failed = file.failed;
        let log = file.log.clone();
        let Some(number) = anchor.line else {
            draw::push_clip(x, y, w, h);
            draw::set_draw_color(theme::SURFACE);
            draw::draw_rectf(x, y, w, h);
            draw::set_draw_color(theme::RULE);
            draw::draw_line(x, y + h - 1, x + w, y + h - 1);
            if col == 3 {
                draw::set_font(theme::UI_FONT, theme::SMALL);
                draw::set_draw_color(if failed { theme::ERROR } else { theme::MUTED });
                draw::draw_text2(
                    &format!(
                        "{}{}",
                        log.source.label(),
                        if failed { " · 读取未完成" } else { "" }
                    ),
                    x + 12,
                    y,
                    w - 24,
                    h,
                    Align::Left | Align::Inside,
                );
            }
            draw::pop_clip();
            return;
        };
        if state.row_source != Some(anchor.source)
            || !state.rows.iter().any(|line| line.number == number)
        {
            state.rows = log.read_range(number, 128).unwrap_or_default();
            state.row_source = Some(anchor.source);
        }
        let Some(line) = state.rows.iter().find(|line| line.number == number) else {
            return;
        };
        draw::push_clip(x, y, w, h);
        let selected = table.row_selected(row);
        let background = if selected {
            theme::SELECTION
        } else {
            theme::BACKGROUND
        };
        draw::set_draw_color(background);
        draw::draw_rectf(x, y, w, h);
        draw::set_draw_color(theme::TEXT);
        draw::set_font(theme::MONO_FONT, state.font_size);
        let formatted = StyledLine::parse(&line.raw);
        let record = parse_record(&formatted.text);
        if col == 0 {
            draw::set_draw_color(theme::DISABLED);
            draw::draw_text2(
                &(number + 1).to_string(),
                x + 4,
                y,
                w - 12,
                h,
                Align::Right | Align::Inside,
            );
        } else if col == 1 {
            draw::set_draw_color(theme::MUTED);
            draw::set_font(theme::MONO_FONT, theme::SMALL);
            draw::draw_text2(
                record.time.as_deref().unwrap_or("—"),
                x + 12,
                y,
                w - 18,
                h,
                Align::Left | Align::Inside,
            );
        } else if col == 2 {
            if let Some(level) = &record.level {
                let color = match level.as_str() {
                    "ERROR" | "FATAL" => theme::ERROR,
                    "WARN" => theme::WARNING,
                    "INFO" => theme::SUCCESS,
                    _ => theme::MUTED,
                };
                draw::set_draw_color(theme::CONTROL);
                draw::draw_rounded_rectf(x + 9, y + 5, w - 18, h - 10, 4);
                draw::set_draw_color(color);
                draw::set_font(theme::MONO_FONT, theme::SMALL);
                draw::draw_text2(level, x + 9, y, w - 18, h, Align::Center | Align::Inside);
            }
        } else {
            let mut position = x + 12;
            let mut budget = 6000usize;
            if state.raw {
                let shown = truncate(&line.raw, budget);
                draw::draw_text(shown, position, y + h - 7);
            } else {
                let message = record.message_offset;
                for span in &formatted.spans {
                    if budget == 0 {
                        break;
                    }
                    if span.visible.end <= message {
                        continue;
                    }
                    let start = span.visible.start.max(message);
                    let text = truncate(&formatted.text[start..span.visible.end], budget);
                    let text = if position == x + 12 && record.message_offset > 0 {
                        text.trim_start()
                    } else {
                        text
                    };
                    budget = budget.saturating_sub(text.len());
                    let shown = if state.obfuscate && span.style.obfuscated {
                        text.chars()
                            .map(|ch| if ch.is_whitespace() { ch } else { '*' })
                            .collect::<String>()
                    } else {
                        text.into()
                    };
                    let font = match (span.style.bold, span.style.italic) {
                        (true, true) => Font::CourierBoldItalic,
                        (true, false) => Font::CourierBold,
                        (false, true) => Font::CourierItalic,
                        _ => theme::MONO_FONT,
                    };
                    draw::set_font(font, state.font_size);
                    let width = draw::width(&shown) as i32;
                    if let Some((focus_row, range)) = &state.focus
                        && *focus_row == line.number
                        && state
                            .log
                            .as_ref()
                            .is_some_and(|active| active.source.id() == log.source.id())
                        && range.start < span.raw.end
                        && range.end > span.raw.start
                    {
                        draw::set_draw_color(theme::MATCH);
                        draw::draw_rectf(position, y + 1, width, h - 2);
                    }
                    let color = span.style.color.map(minecraft_color).unwrap_or(theme::TEXT);
                    if span.style.color.is_some() && contrast(color, background) < 4.5 {
                        let backdrop =
                            if contrast(color, Color::White) > contrast(color, Color::Black) {
                                Color::White
                            } else {
                                Color::Black
                            };
                        draw::set_draw_color(backdrop);
                        draw::draw_rectf(position, y + 2, width, h - 4);
                    }
                    draw::set_draw_color(color);
                    draw::draw_text(&shown, position, y + h - 7);
                    if span.style.underline {
                        draw::draw_line(position, y + h - 5, position + width, y + h - 5);
                    }
                    if span.style.strike {
                        draw::draw_line(position, y + h / 2, position + width, y + h / 2);
                    }
                    position += width;
                }
            }
        }
        draw::pop_clip();
    });
    (table, view)
}

fn minecraft_color(index: u8) -> Color {
    if index == 0 {
        return Color::Black;
    }
    const COLORS: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (0, 0, 170),
        (0, 170, 0),
        (0, 170, 170),
        (170, 0, 0),
        (170, 0, 170),
        (255, 170, 0),
        (170, 170, 170),
        (85, 85, 85),
        (85, 85, 255),
        (85, 255, 85),
        (85, 255, 255),
        (255, 85, 85),
        (255, 85, 255),
        (255, 255, 85),
        (255, 255, 255),
    ];
    let (r, g, b) = COLORS[index as usize];
    Color::from_rgb(r, g, b)
}

fn contrast(a: Color, b: Color) -> f64 {
    fn luminance(color: Color) -> f64 {
        let (r, g, b) = color.to_rgb();
        let channel = |byte: u8| {
            let value = byte as f64 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }
    let a = luminance(a);
    let b = luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

pub fn truncate(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
