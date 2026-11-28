//! ER's window tests: the whole app's shell, given ER's and UML's screens as the app registers
//! them, drawn in a harness in both appearances, its controls found and pressed by automation id
//! (ADR 0005: whole-app behaviour with real notations is tested in the app). They reach the shell
//! through `archforge_ui_egui::testing` and `archforge_ui_egui::snapshot`, and the notations'
//! screens through `archforge_er_ui::snapshot` and `archforge_uml_ui::snapshot`. Here are their
//! harnesses, samples, fake hosts and probes; the tests are in the module of their area: ER's
//! canvas, its tools, relationships, tables and Generate in `canvas`; ER's Properties in
//! `properties`; ER's refusals that take a row of the shell's catalogue in `words`; what the shell
//! asks of both notations, each answering its own way, in `notations`, and the Model menu's rows
//! of both in `model_menu`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, reason = "test helpers sit outside the #[test] code that clippy.toml exempts")]

use egui_kittest::Harness;

use archforge_ui_egui::testing::{click, ids};
use archforge_ui_egui::theme::ThemeKind;
use archforge_ui_egui::{Picked, Services, Shell};
// ER's screens' modules, as their crate's `snapshot` lists them for the tests outside it.
use archforge_er_ui::snapshot::{
    er_checks, er_connect, er_keys, er_kind, er_objects, er_place, er_relation, er_said, er_state, er_table, er_types, er_ui,
};
// UML's screens' modules, as their crate's `snapshot` lists them for the tests outside it.
use archforge_uml_ui::snapshot::{
    uml_classifier, uml_inline, uml_nary, uml_panel, uml_relate, uml_sets, uml_state, uml_stereotypes, uml_templates, uml_ui,
};

/// The app's screens parts, ER's then UML's, as the app registers them: every shell the tests make
/// is given them.
static SCREENS: archforge_shell_api::screens::ScreensRegistry =
    archforge_shell_api::screens::ScreensRegistry::new(&[archforge_er_ui::ER, archforge_uml_ui::UML]);

/// Every problem the notations `uis` find in `store`, each notation's in turn, as the shell lists
/// them.
fn problems_all(
    uis: &[&'static dyn archforge_shell_api::notation_ui::NotationUi],
    store: &archforge_mof::Store,
) -> Vec<archforge_shell_api::notation_ui::Problem> {
    let mut out = Vec::new();
    for ui in uis {
        ui.problems(uis, store, &mut out);
    }
    out
}

/// The problems the notations `uis` find in `store`, by the element they sit on, as the canvas
/// badges them.
fn problems_by_element(
    uis: &[&'static dyn archforge_shell_api::notation_ui::NotationUi],
    store: &archforge_mof::Store,
) -> std::collections::BTreeMap<archforge_mof::Obj, Vec<String>> {
    let mut out: std::collections::BTreeMap<archforge_mof::Obj, Vec<String>> = std::collections::BTreeMap::new();
    for p in problems_all(uis, store) {
        out.entry(p.element).or_default().push(p.text);
    }
    out
}

/// The words the canvas hands its lookups through a table, as the shell lists them for the tests
/// across every catalogue.
fn canvas_words() -> impl Iterator<Item = &'static str> {
    archforge_ui_egui::testing::word_lists().into_iter().find(|(name, _)| *name == "canvas").map(|(_, words)| words).unwrap_or_default().into_iter()
}

mod canvas;
mod model_menu;
mod notations;
mod properties;
mod words;

/// ER's own state in `shell`, whose notations hold ER.
fn er_of(shell: &Shell) -> &er_state::ErState {
    er_state::er(archforge_ui_egui::testing::notation_states_of(shell)).expect("the shell's notations hold ER")
}

/// ER's own state in `shell`, to change.
fn er_of_mut(shell: &mut Shell) -> &mut er_state::ErState {
    er_state::er_mut(archforge_ui_egui::snapshot::notation_states(shell)).expect("the shell's notations hold ER")
}

/// UML's own state in `shell`, whose notations hold UML.
fn uml_of(shell: &Shell) -> &uml_state::UmlState {
    uml_state::uml(archforge_ui_egui::testing::notation_states_of(shell)).expect("the shell's notations hold UML")
}

/// UML's own state in `shell`, to change.
fn uml_of_mut(shell: &mut Shell) -> &mut uml_state::UmlState {
    uml_state::uml_mut(archforge_ui_egui::snapshot::notation_states(shell)).expect("the shell's notations hold UML")
}

/// The real shell in the harness at the design's frame size, given the app's screens parts.
fn harness(theme: ThemeKind) -> Harness<'static, Shell> {
    archforge_ui_egui::testing::harness(&archforge_notations::NOTATIONS, &SCREENS, theme)
}

/// Where a test answers a save.
struct SaveAnswer(std::sync::mpsc::Sender<Result<archforge_ui_egui::Saved, archforge_ui_egui::SaveError>>);

impl SaveAnswer {
    /// Gives the save its answer.
    fn send(
        &self,
        answer: Result<bool, archforge_ui_egui::SaveError>,
    ) -> Result<(), std::sync::mpsc::SendError<Result<archforge_ui_egui::Saved, archforge_ui_egui::SaveError>>> {
        self.0.send(
            answer
                .map(|made| if made { archforge_ui_egui::Saved::Version { hash: Some("abc".into()) } } else { archforge_ui_egui::Saved::NothingNew }),
        )
    }
}

