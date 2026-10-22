//! What is selected on the page, to change by the numbers: the page's type,
//! chapter and action title; an element's place, size and turn, and what it
//! holds — its text, its points, a table's cells, a chart's kind and data, a
//! picture's file.

use eframe::egui::{self, RichText, TextEdit};
use omarchy_kit::{Button, Dropdown, NumberField, Section, Size, TextField, field};

use crate::app::{Action, PodiumApp};
use crate::deck::{Body, ChartKind, PageType, Series};
use crate::ui::canvas::TITLE;
use crate::ui::glyph;

const WIDTH: f32 = 300.0;

pub fn show(app: &mut PodiumApp, ctx: &egui::Context, actions: &mut Vec<Action>) {
    egui::SidePanel::right("inspector").resizable(false).exact_width(WIDTH).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.add_space(8.0);
            let inner = ui.available_width() - 8.0;
            page(app, ui, inner);
            ui.add_space(8.0);
            let element = app.selected.clone().filter(|id| id != TITLE);
            match element {
                Some(id) => element_section(app, ui, &id, inner, actions),
                None => {
                    Section::new("insert", "Insert").show(ui, |ui| {
                        for (kind, label) in Body::KINDS.iter().zip(Body::LABELS) {
                            if ui.add(Button::new(label).small().width(inner - 8.0)).clicked() {
                                actions.push(Action::AddElement(kind));
                            }
                        }
                    });
                }
            }
        });
    });
}

fn page(app: &mut PodiumApp, ui: &mut egui::Ui, width: f32) {
    let deck = &mut app.session.deck;
    let names: Vec<String> = std::iter::once("No chapter".to_string()).chain(deck.chapters.iter().map(|c| c.name.clone())).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let chapters: Vec<String> = deck.chapters.iter().map(|c| c.id.clone()).collect();
    let page = &mut deck.pages[app.page];
    Section::new("page", "Page").show(ui, |ui| {
        ui.label("Type");
        let mut kind = page.kind.index();
        if ui.add(Dropdown::new(&mut kind, &PageType::LABELS).width(width - 8.0).name("Page type").id_salt("inspector type")).changed() {
            page.kind = PageType::ALL[kind];
        }
        ui.label("Chapter");
        let mut at = page.chapter.as_ref().and_then(|c| chapters.iter().position(|id| id == c)).map_or(0, |i| i + 1);
        if ui.add(Dropdown::new(&mut at, &names).width(width - 8.0).name("Chapter").id_salt("inspector chapter")).changed() {
            page.chapter = if at == 0 { None } else { chapters.get(at - 1).cloned() };
        }
        ui.label("Action title");
        field(ui, Size::Regular, |ui| {
            let edit = ui.add_sized([width - 24.0, 60.0], TextEdit::multiline(&mut page.title).frame(false).id_salt("inspector title"));
            if edit.changed() {
                page.title = page.title.replace('\n', " ");
            }
            (edit.has_focus(), edit)
        });
    });
}

fn number(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>, suffix: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([56.0, 20.0], egui::Label::new(label));
        ui.add(NumberField::new(value, range).decimals(0).suffix(suffix).small().width(80.0).name(label).id_salt(("inspector", label)));
    });
}

fn element_section(app: &mut PodiumApp, ui: &mut egui::Ui, id: &str, width: f32, actions: &mut Vec<Action>) {
    let page = &mut app.session.deck.pages[app.page];
    let Some(e) = page.elements.iter_mut().find(|e| e.id == id) else { return };
    Section::new("element", e.body.label()).show(ui, |ui| {
        ui.columns(2, |cols| {
            number(&mut cols[0], "X", &mut e.x, -960.0..=1920.0, "");
            number(&mut cols[1], "Y", &mut e.y, -540.0..=1080.0, "");
            number(&mut cols[0], "Width", &mut e.w, 8.0..=2000.0, "");
            number(&mut cols[1], "Height", &mut e.h, 8.0..=2000.0, "");
            number(&mut cols[0], "Turn", &mut e.rotation, -180.0..=180.0, "°");
        });
        ui.add_space(8.0);
        let label = |ui: &mut egui::Ui, text: &str| ui.label(RichText::new(text).weak());
        match &mut e.body {
            Body::Bullets { items } => {
                label(ui, "One point a line");
                let mut text = items.join("\n");
                let edit = multiline(ui, &mut text, width, 140.0, "bullets");
                if edit {
                    *items = text.split('\n').map(String::from).collect();
                }
            }
            Body::Table { rows } => table(ui, rows, width),
            Body::Chart { chart, categories, series } => chart_data(ui, chart, categories, series, width),
            Body::Image { path } => {
                label(ui, "Picture");
                ui.add(TextField::new(path).small().width(width - 8.0).hint("A PNG or JPEG file").id_salt("picture"));
                if ui.add(Button::new("Choose…").small()).clicked()
                    && let Some(file) = rfd::FileDialog::new().add_filter("Picture", &["png", "jpg", "jpeg"]).pick_file()
                {
                    *path = file.display().to_string();
                }
            }
            body => {
                if let Some(text) = body.text_mut() {
                    label(ui, "Text");
                    multiline(ui, text, width, 90.0, "text");
                }
            }
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.add(Button::new("Duplicate").small().hint("Ctrl+D")).clicked() {
                actions.push(Action::DuplicateElement);
            }
            if ui.add(Button::new("Remove").icon(glyph("trash")).small().hint("Delete")).clicked() {
                actions.push(Action::RemoveElement);
            }
        });
    });
}

