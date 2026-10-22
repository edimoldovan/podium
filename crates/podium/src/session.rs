//! The deck being worked on: its file, its undo history, whether it is
//! saved. A deck without a file of its own is kept in Podium's data dir
//! until it is saved somewhere, so nothing typed is lost.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::deck::Deck;

const HISTORY: usize = 300;

pub struct Session {
    pub deck: Deck,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub changed_at: Option<Instant>,
    undo: Vec<Deck>,
    redo: Vec<Deck>,
}

impl Session {
    pub fn new(deck: Deck, path: Option<PathBuf>) -> Session {
        Session { deck, path, dirty: false, changed_at: None, undo: Vec::new(), redo: Vec::new() }
    }

    /// The deck to start with: the file asked for, else the last one worked
    /// on, else the unsaved one, else a new one.
    pub fn start(path: Option<PathBuf>) -> (Session, Option<String>) {
        if let Some(path) = path {
            // A file that does not exist yet: a new deck that will be it.
            if !path.exists() {
                let mut session = Session::new(Deck::new(), Some(with_extension(path)));
                session.touch();
                return (session, None);
            }
            return match Deck::load(&path) {
                Ok(deck) => {
                    remember(&path);
                    (Session::new(deck, Some(path)), None)
                }
                Err(e) => (Session::new(Deck::new(), None), Some(e)),
            };
        }
        if let Some(recent) = recent()
            && let Ok(deck) = Deck::load(&recent)
        {
            return (Session::new(deck, Some(recent)), None);
        }
        if let Ok(deck) = Deck::load(&untitled()) {
            return (Session::new(deck, None), None);
        }
        (Session::new(Deck::new(), None), None)
    }

    /// Where the deck is kept: its file, or the unsaved one.
    pub fn file(&self) -> PathBuf {
        self.path.clone().unwrap_or_else(untitled)
    }

    /// The deck's file name, or Untitled.
    pub fn name(&self) -> String {
        self.path
            .as_deref()
            .and_then(Path::file_stem)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }

    /// Takes note that the deck changed from `before`: one step to undo.
    pub fn record(&mut self, before: Deck) {
        if before == self.deck {
            return;
        }
        self.undo.push(before);
        if self.undo.len() > HISTORY {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.touch();
    }

    /// Marks the deck changed, to be saved.
    pub fn touch(&mut self) {
        self.dirty = true;
        self.changed_at = Some(Instant::now());
    }

    /// Runs a change as one step; a change that fails leaves the deck as it was.
    pub fn change<T>(&mut self, f: impl FnOnce(&mut Deck) -> Result<T, String>) -> Result<T, String> {
        let before = self.deck.clone();
        match f(&mut self.deck) {
            Ok(value) => {
                self.record(before);
                Ok(value)
            }
            Err(e) => {
                self.deck = before;
                Err(e)
            }
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(deck) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.deck, deck));
        self.touch();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(deck) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.deck, deck));
        self.touch();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Writes the deck where it is kept.
    pub fn save(&mut self) -> Result<(), String> {
        self.deck.save(&self.file())?;
        self.dirty = false;
        Ok(())
    }

    /// Gives the deck a file of its own and writes it there.
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        let path = with_extension(path);
        self.deck.save(&path)?;
        remember(&path);
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }

    /// Opens a file in place of the deck, after keeping the deck.
    pub fn open(&mut self, path: PathBuf) -> Result<(), String> {
        let deck = Deck::load(&path)?;
        if self.dirty {
            self.save()?;
        }
        remember(&path);
        *self = Session::new(deck, Some(path));
        Ok(())
    }

    /// Starts a new deck, after keeping this one.
    pub fn new_deck(&mut self) -> Result<(), String> {
        if self.dirty {
            self.save()?;
        }
        // An unsaved deck moves aside, so the new one does not write over it.
        let untitled = untitled();
        if self.path.is_none() && untitled.exists() {
            let seconds = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let aside = untitled.with_file_name(format!("untitled-{seconds}.podium"));
            std::fs::rename(&untitled, &aside).map_err(|e| format!("could not keep {}: {e}", untitled.display()))?;
        }
        forget();
        *self = Session::new(Deck::new(), None);
        self.touch();
        Ok(())
    }
}

fn with_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some_and(|e| e == "podium") { path } else { path.with_extension("podium") }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
        .join("podium")
}

/// `$XDG_CONFIG_HOME/podium`, or `~/.config/podium`.
pub fn config_dir() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}

/// The deck that has no file of its own yet.
pub fn untitled() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("untitled.podium")
}

fn recent() -> Option<PathBuf> {
    let text = std::fs::read_to_string(config_dir().join("recent")).ok()?;
    Some(PathBuf::from(text.trim())).filter(|p| p.exists())
}

fn remember(path: &Path) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let _ = std::fs::write(dir.join("recent"), path.to_string_lossy().as_bytes());
}

fn forget() {
    let _ = std::fs::remove_file(config_dir().join("recent"));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A change is one step back; a failed change leaves no trace.
    #[test]
    fn changes_undo_and_failures_leave_nothing() {
        let mut session = Session::new(Deck::new(), None);
        session.change(|d| Ok(d.storyline.audience = "Board".into())).unwrap();
        let failed: Result<(), String> = session.change(|d| {
            d.storyline.audience = "x".into();
            Err("no".into())
        });
        assert!(failed.is_err());
        assert_eq!(session.deck.storyline.audience, "Board");
        assert!(session.undo());
        assert_eq!(session.deck.storyline.audience, "");
        assert!(!session.undo());
        assert!(session.redo());
        assert_eq!(session.deck.storyline.audience, "Board");
    }
}
