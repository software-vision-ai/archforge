//! Build metadata baked in at compile time.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: Option<&str> = option_env!("ARCHFORGE_BUILD_SHA");
pub const DATE: Option<&str> = option_env!("ARCHFORGE_BUILD_DATE");

/// What `--version` prints: version, commit and date, or "dev build".
pub fn version_line() -> String {
    match (COMMIT, DATE) {
        (Some(commit), Some(date)) if !commit.is_empty() => format!("archforge {VERSION} ({commit}, {date})"),
        _ => format!("archforge {VERSION} (dev build)"),
    }
}
