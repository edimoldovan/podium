//! The action titles: every page in order, one line each — its type, its
//! title, its chapter — and the chapters themselves. Read top to bottom,
//! the titles tell the story.

use eframe::egui::{self, RichText};
use omarchy_kit::{Button, Dropdown, TextField};

use crate::app::{Action, PodiumApp};
use crate::deck::PageType;
use crate::ui::glyph;

pub fn show(app: &mut PodiumApp, ctx: &egui::Context, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = (ui.available_width() - 48.0).min(1040.0);
            let side = (ui.available_width() - width) / 2.0;
            ui.horizontal(|ui| {
                ui.add_space(side);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.add_space(24.0);
                    ui.heading("Action titles");
                    ui.label(RichText::new("One sentence a page, each its message. Read top to bottom, they tell the story.").weak());
                    ui.add_space(16.0);
                    pages(app, ui, width, actions);
                    ui.add_space(28.0);
                    chapters(app, ui, width);
                    ui.add_space(24.0);
                });
            });
        });
    });
}

fn pages(app: &mut PodiumApp, ui: &mut egui::Ui, width: f32, actions: &mut Vec<Action>) {
    let deck = &mut app.session.deck;
    let names: Vec<String> = std::iter::once("No chapter".to_string()).chain(deck.chapters.iter().map(|c| c.name.clone())).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let n = deck.pages.len();
    let mut moved = None;
    let mut removed = None;
    let mut chapter: Option<Option<String>> = None;
    for i in 0..n {
        let this = deck.pages[i].chapter.clone();
        if chapter.as_ref() != Some(&this) {
            let name = this.as_deref().and_then(|c| deck.chapter_name(c)).unwrap_or("No chapter").to_string();
            ui.add_space(if i == 0 { 0.0 } else { 10.0 });
            ui.label(RichText::new(name).strong());
            chapter = Some(this.clone());
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{:>2}", i + 1)).monospace().weak());
            let page = &mut deck.pages[i];
            let mut kind = page.kind.index();
            if ui.add(Dropdown::new(&mut kind, &PageType::LABELS).small().width(170.0).name(format!("Type of page {}", i + 1)).id_salt(("type", i))).changed() {
                page.kind = PageType::ALL[kind];
            }
            let title = width - 170.0 - 150.0 - 4.0 * 28.0 - 7.0 * ui.spacing().item_spacing.x - 24.0;
            ui.add(TextField::new(&mut page.title).small().width(title).name(format!("Title of page {}", i + 1)).id_salt(("title", i)));
            let mut pick = deck_chapter_position(&deck.chapters, deck.pages[i].chapter.as_deref());
            if ui.add(Dropdown::new(&mut pick, &names).small().width(150.0).name(format!("Chapter of page {}", i + 1)).id_salt(("chapter", i))).changed() {
                deck.pages[i].chapter = if pick == 0 { None } else { deck.chapters.get(pick - 1).map(|c| c.id.clone()) };
            }
            if ui.add_enabled(i > 0, Button::icon_only(glyph("up")).mini().quiet().tip("Move up")).clicked() {
                moved = Some((i, i - 1));
            }
            if ui.add_enabled(i + 1 < n, Button::icon_only(glyph("down")).mini().quiet().tip("Move down")).clicked() {
                moved = Some((i, i + 1));
            }
            if ui.add(Button::icon_only(glyph("open")).mini().quiet().tip("Open the page")).clicked() {
                actions.push(Action::GoTo(i));
            }
            if ui.add_enabled(n > 1, Button::icon_only(glyph("trash")).mini().quiet().tip("Remove the page")).clicked() {
                removed = Some(i);
            }
        });
    }
    if let Some((a, b)) = moved {
        deck.pages.swap(a, b);
    }
    if let Some(i) = removed {
        deck.pages.remove(i);
    }
    ui.add_space(10.0);
    let mut pick = usize::MAX;
    if ui.add(Dropdown::new(&mut pick, &PageType::LABELS).placeholder("Add a page…").width(200.0).name("Add a page").id_salt("add page")).changed() {
        let kind = PageType::ALL[pick];
        let chapter = deck.pages.last().and_then(|p| p.chapter.clone());
        let title = match kind {
            PageType::Title => "Title of the presentation",
            PageType::ExecutiveSummary => "Executive summary",
            PageType::Divider => "Chapter",
            _ => "",
        };
        deck.add_page(kind, title, chapter, None);
    }
}

/// Where a page's chapter sits in the chapter list: 0 for none.
fn deck_chapter_position(chapters: &[crate::deck::Chapter], id: Option<&str>) -> usize {
    id.and_then(|id| chapters.iter().position(|c| c.id == id)).map_or(0, |i| i + 1)
}

fn chapters(app: &mut PodiumApp, ui: &mut egui::Ui, width: f32) {
    let deck = &mut app.session.deck;
    ui.label(RichText::new("Chapters").strong());
    ui.label(RichText::new("A longer story in parts; each page shows its chapter in the corner.").weak());
    ui.add_space(6.0);
    let mut removed = None;
    for (i, chapter) in deck.chapters.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(TextField::new(&mut chapter.name).small().width((width - 40.0).min(420.0)).name(format!("Chapter {}", i + 1)).id_salt(("chapter name", i)));
            if ui.add(Button::icon_only(glyph("trash")).mini().quiet().tip("Remove the chapter")).clicked() {
                removed = Some(chapter.id.clone());
            }
        });
    }
    if let Some(id) = removed {
        deck.remove_chapter(&id);
    }
    ui.add_space(6.0);
    if ui.add(Button::new("Add chapter").icon(glyph("plus"))).clicked() {
        let n = deck.chapters.len() + 1;
        deck.add_chapter(&format!("Chapter {n}"));
    }
}
