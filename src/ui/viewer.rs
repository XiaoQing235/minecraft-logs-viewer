use super::theme;
use fltk::{
    draw,
    enums::{Align, Color, Font, FrameType},
    prelude::*,
    table::{TableContext, TableRow, TableRowSelectMode},
};
use minecraft_logs_viewer::{
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
}

pub fn build() -> (TableRow, Rc<RefCell<View>>) {
    let view = Rc::new(RefCell::new(View {
        font_size: 14,
        ..View::default()
    }));
    let mut table = TableRow::default();
    table.set_color(theme::BACKGROUND);
    table.set_frame(FrameType::FlatBox);
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
        let Some(log) = state.log.clone() else {
            return;
        };
        if !state.rows.iter().any(|line| line.number == row as usize) {
            state.rows = log.read_range(row as usize, 128).unwrap_or_default();
        }
        let Some(line) = state.rows.iter().find(|line| line.number == row as usize) else {
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
                &(row + 1).to_string(),
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
                        && range.start < span.raw.end
                        && range.end > span.raw.start
                    {
                        draw::set_draw_color(theme::MATCH);
                        draw::draw_rectf(position, y + 1, width, h - 2);
                    }
                    let color = span.style.color.map(minecraft_color).unwrap_or(theme::TEXT);
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

pub fn truncate(text: &str, limit: usize) -> &str {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