/// A host that writes and saves: it keeps every hand-over of rows, and each
/// save's and opening's sender, so a test answers when it chooses.
#[derive(Default)]
struct SavingHost {
    version_limits: std::sync::Mutex<Vec<usize>>,
    written: std::sync::Mutex<Vec<(std::path::PathBuf, archforge_mof::rows::Rows)>>,
    saves: std::sync::Mutex<Vec<SaveAnswer>>,
    /// Why the last write failed, as the test sets it; none by default.
    write_error: std::sync::Mutex<Option<String>>,
    descriptions: std::sync::Mutex<Vec<Option<String>>>,
    saved_by: std::sync::Mutex<Vec<archforge_ui_egui::SavedBy>>,
    opens: std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::mpsc::Sender<Result<archforge_ui_egui::Opened, archforge_ui_egui::CreateError>>)>>,
    /// The branch each opening was asked to open on.
    opened_on: std::sync::Mutex<Vec<Option<String>>>,
    closed: std::sync::Mutex<Vec<std::path::PathBuf>>,
    /// Each closing's answer, sent by the test.
    closings: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<(), String>>>>,
    /// The projects put back to their last save and closed (Don't save), and those closed without
    /// writing what waited (Close without writing).
    discarded: std::sync::Mutex<Vec<std::path::PathBuf>>,
    unwritten: std::sync::Mutex<Vec<std::path::PathBuf>>,
    /// Each branch delete's answer, sent by the test.
    branch_deletes: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<Vec<archforge_mof::Uuid>, archforge_ui_egui::BranchNotDeleted>>>>,
    /// Each branch made, by its stored name, and where the test answers it.
    branch_makes: std::sync::Mutex<Vec<(String, std::sync::mpsc::Sender<Result<(), archforge_ui_egui::BranchRefused>>)>>,
    /// The version each branch made starts at, in the same order; none for current work.
    branch_starts: std::sync::Mutex<Vec<Option<String>>>,
    /// Each branch rename, from and to as stored, and where the test answers it.
    branch_renames: std::sync::Mutex<Vec<(String, String, std::sync::mpsc::Sender<Result<(), archforge_ui_egui::BranchRefused>>)>>,
    /// Each count of what goes with a branch, by its stored name, and where the test answers it.
    branch_counts: std::sync::Mutex<Vec<(String, std::sync::mpsc::Sender<Result<archforge_ui_egui::BranchCounts, String>>)>>,
    /// Each read of a version waiting for the answer the test sends.
    reads: std::sync::Mutex<Vec<(String, ReadAnswer)>>,
    /// The branch each read of another branch's version was asked on, by its stored name.
    branch_version_reads: std::sync::Mutex<Vec<(String, String)>>,
    /// Each read of the versions waiting for the answer the test sends.
    version_reads: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<archforge_ui_egui::VersionsPage, String>>>>,
    /// The version each read of the versions started after.
    version_asks: std::sync::Mutex<Vec<Option<String>>>,
    /// The branch the project is on, what a read of the branches answers (none for no branches),
    /// whether it says more exist, and how many reads were asked for.
    branch_now: std::sync::Mutex<Option<String>>,
    branch_list: std::sync::Mutex<Option<Result<Vec<archforge_ui_egui::BranchEntry>, String>>>,
    branch_cut: std::sync::Mutex<bool>,
    /// While set, each read of the branches waits there for the test to answer it.
    branch_held: std::sync::Mutex<Option<Vec<std::sync::mpsc::Sender<Result<(Vec<archforge_ui_egui::BranchEntry>, bool), String>>>>>,
    branch_reads: std::sync::Mutex<usize>,
    /// Where the answers of each restore's job go, sent by the test.
    restores: std::sync::Mutex<Vec<RestoreEnds>>,
    /// When each version restored was saved and when its restore was asked for, as its job was told.
    restore_dates: std::sync::Mutex<Vec<(String, String)>>,
    /// Each branch switch asked for, and what the next one is refused with.
    switches: std::sync::Mutex<Vec<SwitchEnds>>,
    switch_refused: std::sync::Mutex<Option<String>>,
}

/// Where a branch switch is answered: the branch, its check, the unsaved mark, and its move.
#[expect(dead_code, reason = "the host keeps each branch switch's ends; no test here answers a switch")]
struct SwitchEnds {
    branch: String,
    check: ReadAnswer,
    unsaved: std::sync::Arc<std::sync::atomic::AtomicBool>,
    moved: RestoreEnds,
}

/// Where a restore's job is answered, and where its reading back is.
type RestoreEnds = (std::sync::mpsc::Sender<Result<(), archforge_ui_egui::replace::JobFailure>>, std::sync::mpsc::Sender<Result<bool, String>>);

/// Where the answer to a read of a version goes.
type ReadAnswer = std::sync::mpsc::Sender<archforge_ui_egui::VersionRead>;

impl archforge_ui_egui::ProjectHost for SavingHost {
    /// A job the test answers through what is kept in `restores`.
    fn restore_job(
        &self,
        _project: &std::path::Path,
        _commit: String,
        (saved_at, before_at): (String, String),
        _by: archforge_ui_egui::SavedBy,
        _repair: archforge_ui_egui::Repair,
    ) -> archforge_ui_egui::replace::Job {
        let (answer, answers) = std::sync::mpsc::channel();
        let (reading, readings) = std::sync::mpsc::channel();
        self.restore_dates.lock().unwrap().push((saved_at, before_at));
        self.restores.lock().unwrap().push((answer, reading));
        archforge_ui_egui::replace::Job { run: Box::new(move || answers), read_back: Box::new(move || readings) }
    }

    /// The branch the test says the project is on.
    fn current_branch(&self, _project: &std::path::Path) -> Option<String> {
        self.branch_now.lock().unwrap().clone()
    }

