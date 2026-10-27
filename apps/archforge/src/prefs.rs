//! Preferences: the UI state, stored as `prefs.json` in the settings directory, loaded at start
//! and saved by the exit hook. A file that cannot be read is set aside
//! under a new name, never discarded, and the defaults are used. Every field is applied at
//! start, so no preference goes unread.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use archforge_ui_egui::{Shell, UiState};

pub const FILE: &str = "prefs.json";
/// The most bytes a preferences file may have: the stored state is a few kilobytes, so a file
/// beyond this is damaged or planted and is set aside unread.
pub const MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub ui: UiState,
}

/// Loads the preferences; a damaged file is set aside and reported.
pub fn load(dir: &Path) -> (Prefs, Option<String>) {
    let path = dir.join(FILE);
    let Ok(meta) = std::fs::metadata(&path) else { return (Prefs::default(), None) };
    if meta.len() > MAX_BYTES {
        return (Prefs::default(), Some(damaged(&path, &format!("it is {} bytes long; the limit is {MAX_BYTES}", meta.len()))));
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => return (Prefs::default(), Some(damaged(&path, &e.to_string()))),
    };
    match serde_json::from_str::<Prefs>(&text) {
        Ok(p) => (p, None),
        Err(e) => (Prefs::default(), Some(damaged(&path, &e.to_string()))),
    }
}

/// Sets a damaged preferences file aside and says where it went.
fn damaged(path: &Path, why: &str) -> String {
    let aside = set_aside(path);
    let where_ = aside.map(|p| p.display().to_string()).unwrap_or_else(|_| "nowhere (it could not be moved)".into());
    format!("the preferences could not be read ({why}); the file was set aside as {where_} and the defaults are used")
}

fn set_aside(path: &Path) -> std::io::Result<PathBuf> {
    for n in 1..1000 {
        let target = path.with_extension(format!("json.damaged-{n}"));
        if !target.exists() {
            std::fs::rename(path, &target)?;
            return Ok(target);
        }
    }
    Err(std::io::Error::other("too many damaged preference files"))
}

/// Writes the preferences atomically.
pub fn save(dir: &Path, prefs: &Prefs) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(prefs).map_err(|e| e.to_string())?;
    craft_atomic::atomic_write(&dir.join(FILE), &bytes).map_err(|e| e.to_string())
}

/// Applies every field to the shell.
pub fn apply(prefs: &Prefs, shell: &mut Shell) {
    shell.state = prefs.ui.clone();
    if let Some(warning) = unknown_language(&shell.state.language) {
        log::warn!("{warning}");
        shell.state.language = UiState::default().language;
        archforge_ui_egui::i18n::set_language(&shell.state.language);
    }
}

/// Sets the stored interface language; for one no language is registered under, what the log
/// says before English is used: the value and the accepted codes.
pub fn unknown_language(code: &str) -> Option<String> {
    (!archforge_ui_egui::i18n::set_language(code)).then(|| {
        let default = UiState::default().language;
        format!("the stored language {code:?} is not registered (accepted: {}); {default} is used", archforge_ui_egui::i18n::codes())
    })
}

/// The preferences as they stand now.
pub fn capture(shell: &Shell) -> Prefs {
    Prefs { ui: shell.state.clone() }
}
