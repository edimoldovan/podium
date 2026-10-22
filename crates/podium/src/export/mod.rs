//! A deck written out for others: PDF, PowerPoint, or one HTML file.

mod html;
mod pdf;
mod pptx;

use std::path::Path;

use crate::deck::Deck;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Pptx,
    Html,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Pdf, Format::Pptx, Format::Html];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Pdf => "pdf",
            Format::Pptx => "pptx",
            Format::Html => "html",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Pdf => "PDF",
            Format::Pptx => "PowerPoint",
            Format::Html => "HTML",
        }
    }

    pub fn named(name: &str) -> Option<Format> {
        Format::ALL.into_iter().find(|f| f.extension() == name.to_ascii_lowercase())
    }
}

pub fn export(deck: &Deck, format: Format, path: &Path) -> Result<(), String> {
    let bytes = match format {
        Format::Pdf => pdf::write(deck)?,
        Format::Pptx => pptx::write(deck)?,
        Format::Html => html::write(deck).into_bytes(),
    };
    std::fs::write(path, bytes).map_err(|e| format!("could not write {}: {e}", path.display()))
}
