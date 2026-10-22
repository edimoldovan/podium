//! The deck: what a `.podium` file holds. The storyline, the chapters, and
//! the pages in order, each with its action title and the elements on it.
//!
//! Positions are in page units: a page is 960 × 540, one point each.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const WIDTH: f32 = 960.0;
pub const HEIGHT: f32 = 540.0;
pub const MARGIN: f32 = 48.0;

/// Where a page's body starts, under its title.
pub const BODY_TOP: f32 = 130.0;

/// The file format's version.
pub const FORMAT: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Deck {
    pub podium: u32,
    #[serde(default)]
    pub storyline: Storyline,
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    #[serde(default)]
    pub pages: Vec<Page>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Storyline {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub audience: String,
}

/// A theme by name, and the settings changed from its own.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ThemeChoice {
    pub name: String,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

impl Default for ThemeChoice {
    fn default() -> Self {
        ThemeChoice { name: "classic".into(), settings: BTreeMap::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Chapter {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Page {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: PageType,
    #[serde(default)]
    pub chapter: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PageType {
    Title,
    ExecutiveSummary,
    Divider,
    Detail,
    TwoColumns,
    ChartTakeaway,
}

impl PageType {
    pub const ALL: [PageType; 6] = [
        PageType::Title,
        PageType::ExecutiveSummary,
        PageType::Divider,
        PageType::Detail,
        PageType::TwoColumns,
        PageType::ChartTakeaway,
    ];

    pub const LABELS: [&'static str; 6] = ["Title", "Executive summary", "Chapter divider", "Detail", "Two columns", "Chart with takeaway"];

    pub fn label(self) -> &'static str {
        Self::LABELS[self.index()]
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// The name the file and the tools use.
    pub fn key(self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
    }

    /// Where the action title sits.
    pub fn title_rect(self) -> Rect {
        match self {
            PageType::Title => Rect { x: MARGIN, y: 170.0, w: WIDTH - 2.0 * MARGIN, h: 120.0 },
            PageType::Divider => Rect { x: MARGIN, y: 210.0, w: WIDTH - 2.0 * MARGIN, h: 110.0 },
            _ => Rect { x: MARGIN, y: 44.0, w: WIDTH - 2.0 * MARGIN, h: 72.0 },
        }
    }

    /// Whether the page carries its chapter's name in the corner.
    pub fn tagged(self) -> bool {
        !matches!(self, PageType::Title | PageType::Divider)
    }

    /// What a new page of this type starts with.
    pub fn placeholders(self) -> Vec<(Rect, Body)> {
        let full = Rect { x: MARGIN, y: BODY_TOP, w: WIDTH - 2.0 * MARGIN, h: 340.0 };
        let source = (Rect { x: MARGIN, y: 498.0, w: WIDTH - 2.0 * MARGIN, h: 20.0 }, Body::Source { text: "Source: ".into() });
        let points = |n: usize| Body::Bullets { items: (1..=n).map(|i| format!("Supporting point {i}")).collect() };
        match self {
            PageType::Title => vec![(Rect { x: MARGIN, y: 300.0, w: 600.0, h: 40.0 }, Body::Text { text: "Subtitle".into() })],
            PageType::ExecutiveSummary => vec![(full, points(4))],
            PageType::Divider => Vec::new(),
            PageType::Detail => vec![(full, points(3)), source],
            PageType::TwoColumns => vec![
                (Rect { x: MARGIN, y: BODY_TOP, w: 420.0, h: 340.0 }, points(3)),
                (Rect { x: 492.0, y: BODY_TOP, w: 420.0, h: 340.0 }, points(3)),
                source,
            ],
            PageType::ChartTakeaway => vec![
                (Rect { x: MARGIN, y: BODY_TOP, w: 560.0, h: 340.0 }, Body::sample_chart()),
                (Rect { x: 632.0, y: BODY_TOP, w: 280.0, h: 120.0 }, Body::Callout { text: "Takeaway".into() }),
                source,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Element {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(default)]
    pub rotation: f32,
    #[serde(flatten)]
    pub body: Body,
}

impl Element {
    pub fn rect(&self) -> Rect {
        Rect { x: self.x, y: self.y, w: self.w, h: self.h }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Body {
    Text {
        #[serde(default)]
        text: String,
    },
    Bullets {
        #[serde(default)]
        items: Vec<String>,
    },
    Chart {
        #[serde(default)]
        chart: ChartKind,
        #[serde(default)]
        categories: Vec<String>,
        #[serde(default)]
        series: Vec<Series>,
    },
    Table {
        #[serde(default)]
        rows: Vec<Vec<String>>,
    },
    Image {
        #[serde(default)]
        path: String,
    },
    Callout {
        #[serde(default)]
        text: String,
    },
    Source {
        #[serde(default)]
        text: String,
    },
    Footnote {
        #[serde(default)]
        text: String,
    },
}

impl Body {
    pub const KINDS: [&'static str; 8] = ["text", "bullets", "chart", "table", "image", "callout", "source", "footnote"];
    pub const LABELS: [&'static str; 8] = ["Text", "Bullets", "Chart", "Table", "Image", "Callout", "Source line", "Footnote"];

    pub fn kind(&self) -> &'static str {
        match self {
            Body::Text { .. } => "text",
            Body::Bullets { .. } => "bullets",
            Body::Chart { .. } => "chart",
            Body::Table { .. } => "table",
            Body::Image { .. } => "image",
            Body::Callout { .. } => "callout",
            Body::Source { .. } => "source",
            Body::Footnote { .. } => "footnote",
        }
    }

    pub fn label(&self) -> &'static str {
        Self::LABELS[Self::KINDS.iter().position(|k| *k == self.kind()).unwrap_or(0)]
    }

    /// A new element of a kind, with something in it, and where it goes.
    pub fn new(kind: &str) -> Option<(Rect, Body)> {
        let body = match kind {
            "text" => Body::Text { text: "Text".into() },
            "bullets" => Body::Bullets { items: vec!["Point".into()] },
            "chart" => Body::sample_chart(),
            "table" => Body::Table { rows: vec![vec!["".into(), "A".into(), "B".into()], vec!["Row".into(), "1".into(), "2".into()]] },
            "image" => Body::Image { path: String::new() },
            "callout" => Body::Callout { text: "Callout".into() },
            "source" => Body::Source { text: "Source: ".into() },
            "footnote" => Body::Footnote { text: "1. Footnote".into() },
            _ => return None,
        };
        let rect = match kind {
            "source" => Rect { x: MARGIN, y: 498.0, w: WIDTH - 2.0 * MARGIN, h: 20.0 },
            "footnote" => Rect { x: MARGIN, y: 478.0, w: WIDTH - 2.0 * MARGIN, h: 18.0 },
            "callout" => Rect { x: 632.0, y: BODY_TOP, w: 280.0, h: 100.0 },
            "text" => Rect { x: MARGIN, y: BODY_TOP, w: 420.0, h: 60.0 },
            _ => Rect { x: 240.0, y: BODY_TOP + 20.0, w: 480.0, h: 300.0 },
        };
        Some((rect, body))
    }

    fn sample_chart() -> Body {
        Body::Chart {
            chart: ChartKind::Bar,
            categories: ["2022", "2023", "2024", "2025"].map(String::from).to_vec(),
            series: vec![Series { name: "Value".into(), values: vec![10.0, 14.0, 19.0, 25.0] }],
        }
    }

    /// The text an element shows, for the kinds that are one text.
    pub fn text_mut(&mut self) -> Option<&mut String> {
        match self {
            Body::Text { text } | Body::Callout { text } | Body::Source { text } | Body::Footnote { text } => Some(text),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ChartKind {
    #[default]
    Bar,
    StackedBar,
    Line,
    Waterfall,
}

impl ChartKind {
    pub const ALL: [ChartKind; 4] = [ChartKind::Bar, ChartKind::StackedBar, ChartKind::Line, ChartKind::Waterfall];
    pub const LABELS: [&'static str; 4] = ["Bar", "Stacked bar", "Line", "Waterfall"];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Series {
    pub name: String,
    #[serde(default)]
    pub values: Vec<f64>,
}

impl Default for Deck {
    fn default() -> Self {
        Deck::new()
    }
}

impl Deck {
    /// A deck to start from: its title page and its executive summary.
    pub fn new() -> Deck {
        let mut deck = Deck { podium: FORMAT, storyline: Storyline::default(), theme: ThemeChoice::default(), chapters: Vec::new(), pages: Vec::new() };
        deck.add_page(PageType::Title, "Title of the presentation", None, None);
        deck.add_page(PageType::ExecutiveSummary, "Executive summary", None, None);
        deck
    }

    pub fn load(path: &Path) -> Result<Deck, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
        let deck: Deck = serde_json::from_str(&text).map_err(|e| format!("{} is not a Podium deck: {e}", path.display()))?;
        if deck.podium > FORMAT {
            return Err(format!("{} was made by a newer Podium", path.display()));
        }
        Ok(deck)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("could not make {}: {e}", dir.display()))?;
        }
        let part = path.with_extension("podium.part");
        std::fs::write(&part, text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
        std::fs::rename(&part, path).map_err(|e| format!("could not write {}: {e}", path.display()))
    }

    /// An id not yet used, of the form `{prefix}{n}`.
    pub fn fresh_id(&self, prefix: &str) -> String {
        let used = self
            .chapters
            .iter()
            .map(|c| c.id.as_str())
            .chain(self.pages.iter().map(|p| p.id.as_str()))
            .chain(self.pages.iter().flat_map(|p| p.elements.iter().map(|e| e.id.as_str())));
        let n = used.filter_map(|id| id.strip_prefix(prefix)?.parse::<u32>().ok()).max().unwrap_or(0);
        format!("{prefix}{}", n + 1)
    }

    /// Adds a page with its type's placeholders, at `at` or at the end.
    pub fn add_page(&mut self, kind: PageType, title: &str, chapter: Option<String>, at: Option<usize>) -> String {
        let id = self.fresh_id("p");
        let mut page = Page { id: id.clone(), kind, chapter, title: title.into(), elements: Vec::new(), notes: String::new() };
        for (rect, body) in kind.placeholders() {
            let element = Element { id: self.fresh_element_id(&page), x: rect.x, y: rect.y, w: rect.w, h: rect.h, rotation: 0.0, body };
            page.elements.push(element);
        }
        let at = at.unwrap_or(self.pages.len()).min(self.pages.len());
        self.pages.insert(at, page);
        id
    }

    /// An element id unused in the deck and in a page not yet in it.
    pub fn fresh_element_id(&self, page: &Page) -> String {
        let taken = page.elements.iter().filter_map(|e| e.id.strip_prefix('e')?.parse::<u32>().ok()).max().unwrap_or(0);
        let deck = self.fresh_id("e")[1..].parse::<u32>().unwrap_or(1);
        format!("e{}", deck.max(taken + 1))
    }

    pub fn page_index(&self, id: &str) -> Option<usize> {
        self.pages.iter().position(|p| p.id == id)
    }

    pub fn page_mut(&mut self, id: &str) -> Option<&mut Page> {
        self.pages.iter_mut().find(|p| p.id == id)
    }

    pub fn chapter_name(&self, id: &str) -> Option<&str> {
        self.chapters.iter().find(|c| c.id == id).map(|c| c.name.as_str())
    }

    /// Adds an element to a page; returns its id.
    pub fn add_element(&mut self, page: usize, rect: Rect, body: Body) -> String {
        let id = self.fresh_element_id(&self.pages[page]);
        self.pages[page].elements.push(Element { id: id.clone(), x: rect.x, y: rect.y, w: rect.w, h: rect.h, rotation: 0.0, body });
        id
    }

    pub fn add_chapter(&mut self, name: &str) -> String {
        let id = self.fresh_id("c");
        self.chapters.push(Chapter { id: id.clone(), name: name.into() });
        id
    }

    /// Removes a chapter; its pages stay, in no chapter.
    pub fn remove_chapter(&mut self, id: &str) {
        self.chapters.retain(|c| c.id != id);
        for page in &mut self.pages {
            if page.chapter.as_deref() == Some(id) {
                page.chapter = None;
            }
        }
    }

    /// A copy of a page right after it, with new ids; returns its index.
    pub fn duplicate_page(&mut self, index: usize) -> usize {
        let mut page = self.pages[index].clone();
        page.id = self.fresh_id("p");
        let mut next = self.fresh_id("e")[1..].parse::<u32>().unwrap_or(1);
        for element in &mut page.elements {
            element.id = format!("e{next}");
            next += 1;
        }
        self.pages.insert(index + 1, page);
        index + 1
    }

    /// The deck's name: its title page's title, or its first page's.
    pub fn name(&self) -> String {
        self.pages
            .iter()
            .find(|p| p.kind == PageType::Title)
            .or(self.pages.first())
            .map(|p| p.title.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deck goes to a file and comes back the same, kinds and all.
    #[test]
    fn a_deck_survives_its_file() {
        let mut deck = Deck::new();
        let c = deck.add_chapter("Market");
        deck.add_page(PageType::ChartTakeaway, "Demand doubles", Some(c), None);
        let text = serde_json::to_string(&deck).unwrap();
        assert!(text.contains(r#""type":"chart-takeaway""#) && text.contains(r#""kind":"chart""#), "{text}");
        assert_eq!(serde_json::from_str::<Deck>(&text).unwrap(), deck);
    }

    /// Ids are never used twice, across pages and duplicates.
    #[test]
    fn ids_are_unique() {
        let mut deck = Deck::new();
        deck.add_page(PageType::TwoColumns, "A", None, None);
        deck.duplicate_page(2);
        let mut ids: Vec<&str> = deck.pages.iter().flat_map(|p| p.elements.iter().map(|e| e.id.as_str())).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert_eq!(deck.pages[3].title, "A");
        assert_ne!(deck.pages[3].id, deck.pages[2].id);
    }
}