    /// Answers at once with what the test has set.
    fn branches(&self, _project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(Vec<archforge_ui_egui::BranchEntry>, bool), String>> {
        *self.branch_reads.lock().unwrap() += 1;
        let (tx, rx) = std::sync::mpsc::channel();
        if let Some(held) = self.branch_held.lock().unwrap().as_mut() {
            held.push(tx);
            return rx;
        }
        let listed = self.branch_list.lock().unwrap().clone().unwrap_or(Ok(Vec::new()));
        let _ = tx.send(listed.map(|l| (l, *self.branch_cut.lock().unwrap())));
        rx
    }

    /// A switch the test answers through what is kept in `switches`.
    fn begin_branch_switch(&self, _project: &std::path::Path, branch: String) -> Result<archforge_ui_egui::BranchSwitch, String> {
        if let Some(why) = self.switch_refused.lock().unwrap().take() {
            return Err(why);
        }
        let (check, read) = std::sync::mpsc::channel();
        let unsaved = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (answer, answers) = std::sync::mpsc::channel();
        let (reading, readings) = std::sync::mpsc::channel();
        self.switches.lock().unwrap().push(SwitchEnds { branch, check, unsaved: unsaved.clone(), moved: (answer, reading) });
        Ok(archforge_ui_egui::BranchSwitch {
            read,
            unsaved,
            job: archforge_ui_egui::replace::Job { run: Box::new(move || answers), read_back: Box::new(move || readings) },
        })
    }

    /// Answers nothing yet: the test answers later, through what is kept in `reads`.
    fn read_version(&self, _project: &std::path::Path, commit: String) -> std::sync::mpsc::Receiver<archforge_ui_egui::VersionRead> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.reads.lock().unwrap().push((commit, tx));
        rx
    }

    /// A read of a version, answered as [`SavingHost::read_version`]'s are, and kept with its branch.
    fn read_branch_version(
        &self,
        project: &std::path::Path,
        branch: &str,
        commit: String,
    ) -> std::sync::mpsc::Receiver<archforge_ui_egui::VersionRead> {
        self.branch_version_reads.lock().unwrap().push((branch.to_string(), commit.clone()));
        self.read_version(project, commit)
    }

    /// Makes no project: the receiver never answers.
    fn create(
        &self,
        _folder: &std::path::Path,
        _name: &str,
    ) -> std::sync::mpsc::Receiver<Result<archforge_ui_egui::Opened, archforge_ui_egui::CreateError>> {
        std::sync::mpsc::channel().1
    }

    /// Keeps the answer for the test to send.
    fn versions(
        &self,
        _project: &std::path::Path,
        after: Option<String>,
        limit: usize,
    ) -> std::sync::mpsc::Receiver<Result<archforge_ui_egui::VersionsPage, String>> {
        self.version_asks.lock().unwrap().push(after);
        let (tx, rx) = std::sync::mpsc::channel();
        self.version_limits.lock().unwrap().push(limit);
        self.version_reads.lock().unwrap().push(tx);
        rx
    }

