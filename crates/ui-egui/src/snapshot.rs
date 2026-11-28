//! What the screenshot tool reaches in the shell, behind the `snapshot` feature: the shell's
//! narrow context, its project and notation states, its commands, and the steps that take its own
//! screens to what a picture shows. The tool (the app's `offscreen` and `snapshot` example) and
//! `chat_fixture` build every screen through these and the crate's other public items, so the tool
//! is built outside the crate.
//!
//! Each item hands over to the shell's own code unchanged: none draws, reads or decides anything
//! of its own. What a notation's screens reach is not here.

use archforge_mof::Obj;
use archforge_notations::differences::Difference;
use archforge_shell_api::cx::Ctx;
use archforge_shell_api::notation_ui::NotationStates;

use crate::chat_words::Clock;
use crate::new_project::Dialog;
use crate::ux::Colour;
use crate::{CreateError, Shell, VersionEntry, VersionsPage};

/// Runs `f` with the narrow context over `shell`'s parts, then carries out the requests `f` made,
/// and gives what `f` gave: how a notation's screens are opened from outside the shell.
pub fn cx<R>(shell: &mut Shell, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
    shell.cx(f)
}

/// Makes the session of the project open in `shell`, once per project, as the first frame does.
pub fn adopt_project(shell: &mut Shell) {
    shell.adopt_project();
}

/// What the notations keep between frames in `shell`, each notation's state by its own type.
pub fn notation_states(shell: &mut Shell) -> &mut NotationStates {
    &mut shell.notation_states
}

/// Runs the command `id` with `params` on the open project's session through the narrow context
/// ([`cx`]), and gives its answer, or its refusal as the panel says it.
pub fn run(shell: &mut Shell, id: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    crate::panels::run(shell, id, params)
}

/// The time now, in seconds since 1970, as Home's recent list records it.
pub fn now() -> u64 {
    crate::home::now()
}

/// Opens the search with `query` typed, over the open project or on Home.
pub fn open_home_search(shell: &mut Shell, query: &str) {
    crate::home_search::open_with(shell, query);
}

/// Shows `error` as the New project dialog's last creation's failure.
pub fn show_create_failure(dialog: &mut Dialog, error: CreateError) {
    dialog.show_failure(error);
}

/// The canvas's zoom range: its smallest and its largest zoom.
pub const ZOOM_RANGE: (f32, f32) = crate::canvas::ZOOM_RANGE;

/// The diagram the canvas shows, when the store still has it.
pub fn active_diagram(shell: &Shell) -> Option<Obj> {
    crate::canvas::active_diagram(shell)
}

/// A colour `#rrggbb` as the canvas draws it, or none for anything else.
pub fn colour(hex: &str) -> Option<Colour> {
    crate::canvas::colour(hex)
}

/// Looks for the agent programs, once, when the host has the service and the system runs agents,
/// as the first frame does.
pub fn start_agents(shell: &mut Shell) {
    crate::ai_agents::start(shell);
}

/// Takes in what the agents host has answered, as each frame does.
pub fn poll_agents(shell: &mut Shell, ctx: &egui::Context) {
    crate::ai_agents::poll(shell, ctx);
}

/// Chat's step of a frame: connects the host's events to the window the first time, and takes in
/// what the host sent.
pub fn chat_frame(shell: &mut Shell, ctx: &egui::Context) {
    crate::chat::frame(shell, ctx);
}

/// The Chat panel's step of a frame: the agent's events for every conversation, the project's
/// answers, and undo and redo over the turns of the project shown.
pub fn chat_panel_frame(shell: &mut Shell) {
    crate::chat_panel::frame(shell);
}

/// The step of a frame for deleting conversations: the project's answers to deletions and
/// restorations taken in, a toast whose time is up closed.
pub fn chat_delete_frame(shell: &mut Shell) {
    crate::chat_delete::frame(shell);
}

/// Opens the version chip's list when it can be asked for; a list already open stays as it is.
pub fn open_version_list(shell: &mut Shell) {
    crate::version_chip::open(shell);
}

/// Shows the version `hash`, read `label`, and gives whether it is on its way; a refusal is said
/// as the toast above the toolbar.
pub fn view_version(shell: &mut Shell, hash: String, label: String) -> bool {
    crate::version_chip::view(shell, hash, label)
}

/// A stored time, RFC 3339 in UTC, as the version list says it, local by `clock`.
pub fn version_when(date: &str, clock: Clock) -> String {
    crate::version_chip::when(date, clock)
}

/// A version as a select or a link names it: its date, then what it was described as.
pub fn version_dated(entry: &VersionEntry, clock: Clock) -> String {
    crate::versions::dated(entry, clock)
}

/// Selects the version `entry` and opens its details in the Versions explorer.
pub fn open_version_details(shell: &mut Shell, entry: VersionEntry) {
    crate::version_details::open(shell, entry);
}

/// Compare… from the menu: the Compare page of a version against current work.
pub fn compare_from_menu(shell: &mut Shell) {
    crate::version_details::compare_from_menu(shell);
}

/// Whether the Compare page is open over a version's details.
pub fn compare_open(shell: &Shell) -> bool {
    crate::version_details::compare_open(shell)
}

/// Opens the change `row` in place of a version's details.
pub fn open_change(shell: &mut Shell, row: Difference) {
    crate::change_view::open(shell, row);
}

/// Asks whether to restore a version, given as its commit, when it was saved and what it is
/// called.
pub fn ask_restore(shell: &mut Shell, asked: (String, String, String)) {
    crate::restore_ask::ask(shell, asked);
}

/// The versions the Versions explorer has read, if any.
pub fn versions_page(shell: &Shell) -> Option<&VersionsPage> {
    shell.versions.page.as_ref()
}

/// The changes of current work the Versions explorer shows, from its last comparison.
pub fn changes(shell: &Shell) -> &[Difference] {
    &shell.versions.changes
}

/// Whether a comparison of what is shown now has finished, and none is on its way.
pub fn changes_settled(shell: &Shell) -> bool {
    crate::versions::settled(shell)
}

/// Whether a read of versions, of their steps or of a version to show, or a comparison of one, is
/// still on its way.
pub fn reading_versions(shell: &Shell) -> bool {
    let v = &shell.versions;
    v.reading.is_some()
        || v.element_reading.is_some()
        || v.details_comparing.is_some()
        || v.details_reading.is_some()
        || v.compare_results.reading.is_some()
        || v.compare_results.comparing.is_some()
        || shell.version_list.steps.reading.is_some()
        || shell.loads.waiting()
}
