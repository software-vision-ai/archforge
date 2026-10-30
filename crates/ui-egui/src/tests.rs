//! The shell's tests: Home drawn headless in both appearances, its controls and its lists, the
//! theme switch and the banner, the UI state, and the installed fonts.

use egui_kittest::Harness;

use crate::theme::ThemeKind;
use crate::{Services, Shell, UiState};

/// A click of the primary button at `at`, pointer moved there first.
fn click(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

/// The real shell in the harness at the design's frame size.
fn harness(theme: ThemeKind) -> Harness<'static, Shell> {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services { user_name: Some("Robin Meyer".into()), ..Services::default() });
    shell.state.theme = theme;
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    h
}

fn ids(h: &Harness<'static, Shell>) -> Vec<String> {
    h.state().widgets().iter().map(|w| w.id.clone()).collect()
}

#[test]
fn home_draws_in_both_appearances_with_every_control_registered() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let h = harness(theme);
        let ids = ids(&h);
        for id in ["strip.home", "strip.theme", "home.header.search", "home.open-project", "home.pin-folder", "home.banner.close"] {
            assert!(ids.iter().any(|i| i == id), "{theme:?}: {id} in {ids:?}");
        }
        assert_eq!(ids.iter().filter(|i| i.starts_with("home.start.")).count(), 4, "the four start cards");
        assert_eq!(h.ctx.theme(), if theme.is_dark() { egui::Theme::Dark } else { egui::Theme::Light });
    }
}

#[test]
fn home_lists_its_pinned_folders_and_recent_projects_in_both_appearances() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        crate::i18n::set_language("en");
        let mut shell = Shell::new(Services::default());
        shell.state.theme = theme;
        shell.home.pinned.push(crate::home::Pinned { name: "Models".into(), path: "~/Documents/Models".into() });
        shell.home.recent.push(crate::home::Recent {
            name: "Library system".into(),
            last_view: "Overview".into(),
            elements: 3,
            opened: "Today, 10:24".into(),
        });
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1400.0)).build_ui_state(
            |ui, shell: &mut Shell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            shell,
        );
        h.run();
        let ids = ids(&h);
        for id in ["home.pinned.0", "home.recent.0", "home.pin-folder"] {
            assert!(ids.iter().any(|i| i == id), "{theme:?}: {id} in {ids:?}");
        }
        assert!(!ids.iter().any(|i| i == "home.pinned.1" || i == "home.recent.1"), "{theme:?}: one of each in {ids:?}");
    }
}

#[test]
fn the_theme_switch_flips_the_appearance_and_the_banner_stays_closed() {
    let mut h = harness(ThemeKind::Light);
    let centre = |h: &Harness<'static, Shell>, id: &str| {
        h.state().widgets().iter().find(|w| w.id == id).map(|w| egui::pos2((w.rect[0] + w.rect[2]) / 2.0, (w.rect[1] + w.rect[3]) / 2.0)).unwrap()
    };
    let at = centre(&h, "strip.theme");
    click(&mut h, at);
    assert_eq!(h.state().state.theme, ThemeKind::Dark);
    let at = centre(&h, "home.banner.close");
    click(&mut h, at);
    assert!(h.state().state.home_banner_closed);
    assert!(!ids(&h).iter().any(|i| i == "home.banner.close"), "a closed banner is not drawn");
}

/// The centre of a registered control.
fn centre_of(h: &Harness<'static, Shell>, id: &str) -> egui::Pos2 {
    let w = h.state().widgets().iter().find(|w| w.id == id).unwrap_or_else(|| panic!("{id} is not drawn"));
    egui::pos2((w.rect[0] + w.rect[2]) / 2.0, (w.rect[1] + w.rect[3]) / 2.0)
}

fn open_new_project(h: &mut Harness<'static, Shell>) {
    let at = centre_of(h, "home.start.new-project");
    click(h, at);
    h.run();
    assert!(h.state().new_project.is_some(), "the New project card opens the dialog");
}

#[test]
fn the_new_project_dialog_opens_from_home_in_both_appearances_with_its_controls() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = harness(theme);
        open_new_project(&mut h);
        let ids = ids(&h);
        for id in ["new-project.name", "new-project.location.change", "new-project.close", "new-project.cancel", "new-project.create"] {
            assert!(ids.iter().any(|i| i == id), "{theme:?}: {id} in {ids:?}");
        }
    }
}

#[test]
fn the_name_field_has_the_keyboard_and_takes_a_name() {
    let mut h = harness(ThemeKind::Light);
    open_new_project(&mut h);
    h.event(egui::Event::Text("Commerce platform".into()));
    h.run();
    assert_eq!(h.state().new_project.as_ref().map(|d| d.name.as_str()), Some("Commerce platform"));
}

#[test]
fn cancel_the_close_button_and_escape_close_the_dialog() {
    let mut h = harness(ThemeKind::Light);
    for way in ["new-project.cancel", "new-project.close", "escape"] {
        open_new_project(&mut h);
        if way == "escape" {
            h.key_press(egui::Key::Escape);
        } else {
            let at = centre_of(&h, way);
            click(&mut h, at);
        }
        h.run();
        assert!(h.state().new_project.is_none(), "{way} closes the dialog");
    }
}

#[test]
fn create_project_waits_for_creating_projects_and_home_takes_no_click_behind_the_dialog() {
    let mut h = harness(ThemeKind::Light);
    open_new_project(&mut h);
    h.event(egui::Event::Text("Commerce platform".into()));
    h.run();
    let at = centre_of(&h, "new-project.create");
    click(&mut h, at);
    assert!(h.state().new_project.is_some(), "Create project is not available yet and leaves the dialog open");
    let at = centre_of(&h, "strip.theme");
    click(&mut h, at);
    assert_eq!(h.state().state.theme, ThemeKind::Light, "the theme switch behind the veil took the click");
}

#[test]
fn in_the_smallest_window_the_dialog_keeps_its_buttons_and_field_inside_it() {
    crate::i18n::set_language("en");
    let shell = Shell::new(Services::default());
    let mut h = Harness::builder().with_size(egui::vec2(640.0, 400.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    crate::new_project::open(h.state_mut());
    h.run();
    h.run();
    let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 400.0));
    for id in ["new-project.name", "new-project.cancel", "new-project.create", "new-project.close"] {
        let w = h.state().widgets().iter().find(|w| w.id == id).unwrap_or_else(|| panic!("{id} is not drawn"));
        let r = egui::Rect::from_min_max(egui::pos2(w.rect[0], w.rect[1]), egui::pos2(w.rect[2], w.rect[3]));
        assert!(window.contains_rect(r), "{id} at {r:?} leaves the window");
    }
}

/// A host that keeps each creation's sender, so a test answers when it chooses.
#[derive(Default)]
struct FakeHost {
    pending: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<crate::Opened, crate::CreateError>>>>,
    asked: std::sync::Mutex<Vec<(std::path::PathBuf, String)>>,
}

impl crate::ProjectHost for FakeHost {
    fn create(&self, folder: &std::path::Path, name: &str) -> std::sync::mpsc::Receiver<Result<crate::Opened, crate::CreateError>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.pending.lock().unwrap().push(tx);
        self.asked.lock().unwrap().push((folder.to_path_buf(), name.to_string()));
        rx
    }
}

/// The real shell with a fake host and the usual folder, the dialog open and a name typed.
fn creating_harness() -> (Harness<'static, Shell>, std::sync::Arc<FakeHost>) {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(FakeHost::default());
    let folder = crate::Folder { path: "/Users/robin/Documents/Archforge".into(), label: "Documents / Archforge".into() };
    let services = Services { projects: Some(host.clone()), projects_folder: Some(folder), ..Services::default() };
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        Shell::new(services),
    );
    h.run();
    open_new_project(&mut h);
    h.event(egui::Event::Text("Commerce platform".into()));
    h.run();
    (h, host)
}

/// A click while the dialog waits: it repaints until the answer comes, so a few frames are run
/// rather than waiting for the window to settle.
fn press(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run_steps(4);
}

fn answer(h: &mut Harness<'static, Shell>, host: &FakeHost, result: Result<crate::Opened, crate::CreateError>) {
    let tx = host.pending.lock().unwrap().pop().expect("a creation was asked for");
    tx.send(result).unwrap();
    h.run_steps(4);
}

#[test]
fn create_project_asks_the_host_and_opens_the_project_it_made() {
    let (mut h, host) = creating_harness();
    let at = centre_of(&h, "new-project.create");
    press(&mut h, at);
    assert_eq!(host.asked.lock().unwrap().as_slice(), [("/Users/robin/Documents/Archforge".into(), "Commerce platform".to_string())]);
    assert!(h.state().new_project.as_ref().is_some_and(|d| d.creating()), "the dialog waits");
    // While it waits, nothing closes the dialog and a second click asks nothing.
    for way in ["new-project.cancel", "new-project.close", "new-project.create"] {
        let at = centre_of(&h, way);
        press(&mut h, at);
    }
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    assert!(h.state().new_project.is_some());
    assert_eq!(host.asked.lock().unwrap().len(), 1);
    let path = std::path::PathBuf::from("/Users/robin/Documents/Archforge/Commerce platform.archforge");
    answer(&mut h, &host, Ok(crate::Opened { name: "Commerce platform".into(), path: path.clone() }));
    assert!(h.state().new_project.is_none(), "success closes the dialog");
    assert_eq!(h.state().project.as_ref().map(|p| p.path.clone()), Some(path));
    assert_eq!(h.state().title(), "Commerce platform — Archforge");
    assert_eq!(h.state().home.recent.first().map(|r| r.name.as_str()), Some("Commerce platform"));
}

