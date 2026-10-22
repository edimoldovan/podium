//! The deck as a vector PDF with real text: each page laid out by Blitz,
//! one CSS pixel a point — 960 × 540 pt, PowerPoint's 16:9 sheet.
//!
//! Blitz paints through anyrender 0.14 and pdfrum replays anyrender 0.13
//! scenes; the two recordings differ only in a fill rule on layers and
//! filters pdfrum ignores, so each scene is carried over command by command.

use anyrender::recording::RenderCommand as Command;
use anyrender_13::recording as old;
use pdfrum_anyrender::{GlyphArea, Page, Sources};
use peniko::kurbo::{Affine, Rect, Size};

use crate::deck::{Deck, HEIGHT, WIDTH};
use crate::render;

pub fn write(deck: &Deck) -> Result<Vec<u8>, String> {
    let (w, h) = (WIDTH as f64, HEIGHT as f64);
    let pages: Vec<Page> = (0..deck.pages.len())
        .map(|i| {
            let scene = render::scene(&render::page_document(deck, i));
            Page {
                size: Size::new(w, h),
                scene: anyrender_13::Scene { tolerance: scene.tolerance, commands: scene.commands.into_iter().map(carried).collect() },
                placement: Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, h]),
                clip: Rect::new(0.0, 0.0, w, h),
                area: GlyphArea::Anywhere,
            }
        })
        .collect();
    let written = pdfrum_anyrender::write(&pages, &Sources::default()).map_err(|e| format!("could not write the PDF: {e}"))?;
    Ok(written.bytes)
}

fn carried(command: Command) -> old::RenderCommand {
    match command {
        Command::PushLayer(c) => old::RenderCommand::PushLayer(old::LayerCommand {
            blend: c.blend,
            alpha: c.alpha,
            transform: c.transform,
            clip: c.clip,
            filter: None,
            backdrop_filter: None,
        }),
        Command::PushClipLayer(c) => old::RenderCommand::PushClipLayer(old::ClipCommand { transform: c.transform, clip: c.clip }),
        Command::PopLayer => old::RenderCommand::PopLayer,
        Command::Stroke(c) => old::RenderCommand::Stroke(old::StrokeCommand {
            style: c.style,
            transform: c.transform,
            brush: paint(c.brush),
            brush_transform: c.brush_transform,
            shape: c.shape,
        }),
        Command::Fill(c) => old::RenderCommand::Fill(old::FillCommand {
            fill: c.fill,
            transform: c.transform,
            brush: paint(c.brush),
            brush_transform: c.brush_transform,
            shape: c.shape,
        }),
        Command::GlyphRun(c) => old::RenderCommand::GlyphRun(old::GlyphRunCommand {
            font_data: c.font_data,
            font_size: c.font_size,
            hint: c.hint,
            normalized_coords: c.normalized_coords,
            embolden: c.embolden,
            style: c.style,
            brush: paint(c.brush),
            brush_alpha: c.brush_alpha,
            transform: c.transform,
            glyph_transform: c.glyph_transform,
            glyphs: c.glyphs.into_iter().map(|g| anyrender_13::Glyph { id: g.id, x: g.x, y: g.y }).collect(),
        }),
        Command::BoxShadow(c) => old::RenderCommand::BoxShadow(old::BoxShadowCommand {
            transform: c.transform,
            rect: c.rect,
            brush: c.brush,
            radius: c.radius,
            std_dev: c.std_dev,
        }),
    }
}

fn paint(paint: anyrender::Paint) -> anyrender_13::Paint {
    match paint {
        anyrender::Paint::Solid(color) => anyrender_13::Paint::Solid(color),
        anyrender::Paint::Gradient(gradient) => anyrender_13::Paint::Gradient(gradient),
        anyrender::Paint::Image(image) => anyrender_13::Paint::Image(image),
        anyrender::Paint::Custom(custom) => anyrender_13::Paint::Custom(custom),
        anyrender::Paint::Resource(_) => anyrender_13::Paint::Solid(peniko::Color::TRANSPARENT),
    }
}