    /// Keeps the answer for the test to send.
    fn delete_branch(
        &self,
        _path: &std::path::Path,
        _branch: &str,
    ) -> std::sync::mpsc::Receiver<Result<Vec<archforge_mof::Uuid>, archforge_ui_egui::BranchNotDeleted>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_deletes.lock().unwrap().push(tx);
        rx
    }

    /// Stored in lower case with hyphens, which no listed name shares by accident; empty is refused.
    fn branch_name(&self, typed: &str) -> Result<String, String> {
        Some(typed.trim().to_lowercase().replace(' ', "-")).filter(|s| !s.is_empty()).ok_or_else(|| archforge_ui_egui::i18n::tr("Enter a name."))
    }

    /// Keeps the answer for the test to send.
    fn create_branch(
        &self,
        _path: &std::path::Path,
        name: &str,
        start: Option<&str>,
    ) -> std::sync::mpsc::Receiver<Result<(), archforge_ui_egui::BranchRefused>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_starts.lock().unwrap().push(start.map(str::to_string));
        self.branch_makes.lock().unwrap().push((name.to_string(), tx));
        rx
    }

    /// Keeps the answer for the test to send.
    fn branch_counts(&self, _path: &std::path::Path, branch: &str) -> std::sync::mpsc::Receiver<Result<archforge_ui_egui::BranchCounts, String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_counts.lock().unwrap().push((branch.to_string(), tx));
        rx
    }

    /// Keeps the answer for the test to send.
    fn rename_branch(
        &self,
        _path: &std::path::Path,
        from: &str,
        to: &str,
    ) -> std::sync::mpsc::Receiver<Result<(), archforge_ui_egui::BranchRefused>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_renames.lock().unwrap().push((from.to_string(), to.to_string(), tx));
        rx
    }

    /// Keeps the opening's sender for the test to answer.
    fn open(&self, path: &std::path::Path) -> std::sync::mpsc::Receiver<Result<archforge_ui_egui::Opened, archforge_ui_egui::CreateError>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.opens.lock().unwrap().push((path.to_path_buf(), tx));
        rx
    }

    /// Records the stored branch it was asked for, then opens as `open` does.
    fn open_on(
        &self,
        path: &std::path::Path,
        branch: Option<&str>,
    ) -> std::sync::mpsc::Receiver<Result<archforge_ui_egui::Opened, archforge_ui_egui::CreateError>> {
        self.opened_on.lock().unwrap().push(branch.map(str::to_string));
        self.open(path)
    }

    /// Records the rows handed over.
    fn write(&self, project: &std::path::Path, rows: archforge_mof::rows::Rows) {
        self.written.lock().unwrap().push((project.to_path_buf(), rows));
    }

    /// Builds the rows on a thread of its own, as the app's writer does, then records them.
    fn write_model(&self, project: &std::path::Path, model: archforge_mof::Frozen, notations: &'static archforge_notations::Notations) {
        let (tables, junctions) = notations.storage();
        let rows = std::thread::spawn(move || model.rows(&tables, &junctions)).join().unwrap().unwrap();
        self.write(project, rows);
    }

    /// Nothing waits; the write failed when the test says so.
    fn writing(&self, _project: &std::path::Path) -> archforge_ui_egui::Writing {
        archforge_ui_egui::Writing { pending: false, error: self.write_error.lock().unwrap().clone() }
    }

    /// Records the project closed and keeps the closing's sender for the test to answer.
    fn close(&self, project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        self.closed.lock().unwrap().push(project.to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel();
        self.closings.lock().unwrap().push(tx);
        rx
    }

    /// Records the project put back to its last save and closed, and keeps the closing's sender.
    fn discard_and_close(&self, project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        self.discarded.lock().unwrap().push(project.to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel();
        self.closings.lock().unwrap().push(tx);
        rx
    }

    /// Records the project closed without writing what waited, and keeps the closing's sender.
    fn close_without_writing(&self, project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        self.unwritten.lock().unwrap().push(project.to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel();
        self.closings.lock().unwrap().push(tx);
        rx
    }

    /// Keeps the save's answer and its description for the test.
    fn save(
        &self,
        project: &std::path::Path,
        description: Option<String>,
        by: archforge_ui_egui::SavedBy,
    ) -> std::sync::mpsc::Receiver<Result<archforge_ui_egui::Saved, archforge_ui_egui::SaveError>> {
        assert_eq!(project, std::path::Path::new("/tmp/Commerce platform.archforge"), "the save names the open project");
        self.descriptions.lock().unwrap().push(description);
        self.saved_by.lock().unwrap().push(by);
        let (tx, rx) = std::sync::mpsc::channel();
        self.saves.lock().unwrap().push(SaveAnswer(tx));
        rx
    }
}

/// The shell over a saving host, with a picker that chooses `picked`, and a project open when
/// `opened` is given.
fn saving_harness(opened: Option<archforge_ui_egui::Opened>, picked: Option<&str>) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    archforge_ui_egui::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    let picked = picked.map(std::path::PathBuf::from);
    let services = Services {
        projects: Some(host.clone()),
        ids: Some(test_ids()),
        pick_project: Some(std::sync::Arc::new(move |_| Picked::at_once(picked.clone()))),
        ..Services::default()
    };
    let mut shell = Shell::new(services, &archforge_notations::NOTATIONS, &SCREENS);
    if let Some(opened) = opened {
        shell.open_project(opened);
    }
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run_steps(4);
    (h, host)
}

/// The sample project's opening: Commerce platform, at its path, with a new model.
fn commerce() -> archforge_ui_egui::Opened {
    archforge_ui_egui::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into(), ..Default::default() }
}

/// Whether the open session holds work not saved.
fn dirty(h: &Harness<'static, Shell>) -> bool {
    h.state().session.as_ref().unwrap().is_dirty()
}

/// A saved project with a UML class and an ER table, the class selected in Properties, in
/// `theme`: (harness, host, class, table).
fn described_project(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>, archforge_mof::Uuid, archforge_mof::Uuid) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().state.theme = theme;
    let (class, table) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let pkg = id(s.execute("uml.package.add", &serde_json::json!({ "name": "sales" })).unwrap().value);
        let class = id(s.execute("uml.class.add", &serde_json::json!({ "package": pkg.to_string(), "name": "Order" })).unwrap().value);
        let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
        let table = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema.to_string(), "name": "customer" })).unwrap().value);
        (class, table)
    };
    h.run_steps(2);
    h.state_mut().save();
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
    h.run_steps(2);
    assert!(!dirty(&h), "saved before the description is typed");
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    (h, host, class, table)
}

/// The shell with a project open, in the harness at the design's frame size or another.
fn workspace_harness(theme: ThemeKind, size: egui::Vec2) -> Harness<'static, Shell> {
    archforge_ui_egui::i18n::set_language("en");
    let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() }, &archforge_notations::NOTATIONS, &SCREENS);
    shell.state.theme = theme;
    shell.open_project(archforge_ui_egui::Opened {
        name: "Commerce platform".into(),
        path: "/tmp/Commerce platform.archforge".into(),
        ..Default::default()
    });
    // Frames a sixtieth of a second apart, as on screen, so two clicks in a row read as a double click;
    // enough of them to settle a package switch's fade (AR-95).
    let mut h = Harness::builder().with_size(size).with_step_dt(1.0 / 60.0).with_max_steps(32).build_ui_state(
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

/// The rectangle the last frame drew the control `id` in.
fn rect_of(h: &Harness<'static, Shell>, id: &str) -> egui::Rect {
    let w = h.state().widgets().iter().find(|w| w.id == id).unwrap_or_else(|| panic!("no control {id}"));
    egui::Rect::from_min_max(egui::pos2(w.rect[0], w.rect[1]), egui::pos2(w.rect[2], w.rect[3]))
}

/// The real shell, Home or a project, with the bar drawn as on Windows and Linux.
fn menu_harness(theme: ThemeKind, project: bool) -> Harness<'static, Shell> {
    let mut h = if project { workspace_harness(theme, egui::vec2(1600.0, 1000.0)) } else { harness(theme) };
    h.state_mut().in_window_menu = true;
    h.run();
    h
}

/// Clicks the bar's control `id`.
fn click_menu(h: &mut Harness<'static, Shell>, id: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
}

