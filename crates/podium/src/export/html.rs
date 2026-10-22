//! The deck as one HTML file: every page in order, pictures inside it.

use crate::deck::Deck;
use crate::render::{self, Pictures};
use crate::theme;

const SHOW: &str = "
body { background: #6b6b6b; padding: 24px 0; }
body > .page { margin: 0 auto 24px; box-shadow: 0 2px 12px rgba(0, 0, 0, 0.35); }
";

pub fn write(deck: &Deck) -> String {
    let pages: String = (0..deck.pages.len()).map(|i| render::page(deck, i, Pictures::Inline)).collect();
    render::document(&format!("{}\n{SHOW}", theme::css(&deck.theme)), &pages, &deck.name())
}
