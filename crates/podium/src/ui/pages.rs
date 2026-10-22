//! The pages: the rail of every page down the left, the page being worked on
//! in the middle, what is selected on it to the right, its speaker notes
//! below.

use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind, TextEdit, pos2, vec2};
use omarchy_kit::{Dropdown, Item, Size, context_menu, field};

use crate::app::{Action, PodiumApp};
use crate::deck::{HEIGHT, PageType, WIDTH};
use crate::render;
use crate::ui::{canvas, inspector};

const THUMB: f32 = 150.0;

pub fn show(app: &mut PodiumApp, ctx: &egui::Context, actions: &mut Vec<Action>) {
    egui::TopBottomPanel::bottom("notes").resizable(true).default_height(110.0).min_height(70.0).show(ctx, |ui| {
        ui.add_space(6.0);
        ui.label(RichText::new("Speaker notes").weak());
        let page = &mut app.session.deck.pages[app.page];
        field(ui, Size::Regular, |ui| {
            let size = [ui.available_width() - 16.0, (ui.available_height() - 18.0).max(40.0)];
            let edit = ui.add_sized(size, TextEdit::multiline(&mut page.notes).frame(false).hint_text("What to say on this page").id_salt("notes"));
            (edit.has_focus(), edit)
        });
        ui.add_space(4.0);
    });
    rail(app, ctx, actions);
    inspector::show(app, ctx, actions);
    egui::CentralPanel::default().show(ctx, |ui| canvas::show(app, ui, actions));
}

fn rail(app: &mut PodiumApp, ctx: &egui::Context, actions: &mut Vec<Action>) {
    egui::SidePanel::left("rail").resizable(false).exact_width(THUMB + 46.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add_space(8.0);
            let scale = THUMB / WIDTH * ctx.pixels_per_point();
            let height = THUMB * HEIGHT / WIDTH;
            for i in 0..app.session.deck.pages.len() {
                let html = render::page_document(&app.session.deck, i);
                let key = format!("thumb-{}", app.session.deck.pages[i].id);
                let busy = i == app.page && (app.drag.is_some() || app.editing.is_some());
                let texture = match app.textures.last(&key).filter(|_| busy) {
                    Some(texture) => texture,
                    None => app.textures.get(ctx, &key, &html, scale),
                };
                let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height + 14.0), Sense::click());
                Item { open: i == app.page, keyed: false, hovered: response.hovered(), size: Size::Regular }.paint(ui, rect);
                let text = ui.visuals().weak_text_color();
                ui.painter().text(rect.left_top() + vec2(8.0, 7.0), Align2::LEFT_TOP, format!("{}", i + 1), FontId::proportional(12.0), text);
                let picture = Rect::from_min_size(rect.left_top() + vec2(30.0, 7.0), vec2(THUMB, height));
                ui.painter().image(texture.id(), picture, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                ui.painter().rect_stroke(picture, 0.0, Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color), StrokeKind::Outside);
                let response = response.on_hover_text(app.session.deck.pages[i].title.clone());
                if response.clicked() || response.secondary_clicked() {
                    actions.push(Action::GoTo(i));
                }
                let several = app.session.deck.pages.len() > 1;
                context_menu(&response, |m| {
                    if m.item("Duplicate page").clicked() {
                        actions.push(Action::DuplicatePage);
                    }
                    if m.item("Remove page").enabled(several).clicked() {
                        actions.push(Action::RemovePage);
                    }
                    m.separator();
                    if m.item("Move up").enabled(i > 0).clicked() {
                        actions.push(Action::MovePage(-1));
                    }
                    if m.item("Move down").clicked() {
                        actions.push(Action::MovePage(1));
                    }
                });
                ui.add_space(4.0);
            }
            ui.add_space(6.0);
            let mut pick = usize::MAX;
            if ui.add(Dropdown::new(&mut pick, &PageType::LABELS).placeholder("Add a page…").width(THUMB + 30.0).name("Add a page").id_salt("rail add")).changed() {
                actions.push(Action::AddPage(PageType::ALL[pick]));
            }
            ui.add_space(8.0);
        });
    });
}
