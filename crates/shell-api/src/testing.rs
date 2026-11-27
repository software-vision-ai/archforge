//! The test context, behind the `testing` feature (ADR 0005, "Tests follow the plugin"): a
//! notation's screens tested against the shell's interfaces without the whole shell. A
//! [`TestContext`] owns what the narrow context borrows from the shell, for the notations it is
//! given, and gives the context ([`Ctx`]) and its read-only twin ([`CtxRef`]) over its parts; it
//! keeps every [`Request`] made through the context, in the order made, and carries none out, as
//! there is no shell to carry them out. [`harness`] draws a screen over it in an `egui_kittest`
//! harness, in one language and one appearance, and [`centre`] and [`click`] find and press a
//! control by its automation id.
//!
//! What the shell does around its screens is not here: its panels, its Models tree and canvas,
//! the problems its notations' rules find (a test sets [`TestContext::problems`] itself), and the
//! design's own fonts, which the shell installs (the toolkit's faces stand in for them here).

use std::collections::BTreeMap;

use archforge_diagram::ProjectView;
use archforge_mof::{Obj, Store};
use egui_kittest::Harness;

use crate::cx::{Ctx, CtxRef, Request};
use crate::i18n::{self, LANGUAGES, Unregistered};
use crate::notation_ui::{NotationStates, NotationUi};
use crate::ux::{self, Palette};
use crate::{canvas, menu_bar, panels, widgets};

/// A project's session, as the narrow context holds it.
type Session = archforge_engine::Session<Store, ProjectView>;

/// The parts the narrow context borrows from the shell, owned for a test, with the requests made
/// through the context. The fields carry the context's names.
pub struct TestContext {
    /// The open project's model: none until the test opens one.
    pub session: Option<Session>,
    /// The right panels' own state.
    pub panels: panels::State,
    /// The centre's own state.
    pub canvas: canvas::State,
    /// The controls laid out, for automation; a harness starts a frame of it each frame.
    pub registry: widgets::Registry,
    /// The notations the context was given.
    pub notation_uis: &'static [&'static dyn NotationUi],
    /// Each notation's own state, made by the notation.
    pub notation_states: NotationStates,
    /// The problems by element, as the shell reads them from its notations' rules once a frame:
    /// none until the test sets them.
    pub problems: BTreeMap<Obj, Vec<String>>,
    /// Frames a harness drew so far.
    pub frames: u64,
    /// The inline editor the menu bar holds the keyboard for; there is no bar, so it is never
    /// open and holds an editor only when the context's code says so.
    pub menu_held: Option<egui::Id>,
    /// Whether a version shows, so that looking at it is all that is on.
    pub looking_only: bool,
    /// Every request made through the context, in the order made; none is carried out.
    pub requests: Vec<Request>,
}

impl TestContext {
    /// A test context for the notations `uis`: no project open, each notation's state as it makes
    /// it, and their catalogues registered, so what their screens say is found in the language a
    /// harness draws in. The interfaces' own catalogue registers itself before their first lookup
    /// ([`i18n::tr`]). Refused, with nothing registered from the notation refused, when a notation
    /// brings a catalogue for a code no language has.
    pub fn new(uis: &'static [&'static dyn NotationUi]) -> Result<TestContext, Unregistered> {
        for ui in uis {
            i18n::register(ui.catalogues())?;
        }
        Ok(TestContext {
            session: None,
            panels: panels::State::default(),
            canvas: canvas::State::default(),
            registry: widgets::Registry::default(),
            notation_uis: uis,
            notation_states: NotationStates::new(uis),
            problems: BTreeMap::new(),
            frames: 0,
            menu_held: None,
            looking_only: false,
            requests: Vec::new(),
        })
    }

    /// Runs `f` with the narrow context over these parts, keeps the requests `f` made after those
    /// made before, and gives what `f` gave.
    pub fn cx<R>(&mut self, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
        let mut cx = Ctx {
            session: &mut self.session,
            panels: &mut self.panels,
            canvas: &mut self.canvas,
            registry: &mut self.registry,
            notation_uis: self.notation_uis,
            notation_states: &mut self.notation_states,
            problems: &self.problems,
            frames: self.frames,
            menu_bar: menu_bar::Hold { open: None, chosen: None, back: None, rect: None, held: &mut self.menu_held },
            looking_only: self.looking_only,
            requests: Vec::new(),
        };
        let out = f(&mut cx);
        let made = cx.requests;
        self.requests.extend(made);
        out
    }

    /// The read-only twin of the narrow context over these parts.
    pub fn cx_ref(&self) -> CtxRef<'_> {
        CtxRef {
            session: &self.session,
            canvas: &self.canvas,
            notation_uis: self.notation_uis,
            notation_states: &self.notation_states,
            looking_only: self.looking_only,
        }
    }
}

