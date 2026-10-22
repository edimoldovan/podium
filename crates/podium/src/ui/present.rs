//! Present mode: one page at a time, filling the screen. A click or the
//! arrow keys move on, Esc ends it.

use eframe::egui::{self, Color32, Rect, Sense, pos2, vec2};

use crate::app::{Action, PodiumApp};
use crate::deck::{HEIGHT, WIDTH};
use crate::render;

pub fn show(app: &mut PodiumApp, ctx: &egui::Context, actions: &mut Vec<Action>) {
    let index = app.present.unwrap_or(0).min(app.session.deck.pages.len() - 1);
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(Color32::BLACK)).show(ctx, |ui| {
        let area = ui.max_rect();
        let zoom = (area.width() / WIDTH).min(area.height() / HEIGHT);
        let rect = Rect::from_center_size(area.center(), vec2(WIDTH * zoom, HEIGHT * zoom));
        let scale = ((zoom * ctx.pixels_per_point()) * 20.0).round().max(2.0) / 20.0;
        let texture = app.textures.get(ctx, "present", &render::page_document(&app.session.deck, index), scale);
        ui.painter().image(texture.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let response = ui.interact(area, ui.id().with("present"), Sense::click());
        if response.clicked() {
            actions.push(Action::Next);
        }
        if response.secondary_clicked() {
            actions.push(Action::Previous);
        }
    });
}
