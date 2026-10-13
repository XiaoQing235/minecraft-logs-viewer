use super::theme;
use fltk::{
    app,
    button::{Button, CheckButton},
    draw,
    enums::{Align, Event, FrameType},
    frame::Frame,
    input::{Input, IntInput},
    menu::Choice,
    prelude::*,
    tree::{Tree, TreeConnectorStyle},
};
use std::{cell::Cell, rc::Rc};

pub fn label(text: &str, heading: bool) -> Frame {
    let mut label = Frame::default().with_label(text);
    label.set_frame(FrameType::FlatBox);
    label.set_color(theme::BACKGROUND);
    label.set_align(Align::Left | Align::Inside);
    label.set_label_font(if heading {
        theme::HEADING_FONT
    } else {
        theme::UI_FONT
    });
    label.set_label_size(if heading { 17 } else { theme::SMALL });
    label.set_label_color(if heading { theme::TEXT } else { theme::MUTED });
    label
}

pub fn button(label: &str, primary: bool) -> Button {
    let mut button = Button::default().with_label(label);
    button.super_draw(false);
    button.set_frame(FrameType::FlatBox);
    button.set_label_font(theme::UI_FONT);
    button.set_label_size(theme::FONT_SIZE);
    let hover = Rc::new(Cell::new(false));
    let hover_event = hover.clone();
    button.handle(move |widget, event| {
        match event {
            Event::Enter => hover_event.set(true),
            Event::Leave => hover_event.set(false),
            _ => {}
        }
        if matches!(
            event,
            Event::Enter
                | Event::Leave
                | Event::Focus
                | Event::Unfocus
                | Event::Push
                | Event::Released
        ) {
            widget.redraw();
        }
        false
    });
    button.draw(move |widget| {
        let fill = if !widget.active() {
            theme::SURFACE
        } else if widget.value() {
            theme::SELECTION
        } else if primary && hover.get() {
            theme::ACCENT_HOVER
        } else if primary {
            theme::ACCENT
        } else if hover.get() {
            theme::HOVER
        } else {
            theme::CONTROL
        };
        draw::set_draw_color(fill);
        draw::draw_rounded_rectf(
            widget.x(),
            widget.y(),
            widget.w(),
            widget.h(),
            theme::RADIUS,
        );
        if widget.has_focus() {
            draw::set_draw_color(theme::FOCUS);
            draw::draw_rounded_rect(
                widget.x() + 1,
                widget.y() + 1,
                widget.w() - 3,
                widget.h() - 3,
                theme::RADIUS,
            );
        }
        draw::set_draw_color(if !widget.active() {
            theme::DISABLED
        } else if primary {
            theme::ACCENT_INK
        } else {
            theme::TEXT
        });
        draw::set_font(theme::UI_FONT, theme::FONT_SIZE);
        draw::draw_text2(
            &widget.label(),
            widget.x() + 8,
            widget.y(),
            widget.w() - 16,
            widget.h(),
            Align::Center | Align::Inside,
        );
    });
    button
}

pub fn toggle(label: &str) -> CheckButton {
    let mut button = CheckButton::default().with_label(label);
    button.super_draw(false);
    button.set_align(Align::Left | Align::Inside);
    button.set_label_font(theme::UI_FONT);
    button.set_label_size(theme::SMALL);
    button.handle(|widget, event| {
        if matches!(
            event,
            Event::Enter | Event::Leave | Event::Focus | Event::Unfocus
        ) {
            widget.redraw();
        }
        false
    });
    button.draw(|widget| {
        let x = widget.x() + 2;
        let y = widget.y() + (widget.h() - 16) / 2;
        let checked = widget.is_checked();
        draw::set_draw_color(if checked {
            theme::ACCENT
        } else {
            theme::CONTROL
        });
        draw::draw_rounded_rectf(x, y, 16, 16, 4);
        draw::set_draw_color(if widget.has_focus() {
            theme::FOCUS
        } else {
            theme::RULE
        });
        draw::draw_rounded_rect(x, y, 16, 16, 4);
        if checked {
            draw::set_draw_color(theme::ACCENT_INK);
            draw::draw_line(x + 4, y + 8, x + 7, y + 11);
            draw::draw_line(x + 7, y + 11, x + 12, y + 5);
        }
        draw::set_font(theme::UI_FONT, theme::SMALL);
        draw::set_draw_color(if widget.active() {
            theme::MUTED
        } else {
            theme::DISABLED
        });
        draw::draw_text2(
            &widget.label(),
            x + 24,
            widget.y(),
            widget.w() - 26,
            widget.h(),
            Align::Left | Align::Inside,
        );
    });
    button
}

fn field(widget: &mut impl WidgetExt) {
    widget.set_color(theme::CONTROL);
    widget.set_selection_color(theme::SELECTION);
    widget.set_frame(FrameType::DownBox);
    widget.set_label_color(theme::MUTED);
    widget.set_label_font(theme::UI_FONT);
    widget.set_label_size(theme::SMALL);
}

