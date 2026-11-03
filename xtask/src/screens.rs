//! `cargo xtask screens`: every screen of the application in both appearances, as release
//! evidence.
//!
//! Renders each screen the shell crate's `snapshot` example knows, in the light and the dark
//! appearance, into the ignored `plan/screens/`, and writes beside them one contact sheet per
//! appearance (`light.html`, `dark.html`) with every picture under its screen's name. The list of
//! screens is read from the snapshot example itself, so a screen added there is rendered here too.
//! The pictures are never committed; a picture is compared by eye, not by pixel, so a font or
//! theme change never breaks the gate.

use std::path::Path;

use crate::util;

const SIZE: (u32, u32) = (1440, 900);
const THEMES: [&str; 2] = ["light", "dark"];

/// The screens the snapshot example's `--screen` takes, in its order: each `Some("<name>") =>
/// Screen::…` arm of its match.
pub fn screens(snapshot: &str) -> Vec<String> {
    snapshot.lines().filter_map(|line| line.trim().strip_prefix("Some(\"")?.split_once("\") => Screen::").map(|(name, _)| name.to_string())).collect()
}

fn render(screen: &str, theme: &str, out: &Path) -> Result<(), String> {
    let mut cmd = util::cargo();
    cmd.args(["run", "-q", "-p", "archforge-ui-egui", "--features", "snapshot", "--example", "snapshot", "--"]).arg(out).args([
        theme.to_string(),
        SIZE.0.to_string(),
        SIZE.1.to_string(),
        "1".to_string(),
        "--screen".to_string(),
        screen.to_string(),
    ]);
    util::run(cmd).map_err(|e| format!("the screen {screen} in {theme} could not be rendered: {e}"))
}

/// A contact sheet: every screen's picture in `theme`, under its name, in a grid.
fn sheet(screens: &[String], theme: &str) -> String {
    let (page, ink) = if theme == "dark" { ("#12171E", "#E6EBF2") } else { ("#F5F7FA", "#1B2B3C") };
    let figures: String = screens
        .iter()
        .map(|s| format!("<figure><a href=\"{s}-{theme}.png\"><img src=\"{s}-{theme}.png\" alt=\"{s}\" loading=\"lazy\"></a><figcaption>{s}</figcaption></figure>\n"))
        .collect();
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>Archforge screens, {theme}</title>\n<style>\
         body{{margin:24px;background:{page};color:{ink};font:14px system-ui,sans-serif}}\
         main{{display:grid;grid-template-columns:repeat(auto-fill,minmax(360px,1fr));gap:20px}}\
         figure{{margin:0}}img{{width:100%;border:1px solid #8884;border-radius:6px}}\
         figcaption{{margin-top:6px}}</style>\n<h1>Archforge screens, {theme} ({count})</h1>\n<main>\n{figures}</main>\n",
        count = screens.len()
    )
}

pub fn run() -> Result<(), String> {
    let root = util::root();
    let snapshot = root.join("crates/ui-egui/examples/snapshot.rs");
    let text = std::fs::read_to_string(&snapshot).map_err(|e| format!("{}: {e}", snapshot.display()))?;
    let screens = screens(&text);
    if screens.is_empty() {
        return Err(format!("{} names no screen", snapshot.display()));
    }
    let out = root.join("plan/screens");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    for theme in THEMES {
        for screen in &screens {
            render(screen, theme, &out.join(format!("{screen}-{theme}.png")))?;
        }
        let page = out.join(format!("{theme}.html"));
        craft_atomic::atomic_write(&page, sheet(&screens, theme).as_bytes()).map_err(|e| format!("{}: {e}", page.display()))?;
    }
    println!("screens: {} screens in {} appearances, with light.html and dark.html, under {}", screens.len(), THEMES.len(), out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screens_are_read_from_the_snapshot_examples_match() {
        let text = "    let screen = match x {\n        Some(\"home\") => Screen::Home,\n        Some(\"new-project\") => Screen::NewProject,\n        None => Screen::Home,\n";
        assert_eq!(screens(text), ["home", "new-project"]);
        let page = sheet(&["home".into()], "dark");
        assert!(page.contains("home-dark.png") && page.contains("<figcaption>home</figcaption>") && page.contains("(1)"));
    }

    /// Every screen the offscreen renderer can draw is one the snapshot example names, so the
    /// command renders them all.
    #[test]
    fn every_screen_the_renderer_draws_is_rendered() {
        let root = util::root();
        let snapshot = std::fs::read_to_string(root.join("crates/ui-egui/examples/snapshot.rs")).unwrap();
        let offscreen = std::fs::read_to_string(root.join("crates/ui-egui/src/offscreen.rs")).unwrap();
        let named = screens(&snapshot);
        let variants: Vec<&str> = offscreen
            .split("pub enum Screen {")
            .nth(1)
            .and_then(|rest| rest.split("\n}").next())
            .unwrap()
            .lines()
            .map(str::trim)
            .filter(|l| l.ends_with(',') && l.chars().next().is_some_and(|c| c.is_ascii_uppercase()))
            .map(|l| l.trim_end_matches(','))
            .collect();
        assert!(!variants.is_empty());
        for v in &variants {
            assert!(snapshot.contains(&format!("=> Screen::{v},")), "the snapshot example names no screen for Screen::{v}");
        }
        assert_eq!(named.len(), variants.len(), "one name per screen: {named:?}");
    }
}
