pub mod capture;
pub mod controls;
pub mod editor;
pub mod search;
pub mod theme;
pub mod viewer;

// Windows can draw into an offscreen GDI surface without a display server.
#[cfg(all(test, windows))]
mod redraw_tests;