#[test]
fn enter_in_the_name_field_creates_too() {
    let (mut h, host) = creating_harness();
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    assert_eq!(host.asked.lock().unwrap().len(), 1);
}

#[test]
fn a_failed_creation_keeps_the_dialog_and_the_name_and_says_why() {
    let (mut h, host) = creating_harness();
    for failure in [
        crate::CreateError::NameTaken,
        crate::CreateError::Unfinished,
        crate::CreateError::DoltMissing { detail: "no dolt on the PATH".into() },
        crate::CreateError::Failed { detail: "the server did not start".into() },
    ] {
        let at = centre_of(&h, "new-project.create");
        press(&mut h, at);
        answer(&mut h, &host, Err(failure.clone()));
        let dialog = h.state().new_project.as_ref().expect("the dialog stays");
        assert!(!dialog.creating(), "{failure:?}: it no longer waits");
        assert_eq!(dialog.name, "Commerce platform");
        assert!(h.state().project.is_none());
    }
    // A host thread that ended without answering is a failure too, not a wait for ever.
    let at = centre_of(&h, "new-project.create");
    press(&mut h, at);
    host.pending.lock().unwrap().clear();
    h.run_steps(4);
    assert!(h.state().new_project.as_ref().is_some_and(|d| !d.creating()));
}

#[test]
fn every_ui_state_field_is_read() {
    // Every UI state field is read somewhere outside its own definition.
    let value = serde_json::to_value(UiState::default()).unwrap();
    let fields: Vec<&String> = value.as_object().unwrap().keys().collect();
    let mut sources = String::new();
    fn collect(dir: &std::path::Path, sources: &mut String) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, sources);
            } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("state.rs") && !path.ends_with("tests.rs") {
                sources.push_str(&std::fs::read_to_string(path).unwrap());
            }
        }
    }
    collect(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")), &mut sources);
    let unread: Vec<_> = fields.iter().filter(|f| !sources.contains(&format!(".{f}"))).collect();
    assert!(unread.is_empty(), "UI state fields nothing reads: {unread:?}");
}

/// A context that has run one frame, so its fonts are there.
fn framed_context() -> egui::Context {
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(Default::default(), |_| {});
    out.textures_delta.clear();
    ctx
}

/// Installed fonts over a fake source of egui's own bundled faces: no system font is read. A
/// language whose script the bundled faces do not draw takes an installed family behind them.
mod installed_fonts {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::fonts::{FontEntry, FontSource};
    use crate::i18n;

    fn bytes(key: &str) -> Vec<u8> {
        let defs = egui::FontDefinitions::default();
        match key {
            // Cut inside its table directory: refused by the parser egui loads fonts with.
            "broken" => defs.font_data["Hack"].font[..40].to_vec(),
            key => defs.font_data[key].font.to_vec(),
        }
    }

