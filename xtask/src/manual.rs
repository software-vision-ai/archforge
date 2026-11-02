//! `cargo xtask manual`: the user manual with its pictures.
//!
//! The manual's text lives in `docs/manual/`. Its pictures are never committed: each is a line
//! `<!-- picture: <screen> <light|dark> | <what it shows> -->` naming a screen of the shell crate's
//! `snapshot` example, rendered from the real interface when the manual is built. The built manual
//! goes to the ignored `plan/manual/`: every chapter with its pictures in place, and the pictures
//! beside them, so it never drifts from the application.

use std::path::{Path, PathBuf};

use crate::util;

/// A picture a chapter asks for.
#[derive(Debug, PartialEq, Eq)]
pub struct Picture {
    pub screen: String,
    pub theme: &'static str,
    pub alt: String,
}

impl Picture {
    pub fn file(&self) -> String {
        format!("{}-{}.png", self.screen, self.theme)
    }
}

const SIZE: (u32, u32) = (1440, 900);

/// The picture a line asks for, or why it cannot be read; none for any other line.
pub fn picture(line: &str) -> Option<Result<Picture, String>> {
    let body = line.trim().strip_prefix("<!-- picture:")?.strip_suffix("-->")?;
    let (head, alt) = match body.split_once('|') {
        Some((head, alt)) if !alt.trim().is_empty() => (head, alt.trim()),
        _ => return Some(Err(format!("{line:?} has no description after `|`"))),
    };
    let mut words = head.split_whitespace();
    let (Some(screen), Some(theme), None) = (words.next(), words.next(), words.next()) else {
        return Some(Err(format!("{line:?} is not `<!-- picture: <screen> <light|dark> | <what it shows> -->`")));
    };
    let theme = match theme {
        "light" => "light",
        "dark" => "dark",
        other => return Some(Err(format!("{line:?}: {other:?} is neither light nor dark"))),
    };
    Some(Ok(Picture { screen: screen.to_string(), theme, alt: alt.to_string() }))
}

/// The manual's chapters, in order.
pub fn chapters(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "md")).collect();
    files.sort();
    Ok(files)
}

/// A chapter with its pictures in place, and the pictures it asks for.
pub fn build(text: &str) -> Result<(String, Vec<Picture>), String> {
    let mut out = String::with_capacity(text.len());
    let mut pictures = Vec::new();
    for line in text.lines() {
        match picture(line) {
            Some(Ok(p)) => {
                out.push_str(&format!("![{}]({})\n", p.alt, p.file()));
                pictures.push(p);
            }
            Some(Err(e)) => return Err(e),
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    Ok((out, pictures))
}

fn render(p: &Picture, out: &Path) -> Result<(), String> {
    let mut cmd = util::cargo();
    cmd.args(["run", "-q", "-p", "archforge-ui-egui", "--features", "snapshot", "--example", "snapshot", "--"]).arg(out).args([
        p.theme.to_string(),
        SIZE.0.to_string(),
        SIZE.1.to_string(),
        "1".to_string(),
        "--screen".to_string(),
        p.screen.clone(),
    ]);
    util::run(cmd)
}

pub fn run() -> Result<(), String> {
    let root = util::root();
    let out = root.join("plan/manual");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut rendered = std::collections::BTreeSet::new();
    for chapter in chapters(&root.join("docs/manual"))? {
        let text = std::fs::read_to_string(&chapter).map_err(|e| format!("{}: {e}", chapter.display()))?;
        let (built, pictures) = build(&text).map_err(|e| format!("{}: {e}", chapter.display()))?;
        for p in pictures {
            if rendered.insert(p.file()) {
                render(&p, &out.join(p.file()))?;
            }
        }
        let name = chapter.file_name().ok_or_else(|| format!("{}: no file name", chapter.display()))?;
        std::fs::write(out.join(name), built).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    println!("manual: {} pictures and the chapters under {}", rendered.len(), out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_line_names_a_screen_a_theme_and_what_it_shows() {
        let p = picture("<!-- picture: workspace-diagram dark | Two tables joined by a line -->").unwrap().unwrap();
        assert_eq!(p, Picture { screen: "workspace-diagram".into(), theme: "dark", alt: "Two tables joined by a line".into() });
        assert_eq!(p.file(), "workspace-diagram-dark.png");
        assert!(picture("Just text.").is_none());
        assert!(picture("<!-- picture: home -->").unwrap().is_err(), "no description");
        assert!(picture("<!-- picture: home grey | Home -->").unwrap().is_err(), "no such theme");
        assert!(picture("<!-- picture: home light extra | Home -->").unwrap().is_err(), "one screen and one theme");
        let (built, pictures) = build("# Title\n<!-- picture: home light | Home -->\nText.\n").unwrap();
        assert_eq!(built, "# Title\n![Home](home-light.png)\nText.\n");
        assert_eq!(pictures.len(), 1);
    }

    /// Every chapter's pictures name a screen the snapshot example draws, and every link between
    /// chapters, or to a file of the repository, leads somewhere.
    #[test]
    fn the_manual_names_only_screens_that_exist_and_links_only_to_files_that_do() {
        let root = util::root();
        let snapshot = std::fs::read_to_string(root.join("crates/ui-egui/examples/snapshot.rs")).unwrap();
        let dir = root.join("docs/manual");
        let files = chapters(&dir).unwrap();
        assert!(files.len() >= 3, "the manual has its chapters");
        for chapter in files {
            let text = std::fs::read_to_string(&chapter).unwrap();
            let (_, pictures) = build(&text).unwrap_or_else(|e| panic!("{}: {e}", chapter.display()));
            for p in pictures {
                assert!(snapshot.contains(&format!("Some(\"{}\")", p.screen)), "{}: no snapshot screen {:?}", chapter.display(), p.screen);
            }
            for target in text.split("](").skip(1).filter_map(|rest| rest.split(')').next()) {
                let target = target.split('#').next().unwrap_or_default();
                if target.is_empty() || target.contains("://") || target.starts_with("mailto:") {
                    continue;
                }
                assert!(dir.join(target).exists(), "{}: the link {target:?} leads nowhere", chapter.display());
            }
        }
    }
}
