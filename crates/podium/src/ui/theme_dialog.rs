//! The deck's theme and the settings changed from its own: colours and
//! fonts, applied to every page as they are picked.

use eframe::egui::{self, Color32, Painter, Rect, Stroke, StrokeKind};
use omarchy_kit::{Button, ColorButton, Dialog, Dropdown};

use crate::app::PodiumApp;
use crate::deck::ThemeChoice;
use crate::theme::{self, COLOURS, FONT_CHOICES, FONTS, THEME_LABELS, THEMES};
use crate::ui::colour;

fn swatch(painter: &Painter, rect: Rect, colour: Color32) {
    painter.rect_stroke(rect.shrink(3.0), 2.0, Stroke::new(1.5_f32, colour), StrokeKind::Inside);
}

/// Keeps a setting only while it differs from the theme's own.
fn set(choice: &mut ThemeChoice, key: &str, value: String) {
    if value == theme::default_of(&choice.name, key) {
        choice.settings.remove(key);
    } else {
        choice.settings.insert(key.to_string(), value);
    }
}

pub fn show(app: &mut PodiumApp, ctx: &egui::Context) {
    let choice = &mut app.session.deck.theme;
    let shown = Dialog::new("Theme").modeless().width(320.0).show(ctx, |ui| {
        let mut at = theme::index(&choice.name);
        if ui.add(Dropdown::new(&mut at, &THEME_LABELS).width(296.0).name("Theme").id_salt("theme")).changed() {
            choice.name = THEMES[at].name.to_string();
            choice.settings.clear();
        }
        ui.add_space(8.0);
        let look = theme::look(choice);
        for (key, label) in COLOURS {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(150.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| ui.label(label));
                let mut picked = Some(colour(look.get(key)));
                ui.add(ColorButton::new(&mut picked, swatch).tip(label));
                if let Some(c) = picked
                    && c != colour(look.get(key))
                {
                    set(choice, key, format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b()));
                }
            });
        }
        for (key, label) in FONTS {
            ui.label(label);
            let current = look.get(key).to_string();
            let mut at = FONT_CHOICES.iter().position(|f| *f == current).unwrap_or(usize::MAX);
            if ui.add(Dropdown::new(&mut at, &FONT_CHOICES).placeholder(current.clone()).width(296.0).name(label).id_salt(key)).changed() {
                set(choice, key, FONT_CHOICES[at].to_string());
            }
        }
        ui.add_space(8.0);
        if ui.add_enabled(!choice.settings.is_empty(), Button::new("Use the theme's own")).clicked() {
            choice.settings.clear();
        }
    });
    if shown.closed {
        app.theme_open = false;
    }
}
