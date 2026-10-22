//! The window: one deck, three views of it — the storyline, the action
//! titles, the pages — and the present mode over them. Every change the
//! user makes is one undo step: a typed field when it lets go of the
//! keyboard, a drag when the pointer comes up.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use eframe::egui::{self, Align, Key, Layout, Modifiers};
use omarchy_kit::theme::{self as kit_theme, ThemeWatcher};
use omarchy_kit::{Button, Choice, CommandPalette, MenuBar, StatusLine, Toggle};

use crate::deck::{Body, Deck, PageType};
use crate::export::{self, Format};
use crate::mcp::Incoming;
use crate::session::Session;
use crate::ui::{self, glyph};
use crate::{render, tools};

pub const VIEWS: [&str; 3] = ["Storyline", "Titles", "Pages"];
pub const STORYLINE: usize = 0;
pub const TITLES: usize = 1;
pub const PAGES: usize = 2;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Export(Format),
    Undo,
    Redo,
    View(usize),
    GoTo(usize),
    AddPage(PageType),
    DuplicatePage,
    RemovePage,
    MovePage(isize),
    AddElement(&'static str),
    DuplicateElement,
    RemoveElement,
    Order(Order),
    Nudge(f32, f32),
    Edit,
    Deselect,
    Theme,
    Present,
    Next,
    Previous,
    EndPresent,
    Palette,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Order {
    Forward,
    Backward,
    Front,
    Back,
}

/// Pages drawn by Blitz, kept until what they show changes.
#[derive(Default)]
pub struct Textures {
    kept: HashMap<String, (u64, egui::TextureHandle)>,
}

impl Textures {
    pub fn get(&mut self, ctx: &egui::Context, key: &str, html: &str, scale: f32) -> egui::TextureHandle {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        html.hash(&mut hasher);
        scale.to_bits().hash(&mut hasher);
        let hash = hasher.finish();
        if let Some((known, texture)) = self.kept.get(key)
            && *known == hash
        {
            return texture.clone();
        }
        self.draw(ctx, key, html, scale, hash)
    }

    /// What a key showed last, whatever it showed: a page's picture in the
    /// rail while the page is being dragged on or typed into.
    pub fn last(&self, key: &str) -> Option<egui::TextureHandle> {
        self.kept.get(key).map(|(_, texture)| texture.clone())
    }

    fn draw(&mut self, ctx: &egui::Context, key: &str, html: &str, scale: f32, hash: u64) -> egui::TextureHandle {
        let (w, h, pixels) = render::pixels(html, scale);
        let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels);
        let texture = ctx.load_texture(key, image, egui::TextureOptions::LINEAR);
        self.kept.insert(key.to_string(), (hash, texture.clone()));
        texture
    }
}

pub struct PodiumApp {
    pub session: Session,
    pub view: usize,
    pub page: usize,
    /// The element selected on the canvas, or the page's title.
    pub selected: Option<String>,
    /// The element whose text is being typed on the canvas.
    pub editing: Option<String>,
    pub editing_fresh: bool,
    pub drag: Option<ui::canvas::Drag>,
    pub guides: Vec<ui::canvas::Guide>,
    pub textures: Textures,
    pub palette: CommandPalette,
    pub status: String,
    pub mcp: Option<Receiver<Incoming>>,
    pub present: Option<usize>,
    pub theme_open: bool,
    /// Whether the deck is written to disk on its own: after a pause, and
    /// when the window closes.
    pub saves: bool,
    theme_watcher: ThemeWatcher,
    /// The deck before the change under way: a field being typed in, a drag.
    pending: Option<Deck>,
    history_moved: bool,
    title: String,
}

impl PodiumApp {
    pub fn new(session: Session, saves: bool) -> PodiumApp {
        PodiumApp {
            session,
            view: TITLES,
            page: 0,
            selected: None,
            editing: None,
            editing_fresh: false,
            drag: None,
            guides: Vec::new(),
            textures: Textures::default(),
            palette: CommandPalette::default(),
            status: String::new(),
            mcp: None,
            present: None,
            theme_open: false,
            saves,
            theme_watcher: ThemeWatcher::new(),
            pending: None,
            history_moved: false,
            title: String::new(),
        }
    }

