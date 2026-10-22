//! The page being worked on, drawn as the audience will see it, and edited
//! as in PowerPoint: a click selects, a drag moves, the handles resize — with
//! Shift on a corner, keeping the proportions — and the knob above turns.
//! Edges and centres snap to the page's guides and to the other elements;
//! Alt drags free. A double click types the text in place.

use eframe::egui::{self, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, TextEdit, Vec2, pos2, vec2};
use omarchy_kit::context_menu;

use crate::app::{Action, Order, PodiumApp};
use crate::deck::{BODY_TOP, Body, Element, HEIGHT, MARGIN, Page, PageType, WIDTH};
use crate::render;
use crate::theme;
use crate::ui::colour;

/// The selection when it is the page's title, which stays where the page
/// type puts it.
pub const TITLE: &str = "title";

#[derive(Clone)]
pub struct Drag {
    id: String,
    grip: Grip,
    start: Pos2,
    original: Element,
}

#[derive(Clone, Copy, PartialEq)]
enum Grip {
    Move,
    Resize(f32, f32),
    Rotate,
}

/// A line an edge or centre snapped to, while dragging.
#[derive(Clone, Copy)]
pub enum Guide {
    X(f32),
    Y(f32),
}

/// Where the page is on screen.
struct View {
    rect: Rect,
    zoom: f32,
}

impl View {
    fn to_page(&self, p: Pos2) -> Pos2 {
        ((p - self.rect.min) / self.zoom).to_pos2()
    }

    fn to_screen(&self, p: Pos2) -> Pos2 {
        self.rect.min + p.to_vec2() * self.zoom
    }
}

const HANDLES: [(f32, f32); 8] = [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0)];
const KNOB: f32 = 24.0;

