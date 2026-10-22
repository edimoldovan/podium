//! The window's parts.

pub mod canvas;
pub mod inspector;
pub mod pages;
pub mod present;
pub mod storyline;
pub mod theme_dialog;
pub mod titles;

use eframe::egui::{Color32, Painter, Rect};

/// One of the kit's shared glyphs, as an element's icon.
pub fn glyph(name: &'static str) -> impl Fn(&Painter, Rect, Color32) {
    move |painter, rect, colour| {
        omarchy_kit::icons::draw(painter, rect, name, colour);
    }
}

/// A `#rrggbb` setting as a colour.
pub fn colour(hex: &str) -> Color32 {
    let [r, g, b] = crate::theme::rgb(hex);
    Color32::from_rgb(r, g, b)
}
