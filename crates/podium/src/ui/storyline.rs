//! The storyline: what the presentation needs to do, and its audience.
//! Under it, the story the action titles tell so far.

use eframe::egui::{self, RichText, TextEdit};
use omarchy_kit::{Size, TextField, field};

use crate::app::PodiumApp;

pub fn show(app: &mut PodiumApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = (ui.available_width() - 48.0).min(760.0);
            let side = (ui.available_width() - width) / 2.0;
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.add_space(24.0);
                    ui.heading("Storyline");
                    ui.label(RichText::new("What the presentation needs to do. Everything in the deck follows from it.").weak());
                    ui.add_space(16.0);
                    let story = &mut app.session.deck.storyline;
                    ui.label("Audience");
                    ui.add(TextField::new(&mut story.audience).hint("Who is in the room").width(width).id_salt("audience"));
                    ui.add_space(12.0);
                    ui.label("Storyline");
                    field(ui, Size::Regular, |ui| {
                        let edit = ui.add_sized(
                            [width - 16.0, 320.0],
                            TextEdit::multiline(&mut story.text).frame(false).hint_text("The situation, what has changed, what the audience should conclude and do"),
                        );
                        (edit.has_focus(), edit)
                    });
                    ui.add_space(28.0);
                    ui.label(RichText::new("The story so far").strong());
                    ui.add_space(6.0);
                    let deck = &app.session.deck;
                    let mut chapter = None;
                    for (i, page) in deck.pages.iter().enumerate() {
                        if page.chapter != chapter {
                            chapter = page.chapter.clone();
                            if let Some(name) = chapter.as_deref().and_then(|c| deck.chapter_name(c)) {
                                ui.add_space(6.0);
                                ui.label(RichText::new(name).weak().small());
                            }
                        }
                        ui.label(format!("{}.  {}", i + 1, page.title));
                    }
                    ui.add_space(24.0);
                });
            });
        });
    });
}