fn rotate(v: Vec2, degrees: f32) -> Vec2 {
    let (s, c) = degrees.to_radians().sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

fn center(e: &Element) -> Pos2 {
    pos2(e.x + e.w / 2.0, e.y + e.h / 2.0)
}

fn contains(e: &Element, p: Pos2) -> bool {
    let local = rotate(p - center(e), -e.rotation);
    local.x.abs() <= e.w / 2.0 && local.y.abs() <= e.h / 2.0
}

/// A point on an element's outline: −1, 0 or 1 across and down.
fn corner(e: &Element, sx: f32, sy: f32) -> Pos2 {
    center(e) + rotate(vec2(sx * e.w / 2.0, sy * e.h / 2.0), e.rotation)
}

fn knob(e: &Element, zoom: f32) -> Pos2 {
    center(e) + rotate(vec2(0.0, -e.h / 2.0 - KNOB / zoom), e.rotation)
}

/// The page's title as an element, for selecting and typing it.
fn title_element(page: &Page) -> Element {
    let r = page.kind.title_rect();
    Element { id: TITLE.into(), x: r.x, y: r.y, w: r.w, h: r.h, rotation: 0.0, body: Body::Text { text: page.title.clone() } }
}

/// What is under a point: the topmost element, else the title.
fn hit(page: &Page, p: Pos2) -> Option<String> {
    page.elements
        .iter()
        .rev()
        .find(|e| contains(e, p))
        .map(|e| e.id.clone())
        .or_else(|| contains(&title_element(page), p).then(|| TITLE.to_string()))
}

fn grip_at(e: &Element, view: &View, pointer: Pos2) -> Option<Grip> {
    let near = |p: Pos2| view.to_screen(p).distance(pointer) <= 7.0;
    if near(knob(e, view.zoom)) {
        return Some(Grip::Rotate);
    }
    HANDLES.iter().find(|(sx, sy)| near(corner(e, *sx, *sy))).map(|(sx, sy)| Grip::Resize(*sx, *sy))
}

/// Whether an element's text is typed on the canvas.
pub fn editable(page: &Page, id: &str) -> bool {
    id == TITLE || page.elements.iter().any(|e| e.id == id && matches!(e.body, Body::Text { .. } | Body::Bullets { .. } | Body::Callout { .. } | Body::Source { .. } | Body::Footnote { .. }))
}

/// The nearest of `points` to any of `lines`, within reach: how far to
/// move, and the line.
fn nearest(points: &[f32], lines: &[f32], reach: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for p in points {
        for line in lines {
            let d = line - p;
            if d.abs() <= reach && best.is_none_or(|(b, _)| d.abs() < b.abs()) {
                best = Some((d, *line));
            }
        }
    }
    best
}

/// An element as a drag leaves it, with the lines it snapped to.
fn dragged(drag: &Drag, p: Pos2, shift: bool, snap: bool, others: &[Element], zoom: f32) -> (Element, Vec<Guide>) {
    let o = &drag.original;
    let mut e = o.clone();
    let mut guides = Vec::new();
    let reach = 6.0 / zoom;
    let xs: Vec<f32> = [MARGIN, WIDTH / 2.0, WIDTH - MARGIN].into_iter().chain(others.iter().flat_map(|e| [e.x, e.x + e.w / 2.0, e.x + e.w])).collect();
    let ys: Vec<f32> = [BODY_TOP, HEIGHT / 2.0, 470.0, 498.0].into_iter().chain(others.iter().flat_map(|e| [e.y, e.y + e.h / 2.0, e.y + e.h])).collect();
    match drag.grip {
        Grip::Move => {
            let d = p - drag.start;
            e.x = o.x + d.x;
            e.y = o.y + d.y;
            if shift {
                if d.x.abs() > d.y.abs() {
                    e.y = o.y;
                } else {
                    e.x = o.x;
                }
            }
            if snap {
                if let Some((d, at)) = nearest(&[e.x, e.x + e.w / 2.0, e.x + e.w], &xs, reach) {
                    e.x += d;
                    guides.push(Guide::X(at));
                }
                if let Some((d, at)) = nearest(&[e.y, e.y + e.h / 2.0, e.y + e.h], &ys, reach) {
                    e.y += d;
                    guides.push(Guide::Y(at));
                }
            }
        }
        Grip::Resize(sx, sy) => {
            let c = center(o);
            let q = rotate(p - c, -o.rotation);
            let (mut left, mut right, mut top, mut bottom) = (-o.w / 2.0, o.w / 2.0, -o.h / 2.0, o.h / 2.0);
            if sx < 0.0 {
                left = q.x.min(right - 8.0);
            } else if sx > 0.0 {
                right = q.x.max(left + 8.0);
            }
            if sy < 0.0 {
                top = q.y.min(bottom - 8.0);
            } else if sy > 0.0 {
                bottom = q.y.max(top + 8.0);
            }
            if shift && sx != 0.0 && sy != 0.0 {
                // Scaled: both sides by the same factor, from the far corner.
                let s = ((right - left) / o.w).max((bottom - top) / o.h);
                let (w, h) = (o.w * s, o.h * s);
                if sx < 0.0 { left = right - w } else { right = left + w }
                if sy < 0.0 { top = bottom - h } else { bottom = top + h }
            } else if snap && o.rotation == 0.0 {
                let edge = |side: f32, at: f32, origin: f32, lines: &[f32], guides: &mut Vec<Guide>, x: bool| -> f32 {
                    if side == 0.0 {
                        return at;
                    }
                    match nearest(&[origin + at], lines, reach) {
                        Some((d, line)) => {
                            guides.push(if x { Guide::X(line) } else { Guide::Y(line) });
                            at + d
                        }
                        None => at,
                    }
                };
                if sx < 0.0 {
                    left = edge(sx, left, c.x, &xs, &mut guides, true);
                } else {
                    right = edge(sx, right, c.x, &xs, &mut guides, true);
                }
                if sy < 0.0 {
                    top = edge(sy, top, c.y, &ys, &mut guides, false);
                } else {
                    bottom = edge(sy, bottom, c.y, &ys, &mut guides, false);
                }
            }
            let middle = c + rotate(vec2((left + right) / 2.0, (top + bottom) / 2.0), o.rotation);
            e.w = right - left;
            e.h = bottom - top;
            e.x = middle.x - e.w / 2.0;
            e.y = middle.y - e.h / 2.0;
        }
        Grip::Rotate => {
            let v = p - center(o);
            let mut a = v.y.atan2(v.x).to_degrees() + 90.0;
            if a > 180.0 {
                a -= 360.0;
            }
            if shift {
                a = (a / 15.0).round() * 15.0;
            } else if snap {
                let right = (a / 90.0).round() * 90.0;
                if (a - right).abs() < 4.0 {
                    a = right;
                }
            }
            if a <= -180.0 {
                a += 360.0;
            }
            e.rotation = (a * 10.0).round() / 10.0 + 0.0;
        }
    }
    let half = |v: f32| (v * 2.0).round() / 2.0;
    (e.x, e.y, e.w, e.h) = (half(e.x), half(e.y), half(e.w), half(e.h));
    (e, guides)
}

pub fn show(app: &mut PodiumApp, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    let ctx = ui.ctx().clone();
    let area = ui.available_rect_before_wrap();
    let zoom = ((area.width() - 48.0) / WIDTH).min((area.height() - 48.0) / HEIGHT).max(0.05);
    let view = View { rect: Rect::from_center_size(area.center(), vec2(WIDTH * zoom, HEIGHT * zoom)), zoom };
    let scale = ((zoom * ctx.pixels_per_point()) * 20.0).round().max(2.0) / 20.0;
    let index = app.page;
    let html = render::page_document(&app.session.deck, index);
    let texture = app.textures.get(&ctx, "canvas", &html, scale);
    let accent = ui.visuals().hyperlink_color;
    let painter = ui.painter_at(area);
    painter.rect_filled(view.rect.translate(vec2(0.0, 2.0)).expand(1.0), 2.0, Color32::from_black_alpha(60));
    painter.image(texture.id(), view.rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);

    let response = ui.interact(area, ui.id().with("canvas"), Sense::click_and_drag());
    let (shift, alt) = ctx.input(|i| (i.modifiers.shift, i.modifiers.alt));

    if response.drag_started()
        && let Some(origin) = ctx.input(|i| i.pointer.press_origin())
    {
        let page = &app.session.deck.pages[index];
        let p = view.to_page(origin);
        let selected = app.selected.as_ref().and_then(|id| page.elements.iter().find(|e| &e.id == id));
        let start = selected
            .and_then(|e| grip_at(e, &view, origin).map(|g| (e.clone(), g)))
            .or_else(|| hit(page, p).and_then(|id| page.elements.iter().find(|e| e.id == id)).map(|e| (e.clone(), Grip::Move)));
        if let Some((e, grip)) = start {
            app.selected = Some(e.id.clone());
            app.editing = None;
            app.drag = Some(Drag { id: e.id.clone(), grip, start: p, original: e });
        }
    }
    if response.dragged()
        && let (Some(drag), Some(pointer)) = (app.drag.clone(), response.interact_pointer_pos())
    {
        let page = &mut app.session.deck.pages[index];
        let others: Vec<Element> = page.elements.iter().filter(|e| e.id != drag.id).cloned().collect();
        let (moved, guides) = dragged(&drag, view.to_page(pointer), shift, !alt, &others, zoom);
        if let Some(e) = page.elements.iter_mut().find(|e| e.id == drag.id) {
            *e = moved;
        }
        app.guides = guides;
    }
    if response.drag_stopped() {
        app.drag = None;
        app.guides.clear();
    }

    if (response.clicked() || response.secondary_clicked())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let under = hit(&app.session.deck.pages[index], view.to_page(pointer));
        if app.editing.is_some() && app.editing != under {
            app.editing = None;
        }
        app.selected = under;
    }
    if response.double_clicked() && app.selected.as_deref().is_some_and(|id| editable(&app.session.deck.pages[index], id)) {
        app.editing = app.selected.clone();
        app.editing_fresh = true;
    }

    // The pointer's shape says what a drag would do.
    let page = &app.session.deck.pages[index];
    if app.drag.is_some() {
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    } else if let Some(pointer) = response.hover_pos() {
        let selected = app.selected.as_ref().and_then(|id| page.elements.iter().find(|e| &e.id == id));
        let icon = match selected.and_then(|e| grip_at(e, &view, pointer)) {
            Some(Grip::Rotate) => CursorIcon::Alias,
            Some(Grip::Resize(sx, sy)) => {
                if sx == 0.0 {
                    CursorIcon::ResizeVertical
                } else if sy == 0.0 {
                    CursorIcon::ResizeHorizontal
                } else if sx == sy {
                    CursorIcon::ResizeNwSe
                } else {
                    CursorIcon::ResizeNeSw
                }
            }
            Some(Grip::Move) | None => match hit(page, view.to_page(pointer)) {
                Some(id) if id == TITLE => CursorIcon::Text,
                Some(_) => CursorIcon::Grab,
                None => CursorIcon::Default,
            },
        };
        ctx.set_cursor_icon(icon);
    }

    // The selection: its outline, its handles, the knob that turns it.
    let selected = app.selected.as_deref().and_then(|id| if id == TITLE { Some(title_element(page)) } else { page.elements.iter().find(|e| e.id == id).cloned() });
    if let Some(e) = &selected {
        let outline: Vec<Pos2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().map(|(sx, sy)| view.to_screen(corner(e, *sx, *sy))).collect();
        painter.add(Shape::closed_line(outline, Stroke::new(1.5_f32, accent)));
        if e.id != TITLE && app.editing.is_none() {
            let fill = ui.visuals().panel_fill;
            let top = view.to_screen(corner(e, 0.0, -1.0));
            let turn = view.to_screen(knob(e, zoom));
            painter.line_segment([top, turn], Stroke::new(1.0_f32, accent));
            painter.circle(turn, 5.0, fill, Stroke::new(1.5_f32, accent));
            for (sx, sy) in HANDLES {
                let at = view.to_screen(corner(e, sx, sy));
                painter.rect(Rect::from_center_size(at, vec2(8.0, 8.0)), 1.0, fill, Stroke::new(1.5_f32, accent), StrokeKind::Middle);
            }
        }
    }
    for guide in &app.guides {
        let (a, b) = match *guide {
            Guide::X(x) => (pos2(x, 0.0), pos2(x, HEIGHT)),
            Guide::Y(y) => (pos2(0.0, y), pos2(WIDTH, y)),
        };
        painter.line_segment([view.to_screen(a), view.to_screen(b)], Stroke::new(1.0_f32, accent));
    }

    if let Some(id) = app.editing.clone() {
        type_in_place(app, ui, &view, &id);
    }

    let element = selected.as_ref().is_some_and(|e| e.id != TITLE);
    context_menu(&response, |m| {
        if element {
            if m.item("Duplicate").key("Ctrl+D").clicked() {
                actions.push(Action::DuplicateElement);
            }
            if m.item("Delete").key("Delete").clicked() {
                actions.push(Action::RemoveElement);
            }
            m.separator();
            for (label, order) in [("Bring forward", Order::Forward), ("Send backward", Order::Backward), ("Bring to front", Order::Front), ("Send to back", Order::Back)] {
                if m.item(label).clicked() {
                    actions.push(Action::Order(order));
                }
            }
            m.separator();
        }
        m.submenu("Insert", |m| {
            for (kind, label) in Body::KINDS.iter().zip(Body::LABELS) {
                if m.item(label).clicked() {
                    actions.push(Action::AddElement(kind));
                }
            }
        });
    });
}

