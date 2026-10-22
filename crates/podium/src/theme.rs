//! Themes: a page's look. A theme is CSS over the page's HTML, and its
//! settings — colours and fonts — become the CSS variables it reads. A deck
//! names its theme and keeps only the settings changed from the theme's own.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::deck::ThemeChoice;

pub struct Theme {
    pub name: &'static str,
    pub label: &'static str,
    css: &'static str,
}

pub const THEMES: [Theme; 3] = [
    Theme { name: "classic", label: "Classic", css: CLASSIC },
    Theme { name: "modern", label: "Modern", css: MODERN },
    Theme { name: "omarchy", label: "Omarchy", css: "" },
];

pub const THEME_LABELS: [&str; 3] = ["Classic", "Modern", "Omarchy"];

/// The settings a theme takes, with their names in the UI.
pub const COLOURS: [(&str, &str); 4] = [("paper", "Background"), ("ink", "Text"), ("accent", "Accent"), ("muted", "Quiet text")];
pub const FONTS: [(&str, &str); 2] = [("title_font", "Title font"), ("body_font", "Body font")];

/// Fonts to choose from; each with what stands in where it is missing.
pub const FONT_CHOICES: [&str; 8] = [
    "Georgia, serif",
    "Times New Roman, serif",
    "Arial, sans-serif",
    "Helvetica, sans-serif",
    "Calibri, sans-serif",
    "Inter, sans-serif",
    "Liberation Sans, sans-serif",
    "system-ui, sans-serif",
];

/// A theme with its settings resolved: what pages are drawn with.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub paper: String,
    pub ink: String,
    pub accent: String,
    pub muted: String,
    pub title_font: String,
    pub body_font: String,
}

impl Look {
    /// A setting by name.
    pub fn get(&self, key: &str) -> &str {
        match key {
            "paper" => &self.paper,
            "ink" => &self.ink,
            "accent" => &self.accent,
            "muted" => &self.muted,
            "title_font" => &self.title_font,
            _ => &self.body_font,
        }
    }

    fn set(&mut self, key: &str, value: &str) {
        let slot = match key {
            "paper" => &mut self.paper,
            "ink" => &mut self.ink,
            "accent" => &mut self.accent,
            "muted" => &mut self.muted,
            "title_font" => &mut self.title_font,
            "body_font" => &mut self.body_font,
            _ => return,
        };
        *slot = value.to_string();
    }

    /// The accent a part of the way to the background: chart fills, rules.
    pub fn soft(&self, t: f32) -> String {
        mix(&self.accent, &self.paper, t)
    }

    /// Colours for a chart's series, the first the accent.
    pub fn series(&self, i: usize) -> String {
        match i % 5 {
            0 => self.accent.clone(),
            1 => self.soft(0.45),
            2 => self.muted.clone(),
            3 => mix(&self.muted, &self.paper, 0.5),
            _ => self.ink.clone(),
        }
    }
}

/// A theme's own settings.
fn defaults(name: &str) -> Look {
    match name {
        "modern" => Look {
            paper: "#ffffff".into(),
            ink: "#111827".into(),
            accent: "#0f766e".into(),
            muted: "#6b7280".into(),
            title_font: "Inter, sans-serif".into(),
            body_font: "Inter, sans-serif".into(),
        },
        "omarchy" => {
            let palette = omarchy();
            let hex = |c: eframe::egui::Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
            Look {
                paper: hex(palette.bg),
                ink: hex(palette.fg),
                accent: hex(palette.accent),
                muted: hex(omarchy_kit::theme::mix(palette.fg, palette.bg, 0.4)),
                title_font: "system-ui, sans-serif".into(),
                body_font: "system-ui, sans-serif".into(),
            }
        }
        _ => Look {
            paper: "#ffffff".into(),
            ink: "#0b2a4a".into(),
            accent: "#0b2a4a".into(),
            muted: "#5b6b7d".into(),
            title_font: "Georgia, serif".into(),
            body_font: "Arial, sans-serif".into(),
        },
    }
}

/// The Omarchy theme's colours, read at most once a second: every page
/// drawn asks for them.
fn omarchy() -> omarchy_kit::theme::Palette {
    static KEPT: Mutex<Option<(Instant, omarchy_kit::theme::Palette)>> = Mutex::new(None);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, palette)) = *kept
        && at.elapsed() < Duration::from_secs(1)
    {
        return palette;
    }
    let palette = omarchy_kit::theme::load();
    *kept = Some((Instant::now(), palette));
    palette
}

