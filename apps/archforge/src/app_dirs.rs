//! Where the application keeps its settings, logs and graphics marker.

use std::path::PathBuf;

pub struct AppDirs {
    pub settings: PathBuf,
}

impl AppDirs {
    /// The per-user settings directory, or a folder in the temporary directory when the platform
    /// gives none.
    pub fn discover() -> Self {
        // The same folder archforge-cli looks in for the control token.
        let settings = archforge_control::settings_folder();
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