/// The text of the element being edited, typed where it sits on the page.
fn type_in_place(app: &mut PodiumApp, ui: &mut egui::Ui, view: &View, id: &str) {
    let look = theme::look(&app.session.deck.theme);
    let page = &mut app.session.deck.pages[app.page];
    let (rect, size, paper, ink) = if id == TITLE {
        let r = page.kind.title_rect();
        let size = match page.kind {
            PageType::Title => 40.0,
            PageType::Divider => 36.0,
            _ => 26.0,
        };
        let (paper, ink) = if page.kind == PageType::Divider { (&look.accent, &look.paper) } else { (&look.paper, &look.ink) };
        (crate::deck::Rect { x: r.x, y: r.y, w: r.w, h: r.h }, size, colour(paper), colour(ink))
    } else {
        let Some(e) = page.elements.iter().find(|e| e.id == id) else {
            app.editing = None;
            return;
        };
        let (size, paper, ink) = match e.body {
            Body::Callout { .. } => (16.0, colour(&look.accent), colour(&look.paper)),
            Body::Source { .. } | Body::Footnote { .. } => (10.0, colour(&look.paper), colour(&look.muted)),
            _ => (16.0, colour(&look.paper), colour(&look.ink)),
        };
        (e.rect(), size, paper, ink)
    };
    let screen = Rect::from_min_size(view.to_screen(pos2(rect.x, rect.y)), vec2(rect.w, rect.h) * view.zoom);
    ui.painter().rect_filled(screen.expand(4.0), 2.0, paper);
    ui.painter().rect_stroke(screen.expand(4.0), 2.0, Stroke::new(1.5_f32, ui.visuals().hyperlink_color), StrokeKind::Outside);
    let font = FontId::proportional(size * view.zoom);
    let edit = |ui: &mut egui::Ui, text: &mut String| ui.put(screen, TextEdit::multiline(text).frame(false).font(font.clone()).text_color(ink).desired_width(screen.width()));
    let response = if id == TITLE {
        let mut text = page.title.clone();
        let r = edit(ui, &mut text);
        page.title = text.replace('\n', " ");
        r
    } else {
        let Some(e) = page.elements.iter_mut().find(|e| e.id == id) else { return };
        match &mut e.body {
            Body::Bullets { items } => {
                let mut text = items.join("\n");
                let r = edit(ui, &mut text);
                if r.changed() {
                    *items = text.split('\n').map(String::from).collect();
                }
                r
            }
            body => match body.text_mut() {
                Some(text) => edit(ui, text),
                None => return,
            },
        }
    };
    if std::mem::take(&mut app.editing_fresh) {
        response.request_focus();
    } else if response.lost_focus() {
        app.editing = None;
    }
}
