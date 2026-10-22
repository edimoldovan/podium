//! The MCP tools: what Claude does to the deck in the window. Each call runs
//! on the window's thread, against the deck on screen, as one undo step.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{Map, Value, json};

use crate::deck::{Body, Deck, Element, PageType};
use crate::export::{self, Format};
use crate::render;
use crate::session::Session;
use crate::theme;

/// What a call comes to: the reply, and where the window should look.
#[derive(Default)]
pub struct Outcome {
    pub text: String,
    pub png: Option<Vec<u8>>,
    /// The view to show: 0 storyline, 1 titles, 2 pages.
    pub view: Option<usize>,
    pub page: Option<usize>,
    /// The deck was replaced: a new one, or another file.
    pub replaced: bool,
}

impl Outcome {
    fn text(text: impl Into<String>) -> Outcome {
        Outcome { text: text.into(), ..Default::default() }
    }

    fn on(mut self, view: usize, page: Option<usize>) -> Outcome {
        self.view = Some(view);
        self.page = page;
        self
    }
}

/// The reply as MCP's `tools/call` result.
pub fn reply(result: Result<Outcome, String>) -> Value {
    match result {
        Ok(outcome) => {
            let mut content = vec![json!({"type": "text", "text": outcome.text})];
            if let Some(png) = outcome.png {
                content.push(json!({"type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png"}));
            }
            json!({"content": content, "isError": false})
        }
        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
    }
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required},
    })
}

fn element_fields() -> Map<String, Value> {
    let fields = json!({
        "kind": {"type": "string", "enum": Body::KINDS},
        "x": {"type": "number", "description": "Left, in page units (the page is 960 × 540)"},
        "y": {"type": "number", "description": "Top, in page units"},
        "w": {"type": "number"},
        "h": {"type": "number"},
        "rotation": {"type": "number", "description": "Degrees, clockwise"},
        "text": {"type": "string", "description": "text, callout, source, footnote"},
        "items": {"type": "array", "items": {"type": "string"}, "description": "bullets: one point each"},
        "rows": {"type": "array", "items": {"type": "array", "items": {"type": "string"}}, "description": "table: the first row is the header"},
        "path": {"type": "string", "description": "image: an absolute path to a PNG or JPEG"},
        "chart": {"type": "string", "enum": ["bar", "stacked-bar", "line", "waterfall"], "description": "A waterfall's first and last values are totals, the ones between changes"},
        "categories": {"type": "array", "items": {"type": "string"}},
        "series": {"type": "array", "items": {"type": "object", "properties": {"name": {"type": "string"}, "values": {"type": "array", "items": {"type": "number"}}}}},
    });
    fields.as_object().cloned().unwrap_or_default()
}

fn with(mut base: Map<String, Value>, more: Value) -> Value {
    if let Value::Object(more) = more {
        base.extend(more);
    }
    Value::Object(base)
}

const PAGE: &str = "A page's id (\"p3\") or its number, from 1";