    /// One whole window frame; the viewport commands it sent.
    fn frame(s: &mut Shell, ctx: &egui::Context) -> Vec<egui::ViewportCommand> {
        let input =
            egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            s.logic(ui.ctx());
            s.ui(ui);
        });
        out.textures_delta.clear();
        out.viewport_output.remove(&egui::ViewportId::ROOT).map(|v| v.commands).unwrap_or_default()
    }

    struct Panicking;

    impl FontSource for Panicking {
        fn scan(&self, _cap: u64) -> Vec<FontEntry> {
            panic!("a scan that fails");
        }
        fn read(&self, _entry: &FontEntry, _cap: u64) -> Result<Vec<u8>, String> {
            Err("never read".into())
        }
    }

    #[test]
    fn a_scan_that_panics_leaves_nothing_to_wait_for() {
        let mut s = Shell::new(Services::default());
        s.services.fonts = Some(Arc::new(Panicking));
        let ctx = framed_context();
        frame(&mut s, &ctx);
        s.fonts.wait(&ctx, Duration::from_secs(5));
        assert!(s.fonts.fallback_order().is_empty());
        assert!(s.fonts.settled(&ctx), "the scan reported: nothing in flight");
    }

    /// Installed fonts named as the script fallbacks, each served from a file of its own: egui's
    /// bundled faces stand in for them, or real faces for the glyph test.
    struct Scripts(Vec<(&'static str, Vec<u8>)>);

    impl FontSource for Scripts {
        fn scan(&self, _cap: u64) -> Vec<FontEntry> {
            self.0
                .iter()
                .map(|(family, b)| FontEntry {
                    families: vec![family.to_string()],
                    post_script: String::new(),
                    weight: 400,
                    italic: false,
                    stretch: 5,
                    key: family.to_string(),
                    index: 0,
                    len: b.len() as u64,
                })
                .collect()
        }
        fn read(&self, entry: &FontEntry, _cap: u64) -> Result<Vec<u8>, String> {
            self.0.iter().find(|(f, _)| *f == entry.key).map(|(_, b)| b.clone()).ok_or_else(|| "gone".to_string())
        }
    }

    fn scripted(source: Scripts) -> (Shell, egui::Context) {
        let mut s = Shell::new(Services::default());
        s.services.fonts = Some(Arc::new(source));
        let ctx = framed_context();
        frame(&mut s, &ctx);
        s.fonts.wait(&ctx, Duration::from_secs(5));
        (s, ctx)
    }

    /// Frames until `done` holds, at most 5 s.
    fn until(s: &mut Shell, ctx: &egui::Context, done: impl Fn(&Shell) -> bool) {
        for _ in 0..500 {
            frame(s, ctx);
            s.fonts.wait(ctx, Duration::from_millis(10));
            if done(s) {
                frame(s, ctx);
                return;
            }
        }
        panic!("the fallback fonts did not settle: {:?}", s.fonts.fallback_order());
    }

    fn proportional(ctx: &egui::Context) -> Vec<String> {
        ctx.fonts(|f| f.definitions().families[&egui::FontFamily::Proportional].clone())
    }

    #[test]
    fn an_installed_cjk_family_goes_behind_every_family_in_the_order_the_language_wants() {
        let hack = bytes("Hack");
        let (mut s, ctx) = scripted(Scripts(vec![("Hiragino Sans", hack.clone()), ("Noto Sans CJK SC", hack.clone()), ("Hack", hack)]));
        s.state.language = "en".into();
        i18n::set_language("en");
        // In English only the Chinese fallback is loaded: it draws kana and the language list too.
        until(&mut s, &ctx, |s| s.fonts.fallback_order().len() == 1);
        assert_eq!(s.fonts.fallback_order(), ["fallback:Noto Sans CJK SC"]);
        let chain = proportional(&ctx);
        assert_eq!(chain.last().map(String::as_str), Some("fallback:Noto Sans CJK SC"), "{chain:?}");
        assert_eq!(chain.first().map(String::as_str), Some("Ubuntu-Light"), "the bundled face still draws Latin text");
        assert_eq!(ctx.fonts(|f| f.definitions().families[&egui::FontFamily::Monospace].last().cloned()), Some("fallback:Noto Sans CJK SC".into()));
        // Japanese loads its own family and puts it first, so kanji take the Japanese forms.
        s.state.language = "ja".into();
        i18n::set_language("ja");
        until(&mut s, &ctx, |s| s.fonts.fallback_order().len() == 2);
        assert_eq!(s.fonts.fallback_order(), ["fallback:Hiragino Sans", "fallback:Noto Sans CJK SC"]);
        let chain = proportional(&ctx);
        assert_eq!(chain[chain.len() - 2..], ["fallback:Hiragino Sans".to_string(), "fallback:Noto Sans CJK SC".to_string()], "{chain:?}");
        // Back in Chinese the Chinese family leads again; nothing is loaded twice.
        s.state.language = "zh-Hans".into();
        i18n::set_language("zh-Hans");
        until(&mut s, &ctx, |s| s.fonts.fallback_order().first().is_some_and(|k| k == "fallback:Noto Sans CJK SC"));
        assert_eq!(s.fonts.fallback_order(), ["fallback:Noto Sans CJK SC", "fallback:Hiragino Sans"]);
        assert_eq!(s.fonts.in_flight(), 0);
        // Each fallback is given to egui once, moved onto the bundled face's baseline.
        let data = ctx.fonts(|f| f.definitions().font_data.clone());
        for family in ["Hiragino Sans", "Noto Sans CJK SC"] {
            assert_ne!(data[&format!("fallback:{family}")].tweak.y_offset_factor, 0.0, "{family} as a fallback sits on the bundled baseline");
            assert_eq!(data.keys().filter(|k| k.ends_with(family)).count(), 1, "{family} once: {:?}", data.keys());
        }
        let shift = crate::fonts::baseline_shift(&bytes("Hack"), 0, &bytes("Ubuntu-Light"), 0);
        assert_eq!(data["fallback:Hiragino Sans"].tweak.y_offset_factor, shift);
        assert_eq!(
            crate::fonts::baseline_shift(&bytes("Ubuntu-Light"), 0, &bytes("Ubuntu-Light"), 0),
            0.0,
            "a face on its own baseline moves nowhere"
        );
        assert_eq!(crate::fonts::baseline_shift(b"not a font", 0, &bytes("Ubuntu-Light"), 0), 0.0);
        // Served, nothing is said.
        assert_eq!((s.fonts.unserved("ja"), s.fonts.unserved("zh-Hans"), s.fonts.take_notice()), (None, None, None));
        s.state.language = "en".into();
        i18n::set_language("en");
    }

    #[test]
    fn without_an_installed_cjk_family_nothing_is_asked_for_and_the_fonts_settle() {
        let (mut s, ctx) = scripted(Scripts(vec![("Hack", bytes("Hack"))]));
        s.state.language = "ja".into();
        i18n::set_language("ja");
        for _ in 0..3 {
            frame(&mut s, &ctx);
        }
        assert!(s.fonts.fallback_order().is_empty());
        assert_eq!(s.fonts.in_flight(), 0);
        assert!(s.fonts.settled(&ctx), "a screenshot does not wait for a font that is not there");
        // Logged once, in English.
        use crate::fonts::Unserved;
        assert_eq!(
            (s.fonts.unserved("ja"), s.fonts.unserved("zh-Hans"), s.fonts.unserved("de")),
            (Some(Unserved::NoneInstalled), Some(Unserved::NoneInstalled), None)
        );
        assert_eq!(s.font_notices, [crate::fonts::unserved_message("ja")]);
        assert!(s.font_notices[0].starts_with("日本語 (ja) needs a font"), "{}", s.font_notices[0]);
        frame(&mut s, &ctx);
        assert_eq!(s.font_notices.len(), 1, "once");
        // Installed but refused at a limit: the cause is a refusal, not a missing font.
        let (mut s, ctx) = scripted(Scripts(vec![("Noto Sans CJK SC", bytes("broken"))]));
        until(&mut s, &ctx, |s| s.fonts.in_flight() == 0 && s.fonts.unserved("zh-Hans").is_some());
        assert_eq!(s.fonts.unserved("zh-Hans"), Some(Unserved::Refused));
        // A host with no fonts at all (the web) knows at once.
        let mut web = Shell::new(Services::default());
        let ctx = framed_context();
        frame(&mut web, &ctx);
        assert_eq!(web.fonts.unserved("ja"), Some(Unserved::NoSource));
        // A broken Chinese family is passed over for the next one installed.
        let (mut s, ctx) = scripted(Scripts(vec![("PingFang SC", bytes("broken")), ("Hiragino Sans GB", bytes("Hack"))]));
        s.state.language = "en".into();
        i18n::set_language("en");
        until(&mut s, &ctx, |s| !s.fonts.fallback_order().is_empty());
        assert_eq!(s.fonts.fallback_order(), ["fallback:Hiragino Sans GB"]);
    }

    /// The glyph test: with the fonts input there (`CRAFT_FONTS_DIR`), a Japanese and a
    /// Chinese face from it, served as installed fonts, draw every character of the Japanese and
    /// Chinese catalogues and of the languages' own names; without it the test says so and passes.
    #[test]
    fn the_cjk_fallbacks_draw_every_character_of_the_japanese_and_chinese_catalogues() {
        let Some(dir) = std::env::var_os("CRAFT_FONTS_DIR").map(std::path::PathBuf::from) else {
            println!("CRAFT_FONTS_DIR is not set: the CJK glyph test is skipped");
            return;
        };
        let read = |rel: &str| std::fs::read(dir.join(rel));
        let (Ok(jp), Ok(sc)) = (read("fonts/biz-ud-pgothic/BIZUDPGothic-Regular.ttf"), read("fonts/noto-sans-cjk-sc/NotoSansCJKsc-Regular.otf"))
        else {
            println!("the fonts input lacks its Japanese or Chinese face: the CJK glyph test is skipped");
            return;
        };
        let (mut s, ctx) = scripted(Scripts(vec![("BIZ UDPGothic", jp), ("Noto Sans CJK SC", sc)]));
        for code in ["ja", "zh-Hans"] {
            i18n::set_language(code);
            until(&mut s, &ctx, |s| !s.fonts.fallback_order().is_empty());
            let lang = i18n::LANGUAGES.iter().find(|l| l.code == code).unwrap();
            let text = i18n::catalogue_text(lang);
            let mut wanted: std::collections::BTreeSet<char> = text
                .lines()
                .filter(|l| !l.starts_with('#'))
                .flat_map(|l| l.split('\t').nth(3).unwrap_or_default().chars().collect::<Vec<_>>())
                .collect();
            wanted.extend(i18n::LANGUAGES.iter().flat_map(|l| l.native_name.chars()));
            let missing: Vec<char> = ctx.fonts_mut(|f| {
                let covered = f.fonts.font(&egui::FontFamily::Proportional).characters().clone();
                wanted.iter().copied().filter(|c| !c.is_whitespace() && !covered.contains_key(c)).collect()
            });
            assert!(missing.is_empty(), "{code}: {} character(s) drawn as boxes: {missing:?}", missing.len());
        }
        s.state.language = "en".into();
        i18n::set_language("en");
    }
}

/// The Latin-script catalogues draw in egui's bundled faces alone (French's narrow no-break space
/// U+202F egui draws as a narrow space itself), so they need no installed font.
#[test]
fn the_bundled_faces_draw_every_character_of_the_latin_script_catalogues() {
    let ctx = framed_context();
    ctx.set_fonts(crate::fonts::definitions());
    let mut out = ctx.run_ui(Default::default(), |_| {});
    out.textures_delta.clear();
    for lang in crate::i18n::LANGUAGES.iter().filter(|l| !matches!(l.code, "ja" | "zh-Hans")) {
        let text = crate::i18n::catalogue_text(lang);
        let mut wanted: std::collections::BTreeSet<char> =
            text.lines().filter(|l| !l.starts_with('#')).flat_map(|l| l.split('\t').nth(3).unwrap_or_default().chars().collect::<Vec<_>>()).collect();
        wanted.extend(lang.native_name.chars());
        let missing: Vec<char> = ctx.fonts_mut(|f| {
            let covered = f.fonts.font(&egui::FontFamily::Proportional).characters().clone();
            wanted.iter().copied().filter(|c| !matches!(c, ' ' | '\u{202F}') && !covered.contains_key(c)).collect()
        });
        assert!(missing.is_empty(), "{}: {missing:?}", lang.code);
    }
}

/// The shell with a project open, in the harness at the design's frame size or another.
fn workspace_harness(theme: ThemeKind, size: egui::Vec2) -> Harness<'static, Shell> {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services { user_name: Some("Robin Meyer".into()), ids: Some(test_ids()), ..Services::default() });
    shell.state.theme = theme;
    shell.open_project(crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into() });
    // Frames a sixtieth of a second apart, as on screen, so two clicks in a row read as a double click.
    let mut h = Harness::builder().with_size(size).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    h
}

/// Ids for a test's model: version 7 from a counter, no clock.
fn test_ids() -> std::sync::Arc<dyn Fn() -> archforge_mof::Uuid + Send + Sync> {
    let n = std::sync::atomic::AtomicU64::new(0);
    std::sync::Arc::new(move || {
        let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&i.to_be_bytes());
        archforge_mof::Uuid::v7(1_761_670_800_000, r)
    })
}

fn rect_of(h: &Harness<'static, Shell>, id: &str) -> egui::Rect {
    let w = h.state().widgets().iter().find(|w| w.id == id).unwrap_or_else(|| panic!("no control {id}"));
    egui::Rect::from_min_max(egui::pos2(w.rect[0], w.rect[1]), egui::pos2(w.rect[2], w.rect[3]))
}

#[test]
fn an_open_project_shows_the_workspace_with_its_session_and_every_control() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        let shell = h.state();
        assert!(shell.session.is_some(), "the project's model opens with it");
        let found = ids(&h);
        for id in [
            "strip.home",
            "workspace.header.home",
            "workspace.header.project",
            "workspace.header.search",
            "workspace.header.help",
            "workspace.header.account",
            "workspace.explorers.models",
            "workspace.explorers.diagrams",
            "workspace.explorers.source-control",
            "workspace.explorer.add",
            "workspace.explorer.filter",
            "workspace.explorer.resize",
            "workspace.tabs.add",
            "workspace.properties.name",
            "workspace.panels.agent",
            "workspace.panels.appearance",
            "workspace.panels.properties",
            "workspace.panels.skills",
            "workspace.panel.collapse",
            "workspace.panel.resize",
        ] {
            assert!(found.iter().any(|i| i == id), "{theme:?}: {id} in {found:?}");
        }
        assert!(!found.iter().any(|i| i.starts_with("home.start.")), "Home is not drawn under the workspace");
    }
}