    fn reset(&mut self) {
        self.page = 0;
        self.selected = None;
        self.editing = None;
        self.drag = None;
        self.pending = None;
    }

    fn clamp(&mut self) {
        if self.session.deck.pages.is_empty() {
            self.session.deck.add_page(PageType::Detail, "", None, None);
        }
        self.page = self.page.min(self.session.deck.pages.len() - 1);
        let page = &self.session.deck.pages[self.page];
        if let Some(id) = &self.selected
            && id != ui::canvas::TITLE
            && !page.elements.iter().any(|e| &e.id == id)
        {
            self.selected = None;
            self.editing = None;
        }
    }

    /// Claude's calls and other windows' files, as they came in.
    fn serve(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.mcp else { return };
        let incoming: Vec<Incoming> = rx.try_iter().collect();
        for message in incoming {
            match message {
                Incoming::Open(path) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    if let Some(path) = path {
                        self.open(path);
                    }
                }
                Incoming::Call { tool, args, reply } => {
                    self.editing = None;
                    let result = tools::call(&mut self.session, &tool, &args);
                    match &result {
                        Ok(outcome) => {
                            if outcome.replaced {
                                self.reset();
                            }
                            if let Some(view) = outcome.view {
                                self.view = view;
                            }
                            if let Some(page) = outcome.page {
                                self.page = page;
                            }
                            let said: String = outcome.text.lines().next().unwrap_or_default().chars().take(90).collect();
                            self.status = format!("Claude: {said}");
                        }
                        Err(e) => self.status = format!("Claude: {e}"),
                    }
                    let _ = reply.send(tools::reply(result));
                }
            }
        }
    }

    fn open(&mut self, path: PathBuf) {
        match self.session.open(path) {
            Ok(()) => {
                self.reset();
                self.status = String::new();
            }
            Err(e) => self.status = e,
        }
    }

    /// The keys that act anywhere, read before the widgets take them.
    fn keys(&mut self, ctx: &egui::Context) -> Vec<Action> {
        let mut out = Vec::new();
        let typing = ctx.wants_keyboard_input();
        let menu = omarchy_kit::menu_open(ctx);
        let presenting = self.present.is_some();
        let pages = self.view == PAGES;
        let selected = self.selected.is_some();
        ctx.input_mut(|i| {
            let ctrl = Modifiers::COMMAND;
            let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
            if i.consume_key(ctrl, Key::K) {
                out.push(Action::Palette);
            }
            if presenting {
                for key in [Key::ArrowRight, Key::ArrowDown, Key::Space, Key::PageDown, Key::Enter, Key::N] {
                    if i.consume_key(Modifiers::NONE, key) {
                        out.push(Action::Next);
                    }
                }
                for key in [Key::ArrowLeft, Key::ArrowUp, Key::PageUp, Key::Backspace, Key::P] {
                    if i.consume_key(Modifiers::NONE, key) {
                        out.push(Action::Previous);
                    }
                }
                if i.consume_key(Modifiers::NONE, Key::Escape) || i.consume_key(Modifiers::NONE, Key::F5) {
                    out.push(Action::EndPresent);
                }
                return;
            }
            if i.consume_key(ctrl_shift, Key::S) {
                out.push(Action::SaveAs);
            } else if i.consume_key(ctrl, Key::S) {
                out.push(Action::Save);
            }
            if i.consume_key(ctrl, Key::O) {
                out.push(Action::Open);
            }
            if i.consume_key(ctrl, Key::N) {
                out.push(Action::New);
            }
            if i.consume_key(Modifiers::NONE, Key::F5) {
                out.push(Action::Present);
            }
            for (key, view) in [(Key::Num1, STORYLINE), (Key::Num2, TITLES), (Key::Num3, PAGES)] {
                if i.consume_key(ctrl, key) {
                    out.push(Action::View(view));
                }
            }
            if typing || menu {
                return;
            }
            if i.consume_key(ctrl_shift, Key::Z) || i.consume_key(ctrl, Key::Y) {
                out.push(Action::Redo);
            } else if i.consume_key(ctrl, Key::Z) {
                out.push(Action::Undo);
            }
            if !pages {
                return;
            }
            if i.consume_key(Modifiers::NONE, Key::PageDown) {
                out.push(Action::GoTo(self.page + 1));
            }
            if i.consume_key(Modifiers::NONE, Key::PageUp) {
                out.push(Action::GoTo(self.page.saturating_sub(1)));
            }
            if !selected {
                return;
            }
            if i.consume_key(ctrl, Key::D) {
                out.push(Action::DuplicateElement);
            }
            if i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace) {
                out.push(Action::RemoveElement);
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                out.push(Action::Deselect);
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) || i.consume_key(Modifiers::NONE, Key::F2) {
                out.push(Action::Edit);
            }
            for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
                // Shift first: a key with no modifiers named takes Shift too.
                if i.consume_key(Modifiers::SHIFT, key) {
                    out.push(Action::Nudge(dx * 10.0, dy * 10.0));
                } else if i.consume_key(Modifiers::NONE, key) {
                    out.push(Action::Nudge(dx, dy));
                }
            }
        });
        out
    }

    /// Every action, for the command palette.
    fn choices(&self) -> Vec<(Choice, Action)> {
        let c = |label: &str, key: Option<&str>, keywords: &str, action: Action| {
            (Choice { label: label.into(), key: key.map(String::from), keywords: keywords.into() }, action)
        };
        let mut all = vec![
            c("New deck", Some("Ctrl+N"), "create start presentation", Action::New),
            c("Open…", Some("Ctrl+O"), "file load", Action::Open),
            c("Save", Some("Ctrl+S"), "write", Action::Save),
            c("Save as…", Some("Ctrl+Shift+S"), "rename file", Action::SaveAs),
            c("Undo", Some("Ctrl+Z"), "back revert", Action::Undo),
            c("Redo", Some("Ctrl+Shift+Z"), "again", Action::Redo),
            c("Storyline", Some("Ctrl+1"), "audience brief what it needs to do", Action::View(STORYLINE)),
            c("Action titles", Some("Ctrl+2"), "outline story chapters", Action::View(TITLES)),
            c("Pages", Some("Ctrl+3"), "slides canvas detail", Action::View(PAGES)),
            c("Present", Some("F5"), "slideshow full screen play", Action::Present),
            c("Theme…", None, "colours colors fonts style look", Action::Theme),
            c("Duplicate page", None, "copy slide", Action::DuplicatePage),
            c("Remove page", None, "delete slide", Action::RemovePage),
        ];
        for format in Format::ALL {
            all.push(c(&format!("Export {}…", format.label()), None, &format!("{} write share", format.extension()), Action::Export(format)));
        }
        for kind in PageType::ALL {
            all.push(c(&format!("New page: {}", kind.label()), None, "add slide", Action::AddPage(kind)));
        }
        for (kind, label) in Body::KINDS.iter().zip(Body::LABELS) {
            all.push(c(&format!("Insert {}", label.to_lowercase()), None, "add element", Action::AddElement(kind)));
        }
        for (i, page) in self.session.deck.pages.iter().enumerate() {
            all.push(c(&format!("Go to page {}: {}", i + 1, page.title), None, "slide", Action::GoTo(i)));
        }
        all
    }

    pub fn run(&mut self, ctx: &egui::Context, action: Action) {
        let deck = &mut self.session.deck;
        match action {
            Action::New => match self.session.new_deck() {
                Ok(()) => {
                    self.reset();
                    self.view = STORYLINE;
                }
                Err(e) => self.status = e,
            },
            Action::Open => {
                if let Some(path) = rfd::FileDialog::new().add_filter("Podium deck", &["podium"]).pick_file() {
                    self.open(path);
                }
            }
            Action::Save if self.session.path.is_none() => self.run(ctx, Action::SaveAs),
            Action::Save => match self.session.save() {
                Ok(()) => self.status = format!("Saved {}", self.session.file().display()),
                Err(e) => self.status = e,
            },
            Action::SaveAs => {
                let name = format!("{}.podium", file_name(&self.session.deck.name()));
                if let Some(path) = rfd::FileDialog::new().add_filter("Podium deck", &["podium"]).set_file_name(name).save_file() {
                    match self.session.save_as(path) {
                        Ok(()) => self.status = format!("Saved {}", self.session.file().display()),
                        Err(e) => self.status = e,
                    }
                }
            }
            Action::Export(format) => {
                let name = format!("{}.{}", file_name(&self.session.deck.name()), format.extension());
                let dialog = rfd::FileDialog::new().add_filter(format.label(), &[format.extension()]).set_file_name(name);
                let dialog = match self.session.path.as_deref().and_then(|p| p.parent()) {
                    Some(dir) => dialog.set_directory(dir),
                    None => dialog,
                };
                if let Some(path) = dialog.save_file() {
                    match export::export(&self.session.deck, format, &path) {
                        Ok(()) => self.status = format!("Exported {}", path.display()),
                        Err(e) => self.status = e,
                    }
                }
            }
            Action::Undo => {
                self.history_moved |= self.session.undo();
            }
            Action::Redo => {
                self.history_moved |= self.session.redo();
            }
            Action::View(view) => {
                self.view = view;
                self.editing = None;
            }
            Action::GoTo(page) => {
                self.page = page.min(deck.pages.len().saturating_sub(1));
                self.view = PAGES;
                self.selected = None;
                self.editing = None;
            }
            Action::AddPage(kind) => {
                let chapter = deck.pages.get(self.page).and_then(|p| p.chapter.clone());
                let title = match kind {
                    PageType::Title => "Title of the presentation".to_string(),
                    PageType::ExecutiveSummary => "Executive summary".to_string(),
                    PageType::Divider => chapter.as_deref().and_then(|c| deck.chapter_name(c)).unwrap_or("Chapter").to_string(),
                    _ => "The page's message, in one sentence".to_string(),
                };
                let at = (self.page + 1).min(deck.pages.len());
                deck.add_page(kind, &title, chapter, Some(at));
                self.page = at;
                self.selected = None;
            }
            Action::DuplicatePage => {
                self.page = deck.duplicate_page(self.page);
            }
            Action::RemovePage => {
                if deck.pages.len() > 1 {
                    deck.pages.remove(self.page);
                    self.page = self.page.min(deck.pages.len() - 1);
                }
            }
            Action::MovePage(by) => {
                let to = self.page as isize + by;
                if to >= 0 && (to as usize) < deck.pages.len() {
                    deck.pages.swap(self.page, to as usize);
                    self.page = to as usize;
                }
            }
            Action::AddElement(kind) => {
                if let Some((rect, body)) = Body::new(kind) {
                    let id = deck.add_element(self.page, rect, body);
                    self.selected = Some(id);
                    self.view = PAGES;
                }
            }
            Action::DuplicateElement => {
                if let Some(element) = self.selected.as_ref().and_then(|id| deck.pages[self.page].elements.iter().find(|e| &e.id == id)).cloned() {
                    let mut copy = element;
                    copy.id = deck.fresh_element_id(&deck.pages[self.page]);
                    copy.x += 12.0;
                    copy.y += 12.0;
                    self.selected = Some(copy.id.clone());
                    deck.pages[self.page].elements.push(copy);
                }
            }
            Action::RemoveElement => {
                if let Some(id) = self.selected.take() {
                    deck.pages[self.page].elements.retain(|e| e.id != id);
                    self.editing = None;
                }
            }
            Action::Order(order) => {
                let elements = &mut deck.pages[self.page].elements;
                if let Some(i) = self.selected.as_ref().and_then(|id| elements.iter().position(|e| &e.id == id)) {
                    let last = elements.len() - 1;
                    match order {
                        Order::Forward if i < last => elements.swap(i, i + 1),
                        Order::Backward if i > 0 => elements.swap(i, i - 1),
                        Order::Front => {
                            let e = elements.remove(i);
                            elements.push(e);
                        }
                        Order::Back => {
                            let e = elements.remove(i);
                            elements.insert(0, e);
                        }
                        _ => {}
                    }
                }
            }
            Action::Nudge(dx, dy) => {
                if let Some(e) = self.selected.as_ref().and_then(|id| deck.pages[self.page].elements.iter_mut().find(|e| &e.id == id)) {
                    e.x += dx;
                    e.y += dy;
                }
            }
            Action::Edit => {
                if self.selected.as_deref().is_some_and(|id| ui::canvas::editable(&deck.pages[self.page], id)) {
                    self.editing = self.selected.clone();
                    self.editing_fresh = true;
                }
            }
            Action::Deselect => {
                self.selected = None;
                self.editing = None;
            }
            Action::Theme => self.theme_open = !self.theme_open,
            Action::Present => {
                self.present = Some(self.page);
                self.editing = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            }
            Action::Next => {
                self.present = self.present.map(|p| (p + 1).min(deck.pages.len() - 1));
            }
            Action::Previous => {
                self.present = self.present.map(|p| p.saturating_sub(1));
            }
            Action::EndPresent => {
                if let Some(at) = self.present.take() {
                    self.page = at;
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
            Action::Palette => self.palette.toggle(),
        }
    }

    fn top(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let (can_undo, can_redo) = (self.session.can_undo(), self.session.can_redo());
        let element = self.selected.as_deref().is_some_and(|id| id != ui::canvas::TITLE) && self.view == PAGES;
        let several = self.session.deck.pages.len() > 1;
        let mut view = self.view;
        let theme_open = self.theme_open;
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                MenuBar::new().show(ui, |bar| {
                    bar.menu("File", |m| {
                        if m.item("New").key("Ctrl+N").clicked() {
                            actions.push(Action::New);
                        }
                        if m.item("Open…").key("Ctrl+O").clicked() {
                            actions.push(Action::Open);
                        }
                        if m.item("Save").key("Ctrl+S").clicked() {
                            actions.push(Action::Save);
                        }
                        if m.item("Save as…").key("Ctrl+Shift+S").clicked() {
                            actions.push(Action::SaveAs);
                        }
                        m.separator();
                        for format in Format::ALL {
                            if m.item(format!("Export {}…", format.label())).clicked() {
                                actions.push(Action::Export(format));
                            }
                        }
                    });
                    bar.menu("Edit", |m| {
                        if m.item("Undo").key("Ctrl+Z").enabled(can_undo).clicked() {
                            actions.push(Action::Undo);
                        }
                        if m.item("Redo").key("Ctrl+Shift+Z").enabled(can_redo).clicked() {
                            actions.push(Action::Redo);
                        }
                        m.separator();
                        if m.item("Duplicate").key("Ctrl+D").enabled(element).clicked() {
                            actions.push(Action::DuplicateElement);
                        }
                        if m.item("Delete").key("Delete").enabled(element).clicked() {
                            actions.push(Action::RemoveElement);
                        }
                        m.separator();
                        for (label, order) in [("Bring forward", Order::Forward), ("Send backward", Order::Backward), ("Bring to front", Order::Front), ("Send to back", Order::Back)] {
                            if m.item(label).enabled(element).clicked() {
                                actions.push(Action::Order(order));
                            }
                        }
                    });
                    bar.menu("Insert", |m| {
                        for (kind, label) in Body::KINDS.iter().zip(Body::LABELS) {
                            if m.item(label).clicked() {
                                actions.push(Action::AddElement(kind));
                            }
                        }
                    });
                    bar.menu("Page", |m| {
                        m.submenu("New page", |m| {
                            for kind in PageType::ALL {
                                if m.item(kind.label()).clicked() {
                                    actions.push(Action::AddPage(kind));
                                }
                            }
                        });
                        if m.item("Duplicate page").clicked() {
                            actions.push(Action::DuplicatePage);
                        }
                        if m.item("Remove page").enabled(several).clicked() {
                            actions.push(Action::RemovePage);
                        }
                        m.separator();
                        if m.item("Move up").clicked() {
                            actions.push(Action::MovePage(-1));
                        }
                        if m.item("Move down").clicked() {
                            actions.push(Action::MovePage(1));
                        }
                    });
                    bar.menu("View", |m| {
                        for (i, (label, key)) in [("Storyline", "Ctrl+1"), ("Action titles", "Ctrl+2"), ("Pages", "Ctrl+3")].into_iter().enumerate() {
                            if m.item(label).key(key).clicked() {
                                actions.push(Action::View(i));
                            }
                        }
                        m.separator();
                        if m.item("Theme…").clicked() {
                            actions.push(Action::Theme);
                        }
                        if m.item("Present").key("F5").clicked() {
                            actions.push(Action::Present);
                        }
                    });
                });
                ui.add_space(16.0);
                if ui.add(Toggle::new(&mut view, &VIEWS)).changed() {
                    actions.push(Action::View(view));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(Button::icon_only(glyph("keyboard")).quiet().tip("Commands").hint("Ctrl+K")).clicked() {
                        actions.push(Action::Palette);
                    }
                    if ui.add(Button::new("Present").icon(glyph("play")).quiet().hint("F5")).clicked() {
                        actions.push(Action::Present);
                    }
                    if ui.add(Button::new("Theme").quiet().selected(theme_open)).clicked() {
                        actions.push(Action::Theme);
                    }
                });
            });
            ui.add_space(4.0);
        });
    }

    fn status_line(&self, ctx: &egui::Context) {
        let n = self.session.deck.pages.len();
        let place = format!("Page {} of {n}", self.page + 1);
        let state = if self.session.dirty { "Unsaved changes" } else { "Saved" };
        let file = format!("{} · {state}", self.session.name());
        StatusLine::new("status")
            .place(place)
            .message(self.status.clone())
            .end(move |ui| {
                ui.label(file);
            })
            .show(ctx);
    }

    /// One undo step per change: when a field lets go of the keyboard or
    /// the pointer comes up.
    fn bookkeep(&mut self, ctx: &egui::Context, before: Deck) {
        if std::mem::take(&mut self.history_moved) {
            self.pending = None;
            return;
        }
        if self.session.deck != before {
            self.pending.get_or_insert(before);
            self.session.touch();
        }
        let busy = ctx.wants_keyboard_input() || ctx.input(|i| i.pointer.any_down());
        if !busy && let Some(before) = self.pending.take() {
            self.session.record(before);
        }
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        if !self.saves || !self.session.dirty || self.pending.is_some() {
            return;
        }
        let Some(at) = self.session.changed_at else { return };
        let wait = Duration::from_millis(1500);
        if at.elapsed() >= wait {
            if let Err(e) = self.session.save() {
                self.status = e;
            }
        } else {
            ctx.request_repaint_after(wait - at.elapsed());
        }
    }

    fn window_title(&mut self, ctx: &egui::Context) {
        let title = format!("{} — Podium", self.session.name());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

/// A deck's name as a file name.
fn file_name(name: &str) -> String {
    let name: String = name.chars().filter(|c| !"/\\:*?\"<>|".contains(*c)).take(60).collect();
    if name.trim().is_empty() { "Untitled".into() } else { name.trim().to_string() }
}

impl eframe::App for PodiumApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}