/// The bar's controls in the frame.
fn menu_ids(h: &Harness<'static, Shell>) -> Vec<String> {
    ids(h).into_iter().filter(|i| i.starts_with("strip.menu.")).collect()
}

/// A press and release of `key` with `modifiers`, and the frames it starts.
fn key(h: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers) {
    h.key_down_modifiers(modifiers, key);
    h.key_up_modifiers(modifiers, key);
    h.run();
}

/// What a key meant alone must not act with held: Shift, Alt, Control, and the command key in the
/// two shapes keyboards send it, a Mac's ⌘ and the Ctrl of Windows and Linux.
const NOT_ALONE: [egui::Modifiers; 5] = [
    egui::Modifiers::SHIFT,
    egui::Modifiers::ALT,
    egui::Modifiers::CTRL,
    egui::Modifiers::MAC_CMD.plus(egui::Modifiers::COMMAND),
    egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND),
];

/// The names of the open model's live schemas.
fn schema_names(h: &Harness<'static, Shell>) -> Vec<String> {
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).map(|o| archforge_core::name_of(st, o)).collect()
}

/// The centre of the Properties control `id`, the panel's body wheeled until that centre is inside
/// it: a control under the fold or above the body is not listed, and one an edge cuts is listed
/// with a centre no press reaches. It is looked for down to the body's end, then up to its top,
/// and it is a failure when neither way has it (AR-467).
fn reached_in_properties(h: &mut Harness<'static, Shell>, id: &str) -> egui::Pos2 {
    let mut down = true;
    for _ in 0..120 {
        let body = h.state().panels.body.expect("the Properties body");
        let at = h.state().widgets().iter().find(|w| w.id == id).map(|w| egui::pos2((w.rect[0] + w.rect[2]) / 2.0, (w.rect[1] + w.rect[3]) / 2.0));
        if let Some(at) = at.filter(|at| body.contains(*at)) {
            return at;
        }
        let before = h.state().panels.scroll;
        h.event(egui::Event::PointerMoved(body.center()));
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, if down { -120.0 } else { 120.0 }),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run();
        // At an end of the body: the search turns at its foot and gives up at its top.
        if (h.state().panels.scroll - before).abs() < 0.5 {
            assert!(down, "no control {id} in reach of the wheel in Properties");
            down = false;
        }
    }
    panic!("no control {id} in reach of the wheel in Properties");
}

/// The centre of the Properties control `id` while it is inside the body as drawn, with no wheel.
fn in_view(h: &Harness<'static, Shell>, id: &str) -> Option<egui::Pos2> {
    let body = h.state().panels.body?;
    let w = h.state().widgets().iter().find(|w| w.id == id)?;
    Some(egui::pos2((w.rect[0] + w.rect[2]) / 2.0, (w.rect[1] + w.rect[3]) / 2.0)).filter(|at| body.contains(*at))
}

/// The body wheeled by `dy` points with the pointer over it, as a person scrolls it.
fn wheel_properties(h: &mut Harness<'static, Shell>, dy: f32) {
    let body = h.state().panels.body.expect("the Properties body");
    h.event(egui::Event::PointerMoved(body.center()));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, dy),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    h.run();
}

/// A project with the schema Shop and its table Customer (`customer_id`, `email`) selected, in
/// light: the harness and the table's id.
fn table_harness() -> (Harness<'static, Shell>, String) {
    table_harness_in(ThemeKind::Light)
}

/// [`table_harness`] in `theme`.
fn table_harness_in(theme: ThemeKind) -> (Harness<'static, Shell>, String) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
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

/// The table harness with Order, the relationship `places` and an ER diagram of the schema
/// showing both tables and the relationship, open in the centre: `(harness, customer, diagram)`.
fn diagram_harness() -> (Harness<'static, Shell>, String, String) {
    diagram_harness_in(ThemeKind::Light)
}

/// [`diagram_harness`] in `theme`.
fn diagram_harness_in(theme: ThemeKind) -> (Harness<'static, Shell>, String, String) {
    let (mut h, customer) = table_harness_in(theme);
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
        let diagram = id(s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "Shop schema" }))
            .unwrap()
            .value);
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
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&diagram).unwrap());
    h.run();
    (h, customer, diagram)
}

/// Asserts that `words` are read out as a polite live status, inside the area recorded as `id`.
fn read_out(h: &Harness<'static, Shell>, words: &str, id: &str) {
    use egui_kittest::kittest::{NodeT, Queryable};
    let status = h.get_by_label(words).accesskit_node();
    assert_eq!((status.role(), status.live()), (egui::accesskit::Role::Status, egui::accesskit::Live::Polite), "{words}: read out");
    assert!(ids(h).iter().any(|i| i == id), "{words}: recorded as {id}");
}

/// A drag with the primary button from `from` to `to`, a frame at each step.
fn drag(h: &mut Harness<'static, Shell>, from: egui::Pos2, to: egui::Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    h.event(egui::Event::PointerMoved(to));
    h.run();
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

/// The name of the first element selected: a diagram's as the Diagrams explorer says it, any other
/// element's own.
fn selected_name(h: &Harness<'static, Shell>) -> String {
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    let o = s.view().selection.first().copied().unwrap();
    if st.class_of(o).ok() == Some(archforge_diagram::DIAGRAM) {
        archforge_ui_egui::diagrams::diagram_name(st, o)
    } else {
        archforge_core::name_of(st, o)
    }
}

/// The automation ids of the cards the last frame drew on the canvas.
fn node_ids(h: &Harness<'static, Shell>) -> Vec<String> {
    ids(h).iter().filter(|i| i.starts_with("workspace.canvas.node.")).cloned().collect()
}

/// Where the node `node` sits on its diagram, in the diagram's units.
fn node_bounds(h: &Harness<'static, Shell>, node: &str) -> archforge_diagram::Rect {
    let st = h.state().session.as_ref().unwrap().doc();
    let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
    archforge_diagram::grid::bounds_of(st, n)
}

/// A secondary click, for context menus.
fn right_click(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
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
        let diagram = id(s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_er::DIAGRAM_KIND, "element": schema, "name": "Shop schema" }))
            .unwrap()
            .value);
        s.execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": customer, "x": 100, "y": 100, "width": 240, "height": 166 }))
            .unwrap();
        s.execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": order, "x": 500, "y": 100, "width": 240, "height": 166 }))
            .unwrap();
        (order, diagram)
    };
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&diagram).unwrap());
    h.run();
    (h, customer, order)
}

