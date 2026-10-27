//! Where the application keeps its settings, logs and graphics marker.

use std::path::PathBuf;

pub struct AppDirs {
    pub settings: PathBuf,
}

impl AppDirs {
    /// The per-user settings directory, or a folder in the temporary directory when the platform
    /// gives none.
    pub fn discover() -> Self {
        let settings = settings_dir("Archforge").unwrap_or_else(|| std::env::temp_dir().join("archforge"));
        if let Err(e) = std::fs::create_dir_all(&settings) {
            log::warn!("cannot create the settings directory {}: {e}", settings.display());
        }
        Self { settings }
    }

    pub fn logs(&self) -> PathBuf {
        self.settings.join("logs")
    }

    pub fn gpu_marker(&self) -> PathBuf {
        self.settings.join("gpu-starting")
    }
}

/// The settings directory for `app` (the display name), following each platform's convention.
fn settings_dir(app: &str) -> Option<PathBuf> {
    let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        var("APPDATA").map(|d| d.join(app))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|h| h.join("Library").join("Application Support").join(app))
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config"))).map(|d| d.join(app.to_lowercase().replace(' ', "-")))
    }
}