/// `test` in a harness of `size`, drawing `screen` through the narrow context each frame in the
/// language `language` (pinned each frame, whatever was set between frames) and in the dark
/// appearance or the light one, with that appearance's palette; drawn until it settles. Each
/// frame counts in [`TestContext::frames`] and starts a frame of the automation registry. The
/// design's two families are the toolkit's faces. Refused for a code no language has, naming the
/// codes there are.
pub fn harness<'a>(
    test: TestContext,
    language: &str,
    dark: bool,
    size: egui::Vec2,
    mut screen: impl FnMut(&mut Ctx<'_>, &mut egui::Ui, &Palette) + 'a,
) -> Result<Harness<'a, TestContext>, String> {
    let code = LANGUAGES
        .iter()
        .find(|l| l.code == language)
        .map(|l| l.code)
        .ok_or_else(|| format!("{language:?} is no language (accepted: {})", i18n::codes()))?;
    let p = ux::palette(dark);
    let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
    let draw = move |ui: &mut egui::Ui, test: &mut TestContext| {
        i18n::set_language(code);
        test.frames = test.frames.wrapping_add(1);
        test.registry.begin_frame();
        test.cx(|cx| screen(cx, ui, p));
    };
    Ok(Harness::builder().with_size(size).with_theme(theme).with_context_setup(design_families).build_ui_state(draw, test))
}

/// Gives `ctx` the design's two families before the first frame, as the toolkit's proportional
/// faces: text in a family egui does not have stops the program.
fn design_families(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let proportional = defs.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    for family in [ux::regular(), ux::semibold()] {
        defs.families.insert(family, proportional.clone());
    }
    ctx.set_fonts(defs);
}

/// The centre of the control `id` as the last frame drew it, if it drew it.
pub fn centre(h: &Harness<'_, TestContext>, id: &str) -> Option<egui::Pos2> {
    let entry = h.state().registry.entries().iter().find(|e| e.id == id)?;
    let [left, top, right, bottom] = entry.rect;
    Some(egui::pos2((left + right) / 2.0, (top + bottom) / 2.0))
}