/// The live relationships of the open model.
fn relationships(h: &Harness<'static, Shell>) -> Vec<archforge_mof::Obj> {
    let st = h.state().session.as_ref().unwrap().doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).collect()
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

/// The foreign key that realises the relationship `rel`, if any.
fn fk_of(h: &Harness<'static, Shell>, rel: archforge_mof::Obj) -> Option<archforge_mof::Obj> {
    let st = h.state().session.as_ref().unwrap().doc();
    st.referrers(rel).into_iter().find(|(_, p)| *p == archforge_er::foreign_key::RELATIONSHIP).map(|(f, _)| f)
}

/// `Customer.email` of [`table_harness_in`] given `steps` through the engine (its type, flags,
/// default and generation as a test starts it), each with the column as its `attribute`.
fn give_email(h: &mut Harness<'static, Shell>, steps: &[(&str, serde_json::Value)]) {
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let email = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap()).unwrap().to_string();
    for (cmd, params) in steps {
        let mut params = params.clone();
        params["attribute"] = serde_json::json!(email);
        s.execute(cmd, &params).unwrap();
    }
}

/// `Customer.email` of [`table_harness_in`] given `steps` first ([`give_email`]), then its column
/// editor opened from its row, the generation numbered `generation` chosen in the select, `change`
/// made to the rest of what is typed, and Apply clicked. Gives the column and the refusal the
/// editor kept, if any.
fn apply_email_edit(
    h: &mut Harness<'static, Shell>,
    steps: &[(&str, serde_json::Value)],
    generation: usize,
    change: impl FnOnce(&mut er_table::ColumnEdit),
) -> (archforge_mof::Obj, Option<String>) {
    give_email(h, steps);
    h.run();
    let rows: Vec<String> = ids(h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(h, &format!("{}.edit", rows[1])).center();
    click(h, at);
    let at = rect_of(h, "workspace.properties.column.generation").center();
    click(h, at);
    let at = rect_of(h, &format!("workspace.properties.column.generation.{generation}")).center();
    click(h, at);
    change(er_of_mut(h.state_mut()).column.as_mut().unwrap());
    h.run();
    let at = rect_of(h, "workspace.properties.column.apply").center();
    click(h, at);
    let refused = er_of(h.state()).column.as_ref().map(|c| c.refused.clone().unwrap_or_else(|| "the editor stayed open".into()));
    let st = h.state().session.as_ref().unwrap().doc();
    (st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap(), refused)
}

/// Every text painted in the last frame, in the order it was painted.
fn painted<S>(h: &Harness<'_, S>) -> Vec<String> {
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

/// Runs `check` in both appearances.
fn in_both_appearances(check: impl Fn(ThemeKind)) {
    [ThemeKind::Light, ThemeKind::Dark].into_iter().for_each(check);
}

/// The element menu's rows as drawn, top to bottom: each key, with the gap above it from the row
/// before (a section's rule adds a point).
fn menu_drawn(h: &Harness<'static, Shell>) -> Vec<(String, f32)> {
    let mut rows: Vec<(String, egui::Rect)> = ids(h)
        .into_iter()
        .filter_map(|i| i.strip_prefix("workspace.canvas.menu.").map(str::to_string))
        .map(|key| {
            let r = rect_of(h, &format!("workspace.canvas.menu.{key}"));
            (key, r)
        })
        .collect();
    rows.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    let mut above = None;
    rows.into_iter()
        .map(|(key, r)| {
            let gap = above.map_or(0.0, |b: f32| (r.top() - b).round());
            above = Some(r.bottom());
            (key, gap)
        })
        .collect()
}

/// The class diagram the canvas shows, and its classes' names on it.
fn class_diagram_shown(h: &Harness<'static, Shell>) -> (String, Vec<String>) {
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.expect("a diagram is open")).unwrap();
    let kind = st.get(d, archforge_diagram::diagram::KIND).unwrap().as_str().unwrap().to_string();
    let classes = st
        .list(d, archforge_diagram::diagram::NODE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter_map(|n| st.get(n, archforge_diagram::node::ELEMENT).ok()?.as_ref())
        .map(|e| archforge_core::name_of(st, e))
        .collect();
    (kind, classes)
}

/// A window over `shell`, as the workspace harness builds one.
fn harness_over(shell: Shell) -> Harness<'static, Shell> {
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    h
}

/// [`related_sample`] in `theme` in a window of `size`.
fn related_sample_in(theme: ThemeKind, size: egui::Vec2) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>) {
    let mut h = workspace_harness(theme, size);
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    let nodes = st
        .list(d, archforge_diagram::diagram::NODE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter_map(|n| {
            let c = st.get(n, archforge_diagram::node::ELEMENT).ok()?.as_ref()?;
            Some((archforge_core::name_of(st, c), st.uuid(n).ok()?.to_string()))
        })
        .collect();
    (h, nodes)
}

/// The id of the live element of `class` named `name`.
fn named(h: &Harness<'static, Shell>, class: archforge_mof::ClassId, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).expect(name);
    st.uuid(o).unwrap()
}

/// The sample with its payment kinds, BankTransfer's generalization alone in the set Kinds and
/// named by the constraint settled on Payment beside Payment: the generalizations' ids and
/// Payment's.
fn sample_with_a_set_and_a_rule(theme: ThemeKind) -> (Harness<'static, Shell>, [String; 3], String) {
    let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let (gens, payment) = archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::put_payment_kinds).unwrap();
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("uml.generalization-set.add", &serde_json::json!({ "generalizations": [gens[1]], "name": "Kinds" })).unwrap();
    let rule = serde_json::json!({ "context": payment, "name": "settled", "expression": "x", "constrained": [gens[1], payment] });
    s.execute("uml.constraint.add", &rule).unwrap();
    h.run();
    (h, gens, payment)
}

