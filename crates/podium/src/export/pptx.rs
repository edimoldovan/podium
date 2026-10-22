//! The deck as a native PowerPoint file: text boxes PowerPoint edits as its
//! own, tables, pictures, charts whose data sits in an embedded workbook,
//! and each page's speaker notes.
//!
//! Page units are points; a point is 12 700 EMU.

use std::io::Write as _;

use rust_xlsxwriter::Workbook;
use zip::write::SimpleFileOptions;

use crate::chart::waterfall;
use crate::deck::{Body, ChartKind, Deck, Element, HEIGHT, Page, PageType, Series, WIDTH};
use crate::theme::{self, Look};

const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const GROUP: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;

/// The parts of the package as they are written.
#[derive(Default)]
struct Package {
    parts: Vec<(String, Vec<u8>)>,
    overrides: Vec<(String, &'static str)>,
}

impl Package {
    fn put(&mut self, name: &str, bytes: impl Into<Vec<u8>>) {
        self.parts.push((name.to_string(), bytes.into()));
    }

    /// A part with its content type, `application/vnd.openxmlformats-officedocument.{kind}`.
    fn typed(&mut self, name: &str, kind: &'static str, bytes: impl Into<Vec<u8>>) {
        self.overrides.push((format!("/{name}"), kind));
        self.put(name, bytes);
    }
}

pub fn write(deck: &Deck) -> Result<Vec<u8>, String> {
    let look = theme::look(&deck.theme);
    let mut pkg = Package::default();
    let mut charts = 0;
    let mut pictures = 0;
    let mut slide_rels = Vec::new();
    for (i, page) in deck.pages.iter().enumerate() {
        let n = i + 1;
        let mut rels = vec![("slideLayout".to_string(), "../slideLayouts/slideLayout1.xml".to_string())];
        let shapes = slide_shapes(deck, page, &look, &mut rels, &mut pkg, &mut charts, &mut pictures)?;
        if !page.notes.trim().is_empty() {
            rels.push(("notesSlide".into(), format!("../notesSlides/notesSlide{n}.xml")));
            pkg.typed(&format!("ppt/notesSlides/notesSlide{n}.xml"), "presentationml.notesSlide+xml", notes(&page.notes));
            pkg.put(&format!("ppt/notesSlides/_rels/notesSlide{n}.xml.rels"), rels_xml(&[("notesMaster", "../notesMasters/notesMaster1.xml"), ("slide", &format!("../slides/slide{n}.xml"))]));
        }
        let background = if page.kind == PageType::Divider { &look.accent } else { &look.paper };
        let slide = format!(
            r#"{XML}<p:sld {NS}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>{GROUP}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#,
            hex(background)
        );
        pkg.typed(&format!("ppt/slides/slide{n}.xml"), "presentationml.slide+xml", slide);
        let refs: Vec<(&str, &str)> = rels.iter().map(|(k, t)| (k.as_str(), t.as_str())).collect();
        pkg.put(&format!("ppt/slides/_rels/slide{n}.xml.rels"), rels_xml(&refs));
        slide_rels.push(format!("slides/slide{n}.xml"));
    }

    let mut presentation_rels: Vec<(&str, &str)> = vec![("slideMaster", "slideMasters/slideMaster1.xml"), ("notesMaster", "notesMasters/notesMaster1.xml")];
    presentation_rels.extend(slide_rels.iter().map(|s| ("slide", s.as_str())));
    presentation_rels.extend([("theme", "theme/theme1.xml"), ("presProps", "presProps.xml"), ("viewProps", "viewProps.xml"), ("tableStyles", "tableStyles.xml")]);
    let slide_ids: String = (0..deck.pages.len()).map(|i| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 256 + i, i + 3)).collect();
    pkg.typed(
        "ppt/presentation.xml",
        "presentationml.presentation.main+xml",
        format!(
            r#"{XML}<p:presentation {NS} saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:notesMasterIdLst><p:notesMasterId r:id="rId2"/></p:notesMasterIdLst><p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="{}" cy="{}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#,
            emu(WIDTH),
            emu(HEIGHT)
        ),
    );
    pkg.put("ppt/_rels/presentation.xml.rels", rels_xml(&presentation_rels));
    pkg.typed("ppt/presProps.xml", "presentationml.presProps+xml", format!("{XML}<p:presentationPr {NS}/>"));
    pkg.typed("ppt/viewProps.xml", "presentationml.viewProps+xml", format!("{XML}<p:viewPr {NS}/>"));
    pkg.typed("ppt/tableStyles.xml", "presentationml.tableStyles+xml", format!(r#"{XML}<a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}"/>"#));
    pkg.typed("ppt/theme/theme1.xml", "theme+xml", theme_xml(&look));
    pkg.typed("ppt/theme/theme2.xml", "theme+xml", theme_xml(&look));
    pkg.typed("ppt/slideMasters/slideMaster1.xml", "presentationml.slideMaster+xml", master());
    pkg.put("ppt/slideMasters/_rels/slideMaster1.xml.rels", rels_xml(&[("slideLayout", "../slideLayouts/slideLayout1.xml"), ("theme", "../theme/theme1.xml")]));
    pkg.typed("ppt/slideLayouts/slideLayout1.xml", "presentationml.slideLayout+xml", layout());
    pkg.put("ppt/slideLayouts/_rels/slideLayout1.xml.rels", rels_xml(&[("slideMaster", "../slideMasters/slideMaster1.xml")]));
    pkg.typed("ppt/notesMasters/notesMaster1.xml", "presentationml.notesMaster+xml", notes_master());
    pkg.put("ppt/notesMasters/_rels/notesMaster1.xml.rels", rels_xml(&[("theme", "../theme/theme2.xml")]));
    pkg.typed(
        "docProps/app.xml",
        "extended-properties+xml",
        format!(r#"{XML}<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Podium</Application></Properties>"#),
    );
    pkg.put("_rels/.rels", rels_xml(&[("officeDocument", "ppt/presentation.xml"), ("extended-properties", "docProps/app.xml")]));

    let mut types = format!(
        r#"{XML}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="xlsx" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpeg" ContentType="image/jpeg"/>"#
    );
    for (part, kind) in &pkg.overrides {
        types += &format!(r#"<Override PartName="{part}" ContentType="application/vnd.openxmlformats-officedocument.{kind}"/>"#);
    }
    types += "</Types>";

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let mut put = |name: &str, bytes: &[u8]| -> Result<(), String> {
        zip.start_file(name, SimpleFileOptions::default()).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())
    };
    put("[Content_Types].xml", types.as_bytes())?;
    for (name, bytes) in &pkg.parts {
        put(name, bytes)?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}

fn rels_xml(targets: &[(&str, &str)]) -> String {
    let mut out = format!(r#"{XML}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#);
    for (i, (kind, target)) in targets.iter().enumerate() {
        out += &format!(r#"<Relationship Id="rId{}" Type="{REL}/{kind}" Target="{target}"/>"#, i + 1);
    }
    out + "</Relationships>"
}

fn emu(points: f32) -> i64 {
    (points as f64 * 12_700.0).round() as i64
}

fn hex(colour: &str) -> String {
    colour.trim_start_matches('#').to_ascii_uppercase()
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn xfrm(tag: &str, (x, y, w, h): (f32, f32, f32, f32), degrees: f32) -> String {
    // PowerPoint keeps angles positive, in 60 000ths of a degree.
    let rot = ((degrees as f64).rem_euclid(360.0) * 60_000.0).round() as i64;
    let rot = if rot == 0 { String::new() } else { format!(r#" rot="{rot}""#) };
    format!(r#"<{tag}{rot}><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></{tag}>"#, emu(x), emu(y), emu(w), emu(h))
}

/// How a run of text looks.
#[derive(Clone)]
struct Font {
    size: f32,
    colour: String,
    family: String,
    bold: bool,
    caps: bool,
}

fn run(text: &str, f: &Font) -> String {
    format!(
        r#"<a:r><a:rPr lang="en-US" sz="{}"{}{} dirty="0"><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{}"/><a:cs typeface="{}"/></a:rPr><a:t>{}</a:t></a:r>"#,
        (f.size * 100.0).round() as i64,
        if f.bold { r#" b="1""# } else { "" },
        if f.caps { r#" cap="all""# } else { "" },
        hex(&f.colour),
        esc(&f.family),
        esc(&f.family),
        esc(text)
    )
}

/// Paragraphs, one a line.
fn lines(text: &str, f: &Font) -> String {
    text.split('\n').map(|line| if line.is_empty() { "<a:p/>".to_string() } else { format!("<a:p>{}</a:p>", run(line, f)) }).collect()
}

/// A text box with its paragraphs written; filled when a colour is given.
fn text_box(id: u32, name: &str, rect: (f32, f32, f32, f32), degrees: f32, fill: Option<&str>, inset: (f32, f32), paragraphs: &str) -> String {
    let fill = fill.map_or("<a:noFill/>".to_string(), |c| format!(r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(c)));
    let (v, h) = (emu(inset.0), emu(inset.1));
    let paragraphs = if paragraphs.is_empty() { "<a:p/>" } else { paragraphs };
    format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{fill}</p:spPr><p:txBody><a:bodyPr wrap="square" lIns="{h}" tIns="{v}" rIns="{h}" bIns="{v}" anchor="t"><a:noAutofit/></a:bodyPr><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#,
        esc(name),
        xfrm("a:xfrm", rect, degrees)
    )
}

#[allow(clippy::too_many_arguments)]
fn slide_shapes(deck: &Deck, page: &Page, look: &Look, rels: &mut Vec<(String, String)>, pkg: &mut Package, charts: &mut usize, pictures: &mut usize) -> Result<String, String> {
    let body = theme::family(&look.body_font).to_string();
    let title_family = theme::family(&look.title_font).to_string();
    let font = |size: f32, colour: &str| Font { size, colour: colour.to_string(), family: body.clone(), bold: false, caps: false };
    let mut id = 1;
    let mut next = || {
        id += 1;
        id
    };
    let mut out = String::new();

    if page.kind.tagged()
        && let Some(name) = page.chapter.as_deref().and_then(|c| deck.chapter_name(c))
    {
        let f = Font { caps: true, ..font(11.0, &look.muted) };
        out += &text_box(next(), "Chapter", (48.0, 24.0, 600.0, 14.0), 0.0, None, (0.0, 0.0), &lines(name, &f));
    }
    let (size, colour) = match page.kind {
        PageType::Title => (40.0, &look.accent),
        PageType::Divider => (36.0, &look.paper),
        _ => (26.0, &look.ink),
    };
    let t = page.kind.title_rect();
    let f = Font { family: title_family, bold: deck.theme.name == "modern", ..font(size, colour) };
    out += &text_box(next(), "Title", (t.x, t.y, t.w, t.h), 0.0, None, (0.0, 0.0), &lines(&page.title, &f));

    for e in &page.elements {
        let rect = (e.x, e.y, e.w, e.h);
        out += &match &e.body {
            Body::Text { text } => {
                let f = if page.kind == PageType::Title { font(20.0, &look.muted) } else { font(16.0, &look.ink) };
                text_box(next(), "Text", rect, e.rotation, None, (0.0, 0.0), &lines(text, &f))
            }
            Body::Bullets { items } => {
                let f = font(16.0, &look.ink);
                let paragraphs: String = items
                    .iter()
                    .map(|item| format!(r#"<a:p><a:pPr marL="279400" indent="-279400"><a:lnSpc><a:spcPct val="115000"/></a:lnSpc><a:spcAft><a:spcPts val="1000"/></a:spcAft><a:buFont typeface="Arial"/><a:buChar char="•"/></a:pPr>{}</a:p>"#, run(item, &f)))
                    .collect();
                text_box(next(), "Bullets", rect, e.rotation, None, (0.0, 0.0), &paragraphs)
            }
            Body::Callout { text } => {
                let f = Font { bold: true, ..font(16.0, &look.paper) };
                text_box(next(), "Callout", rect, e.rotation, Some(&look.accent), (12.0, 14.0), &lines(text, &f))
            }
            Body::Source { text } => text_box(next(), "Source", rect, e.rotation, None, (0.0, 0.0), &lines(text, &font(10.0, &look.muted))),
            Body::Footnote { text } => text_box(next(), "Footnote", rect, e.rotation, None, (0.0, 0.0), &lines(text, &font(10.0, &look.muted))),
            Body::Table { rows } => table(next(), e, rows, look, &body),
            Body::Image { path } => {
                let Ok(bytes) = std::fs::read(path) else { continue };
                let png = bytes.starts_with(b"\x89PNG");
                *pictures += 1;
                let name = format!("image{}.{}", pictures, if png { "png" } else { "jpeg" });
                pkg.put(&format!("ppt/media/{name}"), bytes);
                rels.push(("image".into(), format!("../media/{name}")));
                // Fitted inside the element, as the page shows it.
                let (iw, ih) = image::image_dimensions(path).map(|(w, h)| (w as f32, h as f32)).unwrap_or((e.w, e.h));
                let scale = (e.w / iw).min(e.h / ih);
                let (w, h) = (iw * scale, ih * scale);
                let fitted = (e.x + (e.w - w) / 2.0, e.y + (e.h - h) / 2.0, w, h);
                format!(
                    r#"<p:pic><p:nvPicPr><p:cNvPr id="{}" name="Picture"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId{}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr>{}<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
                    next(),
                    rels.len(),
                    xfrm("a:xfrm", fitted, e.rotation)
                )
            }
            Body::Chart { chart, categories, series } => {
                *charts += 1;
                let n = *charts;
                let (xml, book) = chart_xml(*chart, categories, series, look)?;
                pkg.typed(&format!("ppt/charts/chart{n}.xml"), "drawingml.chart+xml", xml);
                pkg.put(&format!("ppt/charts/_rels/chart{n}.xml.rels"), rels_xml(&[("package", &format!("../embeddings/Microsoft_Excel_Worksheet{n}.xlsx"))]));
                pkg.put(&format!("ppt/embeddings/Microsoft_Excel_Worksheet{n}.xlsx"), book);
                rels.push(("chart".into(), format!("../charts/chart{n}.xml")));
                format!(
                    r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{}" name="Chart"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>{}<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" r:id="rId{}"/></a:graphicData></a:graphic></p:graphicFrame>"#,
                    next(),
                    xfrm("p:xfrm", rect, e.rotation),
                    rels.len()
                )
            }
        };
    }
    Ok(out)
}

fn table(id: u32, e: &Element, rows: &[Vec<String>], look: &Look, family: &str) -> String {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let col = e.w / cols as f32;
    let row_h = e.h / rows.len().max(1) as f32;
    let grid: String = (0..cols).map(|_| format!(r#"<a:gridCol w="{}"/>"#, emu(col))).collect();
    let none = |side: &str| format!(r#"<a:{side} w="0"><a:noFill/></a:{side}>"#);
    let rule = format!(r#"<a:lnB w="12700"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:lnB>"#, hex(&look.soft(0.8)));
    let body: String = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let head = i == 0;
            let f = Font { size: 13.0, colour: if head { look.paper.clone() } else { look.ink.clone() }, family: family.to_string(), bold: head, caps: false };
            let cells: String = (0..cols)
                .map(|c| {
                    let text = row.get(c).map(String::as_str).unwrap_or("");
                    let paragraph = if text.is_empty() { "<a:p/>".to_string() } else { format!("<a:p>{}</a:p>", run(text, &f)) };
                    let fill = if head { format!(r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#, hex(&look.accent)) } else { "<a:noFill/>".into() };
                    let bottom = if head { none("lnB") } else { rule.clone() };
                    format!(
                        r#"<a:tc><a:txBody><a:bodyPr/><a:lstStyle/>{paragraph}</a:txBody><a:tcPr marL="101600" marR="101600" marT="76200" marB="76200">{}{}{}{bottom}{fill}</a:tcPr></a:tc>"#,
                        none("lnL"),
                        none("lnR"),
                        none("lnT")
                    )
                })
                .collect();
            format!(r#"<a:tr h="{}">{cells}</a:tr>"#, emu(row_h))
        })
        .collect();
    format!(
        r#"<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="{id}" name="Table"/><p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr><p:nvPr/></p:nvGraphicFramePr>{}<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl><a:tblPr firstRow="1"/><a:tblGrid>{grid}</a:tblGrid>{body}</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        xfrm("p:xfrm", (e.x, e.y, e.w, e.h), e.rotation)
    )
}

/// A spreadsheet column's letter, from 0.
fn column(i: usize) -> char {
    (b'A' + i as u8) as char
}

fn text_props(size: f32, colour: &str, family: &str) -> String {
    format!(
        r#"<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="{}"><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{}"/></a:defRPr></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr>"#,
        (size * 100.0) as i64,
        hex(colour),
        esc(family)
    )
}

fn labels(position: &str, colour: &str, family: &str) -> String {
    format!(
        r#"<c:dLbls><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}<c:dLblPos val="{position}"/><c:showLegendKey val="0"/><c:showVal val="1"/><c:showCatName val="0"/><c:showSerName val="0"/><c:showPercent val="0"/><c:showBubbleSize val="0"/></c:dLbls>"#,
        text_props(11.0, colour, family)
    )
}

fn solid(colour: &str) -> String {
    format!(r#"<c:spPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill></c:spPr>"#, hex(colour))
}

/// White or ink, whichever reads on a fill.
fn on(fill: &str) -> &'static str {
    let [r, g, b] = theme::rgb(fill);
    if 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 150.0 { "#111111" } else { "#ffffff" }
}

/// A chart part and the workbook behind it.
fn chart_xml(kind: ChartKind, categories: &[String], series: &[Series], look: &Look) -> Result<(String, Vec<u8>), String> {
    let n = categories.len().max(series.iter().map(|s| s.values.len()).max().unwrap_or(0)).max(1);
    let family = theme::family(&look.body_font).to_string();
    let value = |s: &Series, i: usize| s.values.get(i).copied().unwrap_or(0.0);

    // What the workbook holds, column by column after the categories: a
    // waterfall's invisible base, then the bars.
    let columns: Vec<(String, Vec<f64>)> = match kind {
        ChartKind::Waterfall => {
            let first = series.first().cloned().unwrap_or_default();
            let steps = waterfall(&first, n);
            vec![
                ("Base".into(), steps.iter().map(|(a, b)| a.min(*b)).collect()),
                (first.name.clone(), steps.iter().map(|(a, b)| (b - a).abs()).collect()),
            ]
        }
        _ => series.iter().map(|s| (s.name.clone(), (0..n).map(|i| value(s, i)).collect())).collect(),
    };

    let last = n + 1;
    let cat_cache: String = (0..n).map(|i| format!(r#"<c:pt idx="{i}"><c:v>{}</c:v></c:pt>"#, esc(categories.get(i).map(String::as_str).unwrap_or("")))).collect();
    let cat = format!(r#"<c:cat><c:strRef><c:f>Sheet1!$A$2:$A${last}</c:f><c:strCache><c:ptCount val="{n}"/>{cat_cache}</c:strCache></c:strRef></c:cat>"#);
    let ser_head = |k: usize, name: &str| {
        let col = column(k + 1);
        format!(r#"<c:idx val="{k}"/><c:order val="{k}"/><c:tx><c:strRef><c:f>Sheet1!${col}$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>{}</c:v></c:pt></c:strCache></c:strRef></c:tx>"#, esc(name))
    };
    let val = |k: usize, values: &[f64]| {
        let col = column(k + 1);
        let pts: String = values.iter().enumerate().map(|(i, v)| format!(r#"<c:pt idx="{i}"><c:v>{v}</c:v></c:pt>"#)).collect();
        format!(r#"<c:val><c:numRef><c:f>Sheet1!${col}$2:${col}${last}</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="{n}"/>{pts}</c:numCache></c:numRef></c:val>"#)
    };

    let plot = match kind {
        ChartKind::Bar | ChartKind::StackedBar => {
            let stacked = kind == ChartKind::StackedBar;
            let sers: String = columns
                .iter()
                .enumerate()
                .map(|(k, (name, values))| {
                    let colour = look.series(k);
                    let dl = if stacked { labels("ctr", on(&colour), &family) } else { labels("outEnd", &look.ink, &family) };
                    format!(r#"<c:ser>{}{}<c:invertIfNegative val="0"/>{dl}{cat}{}</c:ser>"#, ser_head(k, name), solid(&colour), val(k, values))
                })
                .collect();
            format!(
                r#"<c:barChart><c:barDir val="col"/><c:grouping val="{}"/><c:varyColors val="0"/>{sers}<c:gapWidth val="60"/>{}<c:axId val="1001"/><c:axId val="1002"/></c:barChart>"#,
                if stacked { "stacked" } else { "clustered" },
                if stacked { r#"<c:overlap val="100"/>"# } else { "" }
            )
        }
        ChartKind::Line => {
            let sers: String = columns
                .iter()
                .enumerate()
                .map(|(k, (name, values))| {
                    let colour = hex(&look.series(k));
                    format!(
                        r#"<c:ser>{}<c:spPr><a:ln w="31750" cap="rnd"><a:solidFill><a:srgbClr val="{colour}"/></a:solidFill></a:ln></c:spPr><c:marker><c:symbol val="circle"/><c:size val="6"/><c:spPr><a:solidFill><a:srgbClr val="{colour}"/></a:solidFill><a:ln><a:noFill/></a:ln></c:spPr></c:marker>{}{cat}{}<c:smooth val="0"/></c:ser>"#,
                        ser_head(k, name),
                        labels("t", &look.ink, &family),
                        val(k, values)
                    )
                })
                .collect();
            format!(r#"<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>{sers}<c:marker val="1"/><c:axId val="1001"/><c:axId val="1002"/></c:lineChart>"#)
        }
        ChartKind::Waterfall => {
            let steps = waterfall(&series.first().cloned().unwrap_or_default(), n);
            let points: String = steps
                .iter()
                .enumerate()
                .map(|(i, (from, to))| {
                    let colour = if i == 0 || i + 1 == n { look.accent.clone() } else if to >= from { look.soft(0.45) } else { look.muted.clone() };
                    format!(r#"<c:dPt><c:idx val="{i}"/><c:invertIfNegative val="0"/><c:bubble3D val="0"/>{}</c:dPt>"#, solid(&colour))
                })
                .collect();
            let base = format!(r#"<c:ser>{}<c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr><c:invertIfNegative val="0"/>{cat}{}</c:ser>"#, ser_head(0, &columns[0].0), val(0, &columns[0].1));
            let bars = format!(
                r#"<c:ser>{}{}<c:invertIfNegative val="0"/>{points}{}{cat}{}</c:ser>"#,
                ser_head(1, &columns[1].0),
                solid(&look.accent),
                labels("inEnd", &look.ink, &family),
                val(1, &columns[1].1)
            );
            format!(r#"<c:barChart><c:barDir val="col"/><c:grouping val="stacked"/><c:varyColors val="0"/>{base}{bars}<c:gapWidth val="60"/><c:overlap val="100"/><c:axId val="1001"/><c:axId val="1002"/></c:barChart>"#)
        }
    };
    let legend = if series.len() > 1 && kind != ChartKind::Waterfall {
        format!(r#"<c:legend><c:legendPos val="t"/><c:overlay val="0"/>{}</c:legend>"#, text_props(11.0, &look.muted, &family))
    } else {
        String::new()
    };
    let axes = format!(
        r#"<c:catAx><c:axId val="1001"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="0"/><c:axPos val="b"/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="low"/><c:spPr><a:ln w="9525"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln></c:spPr>{}<c:crossAx val="1002"/><c:crosses val="autoZero"/><c:auto val="1"/><c:lblAlgn val="ctr"/><c:lblOffset val="100"/></c:catAx><c:valAx><c:axId val="1002"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:delete val="1"/><c:axPos val="l"/><c:numFmt formatCode="General" sourceLinked="1"/><c:majorTickMark val="none"/><c:minorTickMark val="none"/><c:tickLblPos val="nextTo"/><c:crossAx val="1001"/><c:crosses val="autoZero"/><c:crossBetween val="between"/></c:valAx>"#,
        hex(&look.muted),
        text_props(12.0, &look.muted, &family)
    );
    let xml = format!(
        r#"{XML}<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="{REL}"><c:roundedCorners val="0"/><c:chart><c:autoTitleDeleted val="1"/><c:plotArea><c:layout/>{plot}{axes}</c:plotArea>{legend}<c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/></c:chart><c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>{}<c:externalData r:id="rId1"><c:autoUpdate val="0"/></c:externalData></c:chartSpace>"#,
        text_props(12.0, &look.muted, &family)
    );

    let mut book = Workbook::new();
    let sheet = book.add_worksheet().set_name("Sheet1").map_err(|e| e.to_string())?;
    for i in 0..n {
        sheet.write_string(i as u32 + 1, 0, categories.get(i).map(String::as_str).unwrap_or("")).map_err(|e| e.to_string())?;
    }
    for (k, (name, values)) in columns.iter().enumerate() {
        sheet.write_string(0, k as u16 + 1, name).map_err(|e| e.to_string())?;
        for (i, v) in values.iter().enumerate() {
            sheet.write_number(i as u32 + 1, k as u16 + 1, *v).map_err(|e| e.to_string())?;
        }
    }
    let bytes = book.save_to_buffer().map_err(|e| e.to_string())?;
    Ok((xml, bytes))
}

fn theme_xml(look: &Look) -> String {
    let solid = r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#;
    let line = format!(r#"<a:ln w="6350">{solid}</a:ln>"#);
    let effect = "<a:effectStyle><a:effectLst/></a:effectStyle>";
    let c = |v: &str| format!(r#"<a:srgbClr val="{}"/>"#, hex(v));
    format!(
        r#"{XML}<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Podium"><a:themeElements><a:clrScheme name="Podium"><a:dk1>{}</a:dk1><a:lt1>{}</a:lt1><a:dk2>{}</a:dk2><a:lt2>{}</a:lt2><a:accent1>{}</a:accent1><a:accent2>{}</a:accent2><a:accent3>{}</a:accent3><a:accent4>{}</a:accent4><a:accent5>{}</a:accent5><a:accent6>{}</a:accent6><a:hlink>{}</a:hlink><a:folHlink>{}</a:folHlink></a:clrScheme><a:fontScheme name="Podium"><a:majorFont><a:latin typeface="{}"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="{}"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Podium"><a:fillStyleLst>{solid}{solid}{solid}</a:fillStyleLst><a:lnStyleLst>{line}{line}{line}</a:lnStyleLst><a:effectStyleLst>{effect}{effect}{effect}</a:effectStyleLst><a:bgFillStyleLst>{solid}{solid}{solid}</a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#,
        c(&look.ink),
        c(&look.paper),
        c(&look.accent),
        c(&look.soft(0.85)),
        c(&look.series(0)),
        c(&look.series(1)),
        c(&look.series(2)),
        c(&look.series(3)),
        c(&look.series(4)),
        c(&look.muted),
        c(&look.accent),
        c(&look.muted),
        esc(theme::family(&look.title_font)),
        esc(theme::family(&look.body_font)),
    )
}

const CLR_MAP: &str = r#"<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>"#;

fn master() -> String {
    format!(
        r#"{XML}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{GROUP}</p:spTree></p:cSld>{CLR_MAP}<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst></p:sldMaster>"#
    )
}

fn layout() -> String {
    format!(r#"{XML}<p:sldLayout {NS} type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>{GROUP}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#)
}

fn notes_master() -> String {
    format!(
        r#"{XML}<p:notesMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{GROUP}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="1143000"/><a:ext cx="5486400" cy="3086100"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4400550"/><a:ext cx="5486400" cy="3600450"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp></p:spTree></p:cSld>{CLR_MAP}</p:notesMaster>"#
    )
}

fn notes(text: &str) -> String {
    let f = Font { size: 12.0, colour: "#000000".into(), family: "Arial".into(), bold: false, caps: false };
    format!(
        r#"{XML}<p:notes {NS}><p:cSld><p:spTree>{GROUP}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#,
        lines(text, &f)
    )
}