/// A text box over several lines; true when it changed.
fn multiline(ui: &mut egui::Ui, text: &mut String, width: f32, height: f32, salt: &str) -> bool {
    field(ui, Size::Regular, |ui| {
        let edit = ui.add_sized([width - 24.0, height], TextEdit::multiline(text).frame(false).id_salt(("inspector", salt)));
        (edit.has_focus(), edit.changed())
    })
}

fn table(ui: &mut egui::Ui, rows: &mut Vec<Vec<String>>, width: f32) {
    ui.label(RichText::new("Cells; the first row heads the table").weak());
    let cols = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    for row in rows.iter_mut() {
        row.resize(cols, String::new());
    }
    let cell = ((width - 8.0) / cols as f32 - ui.spacing().item_spacing.x).max(30.0);
    egui::ScrollArea::horizontal().id_salt("table cells").show(ui, |ui| {
        for (r, row) in rows.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                for (c, text) in row.iter_mut().enumerate() {
                    ui.add(TextField::new(text).small().width(cell).name(format!("Row {} column {}", r + 1, c + 1)).id_salt(("cell", r, c)));
                }
            });
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.add(Button::new("Row").icon(glyph("plus")).small()).clicked() {
            rows.push(vec![String::new(); cols]);
        }
        if ui.add(Button::new("Column").icon(glyph("plus")).small()).clicked() {
            for row in rows.iter_mut() {
                row.push(String::new());
            }
        }
        if ui.add_enabled(rows.len() > 1, Button::new("Row").icon(glyph("trash")).small()).clicked() {
            rows.pop();
        }
        if ui.add_enabled(cols > 1, Button::new("Column").icon(glyph("trash")).small()).clicked() {
            for row in rows.iter_mut() {
                row.pop();
            }
        }
    });
}

fn chart_data(ui: &mut egui::Ui, chart: &mut ChartKind, categories: &mut Vec<String>, series: &mut Vec<Series>, width: f32) {
    let mut kind = chart.index();
    if ui.add(Dropdown::new(&mut kind, &ChartKind::LABELS).width(width - 8.0).name("Chart").id_salt("chart kind")).changed() {
        *chart = ChartKind::ALL[kind];
    }
    if *chart == ChartKind::Waterfall {
        ui.label(RichText::new("The first and last values are totals, the ones between changes.").weak());
    }
    ui.add_space(4.0);
    for s in series.iter_mut() {
        s.values.resize(categories.len(), 0.0);
    }
    let cols = series.len() + 1;
    let cell = ((width - 8.0) / cols as f32 - ui.spacing().item_spacing.x).max(40.0);
    egui::ScrollArea::horizontal().id_salt("chart data").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add_sized([cell, 20.0], egui::Label::new(RichText::new("").weak()));
            for (k, s) in series.iter_mut().enumerate() {
                ui.add(TextField::new(&mut s.name).small().width(cell).name(format!("Series {}", k + 1)).id_salt(("series", k)));
            }
        });
        for (i, category) in categories.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(TextField::new(category).small().width(cell).name(format!("Category {}", i + 1)).id_salt(("category", i)));
                for (k, s) in series.iter_mut().enumerate() {
                    ui.add(NumberField::new(&mut s.values[i], f64::MIN..=f64::MAX).decimals(2).small().width(cell).name(format!("{} {}", s.name, i + 1)).id_salt(("value", i, k)));
                }
            });
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.add(Button::new("Category").icon(glyph("plus")).small()).clicked() {
            categories.push(String::new());
        }
        if ui.add(Button::new("Series").icon(glyph("plus")).small()).clicked() {
            series.push(Series { name: format!("Series {}", series.len() + 1), values: vec![0.0; categories.len()] });
        }
        if ui.add_enabled(categories.len() > 1, Button::new("Category").icon(glyph("trash")).small()).clicked() {
            categories.pop();
        }
        if ui.add_enabled(series.len() > 1, Button::new("Series").icon(glyph("trash")).small()).clicked() {
            series.pop();
        }
    });
}