/// The names of the enumeration `enumeration`'s literals, in their order.
fn literal_names(h: &Harness<'static, Shell>, enumeration: archforge_mof::Uuid) -> Vec<String> {
    let st = h.state().session.as_ref().unwrap().doc();
    archforge_uml::commands::literals(st, st.by_uuid(enumeration).unwrap()).into_iter().map(|l| archforge_core::name_of(st, l)).collect()
}

/// [`enumeration_sample`] with OrderStatus holding `literals` and selected: the harness, every
/// card's node by name, OrderStatus and its node.
fn literal_sample(
    theme: ThemeKind,
    literals: &[&str],
) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>, archforge_mof::Uuid, String) {
    let (mut h, nodes, [status_node, _]) = enumeration_sample(theme);
    let status = named(&h, archforge_uml::ENUMERATION, "OrderStatus");
    if !literals.is_empty() {
        archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_ui::put_literals(cx, status, literals)).unwrap();
    }
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [status.to_string()] })).unwrap();
    h.run();
    assert!(h.state().canvas.zoom >= archforge_shell_api::canvas::DETAIL_ZOOM, "the cards show their rows");
    (h, nodes, status, status_node)
}

/// The sample class model in `theme` with two enumerations, OrderStatus and ClosingStatus, placed in
/// its empty bottom row and the whole diagram in view: the harness, every card's node by name, and
/// the enumerations' nodes.
fn enumeration_sample(theme: ThemeKind) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>, [String; 2]) {
    let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    let d = h.state().canvas.active.unwrap().to_string();
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let commerce =
        st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::PACKAGE) && archforge_core::name_of(st, *o) == "Commerce").unwrap();
    let commerce = st.uuid(commerce).unwrap().to_string();
    let mut place = |name: &str, x: i32| {
        let id = s.execute("uml.enumeration.add", &serde_json::json!({ "package": commerce, "name": name })).unwrap().value["id"]
            .as_str()
            .unwrap()
            .to_string();
        let n =
            s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": id, "x": x, "y": 1000, "width": 290, "height": 136 })).unwrap();
        n.value["id"].as_str().unwrap().to_string()
    };
    let enumerations = [place("OrderStatus", 560), place("ClosingStatus", 1040)];
    h.state_mut().canvas.fit_pending = true;
    h.run();
    (h, nodes, enumerations)
}

/// The open Delete from the model dialog: the diagrams it says the element disappears from, and
/// whether it says so in those words. Cancelled after, so nothing changes.
fn asked_diagrams(h: &mut Harness<'static, Shell>) -> (usize, Vec<String>) {
    h.run();
    let asked = h.state().explorer.delete.clone().expect("the dialog asks");
    let said = painted(h).into_iter().filter(|t| t.starts_with("It disappears from")).collect();
    let at = rect_of(h, "workspace.explorer.delete.cancel").center();
    click(h, at);
    assert!(h.state().explorer.delete.is_none());
    (asked.diagrams, said)
}

/// Selects the element of `class` named `name` and gives its id.
fn select_named(h: &mut Harness<'static, Shell>, class: archforge_mof::ClassId, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).unwrap();
    let id = st.uuid(o).unwrap();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
    h.run();
    id
}

/// A package Shop open on its diagram in `theme` (AR-126): the classes Item and Order, Order
/// specializing Item; the enumeration Status with Placed and Paid; Item's `parts : Item [*]`,
/// Order's `lines : Item [*]` and `status : Status`; an association from Order to Item whose
/// second end is `items [*]`. Order is selected with Properties showing it. Gives the harness and
/// the ids by name: Item, Order, Status, parts, lines, status and the association, `holds`.
fn modifiers_sample(theme: ThemeKind) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1400.0));
    let mut ids = std::collections::BTreeMap::new();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let pkg = run("uml.package.add", serde_json::json!({ "name": "Shop" }));
        let d = run("diagram.add", serde_json::json!({ "kind": archforge_uml::STRUCTURE_DIAGRAM, "element": pkg, "name": "Shop" }));
        let item = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Item" }));
        let order = run("uml.class.add", serde_json::json!({ "package": pkg, "name": "Order" }));
        run("uml.generalization.add", serde_json::json!({ "specific": order, "general": item }));
        let status = run("uml.enumeration.add", serde_json::json!({ "package": pkg, "name": "Status" }));
        for l in ["Placed", "Paid"] {
            run("uml.literal.add", serde_json::json!({ "enumeration": status, "name": l }));
        }
        let parts = run("uml.attribute.add", serde_json::json!({ "class": item, "name": "parts", "type": item, "multiplicity": "*" }));
        let lines = run("uml.attribute.add", serde_json::json!({ "class": order, "name": "lines", "type": item, "multiplicity": "*" }));
        let st = run("uml.attribute.add", serde_json::json!({ "class": order, "name": "status", "type": status }));
        let holds = run("uml.association.add", serde_json::json!({ "from": order, "to": item }));
        run("uml.association.set-end", serde_json::json!({ "association": holds, "end": 1, "role": "items", "multiplicity": "*" }));
        for (element, x, y) in [(&item, 80, 80), (&order, 80, 420), (&status, 520, 80)] {
            run("diagram.place", serde_json::json!({ "diagram": d, "element": element, "x": x, "y": y, "width": 260, "height": 160 }));
        }
        run("diagram.connect-all", serde_json::json!({ "diagram": d }));
        for (k, v) in [
            ("Item", item),
            ("Order", order.clone()),
            ("Status", status),
            ("parts", parts),
            ("lines", lines),
            ("status", st),
            ("holds", holds),
            ("diagram", d),
        ] {
            ids.insert(k, v);
        }
    }
    archforge_ui_egui::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&ids["diagram"]).unwrap());
    h.run();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [ids["Order"]] })).unwrap();
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    (h, ids)
}

