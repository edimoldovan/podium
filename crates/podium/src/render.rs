//! Pages as HTML, and the HTML drawn by Blitz: to pixels for the window,
//! to a recorded scene for the PDF.
//!
//! Each element is a semantic element of its own, absolutely placed at its
//! page units and turned by a CSS transform; the theme's CSS does the rest.

use std::sync::Arc;

use anyrender::recording::Scene;
use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use base64::Engine as _;
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request, Url};
use blitz_traits::shell::{ColorScheme, Viewport};

use crate::chart;
use crate::deck::{Body, Deck, Element, HEIGHT, WIDTH};
use crate::theme;

/// How an image element refers to its picture.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pictures {
    /// By its file: what the window draws.
    Linked,
    /// Its bytes in the page: a file that travels on its own.
    Inline,
}

/// One page as a whole HTML document.
pub fn page_document(deck: &Deck, index: usize) -> String {
    document(&theme::css(&deck.theme), &page(deck, index, Pictures::Linked), &deck.name())
}

pub fn document(css: &str, body: &str, title: &str) -> String {
    format!("<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>{css}</style></head><body>{body}</body></html>", esc(title))
}

/// One page's HTML: its chapter, its action title, its elements.
pub fn page(deck: &Deck, index: usize, pictures: Pictures) -> String {
    let page = &deck.pages[index];
    let look = theme::look(&deck.theme);
    let mut out = format!(r#"<div class="page" data-type="{}">"#, page.kind.key());
    if page.kind.tagged()
        && let Some(name) = page.chapter.as_deref().and_then(|id| deck.chapter_name(id))
    {
        out += &format!(r#"<header style="left:48px;top:24px;width:600px;height:14px">{}</header>"#, esc(name));
    }
    let t = page.kind.title_rect();
    out += &format!(r#"<h1 style="left:{}px;top:{}px;width:{}px;height:{}px">{}</h1>"#, t.x, t.y, t.w, t.h, esc(&page.title));
    for element in &page.elements {
        out += &self::element(element, &look, pictures);
    }
    out + "</div>"
}

fn element(e: &Element, look: &theme::Look, pictures: Pictures) -> String {
    let turn = if e.rotation == 0.0 { String::new() } else { format!(";transform:rotate({}deg)", e.rotation) };
    let style = format!("left:{}px;top:{}px;width:{}px;height:{}px{turn}", e.x, e.y, e.w, e.h);
    match &e.body {
        Body::Text { text } => format!(r#"<p style="{style}">{}</p>"#, esc(text)),
        Body::Bullets { items } => {
            let items: String = items.iter().map(|i| format!("<li>{}</li>", esc(i))).collect();
            format!(r#"<ul style="{style}">{items}</ul>"#)
        }
        Body::Chart { chart, categories, series } => format!(r#"<figure style="{style}">{}</figure>"#, chart::svg(*chart, categories, series, e.w, e.h, look)),
        Body::Table { rows } => {
            let mut table = format!(r#"<table style="{style}">"#);
            if let Some((head, body)) = rows.split_first() {
                table += "<thead><tr>";
                for cell in head {
                    table += &format!("<th>{}</th>", esc(cell));
                }
                table += "</tr></thead><tbody>";
                for row in body {
                    table += "<tr>";
                    for cell in row {
                        table += &format!("<td>{}</td>", esc(cell));
                    }
                    table += "</tr>";
                }
                table += "</tbody>";
            }
            table + "</table>"
        }
        Body::Image { path } => {
            let src = match pictures {
                Pictures::Linked => Url::from_file_path(path).map(|u| u.to_string()).unwrap_or_default(),
                Pictures::Inline => inline(path).unwrap_or_default(),
            };
            format!(r#"<img style="{style}" src="{}">"#, esc(&src))
        }
        Body::Callout { text } => format!(r#"<aside style="{style}">{}</aside>"#, esc(text)),
        Body::Source { text } => format!(r#"<footer style="{style}">{}</footer>"#, esc(text)),
        Body::Footnote { text } => format!(r#"<small style="{style}">{}</small>"#, esc(text)),
    }
}

/// A picture file as a `data:` URL.
fn inline(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let kind = if bytes.starts_with(b"\x89PNG") { "png" } else { "jpeg" };
    Some(format!("data:image/{kind};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

pub fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Hands Blitz the picture files a page names, as it asks for them.
struct Files;

impl NetProvider for Files {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url;
        let bytes = match url.scheme() {
            "file" => url.to_file_path().ok().and_then(|p| std::fs::read(p).ok()),
            "data" => url
                .as_str()
                .split_once(";base64,")
                .and_then(|(_, data)| base64::engine::general_purpose::STANDARD.decode(data).ok()),
            _ => None,
        };
        if let Some(bytes) = bytes {
            handler.bytes(url.to_string(), Bytes::from(bytes));
        }
    }
}

fn laid_out(html: &str, scale: f32) -> (HtmlDocument, u32, u32) {
    let (w, h) = ((WIDTH * scale).round() as u32, (HEIGHT * scale).round() as u32);
    let mut doc = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(w, h, scale, ColorScheme::Light)),
            net_provider: Some(Arc::new(Files)),
            ..Default::default()
        },
    );
    doc.as_mut().resolve(0.0);
    (doc, w, h)
}

/// A page document drawn at `scale` pixels a unit: width, height, RGBA.
pub fn pixels(html: &str, scale: f32) -> (usize, usize, Vec<u8>) {
    let (mut doc, w, h) = laid_out(html, scale);
    let pixels = render_to_buffer::<VelloCpuImageRenderer, _>(|scene| paint_scene(scene, doc.as_mut(), scale as f64, w, h, 0, 0), w, h);
    (w as usize, h as usize, pixels)
}

/// A page document as a PNG.
pub fn png(html: &str, scale: f32) -> Vec<u8> {
    let (w, h, rgba) = pixels(html, scale);
    let mut out = Vec::new();
    let image = image::RgbaImage::from_raw(w as u32, h as u32, rgba).expect("the buffer is the image's size");
    image.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).expect("a PNG in memory");
    out
}

/// A page document as a recorded scene, one unit a point.
pub fn scene(html: &str) -> Scene {
    let (mut doc, w, h) = laid_out(html, 1.0);
    let mut scene = Scene::new();
    paint_scene(&mut scene, doc.as_mut(), 1.0, w, h, 0, 0);
    scene
}