/// The tools, as `tools/list` gives them.
pub fn list() -> Value {
    let page_types: Vec<String> = PageType::ALL.iter().map(|t| t.key()).collect();
    let themes: Vec<&str> = theme::THEMES.iter().map(|t| t.name).collect();
    json!([
        tool("podium_deck", "The deck: its file, storyline, chapters and every page with its type, action title, elements and notes.", json!({}), &[]),
        tool("podium_storyline", "Sets the storyline — what the presentation needs to do — and its audience.", json!({"text": {"type": "string"}, "audience": {"type": "string"}}), &[]),
        tool("podium_chapter_add", "Adds a chapter; returns its id.", json!({"name": {"type": "string"}}), &["name"]),
        tool("podium_chapter_set", "Renames a chapter.", json!({"chapter": {"type": "string"}, "name": {"type": "string"}}), &["chapter", "name"]),
        tool("podium_chapter_remove", "Removes a chapter; its pages stay, in no chapter.", json!({"chapter": {"type": "string"}}), &["chapter"]),
        tool(
            "podium_page_add",
            "Adds a page with its action title, starting with its type's placeholders; returns its id. At the end unless `at` (its number, from 1) is given.",
            json!({"type": {"type": "string", "enum": page_types}, "title": {"type": "string"}, "chapter": {"type": "string", "description": "A chapter's id"}, "at": {"type": "integer"}, "placeholders": {"type": "boolean", "description": "Start with the type's placeholders (default true)"}}),
            &["type", "title"]
        ),
        tool(
            "podium_page_set",
            "Changes a page: its action title, type, chapter (empty for none) or speaker notes.",
            json!({"page": {"description": PAGE}, "title": {"type": "string"}, "type": {"type": "string", "enum": page_types}, "chapter": {"type": "string"}, "notes": {"type": "string"}}),
            &["page"]
        ),
        tool("podium_page_move", "Moves a page to another place, its number from 1.", json!({"page": {"description": PAGE}, "to": {"type": "integer"}}), &["page", "to"]),
        tool("podium_page_remove", "Removes a page.", json!({"page": {"description": PAGE}}), &["page"]),
        tool(
            "podium_element_add",
            "Adds an element to a page: what supports its action title. Unset fields take the kind's defaults; returns its id.",
            with(element_fields(), json!({"page": {"description": PAGE}})),
            &["page", "kind"]
        ),
        tool(
            "podium_element_set",
            "Changes an element: only the fields given.",
            with(element_fields(), json!({"page": {"description": PAGE}, "element": {"type": "string"}})),
            &["page", "element"]
        ),
        tool("podium_element_remove", "Removes an element from a page.", json!({"page": {"description": PAGE}, "element": {"type": "string"}}), &["page", "element"]),
        tool(
            "podium_theme",
            "Sets the theme and its settings: paper, ink, accent, muted (#rrggbb), title_font, body_font. Choosing a theme clears the settings; an empty setting goes back to the theme's own.",
            json!({"name": {"type": "string", "enum": themes}, "settings": {"type": "object", "additionalProperties": {"type": "string"}}}),
            &[]
        ),
        tool("podium_look", "Shows a page as the audience will see it, and puts it on the user's screen.", json!({"page": {"description": PAGE}}), &["page"]),
        tool("podium_export", "Writes the deck as PDF, PowerPoint (pptx) or one HTML file.", json!({"format": {"type": "string", "enum": ["pdf", "pptx", "html"]}, "path": {"type": "string"}}), &["format", "path"]),
        tool("podium_open", "Opens a .podium file in the window, after keeping the deck that was there.", json!({"path": {"type": "string"}}), &["path"]),
        tool("podium_new", "Starts a new deck, after keeping the one that was there.", json!({}), &[]),
        tool("podium_save", "Saves the deck; with a path, under that name from now on.", json!({"path": {"type": "string"}}), &[]),
        tool("podium_undo", "Takes back the last change, the user's or Claude's.", json!({}), &[]),
        tool("podium_redo", "Puts back what was taken back.", json!({}), &[]),
    ])
}

fn string(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(String::from)
}

fn need(args: &Value, key: &str) -> Result<String, String> {
    string(args, key).ok_or_else(|| format!("`{key}` is missing"))
}

/// A page named by id or by number from 1.
fn page_of(deck: &Deck, args: &Value) -> Result<usize, String> {
    let page = args.get("page").ok_or("`page` is missing")?;
    let found = match page {
        Value::Number(n) => n.as_u64().and_then(|n| (n as usize).checked_sub(1)).filter(|i| *i < deck.pages.len()),
        Value::String(s) => deck.page_index(s).or_else(|| s.parse::<usize>().ok().and_then(|n| n.checked_sub(1)).filter(|i| *i < deck.pages.len())),
        _ => None,
    };
    found.ok_or_else(|| format!("there is no page {page}"))
}

fn page_type(name: &str) -> Result<PageType, String> {
    serde_json::from_value(json!(name)).map_err(|_| format!("there is no page type \"{name}\""))
}

