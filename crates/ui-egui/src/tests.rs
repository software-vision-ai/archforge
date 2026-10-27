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
        for id in ["home.tab.home", "home.theme", "home.header.search", "home.open-project", "home.pin-folder", "home.banner.close"] {
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
    let at = centre(&h, "home.theme");
    click(&mut h, at);
    assert_eq!(h.state().state.theme, ThemeKind::Dark);
    let at = centre(&h, "home.banner.close");
    click(&mut h, at);
    assert!(h.state().state.home_banner_closed);
    assert!(!ids(&h).iter().any(|i| i == "home.banner.close"), "a closed banner is not drawn");
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
