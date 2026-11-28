//! What the app's window tests reach in the shell, behind the `testing` feature: the window in a
//! harness with its click and its automation ids, the shell's parts those tests read, and the
//! steps of its own they call. With these, the crate's other public items, the screenshot tool's
//! (`crate::snapshot`) and the shell's interfaces, a test that drives the whole window can be
//! built outside the crate. Here too are the words the shell hands its lookups through a table
//! rather than as a literal, which the app's tests across every catalogue read.
//!
//! Each item hands over to the shell's own code unchanged and decides nothing of its own. Where an
//! item draws, the shell's own drawing paints: `harness` and `click` run the window's frames, and
//! the four that draw alone (`drawn_alone_at`, `recorded_alone`, `overlaid_alone`, `badge_alone`)
//! make a frame of their own with the window's fonts, let the shell's code paint one part into it
//! and give back what it painted and recorded. What a notation's screens reach is not here.

use archforge_mof::rows::Rows;
use archforge_mof::{Obj, Store, Uuid};
use archforge_notations::Session;
use archforge_notations::differences::Difference;
use archforge_shell_api::cx::CtxRef;
use archforge_shell_api::kinds::{DiagramKind, ModelRow};
use archforge_shell_api::notation_ui::{NotationStates, NotationUi, Row};
use archforge_shell_api::screens::ScreensRegistry;
use egui_kittest::Harness;
use serde_json::Value;

use crate::explorer::Rename;
use crate::hold::HoldRefused;
use crate::leaving::Leaving;
use crate::project_search::Entry;
use crate::theme::ThemeKind;
use crate::{BranchEntry, Services, Shell, widgets};

/// The whole window in a harness at the design's frame size, in English and the appearance
/// `theme`, its shell given the notations `notations` and their screens `screens` and no host
/// services, drawn once.
pub fn harness(notations: &'static archforge_notations::Notations, screens: &'static ScreensRegistry, theme: ThemeKind) -> Harness<'static, Shell> {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services { ..Services::default() }, notations, screens);
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

/// A click of the primary button at `at`, pointer moved there first, and the frames it starts.
pub fn click(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
}

/// The automation ids of the controls the last frame drew, in the order drawn.
pub fn ids(h: &Harness<'static, Shell>) -> Vec<String> {
    h.state().widgets().iter().map(|w| w.id.clone()).collect()
}

/// The read-only twin of the narrow context over `shell`'s parts.
pub fn cx_ref(shell: &Shell) -> CtxRef<'_> {
    shell.cx_ref()
}

/// What the notations keep between frames in `shell`, to read; `crate::snapshot` gives them to
/// change.
pub fn notation_states_of(shell: &Shell) -> &NotationStates {
    &shell.notation_states
}

/// The notations' screens `shell` asks, in the order it was given them.
pub fn notation_uis(shell: &Shell) -> &'static [&'static dyn NotationUi] {
    shell.notation_uis
}

/// The diagram kinds `shell` knows, in the order it was given them.
pub fn kinds(shell: &Shell) -> &'static [&'static dyn DiagramKind] {
    shell.kinds
}

/// Where the last frame drew each control of `shell`, by automation id.
pub fn registry(shell: &Shell) -> &widgets::Registry {
    &shell.registry
}

/// The menu of the bar open in `shell`, if any.
pub fn open_menu(shell: &Shell) -> Option<&'static str> {
    shell.menu_bar.open
}

/// The row of the bar's open menu the keys are on, if any.
pub fn menu_row(shell: &Shell) -> Option<&'static str> {
    shell.menu_bar.row
}