impl PodiumApp {
    /// One frame of the window.
    pub fn frame(&mut self, ctx: &egui::Context) {
        if let Some(palette) = self.theme_watcher.changed(ctx) {
            ctx.set_visuals(kit_theme::visuals(&palette));
        }
        self.serve(ctx);
        self.clamp();
        let before = self.session.deck.clone();
        let mut actions = self.keys(ctx);
        if self.present.is_some() {
            ui::present::show(self, ctx, &mut actions);
        } else {
            self.top(ctx, &mut actions);
            self.status_line(ctx);
            match self.view {
                STORYLINE => ui::storyline::show(self, ctx),
                TITLES => ui::titles::show(self, ctx, &mut actions),
                _ => ui::pages::show(self, ctx, &mut actions),
            }
            if self.theme_open {
                ui::theme_dialog::show(self, ctx);
            }
            let choices = self.choices();
            let shown: Vec<Choice> = choices.iter().map(|(c, _)| c.clone()).collect();
            if let Some(i) = self.palette.show(ctx, "Ctrl+K", &shown) {
                actions.push(choices[i].1.clone());
            }
        }
        for action in actions {
            self.run(ctx, action);
        }
        self.clamp();
        self.bookkeep(ctx, before);
        self.autosave(ctx);
        self.window_title(ctx);
    }
}

impl Drop for PodiumApp {
    fn drop(&mut self) {
        if self.saves && self.session.dirty {
            let _ = self.session.save();
        }
    }
}