fn chapter_of(deck: &Deck, args: &Value) -> Result<Option<Option<String>>, String> {
    match args.get("chapter") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(s)) if s.is_empty() => Ok(Some(None)),
        Some(Value::String(s)) => match deck.chapters.iter().find(|c| &c.id == s || &c.name == s) {
            Some(c) => Ok(Some(Some(c.id.clone()))),
            None => Err(format!("there is no chapter \"{s}\"")),
        },
        Some(other) => Err(format!("{other} is not a chapter")),
    }
}

const FIELDS: [&str; 13] = ["kind", "x", "y", "w", "h", "rotation", "text", "items", "rows", "path", "chart", "categories", "series"];

/// An element with the given fields laid over it.
fn overlaid(element: &Element, args: &Value) -> Result<Element, String> {
    let mut value = serde_json::to_value(element).map_err(|e| e.to_string())?;
    for key in FIELDS {
        if let Some(v) = args.get(key) {
            value[key] = v.clone();
        }
    }
    serde_json::from_value(value).map_err(|e| format!("not an element: {e}"))
}

/// A path as given, or beside the deck when it is relative.
fn resolved(session: &Session, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return path;
    }
    let base = session.path.as_deref().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_else(|| std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default());
    base.join(path)
}

/// Runs one tool against the session.
pub fn call(session: &mut Session, name: &str, args: &Value) -> Result<Outcome, String> {
    let args = if args.is_null() { &json!({}) } else { args };
    match name {
        "podium_deck" => {
            let page_types: Vec<String> = PageType::ALL.iter().map(|t| t.key()).collect();
            let summary = json!({
                "file": session.file(),
                "page": {"width": 960, "height": 540},
                "page_types": page_types,
                "element_kinds": Body::KINDS,
                "themes": theme::THEMES.iter().map(|t| t.name).collect::<Vec<_>>(),
                "deck": session.deck,
            });
            Ok(Outcome::text(serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())?))
        }
        "podium_storyline" => {
            session.change(|deck| {
                if let Some(text) = string(args, "text") {
                    deck.storyline.text = text;
                }
                if let Some(audience) = string(args, "audience") {
                    deck.storyline.audience = audience;
                }
                Ok(())
            })?;
            Ok(Outcome::text("The storyline is set.").on(0, None))
        }
        "podium_chapter_add" => {
            let name = need(args, "name")?;
            let id = session.change(|deck| Ok(deck.add_chapter(&name)))?;
            Ok(Outcome::text(format!("Chapter {id} added.")).on(1, None))
        }
        "podium_chapter_set" => {
            let (id, name) = (need(args, "chapter")?, need(args, "name")?);
            session.change(|deck| {
                let chapter = deck.chapters.iter_mut().find(|c| c.id == id).ok_or(format!("there is no chapter \"{id}\""))?;
                chapter.name = name;
                Ok(())
            })?;
            Ok(Outcome::text("Renamed.").on(1, None))
        }
        "podium_chapter_remove" => {
            let id = need(args, "chapter")?;
            session.change(|deck| {
                if !deck.chapters.iter().any(|c| c.id == id) {
                    return Err(format!("there is no chapter \"{id}\""));
                }
                deck.remove_chapter(&id);
                Ok(())
            })?;
            Ok(Outcome::text("Removed.").on(1, None))
        }
        "podium_page_add" => {
            let kind = page_type(&need(args, "type")?)?;
            let title = need(args, "title")?;
            let (id, index) = session.change(|deck| {
                let chapter = chapter_of(deck, args)?.flatten();
                let at = args.get("at").and_then(Value::as_u64).map(|n| (n as usize).saturating_sub(1));
                let id = deck.add_page(kind, &title, chapter, at);
                let index = deck.page_index(&id).unwrap_or(0);
                if args.get("placeholders").and_then(Value::as_bool) == Some(false) {
                    deck.pages[index].elements.clear();
                }
                Ok((id, index))
            })?;
            let elements: Vec<String> = session.deck.pages[index].elements.iter().map(|e| format!("{} ({})", e.id, e.body.kind())).collect();
            Ok(Outcome::text(format!("Page {id} added as page {}; its placeholders: {}.", index + 1, if elements.is_empty() { "none".into() } else { elements.join(", ") })).on(1, Some(index)))
        }
        "podium_page_set" => {
            let index = session.change(|deck| {
                let index = page_of(deck, args)?;
                let chapter = chapter_of(deck, args)?;
                let kind = string(args, "type").map(|t| page_type(&t)).transpose()?;
                let page = &mut deck.pages[index];
                if let Some(title) = string(args, "title") {
                    page.title = title;
                }
                if let Some(kind) = kind {
                    page.kind = kind;
                }
                if let Some(chapter) = chapter {
                    page.chapter = chapter;
                }
                if let Some(notes) = string(args, "notes") {
                    page.notes = notes;
                }
                Ok(index)
            })?;
            Ok(Outcome::text("Changed.").on(1, Some(index)))
        }
        "podium_page_move" => {
            let to = args.get("to").and_then(Value::as_u64).ok_or("`to` is missing")? as usize;
            let index = session.change(|deck| {
                let from = page_of(deck, args)?;
                let page = deck.pages.remove(from);
                let at = to.saturating_sub(1).min(deck.pages.len());
                deck.pages.insert(at, page);
                Ok(at)
            })?;
            Ok(Outcome::text(format!("Now page {}.", index + 1)).on(1, Some(index)))
        }
        "podium_page_remove" => {
            let index = session.change(|deck| {
                let index = page_of(deck, args)?;
                if deck.pages.len() == 1 {
                    return Err("a deck keeps at least one page".into());
                }
                deck.pages.remove(index);
                Ok(index)
            })?;
            Ok(Outcome::text("Removed.").on(1, Some(index.saturating_sub(1))))
        }
        "podium_element_add" => {
            let kind = need(args, "kind")?;
            let (index, id) = session.change(|deck| {
                let index = page_of(deck, args)?;
                let (rect, body) = Body::new(&kind).ok_or(format!("there is no element kind \"{kind}\""))?;
                let id = deck.fresh_element_id(&deck.pages[index]);
                let base = Element { id: id.clone(), x: rect.x, y: rect.y, w: rect.w, h: rect.h, rotation: 0.0, body };
                let element = overlaid(&base, args)?;
                deck.pages[index].elements.push(element);
                Ok((index, id))
            })?;
            Ok(Outcome::text(format!("Element {id} added.")).on(2, Some(index)))
        }
        "podium_element_set" => {
            let id = need(args, "element")?;
            let index = session.change(|deck| {
                let index = page_of(deck, args)?;
                let element = deck.pages[index].elements.iter_mut().find(|e| e.id == id).ok_or(format!("there is no element \"{id}\" on that page"))?;
                *element = overlaid(element, args)?;
                Ok(index)
            })?;
            Ok(Outcome::text("Changed.").on(2, Some(index)))
        }
        "podium_element_remove" => {
            let id = need(args, "element")?;
            let index = session.change(|deck| {
                let index = page_of(deck, args)?;
                let elements = &mut deck.pages[index].elements;
                let before = elements.len();
                elements.retain(|e| e.id != id);
                if elements.len() == before {
                    return Err(format!("there is no element \"{id}\" on that page"));
                }
                Ok(index)
            })?;
            Ok(Outcome::text("Removed.").on(2, Some(index)))
        }
        "podium_theme" => {
            session.change(|deck| {
                if let Some(name) = string(args, "name") {
                    if !theme::THEMES.iter().any(|t| t.name == name) {
                        return Err(format!("there is no theme \"{name}\""));
                    }
                    deck.theme.name = name;
                    deck.theme.settings.clear();
                }
                if let Some(Value::Object(settings)) = args.get("settings") {
                    for (key, value) in settings {
                        let known = theme::COLOURS.iter().chain(theme::FONTS.iter()).any(|(k, _)| k == key);
                        if !known {
                            return Err(format!("there is no setting \"{key}\""));
                        }
                        match value.as_str() {
                            Some("") | None => deck.theme.settings.remove(key),
                            Some(v) => deck.theme.settings.insert(key.clone(), v.to_string()),
                        };
                    }
                }
                Ok(())
            })?;
            Ok(Outcome::text("The theme is set.").on(2, None))
        }
        "podium_look" => {
            let index = page_of(&session.deck, args)?;
            let png = render::png(&render::page_document(&session.deck, index), 1.0);
            Ok(Outcome { text: format!("Page {} of {}.", index + 1, session.deck.pages.len()), png: Some(png), ..Default::default() }.on(2, Some(index)))
        }
        "podium_export" => {
            let format = Format::named(&need(args, "format")?).ok_or("the format is pdf, pptx or html")?;
            let path = resolved(session, &need(args, "path")?);
            export::export(&session.deck, format, &path)?;
            Ok(Outcome::text(format!("Written to {}.", path.display())))
        }
        "podium_open" => {
            let path = resolved(session, &need(args, "path")?);
            session.open(path.clone())?;
            Ok(Outcome { text: format!("Opened {}.", path.display()), replaced: true, ..Default::default() }.on(1, Some(0)))
        }
        "podium_new" => {
            session.new_deck()?;
            Ok(Outcome { text: "A new deck.".into(), replaced: true, ..Default::default() }.on(0, Some(0)))
        }
        "podium_save" => {
            match string(args, "path") {
                Some(path) => session.save_as(resolved(session, &path))?,
                None => session.save()?,
            }
            Ok(Outcome::text(format!("Saved to {}.", session.file().display())))
        }
        "podium_undo" => Ok(Outcome::text(if session.undo() { "Taken back." } else { "Nothing to take back." })),
        "podium_redo" => Ok(Outcome::text(if session.redo() { "Put back." } else { "Nothing to put back." })),
        _ => Err(format!("there is no tool \"{name}\"")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(session: &mut Session, name: &str, args: Value) -> String {
        match call(session, name, &args) {
            Ok(o) => o.text,
            Err(e) => panic!("{name}: {e}"),
        }
    }

    /// The flow through the tools: storyline, chapter, titled page, details.
    #[test]
    fn a_deck_is_built_through_the_tools() {
        let mut s = Session::new(Deck::new(), None);
        run(&mut s, "podium_storyline", json!({"text": "Win the board's approval", "audience": "Board"}));
        run(&mut s, "podium_chapter_add", json!({"name": "Market"}));
        let added = run(&mut s, "podium_page_add", json!({"type": "chart-takeaway", "title": "Demand doubles by 2028", "chapter": "c1"}));
        assert!(added.contains("page 3"), "{added}");
        let chart = s.deck.pages[2].elements.iter().find(|e| e.body.kind() == "chart").unwrap().id.clone();
        run(&mut s, "podium_element_set", json!({"page": 3, "element": chart, "chart": "waterfall", "categories": ["2024", "Growth", "2028"], "series": [{"name": "k", "values": [10, 5, 15]}], "rotation": 3}));
        let e = s.deck.pages[2].elements.iter().find(|e| e.id == chart).unwrap();
        assert_eq!(e.rotation, 3.0);
        assert!(matches!(&e.body, Body::Chart { chart: crate::deck::ChartKind::Waterfall, categories, .. } if categories.len() == 3));
        run(&mut s, "podium_element_add", json!({"page": "p3", "kind": "footnote", "text": "1. Made up"}));
        assert_eq!(s.deck.pages[2].chapter.as_deref(), Some("c1"));
        assert!(call(&mut s, "podium_page_set", &json!({"page": 9})).is_err());
        run(&mut s, "podium_undo", json!({}));
        assert!(!s.deck.pages[2].elements.iter().any(|e| e.body.kind() == "footnote"));
    }

    /// Every tool listed is one the window answers.
    #[test]
    fn every_tool_listed_is_answered() {
        for t in list().as_array().unwrap() {
            let name = t["name"].as_str().unwrap();
            // These reach the user's files.
            if ["podium_new", "podium_save", "podium_open", "podium_export"].contains(&name) {
                continue;
            }
            let mut s = Session::new(Deck::new(), None);
            if let Err(e) = call(&mut s, name, &json!({})) {
                assert!(!e.starts_with("there is no tool"), "{name}");
            }
        }
    }
}