/// Every Model menu row of the kinds `kinds`, each with the kind it is of, then the shared ones, as
/// the table of actions finds them.
pub fn model_rows(kinds: &[&'static dyn DiagramKind]) -> Vec<(Option<&'static str>, &'static ModelRow)> {
    crate::model_menu::rows(kinds).collect()
}

/// The rows the Model menu can show at once, one list per kind of `kinds`.
pub fn model_menus(kinds: &[&'static dyn DiagramKind]) -> Vec<Vec<&'static str>> {
    crate::model_menu::every_menu(kinds)
}

/// The classes that get a kind word of their own among the notations `uis`: those each creates.
pub fn kind_classes(uis: &[&'static dyn NotationUi]) -> Vec<archforge_mof::ClassId> {
    crate::model_words::kinds(uis)
}

/// Gives `shell` the notations `uis` in place of its screens parts', for the tests of what the
/// shell asks the notations it holds.
pub fn set_notation_uis(shell: &mut Shell, uis: &'static [&'static dyn NotationUi]) {
    shell.notation_uis = uis;
}

/// The Diagrams explorer's rows of `shell` with its groups made of `kinds`, within `visits` visits
/// when given: each row's depth, name and count, and whether the walk stopped short.
pub fn diagram_rows(shell: &Shell, kinds: &[&'static dyn DiagramKind], visits: Option<usize>) -> (Vec<(u8, String, usize)>, bool) {
    let Some(session) = shell.session.as_ref() else { return (Vec::new(), false) };
    let tree = match visits {
        Some(visits) => crate::diagrams::rows_within(session.doc(), &shell.diagrams, kinds, visits),
        None => crate::diagrams::rows(session.doc(), &shell.diagrams, kinds),
    };
    (tree.rows.into_iter().map(|r| (r.depth, r.label, r.count)).collect(), tree.cut)
}

/// The inline editor the bar holds the keyboard for, if any.
pub fn menu_held(shell: &Shell) -> Option<egui::Id> {
    shell.menu_bar.held
}

/// The Chat conversations `shell` lets go of, with their labels, until each has ended.
pub fn letting_go(shell: &mut Shell) -> &mut Vec<(String, Option<String>)> {
    &mut shell.chats.letting_go
}

/// Whether the Versions explorer's comparison of current work is on its way.
pub fn comparing(shell: &Shell) -> bool {
    shell.versions.comparing.is_some()
}

/// Whether the read of a changed element's two sides is on its way.
pub fn reading_element(shell: &Shell) -> bool {
    shell.versions.element_reading.is_some()
}

/// The change whose element the centre shows, if one is open.
pub fn opened_change(shell: &Shell) -> Option<&Difference> {
    shell.versions.element.as_ref().map(|opened| &opened.row)
}

/// Whether the centre holds a version's details.
pub fn details_open(shell: &Shell) -> bool {
    shell.versions.details.is_some()
}

/// What the Compare page last found: the two sides it compared (each a version's commit, or none
/// for current work) and their changes, or why they could not be told.
pub fn compare_found(shell: &Shell) -> Option<(&(Option<String>, Option<String>), Result<&[Difference], &str>)> {
    let (key, found, _) = shell.versions.compare_results.found.as_ref()?;
    Some((&key.sides, found.as_ref().map(|changes| changes.as_slice()).map_err(String::as_str)))
}

/// The branches the Versions explorer has read, if any.
pub fn branches(shell: &Shell) -> Option<&[BranchEntry]> {
    shell.versions.branches.as_deref()
}

/// Whether Start a branch here… is offered for the version `commit`.
pub fn branch_offered_at(shell: &Shell, commit: &str) -> bool {
    crate::branch_ask::offered_at(shell, commit)
}

/// Whether the project shown holds unsaved work.
pub fn shown_unsaved(shell: &Shell) -> bool {
    shell.shown_unsaved()
}

/// Why the Model menu's rows are off, in words: the first thing that keeps them off.
pub fn why_model_menu_off(shell: &Shell) -> String {
    crate::model_menu::why_off(shell)
}

/// What a control that is off says while `shell` shows what it shows.
pub fn off_words(shell: &Shell) -> String {
    crate::home::off_words(shell)
}

/// The Models tree's rows for `store`, by the notations `uis`, with the rows opened and the filter
/// of `state`.
pub fn models_rows(uis: &[&'static dyn NotationUi], store: &Store, state: &crate::explorer::State) -> Vec<Row> {
    crate::explorer::rows(uis, store, state)
}

/// Takes the nodes `nodes` off the diagram shown, their elements kept in the model.
pub fn unplace(shell: &mut Shell, nodes: Vec<Uuid>) {
    crate::editing::unplace(shell, nodes);
}

/// The description of the element `obj` as its notation keeps it, or nothing.
pub fn description_now(shell: &Shell, obj: Obj) -> String {
    crate::panels::description_now(shell, obj)
}

/// What the skill `s` aims at in a project seen through `view`, as the Skills panel says it, by the
/// notations `uis`.
pub fn skill_target_word(view: &archforge_diagram::ProjectView, uis: &[&'static dyn NotationUi], store: &Store, s: Obj) -> String {
    crate::panels::target_word(view, uis, store, s)
}

/// The search's index of the diagrams and elements of `store`, by the kinds `kinds` and the
/// notations `uis`.
pub fn search_index(kinds: &[&'static dyn DiagramKind], uis: &[&'static dyn NotationUi], store: &Store) -> Vec<Entry> {
    crate::project_search::index(kinds, uis, store)
}

/// What the search finds in `index` for `query`: the diagrams, then the elements.
pub fn search_hits<'a>(index: &'a [Entry], query: &str) -> (Vec<&'a Entry>, Vec<&'a Entry>) {
    crate::project_search::hits(index, query)
}

/// Whether a request `method` with `params` from the token of `origin`, in the name of the
/// conversation `conversation`, may go on in `shell`; the refusal in words when not.
pub fn chat_gate(shell: &Shell, origin: Option<&str>, conversation: Option<&str>, method: &str, params: &Value) -> Result<(), String> {
    crate::chat_panel::gate(shell, origin, conversation, method, params)
}

/// A name just made for the element `target`, selected whole so typing replaces it.
pub fn fresh_rename(target: Uuid, text: String) -> Rename {
    Rename::fresh(target, text)
}

/// A session of `rows` as `shell` builds one to take the place of its own.
pub fn build_session(shell: &Shell, rows: &Rows) -> Result<Session, String> {
    shell.build(rows)
}

/// `session` takes the place of `shell`'s session, `repaired` when building it changed the model.
pub fn install_session(shell: &mut Shell, session: Session, repaired: bool) {
    shell.install(session, repaired);
}

/// Whether `shell` lets current work be left for `then`, or what stops it; the proof the shell
/// makes when it does stays inside the shell.
pub fn clear_to_leave(shell: &mut Shell, then: Option<Leaving>) -> Result<(), HoldRefused> {
    shell.clear_to_leave(then).map(|_| ())
}

/// Whether `shell`'s replacement is still waiting for its comparison off the frame, which a test
/// waits out before it counts the frames that follow.
pub fn replacement_comparing(shell: &mut Shell) -> bool {
    shell.comparing()
}

/// Every word a module of the shell hands `tr` through a table rather than as a literal, which a
/// scan of the source cannot find, each list with the name of the module that says it (its file,
/// `src/<name>.rs`): the start cards' texts (`home`), then each module's own list of them.
pub fn word_lists() -> Vec<(&'static str, Vec<&'static str>)> {
    macro_rules! lists {
        ($($name:ident),* $(,)?) => {
            [$((stringify!($name), crate::$name::texts().into_iter().collect::<Vec<&'static str>>())),*]
        };
    }
    let mut all = vec![("home", crate::home::start_texts().collect())];
    all.extend(lists!(workspace, panels, chat_view, appearance, skill_tab, canvas));
    all.extend(lists!(editing, settings_agents));
    all
}

/// Every key the labels for what the metamodel names look up for the notations `uis` among the
/// composition `notations`, each with the index of the metamodel that names it.
pub fn model_words(notations: &archforge_notations::Notations, uis: &[&'static dyn NotationUi]) -> Vec<(u16, String)> {
    crate::model_words::texts(notations, uis)
}

/// The keys of the in-window menu bar's access letters, with the diagram kinds `kinds`: each
/// title's and each row's, once.
pub fn access_keys(kinds: &[&'static dyn DiagramKind]) -> Vec<String> {
    crate::menu_bar::access_keys(kinds)
}

/// The texts the last frame of `h` painted, in order.
fn painted_texts<S>(h: &Harness<'_, S>) -> Vec<String> {
    /// Collects the texts of `s` and of the shapes it holds into `out`.
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

/// What the canvas's own drawing paints and records of the diagram `d` of `session`, with no shell
/// anywhere, as `kind` draws its own figures and `kinds` are asked after it for a card they draw
/// themselves, over an area `width` wide, `overrides` as a drag's preview and `problems` badged:
/// the texts, each id recorded with its rectangle, and how many badges it gave back.
pub fn drawn_alone_at(
    session: &Session,
    d: &str,
    (kind, kinds): (Option<&'static dyn DiagramKind>, &'static [&'static dyn DiagramKind]),
    dark: bool,
    width: f32,
    overrides: &std::collections::BTreeMap<Uuid, egui::Rect>,
    problems: &std::collections::BTreeMap<Obj, Vec<String>>,
) -> (Vec<String>, Vec<(String, egui::Rect)>, usize) {
    let store = session.doc();
    let Some(diagram) = Uuid::parse(d).and_then(|id| store.by_uuid(id)) else { return (Vec::new(), Vec::new(), 0) };
    let grid = archforge_diagram::Grid::build(store, diagram);
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 700.0));
    let view = crate::canvas::View { origin: area.min, pan: egui::Vec2::ZERO, zoom: 1.0 };
    let badges = std::cell::Cell::new(0);
    let recorded = std::cell::RefCell::new(widgets::Registry::default());
    let mut h = Harness::builder().with_size(egui::vec2(1000.0, 700.0)).build_ui(|ui| {
        // The window's fonts are the context's, in place a pass later; nothing else is shared.
        if !ui.ctx().fonts(|f| f.definitions().families.contains_key(&egui::FontFamily::Name("ux-regular".into()))) {
            ui.ctx().set_fonts(crate::fonts::definitions());
            return;
        }
        let scene = crate::canvas::Scene {
            p: crate::ux::palette(dark),
            store,
            notations: session.view(),
            kind,
            kinds,
            diagram,
            view: &view,
            grid: &grid,
            area,
        };
        let live = crate::canvas::Live {
            selection: &[],
            overrides,
            key_status: false,
            looking_only: false,
            renaming: None,
            problems,
            hovered: None,
            end_drag: None,
            segment_drag: None,
            draft: None,
            rename_text: None,
            label_drag: None,
        };
        recorded.borrow_mut().begin_frame();
        let drawn = crate::canvas::draw_scene(ui, &ui.painter().with_clip_rect(area), &scene, live, &mut recorded.borrow_mut());
        badges.set(drawn.badges.len());
    });
    h.run();
    let texts = painted_texts(&h);
    drop(h);
    let at = |r: [f32; 4]| egui::Rect::from_min_max(egui::pos2(r[0], r[1]), egui::pos2(r[2], r[3]));
    let ids = recorded.borrow().entries().iter().map(|e| (e.id.clone(), at(e.rect))).collect();
    (texts, ids, badges.get())
}

/// What `draw` records in `shell`'s registry when it draws once with the window's fonts in a context
/// of its own, away from any window: the ids, in order. The window's own registry is set aside
/// meanwhile and put back.
pub fn recorded_alone(shell: &mut Shell, mut draw: impl FnMut(&mut Shell, &egui::Ui)) -> Vec<String> {
    let ctx = egui::Context::default();
    let mut ids = Vec::new();
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            // The window's fonts are the context's, in place a pass later.
            if !ui.ctx().fonts(|f| f.definitions().families.contains_key(&egui::FontFamily::Name("ux-regular".into()))) {
                ui.ctx().set_fonts(crate::fonts::definitions());
                return;
            }
            let kept = std::mem::take(&mut shell.registry);
            draw(shell, ui);
            ids = std::mem::replace(&mut shell.registry, kept).entries().iter().map(|e| e.id.clone()).collect();
        });
        // Nothing is shown: the fonts' textures go unused.
        out.textures_delta.clear();
    }
    ids
}

/// What `notation` draws over the canvas at `r` of `shell`'s open diagram at `layer`, alone in a
/// frame of its own, in the appearance `dark`: how many shapes it painted and the ids it recorded.
pub fn overlaid_alone(
    shell: &mut Shell,
    notation: &dyn NotationUi,
    layer: archforge_shell_api::notation_ui::Layer,
    r: egui::Rect,
    dark: bool,
) -> (usize, Vec<String>) {
    let view = crate::canvas::View { origin: r.min, pan: shell.canvas.pan, zoom: shell.canvas.zoom };
    let Some(d) = shell.session.as_ref().zip(shell.canvas.active).and_then(|(s, id)| s.doc().by_uuid(id)) else { return (0, Vec::new()) };
    let overrides = std::collections::BTreeMap::new();
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::fonts::definitions());
    // The fonts are in place a pass later; that pass draws nothing.
    let mut warm = ctx.run_ui(egui::RawInput::default(), |_| {});
    warm.textures_delta.clear();
    // Both of the registry's frames emptied, so only what this one records is read.
    shell.registry.begin_frame();
    shell.registry.begin_frame();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
    let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
        let on = archforge_shell_api::notation_ui::Overlaid { p: crate::ux::palette(dark), r, view, d, overrides: &overrides };
        shell.cx(|cx| notation.canvas_overlay(cx, ui, layer, &on));
    });
    out.textures_delta.clear();
    (out.shapes.len(), shell.widgets().iter().map(|w| w.id.clone()).collect())
}

/// The texts the canvas's problem badge of `element` paints, drawn alone in a frame of its own in
/// `shell`'s appearance, offering a way out or not as `resolvable` says.
pub fn badge_alone(shell: &mut Shell, element: Uuid, resolvable: bool) -> Vec<String> {
    /// Collects the texts of `s` and of the shapes it holds into `out`.
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::fonts::definitions());
    // The fonts are in place a pass later; that pass draws nothing.
    let mut warm = ctx.run_ui(egui::RawInput::default(), |_| {});
    warm.textures_delta.clear();
    let p = crate::ux::palette(shell.state.theme == ThemeKind::Dark);
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        crate::canvas::badge(shell, ui, p, egui::pos2(100.0, 100.0), element, element, &["A problem".to_string()], resolvable);
    });
    out.textures_delta.clear();
    let mut texts = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut texts));
    texts
}

/// The menus of the shell's own in its one menu slot: Home's sort, recent and pinned menus and the
/// branch menu of the versions.
pub fn shell_menus() -> [archforge_shell_api::notation_ui::MenuId; 4] {
    [crate::home::RECENT_SORT, crate::home::RECENT, crate::home::PINNED, crate::versions::BRANCH_MENU]
}

/// Lets `shell`'s notations drop what their store no longer has, as the shell asks them each frame.
pub fn retain_valid(shell: &mut Shell) {
    if let Some(session) = shell.session.as_ref() {
        shell.notation_states.retain_valid(session.doc(), &shell.canvas);
    }
}
