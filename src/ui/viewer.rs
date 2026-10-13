use fltk::{
    draw,
    enums::{Align, Color, Font},
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
    table.set_color(Color::from_rgb(18, 22, 29));
    table.set_cols(2);
    table.set_col_width(0, 85);
    table.set_col_width(1, 2400);
    table.set_row_height_all(24);
    table.set_col_header(false);
    table.set_row_header(false);
    table.set_type(TableRowSelectMode::Multi);
    table.end();
    let state = view.clone();
    table.draw_cell(move |table, context, row, col, x, y, w, h| {
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
            Color::from_rgb(43, 60, 86)
        } else {
            Color::from_rgb(18, 22, 29)
        };
        draw::set_draw_color(background);
        draw::draw_rectf(x, y, w, h);
        draw::set_draw_color(Color::from_rgb(235, 236, 238));
        draw::set_font(Font::Courier, state.font_size);
        if col == 0 {
            draw::draw_text2(
                &(row + 1).to_string(),
                x + 4,
                y,
                w - 12,
                h,
                Align::Right | Align::Inside,
            );
        } else {
            let formatted = StyledLine::parse(&line.raw);
            let mut position = x + 6;
            let mut budget = 6000usize;
            if state.raw {
                let shown = truncate(&line.raw, budget);
                draw::draw_text(shown, position, y + h - 7);
            } else {
                let level = parse_record(&line.raw).level;
                for span in &formatted.spans {
                    if budget == 0 {
                        break;
                    }
                    let text = truncate(&formatted.text[span.visible.clone()], budget);
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
                        _ => Font::Courier,
                    };
                    draw::set_font(font, state.font_size);
                    let width = draw::width(&shown) as i32;
                    if let Some((focus_row, range)) = &state.focus
                        && *focus_row == line.number
                        && range.start < span.raw.end
                        && range.end > span.raw.start
                    {
                        draw::set_draw_color(Color::from_rgb(104, 78, 24));
                        draw::draw_rectf(position, y + 1, width, h - 2);
                    }
                    let color =
                        span.style.color.map(minecraft_color).unwrap_or_else(|| {
                            match level.as_deref() {
                                Some("ERROR" | "FATAL") => Color::from_rgb(255, 85, 85),
                                Some("WARN") => Color::from_rgb(255, 255, 85),
                                _ => Color::from_rgb(235, 236, 238),
                            }
                        });
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