#[test]
fn the_rails_switch_the_explorer_and_the_panel_and_home_closes_the_project() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    assert_eq!(h.state().state.panel, crate::state::Panel::Properties);
    let at = rect_of(&h, "workspace.panels.skills").center();
    click(&mut h, at);
    assert_eq!(h.state().state.panel, crate::state::Panel::Skills);
    assert_eq!(h.state().state.explorer, crate::state::Explorer::Models);
    let at = rect_of(&h, "workspace.explorers.diagrams").center();
    click(&mut h, at);
    assert_eq!(h.state().state.explorer, crate::state::Explorer::Diagrams);
    // The tab strip's + opens the New diagram dialog; Escape closes it.
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    assert!(h.state().diagrams.new.is_some());
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().diagrams.new.is_none());
    assert!(h.state().project.is_some());
    let at = rect_of(&h, "workspace.header.home").center();
    click(&mut h, at);
    assert!(h.state().project.is_none() && h.state().session.is_none(), "Home closes the project and its session");
    assert!(ids(&h).iter().any(|i| i == "home.open-project"), "Home is back");
}

#[test]
fn a_narrow_window_keeps_the_rails_and_drops_the_panel_then_the_explorer() {
    let h = workspace_harness(ThemeKind::Light, egui::vec2(900.0, 700.0));
    let found = ids(&h);
    assert!(found.iter().any(|i| i == "workspace.explorer.filter"));
    assert!(!found.iter().any(|i| i == "workspace.panel.collapse"), "the panel gives way first");
    assert!(found.iter().any(|i| i == "workspace.panels.properties"), "the rail stays");
    let h = workspace_harness(ThemeKind::Dark, egui::vec2(480.0, 600.0));
    let found = ids(&h);
    assert!(!found.iter().any(|i| i == "workspace.explorer.filter"), "then the explorer");
    assert!(found.iter().any(|i| i == "workspace.explorers.models") && found.iter().any(|i| i == "workspace.tabs.add"));
}

#[test]
fn the_dividers_resize_within_their_ranges_and_the_state_keeps_the_widths() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    let before = h.state().state.explorer_width;
    let from = rect_of(&h, "workspace.explorer.resize").center();
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    let to = from + egui::vec2(60.0, 0.0);
    h.event(egui::Event::PointerMoved(to));
    h.run();
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
    let after = h.state().state.explorer_width;
    assert!(after > before, "{before} -> {after}");
    assert!(after <= 560.0);
    let json = serde_json::to_string(&h.state().state).unwrap();
    let back: crate::UiState = serde_json::from_str(&json).unwrap();
    assert_eq!(back.explorer_width, after);
}

#[test]
fn without_an_id_source_the_status_bar_says_the_model_could_not_open_once() {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services::default());
    shell.open_project(crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into() });
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    h.run();
    assert!(h.state().session.is_none());
    assert_eq!(h.state().session_error, Some(crate::SessionError::NoIdSource));
}

#[test]
fn a_long_project_name_keeps_the_chip_left_of_the_bar_controls_at_every_size_and_language() {
    for lang in crate::i18n::LANGUAGES {
        for (w, h_) in [(1600.0, 1120.0), (900.0, 700.0), (480.0, 600.0), (80.0, 80.0)] {
            crate::i18n::set_language(lang.code);
            let mut shell = Shell::new(Services { user_name: Some("Robin Meyer".into()), ids: Some(test_ids()), ..Services::default() });
            shell.open_project(crate::Opened {
                name: "A project with a very long name that goes on and on".into(),
                path: "/tmp/long.archforge".into(),
            });
            let mut h = Harness::builder().with_size(egui::vec2(w, h_)).build_ui_state(
                |ui, shell: &mut Shell| {
                    shell.logic(ui.ctx());
                    shell.ui(ui);
                },
                shell,
            );
            h.run();
            for e in h.state().widgets() {
                assert!(e.rect.iter().all(|v| v.is_finite()), "{} at {w}x{h_}: {:?}", e.id, e.rect);
            }
            if w >= 480.0 {
                let chip = rect_of(&h, "workspace.header.project");
                let help = rect_of(&h, "workspace.header.help");
                assert!(chip.right() <= help.left(), "{}: chip {chip:?} runs under help {help:?} at {w}", lang.code);
            }
        }
    }
    crate::i18n::set_language("en");
}

fn key(h: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers) {
    h.key_down_modifiers(modifiers, key);
    h.key_up_modifiers(modifiers, key);
    h.run();
}

fn schema_names(h: &Harness<'static, Shell>) -> Vec<String> {
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).map(|o| archforge_er::rules::name_of(st, o)).collect()
}

#[test]
fn the_plus_adds_a_schema_named_in_place_and_enter_renames_it() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    let at = rect_of(&h, "workspace.explorer.add").center();
    click(&mut h, at);
    assert!(h.state().explorer.menu_open);
    let at = rect_of(&h, "workspace.explorer.add.schema").center();
    click(&mut h, at);
    h.run();
    assert_eq!(schema_names(&h), ["Schema 1"]);
    assert!(h.state().explorer.rename.is_some(), "the new schema's name is open for typing");
    assert!(ids(&h).iter().any(|i| i == "workspace.explorer.rename"));
    assert!(h.state().session.as_ref().unwrap().is_dirty());
    // The default name is selected: typing replaces it.
    h.run();
    h.event(egui::Event::Text("Shop".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.is_none(), "{:?}", h.state().explorer.rename);
    assert_eq!(schema_names(&h), ["Shop"]);
    assert!(ids(&h).iter().any(|i| i.starts_with("workspace.explorer.row.")), "the schema has a row");
    // Undo takes the rename back, redo the other way.
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(schema_names(&h), ["Schema 1"]);
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
    assert_eq!(schema_names(&h), ["Shop"]);
}

#[test]
fn with_a_schema_selected_the_plus_adds_a_table_and_delete_asks_before_the_schema_goes() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    crate::explorer::add(h.state_mut(), crate::explorer::Kind::Schema);
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.is_none());
    // The new schema is selected: + offers a table first.
    crate::explorer::add(h.state_mut(), crate::explorer::Kind::Entity);
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let rows = crate::explorer::rows(h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Schema 1", "Table 1"]);
    assert_eq!(rows[1].depth, 1);
    // A column under the table.
    crate::explorer::add(h.state_mut(), crate::explorer::Kind::Attribute);
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let rows = crate::explorer::rows(h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Schema 1", "Table 1", "column_1"]);
    assert_eq!(rows[2].detail.as_deref(), Some("TEXT"));
    // Select the schema by its row and press Delete: it holds a table, so the dialog asks.
    let schema_row = format!("workspace.explorer.row.{}", rows[0].id);
    let at = rect_of(&h, &schema_row).center();
    click(&mut h, at);
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(h.state().explorer.delete.is_some(), "the dialog asks");
    let at = rect_of(&h, "workspace.explorer.delete.cancel").center();
    click(&mut h, at);
    assert!(h.state().explorer.delete.is_none());
    assert_eq!(schema_names(&h), ["Schema 1"]);
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    assert!(schema_names(&h).is_empty(), "the schema and its table are gone");
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(schema_names(&h), ["Schema 1"], "undo brings them back");
}

#[test]
fn the_filter_narrows_the_tree_and_the_keys_walk_it() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    for _ in 0..2 {
        crate::explorer::add(h.state_mut(), crate::explorer::Kind::Schema);
        h.run();
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
    }
    crate::explorer::add(h.state_mut(), crate::explorer::Kind::Schema);
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(schema_names(&h).len(), 3);
    h.state_mut().explorer.filter = "schema 2".into();
    h.run();
    let rows = crate::explorer::rows(h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Schema 2"]);
    h.state_mut().explorer.filter.clear();
    h.run();
    // Down from the selected third schema stays on it; up goes to the second.
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    let selected = h.state().session.as_ref().unwrap().view().selection[0];
    assert_eq!(archforge_er::rules::name_of(h.state().session.as_ref().unwrap().doc(), selected), "Schema 2");
    key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.as_ref().is_some_and(|r| r.text == "Schema 2"));
}

/// A harness with a schema, a table of two columns, the table selected.
fn table_harness() -> (Harness<'static, Shell>, String) {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    let (schema, customer) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "Shop" })).unwrap().value);
        let customer = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Customer" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": customer, "name": "customer_id", "type": "BIGINT", "primary_key": true }))
            .unwrap();
        s.execute("er.attribute.add", &serde_json::json!({ "entity": customer, "name": "email", "type": "TEXT" })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        (schema, customer)
    };
    let _ = schema;
    h.run();
    (h, customer)
}

