//! `cargo xtask shot`: a screenshot and a small copy that is quick to read.
//!
//! Renders the window offscreen through the shell crate's `snapshot` example, in the light or dark
//! appearance at `--size` logical pixels (1440 by 900 by default), showing Home or, with
//! `--screen new-project`, the New project dialog over it. The full image is kept under the
//! ignored `plan/shots/`, and beside it a copy drawn at the scale that keeps its longer edge at most
//! `--max-edge` pixels (1024 by default).

use crate::util;

const DEFAULT_MAX_EDGE: u32 = 1024;
const DEFAULT_SIZE: (u32, u32) = (1440, 900);
const USAGE: &str =
    "usage: cargo xtask shot [light|dark] [--size <width>x<height>] [--max-edge <pixels>] [--screen home|new-project|new-project-failed]";
/// The screens a shot can show.
const SCREENS: &[&str] = &["home", "new-project", "new-project-failed"];

#[derive(Debug, PartialEq)]
pub struct Options {
    pub theme: &'static str,
    pub size: (u32, u32),
    pub max_edge: u32,
    pub screen: String,
}

/// Reads the arguments; an unknown option names the accepted ones.
pub fn options(args: &[String]) -> Result<Options, String> {
    let mut o = Options { theme: "light", size: DEFAULT_SIZE, max_edge: DEFAULT_MAX_EDGE, screen: "home".to_string() };
    let positive = |v: &str, what: &str| {
        v.parse::<u32>().ok().filter(|n| (1..=16384).contains(n)).ok_or_else(|| format!("{what} {v:?} is not a whole number from 1 to 16384"))
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "light" => o.theme = "light",
            "dark" => o.theme = "dark",
            "--screen" => {
                let v = it.next().ok_or("--screen needs a screen")?;
                if !SCREENS.contains(&v.as_str()) {
                    return Err(format!("--screen {v:?} is not one of {}", SCREENS.join(", ")));
                }
                o.screen = v.clone();
            }
            "--max-edge" => o.max_edge = positive(it.next().ok_or("--max-edge needs a number")?, "--max-edge")?,
            "--size" => {
                let v = it.next().ok_or("--size needs <width>x<height>")?;
                let (w, h) = v.split_once('x').ok_or_else(|| format!("--size {v:?} is not <width>x<height>"))?;
                o.size = (positive(w, "--size width")?, positive(h, "--size height")?);
            }
            other => return Err(format!("shot: unknown argument {other:?}\n{USAGE}")),
        }
    }
    Ok(o)
}

fn render(o: &Options, out: &std::path::Path, scale: f32) -> Result<(), String> {
    let mut cmd = util::cargo();
    cmd.args(["run", "-q", "-p", "archforge-ui-egui", "--features", "snapshot", "--example", "snapshot", "--"]).arg(out).args([
        o.theme.to_string(),
        o.size.0.to_string(),
        o.size.1.to_string(),
        scale.to_string(),
        "--screen".to_string(),
        o.screen.clone(),
    ]);
    util::run(cmd)
}

pub fn run(args: &[String]) -> Result<(), String> {
    let o = options(args)?;
    let dir = util::root().join("plan/shots");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("shot: the clock reads before 1970 ({e}); the file names would collide"))?
        .as_millis();
    let free = (0..100u32)
        .map(|n| if n == 0 { format!("shot-{stamp}") } else { format!("shot-{stamp}-{n}") })
        .find(|base| !dir.join(format!("{base}.png")).exists() && !dir.join(format!("{base}-small.png")).exists());
    let base = free.ok_or_else(|| format!("shot: no free file name under {}", dir.display()))?;
    let full = dir.join(format!("{base}.png"));
    let small = dir.join(format!("{base}-small.png"));
    render(&o, &full, 1.0)?;
    let longer = o.size.0.max(o.size.1) as f32;
    render(&o, &small, (o.max_edge as f32 / longer).min(1.0))?;
    println!("shot: full image {}", full.display());
    println!("shot: small copy {}", small.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn shot_reads_the_appearance_a_size_and_a_positive_edge() {
        assert_eq!(options(&args(&[])).unwrap(), Options { theme: "light", size: (1440, 900), max_edge: 1024, screen: "home".into() });
        assert_eq!(
            options(&args(&["dark", "--size", "1600x1000", "--max-edge", "640"])).unwrap(),
            Options { theme: "dark", size: (1600, 1000), max_edge: 640, screen: "home".into() }
        );
        assert!(options(&args(&["--max-edge", "0"])).is_err());
        assert!(options(&args(&["--size", "1600"])).is_err());
        assert!(options(&args(&["--bogus"])).is_err());
        assert_eq!(options(&args(&["--screen", "new-project"])).unwrap().screen, "new-project");
        assert!(options(&args(&["--screen", "elsewhere"])).is_err());
    }
}
