# Plan: Podium

Presentations built from a storyline. A standalone Rust app on
omarchy-kit, next to Post and Grido.

## Settled

- Standalone repo, `~/dev/rust/podium`. Rust, egui/eframe, omarchy-kit
  pinned by git rev, laid out like Grido.
- The document is a `.podium` file: JSON.
- Exports: PDF, native PPTX (editable text, native charts), HTML.
- Podium does not open `.pptx` files.
- 16:9 pages.
- Pages are drawn as HTML + CSS by Blitz, as Post draws mail. Podium
  vendors its own copy, as Post does. Themes are CSS.
- Claude does the AI work through Podium's MCP server; the app calls no
  AI itself. The server is registered with Claude on first start,
  without asking. Claude's edits apply immediately and land in the same
  undo history as the user's.

## The flow

1. **Storyline.** The user writes what the presentation needs to do and
   who the audience is. Everything trickles down from it.
2. **Chapters and action titles.** The storyline split into chapters,
   each chapter into action titles. The first page is always the
   executive summary. Splitting and keeping the titles consistent is
   the user's job in the UI, or Claude's through the MCP.
3. **Structure.** Each action title gets a page type. The deck is shown
   as pages: titles in place, placeholders empty.
4. **Detailing.** Each page gets its body: the details that support its
   action title. The user on the canvas, or Claude through the MCP.
5. **Present and export.**

## The window

One view per step, switched from the top bar:

- **Storyline:** the storyline text and the audience.
- **Titles:** chapters with their action titles, one per line. Add,
  edit, reorder, move between chapters, set each title's page type.
- **Pages:** the page rail, the canvas, the speaker notes under it.

**Present** is full screen: arrows and space move, Esc leaves.

## Pages

Every theme has the same page types: title, executive summary, chapter
divider, detail (title + body), two columns, chart with takeaway. A page
type is a set of placeholders at fixed positions; a new page of that type
starts with them as elements.

Every page in a chapter carries its chapter's name in a small tag in the
corner.

## Elements

Text, bullets, chart, table, image, callout, source line, footnotes.

- Moved, resized, scaled and rotated on the canvas as in PowerPoint,
  snapping to the page's guides and to the other elements.
- Text is edited in place.
- Charts: bar, stacked bar, line, waterfall. Their data is typed into a
  small table in the app.
- The look comes from the theme only.

Each page has speaker notes.

## The document

Positions are in page units: 960 × 540.

```json
{
  "podium": 1,
  "storyline": { "text": "…", "audience": "…" },
  "theme": { "name": "classic", "settings": { "accent": "#0b2a4a", "title_font": "Georgia" } },
  "chapters": [{ "id": "c1", "name": "Market" }],
  "pages": [
    {
      "id": "p1",
      "type": "executive-summary",
      "chapter": null,
      "title": "…",
      "elements": [
        { "id": "e1", "kind": "bullets", "x": 60, "y": 140, "w": 840, "h": 320, "rotation": 0, "items": ["…"] }
      ],
      "notes": "…"
    }
  ]
}
```

## Themes

A theme is a CSS file. Its settings (colours, fonts, sizes) become CSS
variables, picked in the UI. Three themes:

- **Classic:** navy and white, serif titles.
- **Modern:** plain and light.
- **Omarchy:** follows the Omarchy theme.

## Rendering

A page becomes HTML: each element absolutely positioned at its page
units, rotated with a CSS transform, charts as inline SVG Podium writes.
The theme's CSS styles it and Blitz draws it, on the canvas and in
present mode. Selection, handles and snap guides are drawn by egui over
the page. The HTML export is the same HTML.

## MCP

`podium --serve`, `podium_*` tools that follow the flow: the deck's
state, storyline, chapters and titles, page types, elements, notes,
theme, undo, export, and a page drawn as PNG so Claude sees what it
made.

## Steps

1. **Spikes**, each a small program, no UI:
   - Blitz draws a rotated, absolutely positioned element and an inline
     SVG chart on a 960 × 540 page.
   - A native PPTX written from Rust: text box, rotated shape, table and
     a bar chart, opening in PowerPoint, Keynote and LibreOffice with
     text and chart data editable.
   - A PDF of a page with selectable text, from the same layout.

   Done: each works, or this plan changes before step 2.
2. **The document:** the model, `.podium` read and write, undo.
3. **Rendering:** page to HTML, the three themes, the six page types.
4. **The window:** Storyline, Titles and Pages views; launcher entry like
   Post and Grido.
5. **The canvas:** select, move, resize, scale, rotate, snap; editing
   each element kind; speaker notes.
6. **MCP:** the server, registration on first start, Claude's edits live.
7. **Present mode.**
8. **Exports:** HTML, PDF, PPTX.
