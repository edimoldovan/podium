//! Letting Claude use Podium: the `podium` entry in the MCP clients'
//! configs — Claude Desktop's and Claude Code's — and the skill file that
//! says how to build a deck with Podium's tools.
//!
//! Podium registers itself as it starts, pointing at the binary that runs.
//! A client's config is merged, never replaced, and the previous contents
//! are left as a `.bak`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The name Podium's server goes by in a client's `mcpServers`.
const ENTRY: &str = "podium";

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The config files of the MCP clients Podium writes itself into.
fn clients() -> Vec<PathBuf> {
    let Some(home) = home() else {
        return Vec::new();
    };
    vec![home.join(".config/Claude/claude_desktop_config.json"), home.join(".claude.json")]
}

/// Writes the entry into every client config there is, and the skill.
pub fn register() {
    let Ok(exe) = std::env::current_exe() else { return };
    let entry = json!({ "command": exe.display().to_string(), "args": ["mcp"] });
    for path in clients().iter().filter(|path| path.exists()) {
        let _ = register_one(path, &entry);
    }
    let _ = install_skill();
}

/// Adds or refreshes the entry in one client config. Only the keys
/// registration writes are compared, so a client that keeps its own beside
/// them is left as it is. False when it was already right.
pub fn register_one(path: &Path, entry: &Value) -> Result<bool, String> {
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut root = parse(path, &existing)?;
    if let Some(current) = root.pointer(&format!("/mcpServers/{ENTRY}"))
        && ["command", "args"].iter().all(|k| current.get(k) == entry.get(k))
    {
        return Ok(false);
    }
    let servers = root.as_object_mut().expect("an object").entry("mcpServers").or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    servers.as_object_mut().expect("an object").insert(ENTRY.to_string(), entry.clone());
    if !existing.trim().is_empty() {
        let _ = std::fs::write(path.with_extension("json.bak"), &existing);
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("could not write {}: {e}", path.display()))?;
    Ok(true)
}

/// A client config as JSON: an empty or missing file is an empty object.
fn parse(path: &Path, text: &str) -> Result<Value, String> {
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    let root: Value = serde_json::from_str(text).map_err(|_| format!("{} is not valid JSON", path.display()))?;
    Ok(if root.is_object() { root } else { json!({}) })
}

/// Writes the skill if it is missing or out of date.
fn install_skill() -> Result<(), String> {
    let Some(dir) = home().map(|home| home.join(".claude/skills").join(ENTRY)) else {
        return Ok(());
    };
    let path = dir.join("SKILL.md");
    let text = format!("{HEAD}\n{GUIDE}");
    if std::fs::read_to_string(&path).is_ok_and(|current| current == text) {
        return Ok(());
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not make {}: {e}", dir.display()))?;
    std::fs::write(&path, text).map_err(|e| format!("could not write {}: {e}", path.display()))
}

const HEAD: &str = r#"---
name: podium
description: Build or change the user's presentations in Podium through its MCP tools — the storyline, chapters and action titles, the pages and what supports each title, themes, speaker notes, and exports to PDF, PowerPoint and HTML. Use for a deck or presentation the user is making in Podium, a .podium file, or a storyline to turn into pages.
---
"#;

/// How to build a deck with Podium: the skill, and the server's instructions.
pub const GUIDE: &str = r#"# Working with Podium

Podium builds a presentation top-down from its storyline. Its window on the
user's screen shows the deck the tools change; every change lands at once
and podium_undo takes it back. Start with podium_deck.

1. **Storyline.** What the presentation needs to do, and its audience.
   Everything below follows from it. podium_storyline sets it.
2. **Chapters and action titles.** The storyline split into chapters
   (podium_chapter_add) and each chapter into pages (podium_page_add), each
   with an action title: one full sentence that states the page's message,
   never a topic. Read in order, the titles alone tell the whole story. The
   executive summary follows the title page and gives the whole story in a
   few points.
3. **Structure.** Every page has a type: title, executive-summary, divider
   (opens a chapter), detail (title and body), two-columns, chart-takeaway.
   A new page starts with its type's placeholders; podium_page_set changes
   its type, title, chapter or speaker notes.
4. **Detailing.** Each page gets what supports its action title and
   nothing else: bullets, a chart with its data, a table, a callout with the
   takeaway, a source line, footnotes. podium_element_set changes an
   element's fields; podium_element_add adds one.

Pages are 960 × 540 units. The body sits between x 48 and 912, y 130 and
470; the source line at y 498. Look at a page with podium_look before
calling it done. podium_export writes PDF, PowerPoint or HTML.
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// Registering adds the entry and keeps everything else; doing it again
    /// changes nothing.
    #[test]
    fn the_entry_goes_in_and_nothing_else_moves() {
        let dir = std::env::temp_dir().join(format!("podium-claude-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".claude.json");
        std::fs::write(&path, r#"{"theme":"dark","mcpServers":{"other":{"command":"x"}}}"#).unwrap();
        let entry = json!({"command": "/usr/bin/podium", "args": ["mcp"]});
        assert!(register_one(&path, &entry).unwrap());
        assert!(!register_one(&path, &entry).unwrap(), "already right");
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["mcpServers"]["podium"], entry);
        assert_eq!(root["mcpServers"]["other"]["command"], "x");
        assert_eq!(root["theme"], "dark");
        assert!(path.with_extension("json.bak").exists());
    }
}