#[test]
fn properties_lists_a_tables_columns_and_the_editor_changes_them_through_the_engine() {
    let (mut h, customer) = table_harness();
    let found = ids(&h);
    let rows: Vec<String> = found.iter().filter(|i| i.starts_with("workspace.properties.column.0")).cloned().collect();
    assert_eq!(rows.len(), 2, "two column rows: {found:?}");
    assert!(found.iter().any(|i| i == "workspace.properties.kind"));
    // Open the second column's editor and make it unique and not null.
    let at = rect_of(&h, &rows[1]).center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_some(), "the editor opened");
    let at = rect_of(&h, "workspace.properties.column.unique").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.not-null").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_none(), "{:?}", h.state().panels.column);
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    let email_obj = st.alive().find(|o| archforge_er::rules::name_of(st, *o) == "email").unwrap();
    assert!(archforge_er::rules::is_unique(st, email_obj));
    assert!(!archforge_er::rules::is_nullable(st, email_obj));
    assert!(s.can_undo());
    // The + adds a column through the editor.
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.is_none() && c.name == "column_1"));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 3);
    // The kind select makes a mapping table.
    let at = rect_of(&h, "workspace.properties.kind").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::Kind));
    let at = rect_of(&h, "workspace.properties.kind.1").center();
    click(&mut h, at);
    // A mapping table needs its foreign keys: the engine refuses, and the panel says so.
    assert!(h.state().panels.menu.is_none());
    assert!(h.state().panels.notice.is_some(), "the refusal is shown");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(customer_obj, archforge_er::entity::KIND).unwrap(), archforge_mof::Value::Enum(archforge_er::EntityKind::Regular.index()));
}

#[test]
fn a_selected_column_opens_its_editor_once_so_cancel_and_add_stay_closed_and_open() {
    let (mut h, customer) = table_harness();
    let email = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let email = st.alive().find(|o| archforge_er::rules::name_of(st, *o) == "email").unwrap();
        let email = st.uuid(email).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [email] })).unwrap();
        email
    };
    h.run();
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.map(|t| t.to_string()) == Some(email.clone())), "the column's editor opened");
    let at = rect_of(&h, "workspace.properties.column.cancel").center();
    click(&mut h, at);
    h.run();
    assert!(h.state().panels.column.is_none(), "Cancel stays closed: {:?}", h.state().panels.column);
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    h.run();
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.is_none()), "the + opens an editor for a new column");
    // Selecting the table and the column again opens the column's editor once more.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    }
    h.run();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [email] })).unwrap();
    }
    h.run();
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.map(|t| t.to_string()) == Some(email)));
}

#[test]
fn the_skills_tab_adds_a_skill_on_the_table_and_shows_the_schemas_as_inherited() {
    let (mut h, customer) = table_harness();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap();
        let schema_id = st.uuid(schema).unwrap().to_string();
        s.execute("skill.add", &serde_json::json!({ "subject": schema_id, "name": "Table naming", "applies_to": "table", "triggers": ["naming"] }))
            .unwrap();
        // Adding selected the skill; the panel shows the table again.
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    }
    let at = rect_of(&h, "workspace.panels.skills").center();
    click(&mut h, at);
    let found = ids(&h);
    assert_eq!(found.iter().filter(|i| i.starts_with("workspace.skills.row.")).count(), 1, "the schema's skill is inherited: {found:?}");
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    assert!(h.state().panels.skill.as_ref().is_some_and(|e| e.target.is_none() && e.name == "Skill 1"));
    let at = rect_of(&h, "workspace.skills.when.naming").center();
    click(&mut h, at);
    assert!(h.state().panels.skill.as_ref().unwrap().triggers[2]);
    let at = rect_of(&h, "workspace.skills.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.skill.is_none(), "{:?}", h.state().panels.skill);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    let own: Vec<_> =
        archforge_core::skills::skills(st).into_iter().filter(|s| archforge_core::skills::subject_of(st, *s) == Some(customer_obj)).collect();
    assert_eq!(own.len(), 1);
    assert_eq!(archforge_core::skills::triggers_of(st, own[0]), [archforge_core::SkillTrigger::Naming]);
    let found = ids(&h);
    assert_eq!(found.iter().filter(|i| i.starts_with("workspace.skills.row.")).count(), 2, "own and inherited: {found:?}");
    // A refusal stays with the editor: a second skill of the same name on the table.
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.state_mut().panels.skill.as_mut().unwrap().name = "Skill 1".into();
    let at = rect_of(&h, "workspace.skills.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.skill.as_ref().is_some_and(|e| e.refused.is_some()));
}

/// The table harness with Order, the relationship `places` and an ER diagram of the schema
/// showing both tables and the relationship, open in the centre: `(harness, customer, diagram)`.
fn diagram_harness() -> (Harness<'static, Shell>, String, String) {
    let (mut h, customer) = table_harness();
    let diagram = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let order = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Order" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true })).unwrap();
        let places = id(s
            .execute("er.relationship.add", &serde_json::json!({ "from": customer, "to": order, "name": "places", "create_foreign_key": true }))
            .unwrap()
            .value);
        let diagram = id(s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": schema, "name": "Shop schema" })).unwrap().value);
        let c = id(s
            .execute(
                "diagram.place",
                &serde_json::json!({ "diagram": diagram, "element": customer, "x": 100, "y": 100, "width": 240, "height": 166 }),
            )
            .unwrap()
            .value);
        let o = id(s
            .execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": order, "x": 500, "y": 100, "width": 240, "height": 166 }))
            .unwrap()
            .value);
        let edge =
            id(s.execute("diagram.connect", &serde_json::json!({ "diagram": diagram, "element": places, "source": c, "target": o })).unwrap().value);
        s.execute("diagram.label", &serde_json::json!({ "edge": edge, "kind": "name", "at": "middle" })).unwrap();
        diagram
    };
    crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&diagram).unwrap());
    h.run();
    (h, customer, diagram)
}

fn drag(h: &mut Harness<'static, Shell>, from: egui::Pos2, to: egui::Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    h.event(egui::Event::PointerMoved(to));
    h.run();
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

fn selected_name(h: &Harness<'static, Shell>) -> String {
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    let o = s.view().selection.first().copied().unwrap();
    if st.class_of(o).ok() == Some(archforge_diagram::DIAGRAM) { crate::diagrams::diagram_name(st, o) } else { archforge_er::rules::name_of(st, o) }
}

#[test]
fn the_diagrams_explorer_creates_a_diagram_through_the_dialog_and_opens_it_in_a_tab() {
    let (mut h, _customer) = table_harness();
    let at = rect_of(&h, "workspace.explorers.diagrams").center();
    click(&mut h, at);
    assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.diagrams.row.")), "no diagram yet");
    assert!(!ids(&h).iter().any(|i| i == "workspace.canvas"), "the canvas is plain without a diagram");
    // The + opens the dialog with the schema's name and the schema chosen.
    let at = rect_of(&h, "workspace.diagrams.add").center();
    click(&mut h, at);
    let edit = h.state().diagrams.new.clone().expect("the dialog opened");
    assert_eq!(edit.name, "Shop");
    assert!(edit.schema.is_some());
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert!(h.state().diagrams.new.is_none(), "the dialog closed");
    let diagram = {
        let st = h.state().session.as_ref().unwrap().doc();
        let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).expect("the diagram was added");
        st.uuid(d).unwrap()
    };
    assert_eq!(h.state().canvas.active, Some(diagram));
    assert_eq!(h.state().canvas.tabs, vec![diagram]);
    assert_eq!(selected_name(&h), "Shop", "the diagram is selected, so Properties shows it");
    let found = ids(&h);
    for id in [
        format!("workspace.tabs.{diagram}"),
        format!("workspace.diagrams.row.{diagram}"),
        "workspace.canvas".to_string(),
        "workspace.minimap.fit".to_string(),
    ] {
        assert!(found.contains(&id), "{id} in {found:?}");
    }
    // Closing the tab empties the centre; the row opens it again.
    let at = rect_of(&h, &format!("workspace.tabs.{diagram}.close")).center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.active, None);
    assert!(h.state().canvas.tabs.is_empty());
    assert!(!ids(&h).iter().any(|i| i == "workspace.canvas"));
    let at = rect_of(&h, &format!("workspace.diagrams.row.{diagram}")).center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.active, Some(diagram));
    // The filter narrows the rows.
    h.state_mut().diagrams.filter = "zzz".into();
    h.run();
    assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.diagrams.row.")));
    h.state_mut().diagrams.filter = "sho".into();
    h.run();
    assert!(ids(&h).iter().any(|i| i == &format!("workspace.diagrams.row.{diagram}")));
    h.state_mut().diagrams.filter.clear();
    // A second diagram of a new schema: the schema is made with the diagram's name.
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    h.state_mut().diagrams.new.as_mut().unwrap().name = "Billing".into();
    let at = rect_of(&h, "workspace.new-diagram.schema").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.schema.1").center();
    click(&mut h, at);
    assert_eq!(h.state().diagrams.new.as_ref().unwrap().schema, None, "New schema chosen");
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert!(h.state().diagrams.new.is_none(), "{:?}", h.state().diagrams.new);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| st.class_of(o).ok() == Some(archforge_er::SCHEMA) && archforge_er::rules::name_of(st, o) == "Billing"));
    assert_eq!(h.state().canvas.tabs.len(), 2);
    assert_eq!(selected_name(&h), "Billing");
}

