//! Renders the window offscreen to a PNG, without opening one. Usage:
//! `cargo run -p archforge-ui-egui --features snapshot --example snapshot -- <out.png> [light|dark] [width height [scale]] [--screen <name>]`,
//! where `<name>` is a screen's name in `archforge_ui_egui::offscreen::SCREENS` (`home` when none is
//! given).

use archforge_ui_egui::offscreen::{SCREENS, Screen};
use archforge_ui_egui::theme::ThemeKind;

fn main() {
    if let Err(e) = run() {
        eprintln!("snapshot: {e}");
        std::process::exit(1);
    }
}

/// Reads the arguments, renders the screen asked for offscreen and writes it as a PNG.
fn run() -> Result<(), String> {
    const USAGE: &str = "usage: snapshot <out.png> [light|dark] [width height [scale]] [--screen <name>]";
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut screen = Screen::Home;
    if let Some(at) = args.iter().position(|a| a == "--screen") {
        let name = args.get(at + 1).map(String::as_str);
        screen = match name.and_then(Screen::named) {
            Some(screen) => screen,
            None => {
                let names: Vec<&str> = SCREENS.iter().map(|(name, _)| *name).collect();
                let listed = match names.split_last() {
                    Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
                    None => String::new(),
                };
                return Err(format!("--screen {name:?} is not {listed}; {USAGE}"));
            }
        };
        args.drain(at..(at + 2).min(args.len()));
    }
    let out = args.first().ok_or(USAGE)?;
    let theme = match args.get(1).map(String::as_str) {
        None | Some("light") => ThemeKind::Light,
        Some("dark") => ThemeKind::Dark,
        Some(other) => return Err(format!("{other:?} is not light or dark; {USAGE}")),
    };
    let size = |i: usize, default: f32| args.get(i).map_or(Ok(default), |v| v.parse::<f32>().map_err(|_| format!("{v:?} is not a number; {USAGE}")));
    let (w, h, rgba) = archforge_ui_egui::offscreen::render_screen(size(2, 1440.0)?, size(3, 900.0)?, size(4, 1.0)?, theme, screen)?;
    let file = std::fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut writer| writer.write_image_data(&rgba)).map_err(|e| format!("{out}: {e}"))
}
