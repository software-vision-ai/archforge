//! What the app's window tests reach in the shell, behind the `testing` feature: the window in a
//! harness with its click and its automation ids, the shell's parts those tests read, and the
//! steps of its own they call. With these, the crate's other public items, the screenshot tool's
//! (`crate::snapshot`) and the shell's interfaces, a test that drives the whole window can be
//! built outside the crate. Here too are the words the shell hands its lookups through a table
//! rather than as a literal, which the app's tests across every catalogue read.
//!
//! Each item hands over to the shell's own code unchanged: none draws, reads or decides anything
//! of its own. What a notation's screens reach is not here.

use archforge_mof::rows::Rows;
use archforge_mof::{Obj, Store, Uuid};
use archforge_notations::Session;
use archforge_shell_api::cx::CtxRef;
use archforge_shell_api::kinds::DiagramKind;
use archforge_shell_api::notation_ui::{NotationStates, NotationUi, Row};
use archforge_shell_api::screens::ScreensRegistry;
use egui_kittest::Harness;
use serde_json::Value;

use crate::explorer::Rename;
use crate::hold::HoldRefused;
use crate::leaving::Leaving;
use crate::project_search::Entry;
use crate::theme::ThemeKind;
use crate::{Services, Shell, widgets};

/// The whole window in a harness at the design's frame size, in English and the appearance
/// `theme`, its shell given the notations' screens `screens` and no host services, drawn once.
pub fn harness(screens: &'static ScreensRegistry, theme: ThemeKind) -> Harness<'static, Shell> {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services { ..Services::default() }, screens);
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

/// What the skill `s` aims at, as the Skills panel says it, by the notations `uis`.
pub fn skill_target_word(uis: &[&'static dyn NotationUi], store: &Store, s: Obj) -> String {
    crate::panels::target_word(uis, store, s)
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
    all.extend(lists!(er_ui, er_kind, uml_ui, editing, connect, relation, er_table, keys, checks, indexes, types, subtypes, objects));
    all.extend(lists!(uml_panel, uml_relate, uml_sets, settings_agents, uml_qualifiers, uml_templates, uml_bindings, uml_nary, uml_nesting));
    all.extend(lists!(uml_profiles, uml_stereotypes));
    all
}

/// Every key the labels for what the metamodel names look up for the notations `uis`, each with
/// the index of the metamodel that names it.
pub fn model_words(uis: &[&'static dyn NotationUi]) -> Vec<(u16, String)> {
    crate::model_words::texts(uis)
}

/// The keys of the in-window menu bar's access letters, with the diagram kinds `kinds`: each
/// title's and each row's, once.
pub fn access_keys(kinds: &[&'static dyn DiagramKind]) -> Vec<String> {
    crate::menu_bar::access_keys(kinds)
}