#[test]
fn the_canvas_draws_the_diagram_and_the_pointer_selects_pans_and_zooms() {
    let (mut h, customer, diagram) = diagram_harness();
    let found = ids(&h);
    let nodes: Vec<String> = found.iter().filter(|i| i.starts_with("workspace.canvas.node.")).cloned().collect();
    assert_eq!(nodes.len(), 2, "both tables are drawn: {found:?}");
    assert_eq!(found.iter().filter(|i| i.starts_with("workspace.canvas.edge.")).count(), 1, "the relationship is drawn");
    assert_eq!(selected_name(&h), "Shop schema", "opening selects the diagram");
    // The node at the left is Customer: a click selects the table behind it.
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    assert_eq!(selected_name(&h), "Customer");
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, vec![customer_obj]);
    // A click on the empty canvas selects the diagram again.
    let canvas = rect_of(&h, "workspace.canvas");
    click(&mut h, canvas.left_top() + egui::vec2(20.0, 20.0));
    assert_eq!(selected_name(&h), "Shop schema");
    // The pan tool drags the view.
    let at = rect_of(&h, "workspace.tools.pan").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Pan);
    let before = h.state().canvas.pan;
    let from = canvas.left_top() + egui::vec2(20.0, 20.0);
    drag(&mut h, from, from + egui::vec2(50.0, 20.0));
    let after = h.state().canvas.pan;
    assert!((after - before - egui::vec2(50.0, 20.0)).length() < 1.0, "{before:?} -> {after:?}");
    // The zoom buttons and Whole diagram.
    let fitted = h.state().canvas.zoom;
    let at = rect_of(&h, "workspace.zoom.in").center();
    click(&mut h, at);
    assert!(h.state().canvas.zoom > fitted);
    let at = rect_of(&h, "workspace.zoom.out").center();
    click(&mut h, at);
    click(&mut h, at);
    assert!(h.state().canvas.zoom < fitted);
    let at = rect_of(&h, "workspace.minimap.fit").center();
    click(&mut h, at);
    assert!((h.state().canvas.zoom - fitted).abs() < 0.001);
    // The toggles.
    let at = rect_of(&h, "workspace.tools.grid").center();
    click(&mut h, at);
    assert!(!h.state().canvas.grid);
    let at = rect_of(&h, "workspace.tools.key-status").center();
    click(&mut h, at);
    assert!(!h.state().canvas.key_status);
    let at = rect_of(&h, "workspace.minimap.toggle").center();
    click(&mut h, at);
    assert!(!h.state().canvas.minimap_open);
    assert!(!ids(&h).iter().any(|i| i == "workspace.minimap.fit"));
    let at = rect_of(&h, "workspace.minimap.toggle").center();
    click(&mut h, at);
    assert!(h.state().canvas.minimap_open);
    // Properties shows the diagram with its schema when the diagram is selected.
    click(&mut h, canvas.left_top() + egui::vec2(20.0, 20.0));
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.schema"));
    let _ = diagram;
}

fn node_ids(h: &Harness<'static, Shell>) -> Vec<String> {
    ids(h).iter().filter(|i| i.starts_with("workspace.canvas.node.")).cloned().collect()
}

fn node_bounds(h: &Harness<'static, Shell>, node: &str) -> archforge_diagram::Rect {
    let st = h.state().session.as_ref().unwrap().doc();
    let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
    archforge_diagram::grid::bounds_of(st, n)
}

#[test]
fn the_entity_tool_places_a_table_named_in_place_as_one_undo_step() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let at = rect_of(&h, "workspace.tools.entity").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Entity);
    let canvas = rect_of(&h, "workspace.canvas");
    let before = node_ids(&h).len();
    click(&mut h, canvas.left_top() + egui::vec2(120.0, 320.0));
    let nodes = node_ids(&h);
    assert_eq!(nodes.len(), before + 1, "a node was placed: {nodes:?}");
    let rename = h.state().canvas.rename.clone().expect("the name is open for typing");
    assert_eq!(rename.text, "Entity");
    assert_eq!(selected_name(&h), "Entity");
    assert!(ids(&h).iter().any(|i| i == "workspace.canvas.rename"));
    // Typing replaces the selected default; Enter commits the rename.
    h.event(egui::Event::Text("Payment".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().canvas.rename.is_none(), "{:?}", h.state().canvas.rename);
    assert_eq!(selected_name(&h), "Payment");
    // Add and place were one step: one undo removes the node and the table together.
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    assert_eq!(node_ids(&h).len(), before);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!st.alive().any(|o| archforge_er::rules::name_of(st, o) == "Entity"), "the table went with the node");
    // Escape returns to Select.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select);
}

#[test]
fn dragging_moves_and_resizes_nodes_and_the_arrange_bar_aligns_two() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    let zoom = h.state().canvas.zoom;
    // A drag on the node moves it by the drag in diagram units, as one undo step.
    let before = node_bounds(&h, customer_node.trim_start_matches("workspace.canvas.node."));
    let from = rect_of(&h, &customer_node).center();
    drag(&mut h, from, from + egui::vec2(60.0, 30.0));
    let after = node_bounds(&h, customer_node.trim_start_matches("workspace.canvas.node."));
    assert!(((after.x - before.x) as f32 - 60.0 / zoom).abs() < 1.0, "{before:?} -> {after:?}");
    assert!(((after.y - before.y) as f32 - 30.0 / zoom).abs() < 1.0);
    assert!(h.state().canvas.gesture.is_none());
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    let back = node_bounds(&h, customer_node.trim_start_matches("workspace.canvas.node."));
    assert!((back.x - before.x).abs() < 0.01 && (back.y - before.y).abs() < 0.01, "one undo restores the place");
    // The bottom-right handle of the selected node resizes it.
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    let ring = rect_of(&h, &customer_node).expand(4.0);
    drag(&mut h, ring.right_bottom(), ring.right_bottom() + egui::vec2(40.0, 20.0));
    let sized = node_bounds(&h, customer_node.trim_start_matches("workspace.canvas.node."));
    assert!((sized.width as f32 - (before.width as f32 + 40.0 / zoom)).abs() < 1.0, "{sized:?}");
    assert!((sized.height as f32 - (before.height as f32 + 20.0 / zoom)).abs() < 1.0);
    // Shift-click adds the second node; the Arrange bar appears and aligns the tops... and lefts.
    let at = rect_of(&h, &order_node).center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event_modifiers(
            egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::SHIFT },
            egui::Modifiers::SHIFT,
        );
    }
    h.run();
    assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 2);
    assert!(ids(&h).iter().any(|i| i == "workspace.canvas.arrange.left"), "{:?}", ids(&h));
    let at = rect_of(&h, "workspace.canvas.arrange.left").center();
    click(&mut h, at);
    let a = node_bounds(&h, customer_node.trim_start_matches("workspace.canvas.node."));
    let b = node_bounds(&h, order_node.trim_start_matches("workspace.canvas.node."));
    assert!((a.x - b.x).abs() < 0.01, "aligned left: {a:?} {b:?}");
    // The arrows nudge the selection.
    key(&mut h, egui::Key::ArrowRight, egui::Modifiers::NONE);
    let a2 = node_bounds(&h, customer_node.trim_start_matches("workspace.canvas.node."));
    assert!((a2.x - a.x - 1.0).abs() < 0.01, "{a:?} -> {a2:?}");
}

#[test]
fn the_entity_menu_removes_from_the_diagram_and_the_dialog_deletes_from_the_model() {
    let (mut h, customer, _diagram) = diagram_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    // The add-column adorner opens the column editor for the selected table.
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.canvas.adorn.add-column").center();
    click(&mut h, at);
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.is_none() && c.entity.to_string() == customer));
    assert_eq!(h.state().state.panel, crate::state::Panel::Properties);
    // A right click opens the menu; Remove from this diagram keeps the table in the model.
    let at = rect_of(&h, &order_node).center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    assert!(h.state().canvas.menu.is_some(), "the menu opened");
    let at = rect_of(&h, "workspace.canvas.menu.remove").center();
    click(&mut h, at);
    assert_eq!(node_ids(&h).len(), 1, "the node went; notice {:?}, active {:?}, ids {:?}", h.state().canvas.notice, h.state().canvas.active, ids(&h));
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| archforge_er::rules::name_of(st, o) == "Order"), "the table stays");
    // Shift+Delete on the selected Customer asks; Delete confirms and the table goes with its node.
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    let d = h.state().explorer.delete.clone().expect("the dialog asks");
    assert_eq!(d.name, "Customer");
    assert_eq!(d.diagrams, 1);
    assert!(d.node.is_some());
    assert!(ids(&h).iter().any(|i| i == "workspace.explorer.delete.unplace"));
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    assert!(h.state().explorer.delete.is_none());
    assert!(node_ids(&h).is_empty());
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!st.alive().any(|o| archforge_er::rules::name_of(st, o) == "Customer"));
}