/// A click of the primary button at `at`, the pointer moved there first, and the frames it
/// starts.
pub fn click(h: &mut Harness<'_, TestContext>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

#[cfg(test)]
mod tests {
    use egui::{Color32, Rect, pos2, vec2};

    use super::{TestContext, centre, click, harness};
    use crate::cx::Request;
    use crate::i18n::{Rows, set_language, tr};
    use crate::notation_ui::{Entry, Layer, NotationState, NotationUi, Overlaid, Removal, Row, Subject, Tree, Words};
    use crate::state::Panel;
    use crate::ux::{self, Palette};
    use crate::{panels, widgets};

    /// The stub notation's one row, which its own catalogue holds ([`STUB_ROWS`]).
    const STUB_ROW: &str = "A stub row";

    /// The automation id of the stub's one control.
    const STUB_CONTROL: &str = "stub.row";

    /// The stub's catalogue: its one row in every language, German's own text.
    static STUB_ROWS: &[Rows] = &[
        Rows { code: "en", catalogue: "stub/en.tsv", text: "str\t\tA stub row\tA stub row\n" },
        Rows { code: "de", catalogue: "stub/de.tsv", text: "str\t\tA stub row\tEine Stubzeile\n" },
        Rows { code: "es", catalogue: "stub/es.tsv", text: "str\t\tA stub row\tA stub row\n" },
        Rows { code: "fr", catalogue: "stub/fr.tsv", text: "str\t\tA stub row\tA stub row\n" },
        Rows { code: "pt-PT", catalogue: "stub/pt-PT.tsv", text: "str\t\tA stub row\tA stub row\n" },
        Rows { code: "ja", catalogue: "stub/ja.tsv", text: "str\t\tA stub row\tA stub row\n" },
        Rows { code: "zh-Hans", catalogue: "stub/zh-Hans.tsv", text: "str\t\tA stub row\tA stub row\n" },
    ];

    /// A catalogue for a code no language has.
    static NO_LANGUAGE: &[Rows] = &[Rows { code: "xx", catalogue: "stub/xx.tsv", text: "str\t\tA stub row\tA stub row\n" }];

    /// A notation with no elements and the catalogue it is made with, whose Properties body draws
    /// its one row as a control that asks for the Properties panel when clicked.
    struct Stub(&'static [Rows]);

    /// The stub's state: the ink its body last drew its row in.
    #[derive(Default)]
    struct StubState {
        ink: Option<Color32>,
    }

    impl NotationState for StubState {}

    static STUB: Stub = Stub(STUB_ROWS);
    static STUBS: &[&dyn NotationUi] = &[&STUB];

    impl NotationUi for Stub {
        /// A metamodel no class is of.
        fn metamodel(&self) -> u16 {
            u16::MAX
        }
        /// No element, so no top of Models.
        fn is_top(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> bool {
            false
        }
        /// No rows in Models.
        fn tree(&self, _: &archforge_mof::Store, _: archforge_mof::Obj, _: &Tree<'_>) -> Vec<Row> {
            Vec::new()
        }
        /// Nothing for Models' `+`.
        fn offers(&self, _: &archforge_mof::Store, _: Option<archforge_mof::Obj>) -> Vec<crate::notation_ui::Offer> {
            Vec::new()
        }
        /// Adds nothing, as it offers nothing.
        fn add(&self, _: &mut crate::cx::Ctx, _: &str, _: Option<archforge_mof::Obj>) -> Option<Result<crate::notation_ui::Added, String>> {
            None
        }
        /// Renames nothing.
        fn rename(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<(&'static str, &'static str)> {
            None
        }
        /// Nothing worked out, as none of its elements is removed.
        fn before_removal(&self, _: &archforge_mof::Store, _: &[archforge_mof::Obj]) -> Removal {
            Removal::default()
        }
        /// The word alone, as none of its elements is removed.
        fn removed_text(&self, what: &str, _: &Removal) -> String {
            what.to_string()
        }
        /// No row opens.
        fn foldable(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
            Vec::new()
        }
        /// No rows to open.
        fn parents(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
            Vec::new()
        }
        /// No search entry.
        fn entry(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<Entry> {
            None
        }
        /// No words for Properties.
        fn words(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<Words> {
            None
        }
        /// Its row in its words, a control the automation lists, in the palette's text colour,
        /// which it keeps; a click asks for the Properties panel.
        fn body(&self, cx: &mut crate::cx::Ctx, ui: &mut egui::Ui, p: &Palette, r: Rect, y: f32, _: &Subject) {
            let row = Rect::from_min_size(pos2(r.left() + 16.0, r.top() + y), vec2(200.0, 24.0));
            let text = tr(STUB_ROW);
            ux::text(&ui.painter().clone(), row.min, &text, ux::types::UI, p.text, None);
            if let Some(state) = cx.notation_states.own_mut::<StubState>() {
                state.ink = Some(p.text.into());
            }
            if panels::control_cx(cx, ui, STUB_CONTROL, "row", row, true).accessible_name(&text).clicked() {
                cx.requests.push(Request::ShowPanel(Panel::Properties));
            }
        }
        /// The catalogue it was made with.
        fn catalogues(&self) -> &'static [Rows] {
            self.0
        }
        /// Creates nothing.
        fn created_classes(&self) -> &'static [archforge_mof::ClassId] {
            &[]
        }
        /// None of its own.
        fn name_limit(&self) -> usize {
            0
        }
        /// None of its own.
        fn description_limit(&self) -> usize {
            0
        }
        /// Deletes nothing on the canvas.
        fn canvas_delete(&self, _: &mut crate::cx::Ctx, _: bool) -> bool {
            false
        }
        /// Draws nothing over the canvas.
        fn canvas_overlay(&self, _: &mut crate::cx::Ctx, _: &mut egui::Ui, _: Layer, _: &Overlaid<'_>) {}
        /// Its state, with no ink until its body draws.
        fn new_state(&self) -> Box<dyn NotationState> {
            Box::new(StubState::default())
        }
    }

    /// The stub's Properties body in a harness over a context of the stub, in `language` and the
    /// appearance `dark`.
    fn stub_harness(language: &str, dark: bool) -> egui_kittest::Harness<'static, TestContext> {
        set_language("en");
        let test = TestContext::new(STUBS).unwrap();
        harness(test, language, dark, vec2(400.0, 300.0), |cx, ui, p| {
            let r = ui.max_rect();
            STUB.body(cx, ui, p, r, 0.0, &Subject::root("Shop"));
        })
        .unwrap()
    }

    /// The texts painted in the harness's last frame.
    fn painted(h: &egui_kittest::Harness<'_, TestContext>) -> Vec<String> {
        /// The texts of the shape `s` and of the shapes it holds.
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    /// A context of the stub holds its state and registers its catalogue; a notation's catalogue
    /// for no language is refused.
    #[test]
    fn a_test_context_makes_each_notations_state_and_registers_its_catalogue() {
        let test = TestContext::new(STUBS).unwrap();
        assert!(test.notation_states.own::<StubState>().is_some(), "the stub's state is made");
        assert_eq!(test.notation_uis.len(), 1);
        set_language("de");
        assert_eq!(tr(STUB_ROW), "Eine Stubzeile", "the stub's catalogue is registered");
        set_language("en");
        static WRONG: Stub = Stub(NO_LANGUAGE);
        static WRONGS: &[&dyn NotationUi] = &[&WRONG];
        let refused = TestContext::new(WRONGS).err().unwrap();
        assert_eq!((refused.catalogue, refused.code), ("stub/xx.tsv", "xx"));
    }

    /// The context is over the test context's own parts, and keeps every request made through it
    /// after those made before, none carried out.
    #[test]
    fn the_context_is_over_its_parts_and_keeps_every_request_in_order() {
        let mut test = TestContext::new(STUBS).unwrap();
        let obj = archforge_mof::Obj(7);
        test.problems.insert(obj, vec!["a problem".into()]);
        test.looking_only = true;
        test.frames = 3;
        let id = egui::Id::unique("held");
        test.cx(|cx| {
            assert_eq!(cx.problems.get(&obj).map(Vec::len), Some(1));
            assert!(cx.looking_only && cx.cx_ref().looking_only);
            assert_eq!(cx.frames, 3);
            assert!(cx.menu_bar.open.is_none() && cx.menu_bar.chosen.is_none());
            *cx.menu_bar.held = Some(id);
            cx.canvas.notice = Some("noticed".into());
            cx.requests.push(Request::ShowPanel(Panel::Skills));
        });
        assert_eq!(test.menu_held, Some(id));
        assert_eq!(test.canvas.notice.as_deref(), Some("noticed"));
        assert!(test.cx_ref().looking_only && test.cx_ref().session.is_none());
        test.cx(|cx| cx.requests.push(Request::HideVersionDetails));
        assert_eq!(test.requests, [Request::ShowPanel(Panel::Skills), Request::HideVersionDetails]);
    }

    /// The harness draws the screen in the language given, kept each frame though another was set
    /// between frames, and in the appearance given with its palette; each frame counts and lists
    /// the controls drawn in it once.
    #[test]
    fn the_harness_draws_in_its_language_and_appearance_each_frame() {
        for dark in [false, true] {
            let mut h = stub_harness("de", dark);
            assert!(painted(&h).iter().any(|t| t == "Eine Stubzeile"), "{dark}: {:?}", painted(&h));
            assert_eq!(h.ctx.theme(), if dark { egui::Theme::Dark } else { egui::Theme::Light });
            let ink = h.state().notation_states.own::<StubState>().and_then(|s| s.ink);
            assert_eq!(ink, Some(Color32::from(ux::palette(dark).text)), "{dark}: the appearance's palette");
            set_language("en");
            let before = h.state().frames;
            h.run();
            assert!(painted(&h).iter().any(|t| t == "Eine Stubzeile"), "{dark}: the language is pinned: {:?}", painted(&h));
            assert!(h.state().frames > before && before > 0, "{dark}: frames count");
            let listed = h.state().registry.entries().iter().filter(|e| e.id == STUB_CONTROL).count();
            assert_eq!(listed, 1, "{dark}: the control is listed once, for the last frame");
        }
        set_language("en");
    }

    /// A control is found by its automation id at the centre of where it was drawn, and a click
    /// there reaches it: its request is kept. An id not drawn is not found.
    #[test]
    fn a_control_is_found_by_its_id_and_takes_a_click() {
        let mut h = stub_harness("en", false);
        let entry: &widgets::Entry = h.state().registry.entries().iter().find(|e| e.id == STUB_CONTROL).unwrap();
        let expected = egui::pos2((entry.rect[0] + entry.rect[2]) / 2.0, (entry.rect[1] + entry.rect[3]) / 2.0);
        assert_eq!(centre(&h, STUB_CONTROL), Some(expected));
        assert_eq!(centre(&h, "stub.none"), None);
        assert!(h.state().requests.is_empty());
        click(&mut h, expected);
        assert_eq!(h.state().requests, [Request::ShowPanel(Panel::Properties)]);
        set_language("en");
    }

    /// A harness in a code no language has is refused, naming the codes there are.
    #[test]
    fn a_harness_in_no_language_is_refused() {
        let test = TestContext::new(STUBS).unwrap();
        let refused = harness(test, "xx", false, vec2(100.0, 100.0), |_, _, _| {}).err().unwrap();
        assert!(refused.starts_with("\"xx\" is no language") && refused.contains("de, es"), "{refused}");
    }
}