pub fn input() -> Input {
    let mut input = Input::default();
    field(&mut input);
    input.set_text_font(theme::UI_FONT);
    input.set_text_size(theme::FONT_SIZE);
    input.set_text_color(theme::TEXT);
    input.handle(|widget, event| {
        if event == Event::Focus {
            widget.set_frame(FrameType::BorderBox);
        } else if event == Event::Unfocus {
            widget.set_frame(FrameType::DownBox);
        }
        false
    });
    input
}

pub fn number() -> IntInput {
    let mut input = IntInput::default();
    field(&mut input);
    input.set_text_font(theme::MONO_FONT);
    input.set_text_size(theme::FONT_SIZE);
    input.set_text_color(theme::TEXT);
    input
}

pub fn choice() -> Choice {
    let mut choice = Choice::default();
    choice.super_draw(false);
    field(&mut choice);
    choice.set_text_color(theme::TEXT);
    choice.set_text_font(theme::UI_FONT);
    choice.set_text_size(theme::SMALL);
    choice.draw(|widget| {
        draw::set_draw_color(theme::CONTROL);
        draw::draw_rounded_rectf(
            widget.x(),
            widget.y(),
            widget.w(),
            widget.h(),
            theme::RADIUS,
        );
        draw::set_draw_color(if widget.has_focus() {
            theme::FOCUS
        } else {
            theme::RULE
        });
        draw::draw_rounded_rect(
            widget.x(),
            widget.y(),
            widget.w() - 1,
            widget.h() - 1,
            theme::RADIUS,
        );
        draw::set_draw_color(if widget.active() {
            theme::TEXT
        } else {
            theme::DISABLED
        });
        draw::set_font(theme::UI_FONT, theme::SMALL);
        draw::draw_text2(
            &widget.choice().unwrap_or_default(),
            widget.x() + 12,
            widget.y(),
            widget.w() - 35,
            widget.h(),
            Align::Left | Align::Inside,
        );
        let x = widget.x() + widget.w() - 17;
        let y = widget.y() + widget.h() / 2;
        draw::draw_line(x - 3, y - 2, x, y + 1);
        draw::draw_line(x, y + 1, x + 3, y - 2);
    });
    choice
}

pub fn source_tree() -> Tree {
    let mut tree = Tree::default();
    tree.set_frame(FrameType::FlatBox);
    tree.set_color(theme::SURFACE);
    tree.set_item_label_font(theme::UI_FONT);
    tree.set_item_label_size(theme::FONT_SIZE);
    tree.set_item_label_fgcolor(theme::TEXT);
    tree.set_item_label_bgcolor(theme::SURFACE);
    tree.set_connector_style(TreeConnectorStyle::None);
    tree.set_margin_left(12);
    tree.set_margin_top(10);
    tree.set_line_spacing(8);
    tree.set_label_margin_left(5);
    tree.set_select_frame(FrameType::FlatBox);
    tree.set_selection_color(theme::SELECTION);
    tree.set_open_icon(Some(chevron(true)));
    tree.set_close_icon(Some(chevron(false)));
    tree
}

fn chevron(right: bool) -> fltk::image::RgbImage {
    let mut pixels = vec![0u8; 14 * 14 * 4];
    let (r, g, b) = theme::MUTED.to_rgb();
    for offset in 0..5 {
        for (x, y) in if right {
            [(5 + offset, 3 + offset), (5 + offset, 11 - offset)]
        } else {
            [(3 + offset, 5 + offset), (11 - offset, 5 + offset)]
        } {
            let index = (y * 14 + x) * 4;
            pixels[index..index + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    fltk::image::RgbImage::new(&pixels, 14, 14, fltk::enums::ColorDepth::Rgba8).unwrap()
}

pub fn brand() -> Frame {
    let mut frame = Frame::default();
    frame.super_draw(false);
    frame.draw(|widget| {
        let x = widget.x();
        let y = widget.y() + (widget.h() - 34) / 2;
        draw::set_draw_color(theme::SELECTION);
        draw::draw_rounded_rectf(x, y, 34, 34, 8);
        draw::set_draw_color(theme::FOCUS);
        draw::draw_line(x + 9, y + 10, x + 9, y + 24);
        draw::draw_line(x + 9, y + 24, x + 24, y + 24);
        draw::draw_line(x + 15, y + 10, x + 25, y + 10);
        draw::draw_line(x + 15, y + 16, x + 23, y + 16);
        draw::set_font(theme::HEADING_FONT, 18);
        draw::set_draw_color(theme::TEXT);
        draw::draw_text("Minecraft Logs", x + 46, y + 15);
        draw::set_font(theme::UI_FONT, theme::SMALL);
        draw::set_draw_color(theme::MUTED);
        draw::draw_text("日志工作台", x + 46, y + 32);
    });
    frame
}

pub fn header_rule() -> Frame {
    let mut frame = Frame::default();
    frame.draw(|widget| {
        draw::set_draw_color(theme::RULE);
        draw::draw_line(widget.x(), widget.y(), widget.x() + widget.w(), widget.y());
    });
    frame
}

pub fn shortcut_open(button: &mut Button) {
    button.set_shortcut(fltk::enums::Shortcut::Command | 'o');
    app::set_visible_focus(true);
}