#[test]
fn a_table_added_in_models_is_placed_on_the_open_diagram_and_grows_with_its_columns() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let before = node_ids(&h).len();
    // Select the schema in Models and add a table: it lands on the diagram.
    let schema_row = ids(&h).into_iter().find(|i| i.starts_with("workspace.explorer.row.")).unwrap();
    let at = rect_of(&h, &schema_row).center();
    click(&mut h, at);
    crate::explorer::add(h.state_mut(), crate::explorer::Kind::Entity);
    h.run();
    assert_eq!(node_ids(&h).len(), before + 1, "placed on the open diagram");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    // Its node starts at the empty height and grows with the first column.
    let new_node = node_ids(&h).into_iter().max_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap();
    let id = new_node.trim_start_matches("workspace.canvas.node.").to_string();
    assert!((node_bounds(&h, &id).height - crate::editing::ENTITY_EMPTY_HEIGHT).abs() < 0.01);
    crate::explorer::add(h.state_mut(), crate::explorer::Kind::Attribute);
    h.run();
    assert!(
        (node_bounds(&h, &id).height - crate::editing::content_height(1)).abs() < 0.01,
        "{:?} notice {:?} undo {:?}",
        node_bounds(&h, &id),
        h.state().explorer.notice,
        h.state().session.as_ref().unwrap().undo_label()
    );
    // Growth and the column were one step.
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    assert!((node_bounds(&h, &id).height - crate::editing::ENTITY_EMPTY_HEIGHT).abs() < 0.01);
}

fn drag_by(h: &mut Harness<'static, Shell>, button: egui::PointerButton, from: egui::Pos2, to: egui::Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    h.event(egui::Event::PointerMoved(to));
    h.run();
    h.event(egui::Event::PointerButton { pos: to, button, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

#[test]
fn the_marquee_selects_what_it_crosses_and_a_middle_drag_only_pans() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let id = customer_node.trim_start_matches("workspace.canvas.node.").to_string();
    // A middle drag over a node pans the view and moves nothing.
    let before = node_bounds(&h, &id);
    let pan = h.state().canvas.pan;
    let from = rect_of(&h, &customer_node).center();
    drag_by(&mut h, egui::PointerButton::Middle, from, from + egui::vec2(30.0, 10.0));
    assert_eq!(node_bounds(&h, &id), before, "a middle drag moves no node");
    assert!(h.state().canvas.gesture.is_none());
    assert!((h.state().canvas.pan - pan - egui::vec2(30.0, 10.0)).length() < 1.0, "it panned");
    assert_eq!(selected_name(&h), "Shop schema", "the selection is untouched");
    // A marquee from the empty canvas over both nodes selects both tables.
    let canvas = rect_of(&h, "workspace.canvas");
    let a = rect_of(&h, &customer_node);
    let other = nodes.iter().find(|n| **n != customer_node).map(|n| rect_of(&h, n)).unwrap();
    drag(&mut h, egui::pos2(canvas.left() + 10.0, a.top() - 10.0), egui::pos2(other.right() + 10.0, other.bottom() + 10.0));
    assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 2, "both tables selected");
    // Two nudges with the same key coalesce into one undo step.
    let a0 = node_bounds(&h, &id);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    assert!((node_bounds(&h, &id).y - a0.y - 2.0).abs() < 0.01);
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    assert!((node_bounds(&h, &id).y - a0.y).abs() < 0.01, "one undo takes both nudges back");
}

#[test]
fn double_clicks_rename_and_open_columns_and_a_refused_name_stays_open() {
    let (mut h, customer, _diagram) = diagram_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let rect = rect_of(&h, &customer_node);
    // A double click on the header opens the name; "Order" is taken, so the rename is refused and stays open.
    let at = rect.left_top() + egui::vec2(60.0, 30.0);
    double_click(&mut h, at);
    assert!(h.state().canvas.rename.as_ref().is_some_and(|r| r.text == "Customer"), "{:?}", h.state().canvas.rename);
    h.state_mut().canvas.rename.as_mut().unwrap().text = "Order".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let rename = h.state().canvas.rename.clone().expect("a refused name stays open");
    assert!(rename.refused.is_some());
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().canvas.rename.is_none());
    assert_eq!(selected_name(&h), "Customer");
    // A double click on the second column row opens that column's editor.
    let at = rect.left_top() + egui::vec2(100.0, 77.0 + 20.0 + 6.0);
    double_click(&mut h, at);
    let edit = h.state().panels.column.clone().expect("the column editor opened");
    assert_eq!(edit.entity.to_string(), customer);
    assert_eq!(edit.name, "email", "the second row is the email column");
    // With the Entity tool, a click on a node selects it instead of placing one.
    let n = node_ids(&h).len();
    let at = rect_of(&h, "workspace.tools.entity").center();
    click(&mut h, at);
    let order_node = nodes.iter().find(|x| **x != customer_node).unwrap().clone();
    let at = rect_of(&h, &order_node).center();
    click(&mut h, at);
    assert_eq!(node_ids(&h).len(), n);
    assert_eq!(selected_name(&h), "Order");
    // The dialog's third button removes from the diagram only. Time passes first, so the next
    // click on Order is not read as a double click on the one before; the column editor, which
    // would keep the keyboard, is closed.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    for _ in 0..45 {
        h.step();
    }
    h.state_mut().panels.column = None;
    let at = rect_of(&h, &order_node).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    assert!(h.state().explorer.delete.is_some());
    let at = rect_of(&h, "workspace.explorer.delete.unplace").center();
    click(&mut h, at);
    assert!(h.state().explorer.delete.is_none());
    assert_eq!(node_ids(&h).len(), n - 1);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| archforge_er::rules::name_of(st, o) == "Order"), "the table stays in the model");
}

/// Two clicks a frame apart: `run` would settle for longer than egui's double-click window.
fn double_click(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.step();
    }
    h.run();
}

/// The table harness with Order placed beside Customer on a diagram, and no relationship yet.
fn two_tables_harness() -> (Harness<'static, Shell>, String, String) {
    let (mut h, customer) = table_harness();
    let (order, diagram) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let order = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Order" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true })).unwrap();
        let diagram = id(s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": schema, "name": "Shop schema" })).unwrap().value);
        s.execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": customer, "x": 100, "y": 100, "width": 240, "height": 166 }))
            .unwrap();
        s.execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": order, "x": 500, "y": 100, "width": 240, "height": 166 }))
            .unwrap();
        (order, diagram)
    };
    crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&diagram).unwrap());
    h.run();
    (h, customer, order)
}

fn relationships(h: &Harness<'static, Shell>) -> Vec<archforge_mof::Obj> {
    let st = h.state().session.as_ref().unwrap().doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).collect()
}

#[test]
fn the_relationship_tool_draws_a_line_with_its_foreign_key_as_one_step_and_delete_offers_undo() {
    let (mut h, customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Relationship);
    // Drag from Customer to Order: one to many, Customer has a primary key, so the key comes along.
    let (from, to) = (rect_of(&h, &customer_node).center(), rect_of(&h, &order_node).center());
    drag(&mut h, from, to);
    let rels = relationships(&h);
    assert_eq!(rels.len(), 1, "one relationship drawn");
    let st = h.state().session.as_ref().unwrap().doc();
    let rel = rels[0];
    assert_eq!(st.get(rel, archforge_er::relationship::FROM).unwrap().as_ref().and_then(|o| st.uuid(o).ok()).unwrap().to_string(), customer);
    assert!(crate::connect::has_foreign_key(st, rel), "the foreign key came with the line");
    assert!(ids(&h).iter().any(|i| i.starts_with("workspace.canvas.edge.")), "the edge is drawn");
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, vec![rel], "the relationship is selected");
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.name"), "Properties shows its name field");
    // The line, its edge and its label were one step.
    let s = h.state_mut().session.as_mut().unwrap();
    assert_eq!(s.undo_label(), Some("Add relationship"));
    s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    h.run();
    assert!(relationships(&h).is_empty());
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("edit.redo", &serde_json::Value::Null).unwrap();
    h.run();
    // Escape returns to Select; a click on the edge selects the relationship; Delete removes it with a toast.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let edge = ids(&h).into_iter().find(|i| i.starts_with("workspace.canvas.edge.")).unwrap();
    let at = rect_of(&h, &edge).center();
    click(&mut h, at);
    assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 1);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(relationships(&h).is_empty(), "the relationship went");
    assert!(h.state().canvas.toast.is_some(), "the toast offers Undo");
    let at = rect_of(&h, "workspace.toast.undo").center();
    click(&mut h, at);
    assert_eq!(relationships(&h).len(), 1, "Undo brings it back");
    assert!(h.state().canvas.toast.is_none());
    // A step after the removal takes the toast with it: Undo never undoes something else.
    let edge = ids(&h).into_iter().find(|i| i.starts_with("workspace.canvas.edge.")).unwrap();
    let at = rect_of(&h, &edge).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(h.state().canvas.toast.is_some());
    let from = rect_of(&h, &customer_node).center();
    drag(&mut h, from, from + egui::vec2(30.0, 0.0));
    assert!(h.state().canvas.toast.is_none(), "the toast went with the move");
    assert!(!ids(&h).iter().any(|i| i == "workspace.toast.undo"));
}

