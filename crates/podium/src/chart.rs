//! Charts drawn as SVG for the page: bar, stacked bar, line and waterfall,
//! in the theme's colours, with their values written on them.
//!
//! A waterfall's first and last values are totals, drawn from zero; the
//! ones between are changes, each starting where the last one ended.

use crate::deck::{ChartKind, Series};
use crate::theme::Look;

const LABEL: f32 = 12.0;

pub fn svg(kind: ChartKind, categories: &[String], series: &[Series], w: f32, h: f32, look: &Look) -> String {
    let n = categories.len().max(series.iter().map(|s| s.values.len()).max().unwrap_or(0));
    let mut out = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="{}" font-size="{LABEL}">"#, attr(&look.body_font));
    if n == 0 || series.is_empty() {
        return out + "</svg>";
    }
    let legend = series.len() > 1 && kind != ChartKind::Waterfall;
    let top = if legend { 28.0 } else { 6.0 } + LABEL + 4.0;
    let bottom = h - LABEL - 10.0;
    let (left, right) = (4.0, w - 4.0);
    if legend {
        let mut x = left;
        for (i, s) in series.iter().enumerate() {
            out += &format!(r#"<rect x="{x}" y="4" width="10" height="10" fill="{}"/><text x="{}" y="13" fill="{}">{}</text>"#, look.series(i), x + 14.0, look.muted, esc(&s.name));
            x += 24.0 + s.name.chars().count() as f32 * 6.5;
        }
    }
    let value = |s: &Series, i: usize| s.values.get(i).copied().unwrap_or(0.0);

    // The range the bars or lines span, always taking in zero.
    let (lo, hi) = match kind {
        ChartKind::Bar | ChartKind::Line => series.iter().flat_map(|s| (0..n).map(move |i| value(s, i))).fold((0.0f64, 0.0f64), |(a, b), v| (a.min(v), b.max(v))),
        ChartKind::StackedBar => (0..n).fold((0.0, 0.0), |(a, b), i| {
            let up: f64 = series.iter().map(|s| value(s, i).max(0.0)).sum();
            let down: f64 = series.iter().map(|s| value(s, i).min(0.0)).sum();
            (f64::min(a, down), f64::max(b, up))
        }),
        ChartKind::Waterfall => waterfall(&series[0], n).iter().fold((0.0f64, 0.0f64), |(a, b), (from, to)| (a.min(from.min(*to)), b.max(from.max(*to)))),
    };
    let span = if hi - lo == 0.0 { 1.0 } else { hi - lo };
    let y = |v: f64| bottom - ((v - lo) / span) as f32 * (bottom - top);
    let slot = (right - left) / n as f32;
    let zero = y(0.0);

    for i in 0..n {
        let cx = left + slot * (i as f32 + 0.5);
        if let Some(c) = categories.get(i) {
            out += &format!(r#"<text x="{cx}" y="{}" text-anchor="middle" fill="{}">{}</text>"#, h - 4.0, look.muted, esc(c));
        }
    }
    out += &format!(r#"<line x1="{left}" y1="{zero}" x2="{right}" y2="{zero}" stroke="{}" stroke-width="1"/>"#, look.muted);

    match kind {
        ChartKind::Bar => {
            let group = slot * 0.7;
            let bar = group / series.len() as f32;
            for (k, s) in series.iter().enumerate() {
                for i in 0..n {
                    let v = value(s, i);
                    let x = left + slot * i as f32 + (slot - group) / 2.0 + bar * k as f32;
                    let (a, b) = (y(v).min(zero), y(v).max(zero));
                    out += &format!(r#"<rect x="{x}" y="{a}" width="{}" height="{}" fill="{}"/>"#, bar * 0.92, b - a, look.series(k));
                    let ty = if v >= 0.0 { a - 4.0 } else { b + LABEL };
                    out += &format!(r#"<text x="{}" y="{ty}" text-anchor="middle" fill="{}">{}</text>"#, x + bar * 0.46, look.ink, number(v));
                }
            }
        }
        ChartKind::StackedBar => {
            let bar = slot * 0.6;
            for i in 0..n {
                let x = left + slot * i as f32 + (slot - bar) / 2.0;
                let (mut up, mut down) = (0.0, 0.0);
                for (k, s) in series.iter().enumerate() {
                    let v = value(s, i);
                    let base = if v >= 0.0 { &mut up } else { &mut down };
                    let (from, to) = (*base, *base + v);
                    *base = to;
                    let (a, b) = (y(from).min(y(to)), y(from).max(y(to)));
                    out += &format!(r#"<rect x="{x}" y="{a}" width="{bar}" height="{}" fill="{}"/>"#, b - a, look.series(k));
                    if b - a > LABEL + 2.0 {
                        out += &format!(r#"<text x="{}" y="{}" text-anchor="middle" fill="{}">{}</text>"#, x + bar / 2.0, (a + b) / 2.0 + LABEL / 3.0, on(&look.series(k)), number(v));
                    }
                }
                out += &format!(r#"<text x="{}" y="{}" text-anchor="middle" fill="{}" font-weight="bold">{}</text>"#, x + bar / 2.0, y(up) - 4.0, look.ink, number(up + down));
            }
        }
        ChartKind::Line => {
            for (k, s) in series.iter().enumerate() {
                let points: Vec<(f32, f32)> = (0..n).map(|i| (left + slot * (i as f32 + 0.5), y(value(s, i)))).collect();
                let path: Vec<String> = points.iter().map(|(x, y)| format!("{x},{y}")).collect();
                out += &format!(r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2.5"/>"#, path.join(" "), look.series(k));
                for (i, (x, py)) in points.iter().enumerate() {
                    out += &format!(r#"<circle cx="{x}" cy="{py}" r="3.5" fill="{}"/>"#, look.series(k));
                    if series.len() == 1 || i + 1 == n {
                        out += &format!(r#"<text x="{x}" y="{}" text-anchor="middle" fill="{}">{}</text>"#, py - 8.0, look.ink, number(value(s, i)));
                    }
                }
            }
        }
        ChartKind::Waterfall => {
            let bar = slot * 0.6;
            let steps = waterfall(&series[0], n);
            for (i, (from, to)) in steps.iter().enumerate() {
                let x = left + slot * i as f32 + (slot - bar) / 2.0;
                let (a, b) = (y(*from).min(y(*to)), y(*from).max(y(*to)));
                let total = i == 0 || i + 1 == n;
                let fill = if total { look.accent.clone() } else if to >= from { look.soft(0.45) } else { look.muted.clone() };
                out += &format!(r#"<rect x="{x}" y="{a}" width="{bar}" height="{}" fill="{fill}"/>"#, (b - a).max(1.0));
                let v = if total { *to } else { to - from };
                out += &format!(r#"<text x="{}" y="{}" text-anchor="middle" fill="{}">{}</text>"#, x + bar / 2.0, a - 4.0, look.ink, number(v));
                if i + 1 < n {
                    out += &format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="2,2"/>"#, x + bar, y(*to), x + slot, y(*to), look.muted);
                }
            }
        }
    }
    out + "</svg>"
}

/// Each waterfall bar's start and end: totals from zero at both ends,
/// changes between them from where the last one ended.
pub fn waterfall(series: &Series, n: usize) -> Vec<(f64, f64)> {
    let mut level = 0.0;
    (0..n)
        .map(|i| {
            let v = series.values.get(i).copied().unwrap_or(0.0);
            if i == 0 || i + 1 == n {
                level = v;
                (0.0, v)
            } else {
                let from = level;
                level += v;
                (from, level)
            }
        })
        .collect()
}

/// A number as a chart shows it: no trailing zeros.
pub fn number(v: f64) -> String {
    let text = format!("{:.2}", v);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".into() } else { text.into() }
}

/// White or ink, whichever reads on a fill.
fn on(fill: &str) -> &'static str {
    let [r, g, b] = crate::theme::rgb(fill);
    if 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32 > 150.0 { "#111111" } else { "#ffffff" }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn attr(text: &str) -> String {
    esc(text).replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_waterfall_starts_and_ends_with_totals() {
        let s = Series { name: "x".into(), values: vec![100.0, 20.0, -30.0, 90.0] };
        assert_eq!(waterfall(&s, 4), vec![(0.0, 100.0), (100.0, 120.0), (120.0, 90.0), (0.0, 90.0)]);
    }

    #[test]
    fn numbers_lose_their_trailing_zeros() {
        assert_eq!(number(12.0), "12");
        assert_eq!(number(12.5), "12.5");
        assert_eq!(number(-0.001), "0");
    }
}