/// The look a deck's theme choice comes to.
pub fn look(choice: &ThemeChoice) -> Look {
    let mut look = defaults(&choice.name);
    for (key, value) in &choice.settings {
        look.set(key, value);
    }
    look
}

/// The theme's own value of a setting, before the deck changed it.
pub fn default_of(name: &str, key: &str) -> String {
    defaults(name).get(key).to_string()
}

pub fn index(name: &str) -> usize {
    THEMES.iter().position(|t| t.name == name).unwrap_or(0)
}

/// The CSS a page is drawn with: the settings as variables, then the
/// layout every theme shares, then the theme's own rules.
pub fn css(choice: &ThemeChoice) -> String {
    let look = look(choice);
    let theme = &THEMES[index(&choice.name)];
    format!(
        ":root {{ --paper: {}; --ink: {}; --accent: {}; --muted: {}; --soft: {}; --title-font: {}; --body-font: {}; }}\n{BASE}\n{}",
        look.paper,
        look.ink,
        look.accent,
        look.muted,
        look.soft(0.8),
        look.title_font,
        look.body_font,
        theme.css
    )
}

/// The first family of a font setting: what PowerPoint is told.
pub fn family(font: &str) -> &str {
    font.split(',').next().unwrap_or(font).trim()
}

/// `a` moved `t` of the way to `b`, both `#rrggbb`.
pub fn mix(a: &str, b: &str, t: f32) -> String {
    let (a, b) = (rgb(a), rgb(b));
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    format!("#{:02x}{:02x}{:02x}", m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]))
}

/// `#rrggbb` as its three channels; black when it is not one.
pub fn rgb(hex: &str) -> [u8; 3] {
    let h = hex.trim_start_matches('#');
    let c = |i: usize| h.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0);
    [c(0), c(2), c(4)]
}

const BASE: &str = r#"
html, body { margin: 0; background: var(--paper); }
.page { position: relative; width: 960px; height: 540px; overflow: hidden; background: var(--paper); color: var(--ink); font-family: var(--body-font); }
.page > * { position: absolute; box-sizing: border-box; margin: 0; overflow: hidden; }
.page > header { font-size: 11px; line-height: 14px; color: var(--muted); letter-spacing: 0.06em; text-transform: uppercase; }
.page > h1 { font-family: var(--title-font); font-weight: normal; font-size: 26px; line-height: 1.2; color: var(--ink); }
.page > p { font-size: 16px; line-height: 1.4; white-space: pre-wrap; }
.page > ul { font-size: 16px; line-height: 1.4; padding-left: 22px; }
.page > ul > li { margin-bottom: 10px; }
.page > aside { background: var(--accent); color: var(--paper); font-weight: bold; font-size: 16px; line-height: 1.3; padding: 12px 14px; white-space: pre-wrap; }
.page > footer { font-size: 10px; line-height: 1.3; color: var(--muted); white-space: pre-wrap; }
.page > small { font-size: 10px; line-height: 1.3; color: var(--muted); white-space: pre-wrap; }
.page > table { border-collapse: collapse; font-size: 13px; }
.page > table > thead > tr > th { background: var(--accent); color: var(--paper); text-align: left; font-weight: bold; padding: 6px 8px; }
.page > table > tbody > tr > td { padding: 6px 8px; border-bottom: 1px solid var(--soft); }
.page > figure > svg { display: block; width: 100%; height: 100%; }
.page > img { object-fit: contain; }
.page[data-type=title] > h1 { font-size: 40px; line-height: 1.15; color: var(--accent); }
.page[data-type=title] > p { font-size: 20px; color: var(--muted); }
.page[data-type=divider] { background: var(--accent); }
.page[data-type=divider] > h1 { font-size: 36px; color: var(--paper); }
"#;

const CLASSIC: &str = r#"
.page[data-type=title] > h1 { border-bottom: 2px solid var(--accent); padding-bottom: 12px; }
"#;

const MODERN: &str = r#"
.page > h1 { font-weight: bold; letter-spacing: -0.01em; }
.page > aside { border-radius: 8px; }
.page[data-type=title] > h1 { font-size: 44px; }
"#;