#[test]
fn the_cardinality_popover_sets_the_ends_and_a_many_to_many_line_offers_its_mapping_table() {
    let (mut h, _customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    let at = rect_of(&h, "workspace.tools.relationship-types").center();
    click(&mut h, at);
    assert!(h.state().canvas.popover && h.state().canvas.tool == crate::canvas::Tool::Relationship);
    assert!(ids(&h).iter().any(|i| i == "workspace.cardinality.preset.3"), "{:?}", ids(&h));
    let at = rect_of(&h, "workspace.cardinality.preset.3").center();
    click(&mut h, at);
    assert!(h.state().canvas.relationship.many_to_many());
    let at = rect_of(&h, "workspace.cardinality.identifying").center();
    click(&mut h, at);
    assert!(h.state().canvas.relationship.identifying);
    let at = rect_of(&h, "workspace.cardinality.end.1").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.relationship.to, archforge_er::Cardinality::ExactlyOne);
    let at = rect_of(&h, "workspace.cardinality.preset.3").center();
    click(&mut h, at);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!h.state().canvas.popover, "Escape closes the popover");
    // The pick stays: the connect adorner of the selected Customer draws a many-to-many line.
    h.state_mut().canvas.tool = crate::canvas::Tool::Select;
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    let from = rect_of(&h, "workspace.canvas.adorn.connect").center();
    let to = rect_of(&h, &order_node).center();
    drag(&mut h, from, to);
    let rels = relationships(&h);
    assert_eq!(rels.len(), 1);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(archforge_er::rules::is_many_to_many(st, rels[0]));
    assert!(!crate::connect::has_foreign_key(st, rels[0]), "no key for a many-to-many line");
    let offer = h.state().canvas.mapping.clone().expect("the mapping table is offered");
    assert_eq!(offer.name, "CustomerOrder");
    assert!(ids(&h).iter().any(|i| i == "workspace.mapping.add"));
    let at = rect_of(&h, "workspace.mapping.add").center();
    click(&mut h, at);
    assert!(h.state().canvas.mapping.is_none(), "{:?}", h.state().canvas.mapping);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.alive().any(|o| archforge_er::rules::name_of(st, o) == "CustomerOrder"), "the mapping table exists");
}

/// The diagram harness with the relationship `places` selected, its Properties shown.
fn relationship_harness() -> (Harness<'static, Shell>, archforge_mof::Obj) {
    let (mut h, _customer, _diagram) = diagram_harness();
    let rel = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let rel = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).unwrap();
        let id = st.uuid(rel).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
        rel
    };
    h.run();
    (h, rel)
}

fn fk_of(h: &Harness<'static, Shell>, rel: archforge_mof::Obj) -> Option<archforge_mof::Obj> {
    let st = h.state().session.as_ref().unwrap().doc();
    st.referrers(rel).into_iter().find(|(_, p)| *p == archforge_er::foreign_key::RELATIONSHIP).map(|(f, _)| f)
}

#[test]
fn relationship_properties_set_the_ends_roles_identifying_and_the_foreign_key_actions() {
    let (mut h, rel) = relationship_harness();
    let found = ids(&h);
    for id in [
        "workspace.properties.identifying",
        "workspace.properties.end.from.0",
        "workspace.properties.end.to.3",
        "workspace.properties.role.from",
        "workspace.properties.foreign-key",
        "workspace.properties.points-to",
        "workspace.properties.pair.0",
        "workspace.properties.on-delete",
        "workspace.properties.more",
    ] {
        assert!(found.iter().any(|i| i == id), "{id} in {found:?}");
    }
    // The to end becomes one-or-more.
    let at = rect_of(&h, "workspace.properties.end.to.3").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_er::rules::cardinality_of(st, rel, archforge_er::relationship::TO_END), Some(archforge_er::Cardinality::OneOrMore));
    // Identifying: the foreign key's columns join the child's primary key.
    let at = rect_of(&h, "workspace.properties.identifying").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(matches!(st.get(rel, archforge_er::relationship::IDENTIFYING), Ok(archforge_mof::Value::Bool(true))), "{:?}", h.state().panels.notice);
    // A role typed and committed with Enter.
    let at = rect_of(&h, "workspace.properties.role.from").center();
    click(&mut h, at);
    h.event(egui::Event::Text("buyer".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(rel, archforge_er::relationship::FROM_ROLE).unwrap().as_str(), Some("buyer"));
    // On delete: Cascade.
    let at = rect_of(&h, "workspace.properties.on-delete").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.on-delete.2").center();
    click(&mut h, at);
    let fk = fk_of(&h, rel).expect("the foreign key");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(
        st.get(fk, archforge_er::foreign_key::ON_DELETE).unwrap(),
        archforge_mof::Value::Enum(archforge_er::ReferentialAction::Cascade.index())
    );
    // More opens Deferrable; checked at commit.
    let at = rect_of(&h, "workspace.properties.more").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.deferrable").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.deferrable.2").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(
        st.get(fk, archforge_er::foreign_key::DEFERRABLE).unwrap(),
        archforge_mof::Value::Enum(archforge_er::Deferrability::InitiallyDeferred.index())
    );
}

#[test]
fn the_foreign_key_switch_removes_and_adds_the_key_with_its_columns_as_one_step() {
    let (mut h, rel) = relationship_harness();
    let columns_of_order = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let order = st.alive().find(|o| archforge_er::rules::name_of(st, *o) == "Order").unwrap();
        st.list(order, archforge_er::entity::ATTRIBUTE).unwrap().len()
    };
    let before = columns_of_order(&h);
    let fk = fk_of(&h, rel).expect("the foreign key");
    let key_columns = h.state().session.as_ref().unwrap().doc().list(fk, archforge_er::foreign_key::COLUMN).unwrap().len();
    assert!(key_columns >= 1);
    // Off: the key and its columns go together.
    let at = rect_of(&h, "workspace.properties.foreign-key").center();
    click(&mut h, at);
    assert!(fk_of(&h, rel).is_none(), "{:?}", h.state().panels.notice);
    assert_eq!(columns_of_order(&h), before - key_columns);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Remove foreign key"));
    // On again: one Order column per Customer key column, and the key.
    let at = rect_of(&h, "workspace.properties.foreign-key").center();
    click(&mut h, at);
    assert!(fk_of(&h, rel).is_some(), "{:?}", h.state().panels.notice);
    assert_eq!(columns_of_order(&h), before);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add foreign key"));
    // A pair's child column can be an existing column of the same type.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let order = st.uuid(st.alive().find(|o| archforge_er::rules::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
        s.execute("er.attribute.add", &serde_json::json!({ "entity": order, "name": "buyer_ref", "type": "BIGINT" })).unwrap();
        // Adding selected the column; the panel shows the relationship again.
        let rel_id = s.doc().uuid(rel).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [rel_id] })).unwrap();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.pair.0").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::Pair(0)));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.pair.0.")).collect();
    assert!(items.len() >= 2, "{items:?}");
    // The last item is buyer_ref (same type as customer_id).
    let at = rect_of(&h, items.last().unwrap()).center();
    click(&mut h, at);
    let fk = fk_of(&h, rel).unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    let cols: Vec<String> = st
        .list(fk, archforge_er::foreign_key::COLUMN)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .map(|c| archforge_er::rules::name_of(st, c))
        .collect();
    assert!(cols.contains(&"buyer_ref".to_string()), "{cols:?} notice {:?}", h.state().panels.notice);
}

#[test]
fn a_parent_without_a_key_gets_the_card_and_add_id_column_makes_the_key() {
    let (mut h, _customer, _order) = two_tables_harness();
    // Payment without any column, joined from Order as the parent-less side.
    let (payment_rel, payment) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let order = st.uuid(st.alive().find(|o| archforge_er::rules::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
        let payment = s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Payment" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let rel = s.execute("er.relationship.add", &serde_json::json!({ "from": payment, "to": order, "name": "settles" })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [rel] })).unwrap();
        (rel, payment)
    };
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.foreign-key"));
    let at = rect_of(&h, "workspace.properties.foreign-key").center();
    click(&mut h, at);
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.add-id"), "the card offers Add id column: {:?}", ids(&h));
    let at = rect_of(&h, "workspace.properties.add-id").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    let payment_obj = st.by_uuid(archforge_mof::Uuid::parse(&payment).unwrap()).unwrap();
    assert!(archforge_er::rules::primary_key_of(st, payment_obj).is_some(), "Payment has its id key: {:?}", h.state().panels.notice);
    let rel_obj = st.by_uuid(archforge_mof::Uuid::parse(&payment_rel).unwrap()).unwrap();
    assert!(st.referrers(rel_obj).into_iter().any(|(_, p)| p == archforge_er::foreign_key::RELATIONSHIP), "the foreign key exists");
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add id column"));
}