/// Whether a text the last frame painted holds `text`.
fn painted_has(h: &Harness<'static, Shell>, text: &str) -> bool {
    painted(h).iter().any(|t| t.contains(text))
}

/// The sample class model's Recommendations diagram (AR-128): Customer and Product side by side,
/// Catalog under them; `drawn`, the n-ary association recommends between them. In `theme`, in a
/// window tall enough for an end's editor. Gives the ids by name, as `put_recommends` does.
fn nary_sample(theme: ThemeKind, drawn: bool) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    nary_sample_in(theme, drawn, egui::vec2(1600.0, 1200.0))
}

/// [`nary_sample`] in a window of `size`.
fn nary_sample_in(theme: ThemeKind, drawn: bool, size: egui::Vec2) -> (Harness<'static, Shell>, std::collections::BTreeMap<&'static str, String>) {
    let mut h = workspace_harness(theme, size);
    archforge_ui_egui::snapshot::cx(h.state_mut(), uml_ui::open_sample).unwrap();
    h.run();
    let ids = archforge_ui_egui::snapshot::cx(h.state_mut(), |cx| uml_nary::put_recommends(cx, drawn)).unwrap();
    h.state_mut().canvas.fit_pending = false;
    h.state_mut().canvas.zoom = 1.0;
    h.state_mut().canvas.pan = egui::vec2(20.0, 30.0);
    h.state_mut().state.show_panel(archforge_shell_api::state::Panel::Properties);
    h.run();
    (h, ids)
}

/// What `ask` gives for a press at one point of the diagram `shell` shows, with `node` and `line`
/// under it and the primary button `clicked` or not, as the canvas hands a press to its kind.
fn pressed_with<R>(
    shell: &Shell,
    node: Option<archforge_mof::Obj>,
    line: Option<archforge_mof::Obj>,
    clicked: bool,
    ask: impl FnOnce(&archforge_shell_api::kinds::Press<'_>) -> R,
) -> R {
    let s = shell.session.as_ref().unwrap();
    let store = s.doc();
    let d = store.by_uuid(shell.canvas.active.unwrap()).unwrap();
    let selection = s.view().selection.clone();
    let node_at = move |_| node;
    let line_at = move |_| line;
    let at = Some(PRESSED_AT);
    ask(&archforge_shell_api::kinds::Press {
        at,
        clicked,
        double: false,
        store,
        notations: s.view(),
        d,
        selection: &selection,
        node_at: &node_at,
        line_at: &line_at,
    })
}

/// Where [`pressed_with`] presses, on screen.
const PRESSED_AT: egui::Pos2 = egui::pos2(300.0, 200.0);

/// What a kind's answer to a press is, in a word: none left to the shared gestures, nothing taken,
/// or an action of its own; any other is the shared gestures' own.
fn answered(action: Option<archforge_shell_api::editing::Action>) -> (&'static str, Option<Box<dyn FnOnce(&mut archforge_shell_api::cx::Ctx)>>) {
    match action {
        None => ("none", None),
        Some(archforge_shell_api::editing::Action::Nothing) => ("nothing", None),
        Some(archforge_shell_api::editing::Action::Own(run)) => ("own", Some(run)),
        Some(_) => ("shared", None),
    }
}

/// The node of the element `id` on the diagram `shell` shows.
fn node_here(shell: &Shell, id: &str) -> archforge_mof::Obj {
    let st = shell.session.as_ref().unwrap().doc();
    let d = st.by_uuid(shell.canvas.active.unwrap()).unwrap();
    let e = st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).unwrap();
    archforge_diagram::views_of(st, e).2.into_iter().find(|n| archforge_diagram::diagram_of(st, *n) == Some(d)).unwrap()
}

/// What `read` gives when it reads once a frame of its own whose input holds `keys`, each pressed
/// with `modifiers`, away from any window.
fn keyed_alone(shell: &mut Shell, keys: &[egui::Key], modifiers: egui::Modifiers, mut read: impl FnMut(&mut Shell, &egui::Ui) -> bool) -> bool {
    let events = keys.iter().map(|key| egui::Event::Key { key: *key, physical_key: None, pressed: true, repeat: false, modifiers }).collect();
    let mut took = false;
    let mut out = egui::Context::default().run_ui(egui::RawInput { events, ..Default::default() }, |ui| took = read(shell, ui));
    // Nothing is shown: the fonts' textures go unused.
    out.textures_delta.clear();
    took
}
