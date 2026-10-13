// Hallmark · modern-minimal · native workbench · graphite with blue signal accents.
// Pre-emit critique: P4 H4 E4 S5 R5 V4. Desktop UI; native tokens and system fonts.
// Native tokens keep the UI on system fonts and add no bundled font assets.
use fltk::{
    app, draw,
    enums::{Color, Font, FrameType},
};

pub const BACKGROUND: Color = Color::from_rgbi(200);
pub const SURFACE: Color = Color::from_rgbi(201);
pub const CONTROL: Color = Color::from_rgbi(202);
pub const HOVER: Color = Color::from_rgbi(203);
pub const RULE: Color = Color::from_rgbi(204);
pub const TEXT: Color = Color::from_rgbi(205);
pub const MUTED: Color = Color::from_rgbi(206);
pub const DISABLED: Color = Color::from_rgbi(207);
pub const ACCENT: Color = Color::from_rgbi(208);
pub const ACCENT_HOVER: Color = Color::from_rgbi(209);
pub const ACCENT_INK: Color = Color::from_rgbi(210);
pub const FOCUS: Color = Color::from_rgbi(211);
pub const SELECTION: Color = Color::from_rgbi(212);
pub const SUCCESS: Color = Color::from_rgbi(213);
pub const WARNING: Color = Color::from_rgbi(214);
pub const ERROR: Color = Color::from_rgbi(215);
pub const MATCH: Color = Color::from_rgbi(216);
pub const UI_FONT: Font = Font::Helvetica;
pub const HEADING_FONT: Font = Font::HelveticaBold;
pub const MONO_FONT: Font = Font::Courier;
pub const FONT_SIZE: i32 = 14;
pub const SMALL: i32 = 12;
pub const SPACE: i32 = 8;
pub const RADIUS: i32 = 6;
pub const ROW_HEIGHT: i32 = 28;

fn field_box(x: i32, y: i32, w: i32, h: i32, color: Color) {
    draw::set_draw_color(color);
    draw::draw_rounded_rectf(x, y, w, h, RADIUS);
    draw::set_draw_color(RULE);
    draw::draw_rounded_rect(x, y, w - 1, h - 1, RADIUS);
}

fn focused_box(x: i32, y: i32, w: i32, h: i32, color: Color) {
    field_box(x, y, w, h, color);
    draw::set_draw_color(FOCUS);
    draw::draw_rounded_rect(x, y, w - 1, h - 1, RADIUS);
}

pub fn apply(light: bool) {
    const DARK: [u32; 17] = [
        0x10141b, 0x171d27, 0x202938, 0x2c3a4e, 0x354254, 0xe7edf7, 0x9aacc4, 0x64738a, 0x2f66d1,
        0x366ed5, 0xffffff, 0x91bdff, 0x263d60, 0x78dba9, 0xffcd78, 0xff8e96, 0x665125,
    ];
    const LIGHT: [u32; 17] = [
        0xf4f6fa, 0xffffff, 0xebeff5, 0xe0e7f0, 0xb7c3d3, 0x172132, 0x52647c, 0x8995a7, 0x2f66d1,
        0x366ed5, 0xffffff, 0x2356b7, 0xd9e6fd, 0x187749, 0x8e5a00, 0xb31c30, 0xffe6ad,
    ];
    for (index, hex) in if light { LIGHT } else { DARK }.iter().enumerate() {
        let (r, g, b) = Color::from_hex(*hex).to_rgb();
        app::set_color(Color::from_rgbi(200 + index as u32), r, g, b);
    }
    app::set_scheme(app::Scheme::Base);
    let (r, g, b) = BACKGROUND.to_rgb();
    app::background(r, g, b);
    let (r, g, b) = SURFACE.to_rgb();
    app::background2(r, g, b);
    let (r, g, b) = TEXT.to_rgb();
    app::foreground(r, g, b);
    let (r, g, b) = ACCENT.to_rgb();
    app::set_selection_color(r, g, b);
    app::set_font_size(FONT_SIZE);
    app::set_scrollbar_size(10);
    app::set_visible_focus(true);
    app::set_frame_type2(FrameType::UpBox, FrameType::FlatBox);
    app::set_frame_type_cb(FrameType::DownBox, field_box, 8, 4, 16, 8);
    app::set_frame_type_cb(FrameType::BorderBox, focused_box, 8, 4, 16, 8);
    #[cfg(windows)]
    {
        Font::set_font(UI_FONT, "Segoe UI");
        Font::set_font(HEADING_FONT, "Segoe UI Semibold");
        Font::set_font(MONO_FONT, "Consolas");
        Font::set_font(Font::CourierBold, "BConsolas");
        Font::set_font(Font::CourierItalic, "IConsolas");
        Font::set_font(Font::CourierBoldItalic, "PConsolas");
    }
    #[cfg(target_os = "macos")]
    {
        Font::set_font(UI_FONT, "Helvetica");
        Font::set_font(MONO_FONT, "Menlo");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Font::set_font(UI_FONT, "Noto Sans");
        Font::set_font(MONO_FONT, "DejaVu Sans Mono");
    }
}
