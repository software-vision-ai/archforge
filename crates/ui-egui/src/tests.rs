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
    let mut shell = Shell::new(Services { ..Services::default() });
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
        for id in ["strip.home", "strip.theme", "home.header.search", "home.start.open-project", "home.pin-folder", "home.banner.close"] {
            assert!(ids.iter().any(|i| i == id), "{theme:?}: {id} in {ids:?}");
        }
        assert_eq!(ids.iter().filter(|i| i.starts_with("home.start.")).count(), 2, "the two start cards that lead somewhere");
        assert!(!ids.iter().any(|i| i == "home.start.import"), "{theme:?}: Import a model is hidden until importing exists");
        assert!(!ids.iter().any(|i| i == "home.start.example"), "{theme:?}: Explore an example is hidden until there is one");
        assert!(!ids.iter().any(|i| i == "home.open-project"), "{theme:?}: no Open project button in the welcome");
        assert_eq!(h.ctx.theme(), if theme.is_dark() { egui::Theme::Dark } else { egui::Theme::Light });
    }
}

#[test]
fn home_lists_its_pinned_folders_and_recent_projects_in_both_appearances() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        crate::i18n::set_language("en");
        let mut shell = Shell::new(Services::default());
        shell.state.theme = theme;
        shell.state.pin(std::path::Path::new("/Users/robin/Documents/Models"));
        shell.state.remember("Library system", std::path::Path::new("/Users/robin/Documents/Archforge/Library system.archforge"), 1_761_670_800);
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
    answer(&mut h, &host, Ok(crate::Opened { name: "Commerce platform".into(), path: path.clone(), ..Default::default() }));
    assert!(h.state().new_project.is_none(), "success closes the dialog");
    assert_eq!(h.state().project.as_ref().map(|p| p.path.clone()), Some(path));
    assert_eq!(h.state().title(), "Commerce platform — Archforge");
    assert_eq!(h.state().state.recent.first().map(|r| r.name.as_str()), Some("Commerce platform"));
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

/// A host that writes and saves: it keeps every hand-over of rows, and each
/// save's and opening's sender, so a test answers when it chooses.
#[derive(Default)]
struct SavingHost {
    written: std::sync::Mutex<Vec<(std::path::PathBuf, archforge_mof::rows::Rows)>>,
    saves: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<bool, String>>>>,
    opens: std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::mpsc::Sender<Result<crate::Opened, crate::CreateError>>)>>,
    closed: std::sync::Mutex<Vec<std::path::PathBuf>>,
    /// Each closing's answer, sent by the test.
    closings: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<(), String>>>>,
    /// The projects put back to their last save and closed (Don't save), and those closed without
    /// writing what waited (Close without writing).
    discarded: std::sync::Mutex<Vec<std::path::PathBuf>>,
    unwritten: std::sync::Mutex<Vec<std::path::PathBuf>>,
}

impl crate::ProjectHost for SavingHost {
    fn create(&self, _folder: &std::path::Path, _name: &str) -> std::sync::mpsc::Receiver<Result<crate::Opened, crate::CreateError>> {
        std::sync::mpsc::channel().1
    }

    fn open(&self, path: &std::path::Path) -> std::sync::mpsc::Receiver<Result<crate::Opened, crate::CreateError>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.opens.lock().unwrap().push((path.to_path_buf(), tx));
        rx
    }

    fn write(&self, project: &std::path::Path, rows: archforge_mof::rows::Rows) {
        self.written.lock().unwrap().push((project.to_path_buf(), rows));
    }

    fn close(&self, project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        self.closed.lock().unwrap().push(project.to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel();
        self.closings.lock().unwrap().push(tx);
        rx
    }

    fn discard_and_close(&self, project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        self.discarded.lock().unwrap().push(project.to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel();
        self.closings.lock().unwrap().push(tx);
        rx
    }

    fn close_without_writing(&self, project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(), String>> {
        self.unwritten.lock().unwrap().push(project.to_path_buf());
        let (tx, rx) = std::sync::mpsc::channel();
        self.closings.lock().unwrap().push(tx);
        rx
    }

    fn save(&self, project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<bool, String>> {
        assert_eq!(project, std::path::Path::new("/tmp/Commerce platform.archforge"), "the save names the open project");
        let (tx, rx) = std::sync::mpsc::channel();
        self.saves.lock().unwrap().push(tx);
        rx
    }
}

/// The shell over a saving host, with a picker that chooses `picked`, and a project open when
/// `opened` is given.
fn saving_harness(opened: Option<crate::Opened>, picked: Option<&str>) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    let picked = picked.map(std::path::PathBuf::from);
    let services = Services {
        projects: Some(host.clone()),
        ids: Some(test_ids()),
        pick_project: Some(std::sync::Arc::new(move |_| picked.clone())),
        ..Services::default()
    };
    let mut shell = Shell::new(services);
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

fn commerce() -> crate::Opened {
    crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into(), ..Default::default() }
}

fn add_schema(h: &mut Harness<'static, Shell>, name: &str) {
    h.state_mut().session.as_mut().unwrap().execute("er.schema.add", &serde_json::json!({ "name": name })).unwrap();
    h.run_steps(2);
}

fn dirty(h: &Harness<'static, Shell>) -> bool {
    h.state().session.as_ref().unwrap().is_dirty()
}

#[test]
fn every_edit_is_handed_to_the_host_and_save_marks_only_what_it_wrote_saved() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    // A new project's model is handed over once, then each change once.
    assert_eq!(host.written.lock().unwrap().len(), 1);
    assert_eq!(host.written.lock().unwrap()[0].0, std::path::PathBuf::from("/tmp/Commerce platform.archforge"), "rows name their project");
    add_schema(&mut h, "shop");
    assert_eq!(host.written.lock().unwrap().len(), 2);
    h.run_steps(4);
    assert_eq!(host.written.lock().unwrap().len(), 2, "an unchanged model is not handed over again");
    assert!(dirty(&h));
    // Cmd+S asks the host to save; an edit made while it works stays unsaved after it answers.
    h.event(egui::Event::Key { key: egui::Key::S, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.run_steps(2);
    assert!(h.state().is_saving());
    assert_eq!(host.saves.lock().unwrap().len(), 1);
    add_schema(&mut h, "crm");
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
    h.run_steps(2);
    assert!(!h.state().is_saving());
    assert!(dirty(&h), "the edit made during the save is not saved");
    // Undo back to the state the save wrote is saved; a second save covers the rest.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run_steps(2);
    assert!(!dirty(&h));
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run_steps(2);
    h.state_mut().save();
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
    h.run_steps(2);
    assert!(!dirty(&h));
    // A failed save says why and leaves the changes unsaved.
    add_schema(&mut h, "billing");
    h.state_mut().save();
    host.saves.lock().unwrap().pop().unwrap().send(Err("the disk is full".into())).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().save_error.as_deref(), Some("the disk is full"));
    assert!(dirty(&h));
}

#[test]
fn an_agents_save_is_followed_by_its_ticket_whichever_tab_is_shown() {
    use crate::control::answer;
    let none = serde_json::json!({});
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    let begun = answer(h.state_mut(), "app.save", &none).unwrap();
    assert_eq!((begun["saving"].clone(), begun["project"].clone()), (serde_json::json!(true), serde_json::json!("Commerce platform")));
    let ticket = begun["ticket"].clone();
    let saved = |h: &mut Harness<'static, Shell>| answer(h.state_mut(), "app.saved", &serde_json::json!({ "ticket": ticket }));
    // A second request while it runs follows the same save.
    assert_eq!(answer(h.state_mut(), "app.save", &none).unwrap()["ticket"], ticket);
    assert_eq!(host.saves.lock().unwrap().len(), 1);
    assert_eq!(saved(&mut h).unwrap()["saving"], true);
    // The user opens another project, which has nothing to save: the first save still runs.
    h.state_mut().open_project(crate::Opened { name: "Other".into(), path: "/tmp/Other.archforge".into(), ..Default::default() });
    h.run_steps(2);
    assert_eq!(saved(&mut h).unwrap()["saving"], true, "a parked tab's save is not the shown tab's");
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
    h.run_steps(2);
    let done = saved(&mut h).unwrap();
    assert_eq!((done["saved"].clone(), done["saving"].clone()), (serde_json::json!(true), serde_json::json!(false)), "{done}");
    h.state_mut().activate(0);
    h.run_steps(2);
    assert!(!dirty(&h), "the parked project's save marked what it wrote saved");
    // A failed save says why; an unknown ticket is refused.
    add_schema(&mut h, "crm");
    let ticket = answer(h.state_mut(), "app.save", &none).unwrap()["ticket"].clone();
    host.saves.lock().unwrap().pop().unwrap().send(Err("the disk is full".into())).unwrap();
    h.run_steps(2);
    let failed = answer(h.state_mut(), "app.saved", &serde_json::json!({ "ticket": ticket })).unwrap_err();
    assert!(failed.contains("the disk is full"), "{failed}");
    assert!(answer(h.state_mut(), "app.saved", &serde_json::json!({ "ticket": 999 })).unwrap_err().contains("no save 999"));
    assert!(answer(h.state_mut(), "app.saved", &none).unwrap_err().contains("ticket"));
}

#[test]
fn a_save_whose_tab_closes_before_the_host_answers_is_not_known() {
    use crate::control::answer;
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    let ticket = answer(h.state_mut(), "app.save", &serde_json::json!({})).unwrap()["ticket"].clone();
    // Unsaved, so closing asks; Don't save closes it while the save still runs.
    h.state_mut().close_tab(0);
    assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::Unsaved { .. })));
    h.state_mut().ask = None;
    h.state_mut().discard_tab(0, true);
    h.run_steps(2);
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(2);
    assert!(h.state().tabs.is_empty());
    let unknown = answer(h.state_mut(), "app.saved", &serde_json::json!({ "ticket": ticket })).unwrap_err();
    assert!(unknown.contains("not known") && !unknown.contains("failed"), "{unknown}");
}

#[test]
fn open_project_asks_the_picker_and_the_host_and_shows_the_model_it_read() {
    // A model made elsewhere, as the host reads it from a project.
    let rows = {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        add_schema(&mut h, "shop");
        host.written.lock().unwrap().pop().unwrap().1
    };
    let (mut h, host) = saving_harness(None, Some("/Users/robin/Documents/Archforge/Shop.archforge"));
    let at = rect_of(&h, "home.start.open-project").center();
    click(&mut h, at);
    h.run_steps(2);
    let (path, tx) = host.opens.lock().unwrap().pop().expect("the host was asked to open");
    assert_eq!(path, std::path::PathBuf::from("/Users/robin/Documents/Archforge/Shop.archforge"));
    let opened = crate::Opened { name: "Shop".into(), path, model: Some(std::sync::Arc::new(rows)), unsaved: true };
    tx.send(Ok(opened)).unwrap();
    h.run_steps(4);
    let shell = h.state();
    assert_eq!(shell.title(), "• Shop — Archforge", "opened with unsaved changes: the title marks them");
    assert_eq!(shell.state.recent.first().map(|r| r.name.as_str()), Some("Shop"));
    let session = shell.session.as_ref().expect("the model read opens");
    assert!(session.doc().alive().any(|o| archforge_core::name_of(session.doc(), o) == "shop"));
    // Changes the project held but never committed show as unsaved; what was read is not handed
    // back to be written.
    assert!(session.is_dirty());
    assert!(host.written.lock().unwrap().is_empty());
}

#[test]
fn a_table_box_saved_too_short_for_its_columns_opens_grown_without_an_undo_step() {
    // A project written by an earlier version: Account holds two columns in a box that shows none.
    let rows = {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
        let account = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Account" })).unwrap().value);
        for name in ["employee_id", "employee_id_2"] {
            s.execute("er.attribute.add", &serde_json::json!({ "entity": account, "name": name, "type": "UUID" })).unwrap();
        }
        let d = id(s.execute("diagram.add", &serde_json::json!({ "kind": "er", "element": schema, "name": "Shop" })).unwrap().value);
        s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": account, "x": 0, "y": 0, "width": 240, "height": 72 })).unwrap();
        h.run_steps(2);
        host.written.lock().unwrap().pop().unwrap().1
    };
    let opened = crate::Opened { model: Some(std::sync::Arc::new(rows)), ..commerce() };
    let (h, host) = saving_harness(Some(opened), None);
    let session = h.state().session.as_ref().expect("the model read opens");
    let st = session.doc();
    let node = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::NODE)).unwrap();
    assert_eq!(archforge_diagram::grid::bounds_of(st, node).height, crate::editing::content_height(2), "the box shows both columns");
    assert_eq!(session.undo_label(), None, "growing it is no step of the user's");
    assert!(!session.is_dirty(), "nor a change to save");
    assert!(host.written.lock().unwrap().is_empty());
}

#[test]
fn a_project_that_never_held_a_model_opens_with_a_fresh_one() {
    // Its tables hold no row at all: made before projects were saved.
    let empty = crate::Opened { model: Some(std::sync::Arc::new(archforge_mof::rows::Rows::default())), ..commerce() };
    let (mut h, host) = saving_harness(Some(empty), None);
    assert!(h.state().session_error.is_none(), "{:?}", h.state().session_error);
    assert!(h.state().session.is_some());
    // The fresh model is written like a new project's, and takes a schema.
    assert_eq!(host.written.lock().unwrap().len(), 1);
    add_schema(&mut h, "shop");
    assert_eq!(host.written.lock().unwrap().len(), 2);
}

#[test]
fn a_failed_opening_says_why_on_home_and_a_cancelled_picker_opens_nothing() {
    let (mut h, host) = saving_harness(None, None);
    let at = rect_of(&h, "home.start.open-project").center();
    click(&mut h, at);
    assert!(host.opens.lock().unwrap().is_empty(), "no folder chosen, nothing asked");
    let (mut h, host) = saving_harness(None, Some("/tmp/Broken.archforge"));
    click(&mut h, at);
    let (_, tx) = host.opens.lock().unwrap().pop().unwrap();
    tx.send(Err(crate::CreateError::Failed { detail: "the project's model is damaged".into() })).unwrap();
    h.run_steps(4);
    assert!(h.state().project.is_none());
    assert_eq!(h.state().open_error.as_deref(), Some("the project's model is damaged"));
    // An opening whose worker ended without answering says so.
    click(&mut h, at);
    host.opens.lock().unwrap().clear();
    h.run_steps(4);
    assert_eq!(h.state().open_error.as_deref(), Some("The opening stopped without an answer"));
}

#[test]
fn opening_from_the_workspace_says_it_waits_and_why_it_failed_and_keeps_the_project() {
    let (mut h, host) = saving_harness(Some(commerce()), Some("/tmp/Broken.archforge"));
    h.state_mut().open_from_picker();
    h.run_steps(2);
    assert!(h.state().is_opening());
    let (_, tx) = host.opens.lock().unwrap().pop().expect("the host was asked to open");
    tx.send(Err(crate::CreateError::Failed { detail: "the open project's last changes could not be written".into() })).unwrap();
    h.run_steps(4);
    let shell = h.state();
    assert!(!shell.is_opening());
    assert_eq!(shell.title(), "Commerce platform — Archforge", "the open project stays");
    assert!(shell.session.is_some());
    assert_eq!(shell.open_error.as_deref(), Some("the open project's last changes could not be written"));
    // The next change to the open project clears it.
    add_schema(&mut h, "shop");
    assert!(h.state().open_error.is_none());
}

#[test]
fn projects_open_in_tabs_that_keep_their_models_and_home_keeps_them_open() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    let library = crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() };
    h.state_mut().open_project(library.clone());
    h.run_steps(2);
    assert_eq!(h.state().tabs.len(), 2);
    assert_eq!(h.state().active, Some(1));
    assert_eq!(h.state().title(), "Library — Archforge");
    add_schema(&mut h, "books");
    // The first tab shows its own model again, and its rows name it.
    let at = rect_of(&h, "workspace.header.project.0").center();
    click(&mut h, egui::pos2(at.x - 20.0, at.y));
    h.run_steps(2);
    assert_eq!(h.state().active, Some(0));
    assert_eq!(schema_names(&h), ["shop"]);
    add_schema(&mut h, "crm");
    let last = host.written.lock().unwrap().last().unwrap().0.clone();
    assert_eq!(last, std::path::PathBuf::from("/tmp/Commerce platform.archforge"));
    // Home keeps both open; a tab on Home's bar shows its project again.
    let at = rect_of(&h, "workspace.header.home").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(h.state().project.is_none() && h.state().tabs.len() == 2);
    assert!(ids(&h).iter().any(|i| i == "home.start.open-project"));
    let at = rect_of(&h, "workspace.header.project.1").center();
    click(&mut h, egui::pos2(at.x - 20.0, at.y));
    h.run_steps(2);
    assert_eq!(h.state().active, Some(1));
    assert_eq!(schema_names(&h), ["books"]);
    // Opening a project already open shows its tab.
    h.state_mut().open_project(commerce());
    assert_eq!(h.state().active, Some(0));
    assert_eq!(h.state().tabs.len(), 2);
    // The close button asks the host to close its project; the tab stays until it has, and stays
    // when it could not, saying why.
    let at = rect_of(&h, "workspace.header.project.0.close").center();
    click(&mut h, at);
    h.run_steps(2);
    // It holds unsaved changes, so it asks: Save, its label white on the accent (AR-60), and once
    // saved it closes.
    let save = rect_of(&h, "close-ask.save");
    assert_eq!(label_colours(&h, "Save", save), [crate::ux::palette(false).on_accent]);
    let at = save.center();
    click(&mut h, at);
    h.run_steps(2);
    host.saves.lock().unwrap().pop().expect("the save asked for").send(Ok(true)).unwrap();
    h.run_steps(2);
    assert_eq!(host.closed.lock().unwrap().as_slice(), [std::path::PathBuf::from("/tmp/Commerce platform.archforge")]);
    assert!(h.state().tabs.len() == 2 && h.state().tabs[0].closing());
    // While it closes it steps aside: the tab beside it shows, and it cannot be shown.
    assert_eq!(h.state().active, Some(1));
    h.state_mut().activate(0);
    assert_eq!(h.state().active, Some(1), "a closing tab is not shown");
    host.closings.lock().unwrap().pop().unwrap().send(Err("the disk is full".into())).unwrap();
    h.run_steps(2);
    // Refused, it comes back to the front, saying why.
    assert!(h.state().tabs.len() == 2 && !h.state().tabs[0].closing());
    assert_eq!(h.state().active, Some(0));
    assert_eq!(h.state().close_error(), Some("the disk is full"));
    let mut kept = schema_names(&h);
    kept.sort();
    assert_eq!(kept, ["crm", "shop"], "the project stays shown with its model");
    // Its next change clears the word; once closed, the tab beside it shows, and Home after the last.
    add_schema(&mut h, "billing");
    assert_eq!(h.state().close_error(), None);
    // Unsaved again: Don't save closes it without writing, put back to its last save.
    let at = rect_of(&h, "workspace.header.project.0.close").center();
    click(&mut h, at);
    h.run_steps(2);
    let at = rect_of(&h, "close-ask.discard").center();
    click(&mut h, at);
    assert_eq!(host.discarded.lock().unwrap().as_slice(), [std::path::PathBuf::from("/tmp/Commerce platform.archforge")]);
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().tabs.len(), 1);
    assert_eq!(h.state().title(), "• Library — Archforge");
    assert_eq!(schema_names(&h), ["books"]);
    let at = rect_of(&h, "workspace.header.project.0.close").center();
    click(&mut h, at);
    h.run_steps(2);
    let at = rect_of(&h, "close-ask.discard").center();
    click(&mut h, at);
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(2);
    assert!(h.state().tabs.is_empty() && h.state().project.is_none());
    assert!(ids(&h).iter().any(|i| i == "home.start.open-project"), "Home after the last tab");
}

#[test]
fn eight_tabs_stay_left_of_help_at_every_width_down_to_the_smallest_window() {
    for w in [640.0, 900.0, 1600.0] {
        crate::i18n::set_language("en");
        let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() });
        for i in 0..crate::MAX_TABS {
            shell.open_project(crate::Opened {
                name: format!("A project with a rather long name {i}"),
                path: format!("/tmp/P{i}.archforge").into(),
                ..Default::default()
            });
        }
        let mut h = Harness::builder().with_size(egui::vec2(w, 800.0)).build_ui_state(
            |ui, shell: &mut Shell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            shell,
        );
        h.run_steps(3);
        let help = rect_of(&h, "workspace.header.help");
        for i in 0..crate::MAX_TABS {
            let tab = rect_of(&h, &format!("workspace.header.project.{i}"));
            assert!(tab.right() <= help.left() && tab.left() >= 0.0, "{w}: tab {i} at {tab:?}, help at {help:?}");
        }
        let shown = rect_of(&h, &format!("workspace.header.project.{}.close", crate::MAX_TABS - 1));
        assert!(shown.right() <= help.left(), "{w}: the shown tab's close button at {shown:?}");
    }
}

#[test]
fn the_recent_list_keeps_each_project_once_newest_first_and_drops_what_it_cannot_take() {
    let mut state = UiState::default();
    for i in 0..20u64 {
        state.remember(&format!("P{i}"), &std::env::temp_dir().join(format!("P{i}.archforge")), i);
    }
    assert_eq!(state.recent.len(), crate::state::MAX_RECENT);
    assert_eq!(state.recent[0].name, "P19");
    state.remember("P15", &std::env::temp_dir().join("P15.archforge"), 100);
    assert_eq!((state.recent[0].name.as_str(), state.recent[0].opened), ("P15", 100));
    assert_eq!(state.recent.iter().filter(|r| r.name == "P15").count(), 1, "once");
    // A preferences file may hold anything: what the window does not take is dropped.
    let at = |file: &str| std::env::temp_dir().join(file);
    let entry = |name: &str, path: std::path::PathBuf| crate::state::RecentProject { name: name.into(), path, opened: 1 };
    let long = "x".repeat(230);
    state.recent = vec![
        entry("Good", at("Good.archforge")),
        entry(&long, at("Long.archforge")),
        entry("", at("Nameless.archforge")),
        entry("Relative", "relative/path.archforge".into()),
        entry("Two\nlines", at("Lines.archforge")),
        entry("Tab\there", at("Tab.archforge")),
        entry(&"x".repeat(500), at("TooLong.archforge")),
        entry("Again", at("Good.archforge")),
    ];
    state.sanitize();
    assert_eq!(state.recent.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["Good", long.as_str()], "a long valid name stays");
    // One entry of the wrong shape is left out, not the whole state.
    let read: UiState =
        serde_json::from_str(r#"{"theme":"dark","recent":[{"name":"Kept","path":"/k.archforge","opened":5},{"name":null,"opened":-1},7]}"#).unwrap();
    assert_eq!(read.theme, ThemeKind::Dark);
    assert_eq!(read.recent.iter().filter(|r| r.name == "Kept").count(), 1);
    assert_eq!(crate::home::ago(0, 30), "Just now");
    assert_eq!(crate::home::ago(0, 120), "2 minutes ago");
    assert_eq!(crate::home::ago(0, 3_600), "1 hour ago");
    assert_eq!(crate::home::ago(0, 90_000), "Yesterday");
    assert_eq!(crate::home::ago(0, 3 * 86_400), "3 days ago");
}

#[test]
fn a_recent_project_opens_from_its_row_or_shows_its_tab() {
    let (mut h, host) = saving_harness(None, None);
    h.state_mut().state.remember("Shop", std::path::Path::new("/tmp/Shop.archforge"), 1);
    h.run_steps(2);
    let at = rect_of(&h, "home.recent.0").center();
    click(&mut h, at);
    h.run_steps(2);
    let (path, tx) = host.opens.lock().unwrap().pop().expect("the row asks the host to open its project");
    assert_eq!(path, std::path::PathBuf::from("/tmp/Shop.archforge"));
    tx.send(Ok(crate::Opened { name: "Shop".into(), path, ..Default::default() })).unwrap();
    h.run_steps(4);
    assert_eq!(h.state().title(), "Shop — Archforge");
    // On Home again, the row of the open project shows its tab without asking the host.
    let at = rect_of(&h, "workspace.header.home").center();
    click(&mut h, at);
    h.run_steps(2);
    let at = rect_of(&h, "home.recent.0").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(host.opens.lock().unwrap().is_empty());
    assert_eq!(h.state().active, Some(0));
}

#[test]
fn home_sorts_the_recent_projects_by_name_and_shows_them_as_a_gallery() {
    let (mut h, host) = saving_harness(None, None);
    for (name, opened) in [("beta", 3), ("Alpha", 1), ("Gamma", 2)] {
        h.state_mut().state.remember(name, &std::path::PathBuf::from(format!("/tmp/{name}.archforge")), opened);
    }
    h.run_steps(2);
    let first = |h: &mut Harness<'static, Shell>| {
        let at = rect_of(h, "home.recent.0").center();
        click(h, at);
        h.run_steps(2);
        let (path, _tx) = host.opens.lock().unwrap().pop().expect("the first entry asks the host to open its project");
        path
    };
    assert_eq!(first(&mut h), std::path::PathBuf::from("/tmp/beta.archforge"), "the last opened first");
    // Sorted by name, whatever the case.
    let at = rect_of(&h, "home.recent.sort").center();
    click(&mut h, at);
    h.run_steps(2);
    let at = rect_of(&h, "home.recent.sort.1").center();
    click(&mut h, at);
    h.run_steps(2);
    assert_eq!(h.state().state.recent_sort, crate::state::RecentSort::Name);
    assert_eq!(first(&mut h), std::path::PathBuf::from("/tmp/Alpha.archforge"));
    // The gallery: one card a project, which opens it too; the choice is kept with the state.
    let at = rect_of(&h, "home.recent.grid").center();
    click(&mut h, at);
    h.run_steps(2);
    assert_eq!(h.state().state.recent_view, crate::state::RecentView::Gallery);
    let cards = h.state().widgets().iter().filter(|w| w.id.starts_with("home.recent.") && w.kind == "card").count();
    assert_eq!(cards, 3);
    assert_eq!(first(&mut h), std::path::PathBuf::from("/tmp/Alpha.archforge"));
    let saved = serde_json::to_string(&h.state().state).unwrap();
    let read: crate::state::UiState = serde_json::from_str(&saved).unwrap();
    assert_eq!((read.recent_view, read.recent_sort), (crate::state::RecentView::Gallery, crate::state::RecentSort::Name));
}

#[test]
fn a_pinned_folder_is_pinned_from_the_picker_opens_the_project_picker_in_it_and_unpins_from_its_menu() {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    let started_in: std::sync::Arc<std::sync::Mutex<Vec<Option<std::path::PathBuf>>>> = std::sync::Arc::default();
    let seen = started_in.clone();
    let services = Services {
        projects: Some(host.clone()),
        ids: Some(test_ids()),
        pick_project: Some(std::sync::Arc::new(move |start: Option<&std::path::Path>| {
            seen.lock().unwrap().push(start.map(std::path::Path::to_path_buf));
            None
        })),
        pick_folder: Some(std::sync::Arc::new(|| Some(std::path::PathBuf::from("/Users/robin/Work/Models")))),
        ..Services::default()
    };
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        Shell::new(services),
    );
    h.run_steps(2);
    let at = rect_of(&h, "home.pin-folder").center();
    click(&mut h, at);
    h.run_steps(2);
    assert_eq!(h.state().state.pinned, [std::path::PathBuf::from("/Users/robin/Work/Models")]);
    // Pinning it again adds nothing.
    let at = rect_of(&h, "home.pin-folder").center();
    click(&mut h, at);
    h.run_steps(2);
    assert_eq!(h.state().state.pinned.len(), 1);
    // A click on the tile starts the project picker in the folder.
    let at = rect_of(&h, "home.pinned.0").center();
    click(&mut h, at);
    h.run_steps(2);
    assert_eq!(started_in.lock().unwrap().pop(), Some(Some(std::path::PathBuf::from("/Users/robin/Work/Models"))));
    // Its menu unpins it, and the tile goes.
    let at = rect_of(&h, "home.pinned.0.more").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(started_in.lock().unwrap().is_empty(), "the menu button opens no picker");
    let at = rect_of(&h, "home.pinned.0.menu.1").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(h.state().state.pinned.is_empty());
    assert!(!ids(&h).iter().any(|i| i == "home.pinned.0"));
    // A preferences file's pins are checked like its recent projects.
    let mut state: crate::state::UiState = serde_json::from_str(r#"{"pinned":["/a/Models","relative/x","/a/Models",7,"/"]}"#).unwrap();
    state.sanitize();
    assert_eq!(state.pinned, [std::path::PathBuf::from("/a/Models")]);
}

/// A folder whose path is not text is never pinned or listed, so the preferences file can always
/// be written.
#[cfg(unix)]
#[test]
fn a_folder_whose_path_is_not_text_is_neither_pinned_nor_listed_and_the_preferences_still_write() {
    use std::os::unix::ffi::OsStrExt as _;
    let odd = std::path::Path::new(std::ffi::OsStr::from_bytes(b"/tmp/odd\xff"));
    let mut state = crate::state::UiState::default();
    state.pin(odd);
    state.remember("Odd", &odd.join("Odd.archforge"), 1);
    assert!(state.pinned.is_empty() && state.recent.is_empty());
    state.pinned.push(odd.to_path_buf());
    state.sanitize();
    assert!(state.pinned.is_empty(), "a preferences file cannot hold one either");
    assert!(serde_json::to_vec(&state).is_ok());
}

#[test]
fn homes_search_finds_projects_and_actions_and_runs_them_from_the_keyboard() {
    let (mut h, host) = saving_harness(None, None);
    for (name, opened) in [("Shop", 2), ("Library", 1)] {
        h.state_mut().state.remember(name, &std::path::PathBuf::from(format!("/tmp/{name}.archforge")), opened);
    }
    h.run_steps(2);
    let results = |h: &Harness<'static, Shell>| ids(h).iter().filter(|i| i.starts_with("home.header.search.result.")).count();
    assert_eq!(results(&h), 0, "closed until asked");
    // A click opens it: the recent projects and every action, no query needed.
    let at = rect_of(&h, "home.header.search").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(h.state().home_search.open);
    assert!(results(&h) >= 2 + 4, "{:?}", ids(&h));
    // Typing narrows the list; Enter runs the first match.
    h.event(egui::Event::Text("shop".into()));
    h.run_steps(2);
    assert_eq!(results(&h), 1, "only Shop: {:?}", ids(&h));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run_steps(2);
    let (path, _tx) = host.opens.lock().unwrap().pop().expect("Enter opened the project");
    assert_eq!(path, std::path::PathBuf::from("/tmp/Shop.archforge"));
    assert!(!h.state().home_search.open, "running a result closes the search");
    // The shortcut opens it again; an action runs from the keyboard.
    key(&mut h, egui::Key::K, egui::Modifiers::COMMAND);
    h.run_steps(2);
    assert!(h.state().home_search.open);
    h.event(egui::Event::Text("dark".into()));
    h.run_steps(2);
    assert_eq!(results(&h), 1);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run_steps(2);
    assert!(h.state().state.theme.is_dark());
    // The arrows move the highlight round the list; Esc closes and forgets the query.
    key(&mut h, egui::Key::K, egui::Modifiers::COMMAND);
    h.run_steps(2);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    h.run_steps(1);
    assert_eq!(h.state().home_search.highlight, 2);
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    h.run_steps(1);
    assert_eq!(h.state().home_search.highlight, 1);
    h.event(egui::Event::Text("zzz".into()));
    h.run_steps(2);
    assert_eq!(results(&h), 0, "no matches");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    h.run_steps(2);
    assert!(!h.state().home_search.open && h.state().home_search.query.is_empty());
}

#[test]
fn homes_search_finds_the_projects_in_the_pinned_folders_one_search_at_a_time_and_opens_them() {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    type Pending = (std::sync::mpsc::Sender<Vec<crate::FoundProject>>, Box<dyn Fn() + Send>);
    let asked: std::sync::Arc<std::sync::Mutex<Vec<Vec<std::path::PathBuf>>>> = std::sync::Arc::default();
    let pending: std::sync::Arc<std::sync::Mutex<Vec<Pending>>> = std::sync::Arc::default();
    let (seen, waiting) = (asked.clone(), pending.clone());
    let services = Services {
        projects: Some(host.clone()),
        ids: Some(test_ids()),
        pick_project: Some(std::sync::Arc::new(|_| None)),
        find_projects: Some(std::sync::Arc::new(move |folders: Vec<std::path::PathBuf>, wake: Box<dyn Fn() + Send>| {
            seen.lock().unwrap().push(folders);
            let (tx, rx) = std::sync::mpsc::channel();
            waiting.lock().unwrap().push((tx, wake));
            rx
        })),
        ..Services::default()
    };
    let mut shell = Shell::new(services);
    shell.state.pin(std::path::Path::new("/Users/robin/Work"));
    shell.state.remember("Shop", std::path::Path::new("/tmp/Shop.archforge"), 1);
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run_steps(2);
    // However often the search opens, the folders are searched once while an answer is due.
    for _ in 0..3 {
        key(&mut h, egui::Key::K, egui::Modifiers::COMMAND);
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
    key(&mut h, egui::Key::K, egui::Modifiers::COMMAND);
    h.run_steps(2);
    assert_eq!(asked.lock().unwrap().as_slice(), [vec![std::path::PathBuf::from("/Users/robin/Work")]], "one search of the pinned folders");
    // The answer comes, and its wake is called: the projects found join the list.
    let (tx, wake) = pending.lock().unwrap().pop().unwrap();
    tx.send(vec![
        crate::FoundProject { path: "/Users/robin/Work/Client/Billing.archforge".into(), folder: 0, within: vec!["Client".into()] },
        crate::FoundProject { path: "/tmp/Shop.archforge".into(), folder: 0, within: vec![] },
    ])
    .unwrap();
    wake();
    h.run_steps(2);
    let results = |h: &Harness<'static, Shell>| ids(h).iter().filter(|i| i.starts_with("home.header.search.result.")).count();
    h.event(egui::Event::Text("billing".into()));
    h.run_steps(2);
    assert_eq!(results(&h), 1, "{:?}", ids(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // A project that is a recent one too is listed once; one is found by where it lives as well.
    key(&mut h, egui::Key::K, egui::Modifiers::COMMAND);
    h.run_steps(2);
    h.event(egui::Event::Text("shop".into()));
    h.run_steps(2);
    assert_eq!(results(&h), 1, "Shop once: {:?}", ids(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    key(&mut h, egui::Key::K, egui::Modifiers::COMMAND);
    h.run_steps(2);
    h.event(egui::Event::Text("client bill".into()));
    h.run_steps(2);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run_steps(2);
    let (path, _tx) = host.opens.lock().unwrap().pop().expect("Enter opened the project found");
    assert_eq!(path, std::path::PathBuf::from("/Users/robin/Work/Client/Billing.archforge"));
    assert_eq!(asked.lock().unwrap().len(), 2, "each opening after an answer searches again");
}

#[test]
fn a_connection_dropped_on_empty_canvas_makes_a_new_table_joined_to_it_in_one_step() {
    let (mut h, _customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let tables = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::ENTITY)).count()
    };
    let before = (tables(&h), relationships(&h).len(), node_ids(&h).len());
    let at = rect_of(&h, &customer_node).center();
    click(&mut h, at);
    // From the connect handle to a spot well below the tables, where nothing is drawn.
    let from = rect_of(&h, "workspace.canvas.adorn.connect").center();
    let empty = rect_of(&h, &customer_node).center() + egui::vec2(0.0, 320.0);
    drag(&mut h, from, empty);
    h.run_steps(2);
    assert_eq!(
        (tables(&h), relationships(&h).len(), node_ids(&h).len()),
        (before.0 + 1, before.1 + 1, before.2 + 1),
        "{:?}",
        h.state().canvas.notice
    );
    let rename = h.state().canvas.rename.clone().expect("the new table's name is open for typing");
    assert_eq!(rename.text, "Entity");
    let st = h.state().session.as_ref().unwrap().doc();
    let rel = relationships(&h)[0];
    let to = st.get(rel, archforge_er::relationship::TO).unwrap().as_ref().unwrap();
    assert_eq!(st.uuid(to).unwrap(), rename.entity, "the line joins the new table");
    assert!(!crate::connect::has_foreign_key(st, rel), "the new table's relationship starts with its foreign key off");
    // One undo takes the table, its node and its line away together.
    let undo = h.state().session.as_ref().unwrap().undo_label();
    assert_eq!(undo, Some("Add table"));
    h.state_mut().canvas.rename = None;
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run_steps(2);
    assert_eq!((tables(&h), relationships(&h).len(), node_ids(&h).len()), before);
}

#[test]
fn a_line_to_a_table_without_columns_starts_with_its_foreign_key_off() {
    let (mut h, customer, _order) = two_tables_harness();
    // A third table with no columns at all, beside the other two.
    let empty = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let d = st.uuid(s.view().open_diagram.unwrap()).unwrap().to_string();
        let e =
            s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Note" })).unwrap().value["id"].as_str().unwrap().to_string();
        s.execute("diagram.place", &serde_json::json!({ "diagram": d, "element": e, "x": 100, "y": 420, "width": 240, "height": 72 })).unwrap();
        e
    };
    h.run_steps(2);
    let node_of = |h: &Harness<'static, Shell>, element: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let e = st.by_uuid(archforge_mof::Uuid::parse(element).unwrap()).unwrap();
        let n = st
            .alive()
            .find(|n| {
                st.class_of(*n).ok() == Some(archforge_diagram::NODE) && st.get(*n, archforge_diagram::node::ELEMENT).unwrap().as_ref() == Some(e)
            })
            .unwrap();
        format!("workspace.canvas.node.{}", st.uuid(n).unwrap())
    };
    let (from, to) = (node_of(&h, &customer), node_of(&h, &empty));
    let columns_of = |h: &Harness<'static, Shell>, element: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let e = st.by_uuid(archforge_mof::Uuid::parse(element).unwrap()).unwrap();
        st.list(e, archforge_er::entity::ATTRIBUTE).unwrap().len()
    };
    let before = (columns_of(&h, &customer), columns_of(&h, &empty));
    assert_eq!(before.1, 0);
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (a, b) = (rect_of(&h, &from).center(), rect_of(&h, &to).center());
    drag(&mut h, a, b);
    let rels = relationships(&h);
    assert_eq!(rels.len(), 1, "{:?}", h.state().canvas.notice);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!crate::connect::has_foreign_key(st, rels[0]), "Customer has its key, but Note has no columns: the key starts off");
    assert_eq!((columns_of(&h, &customer), columns_of(&h, &empty)), before, "and no column is made on either table");
}

#[test]
fn the_app_icon_is_read_and_averaged_down_with_clean_clear_edges() {
    let src = [255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 0, 0, 255, 0, 0];
    assert_eq!(crate::ux::scale_rgba(&src, 2, 1), [128, 0, 128, 128], "colour and coverage kept, a clear pixel's colour left out");
    let mark = crate::ux::app_icon_rgba(56).expect("the picture built in reads");
    assert_eq!(mark.len(), 56 * 56 * 4);
    assert!(mark[(28 * 56 + 5) * 4 + 3] > 200, "the tile fills the mark's square");
    assert!(crate::ux::app_icon_rgba(4096).is_none(), "never scaled up");
}

#[test]
fn at_most_eight_projects_open_at_once() {
    let (mut h, host) = saving_harness(None, Some("/tmp/Ninth.archforge"));
    for i in 0..crate::MAX_TABS {
        h.state_mut().open_project(crate::Opened { name: format!("P{i}"), path: format!("/tmp/P{i}.archforge").into(), ..Default::default() });
    }
    h.run_steps(2);
    assert_eq!(h.state().tabs.len(), crate::MAX_TABS);
    assert!(!h.state().can_open_project(), "a ninth waits for a tab to close");
    h.state_mut().close_tab(3);
    assert!(!h.state().can_open_project(), "not before the project has closed");
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(2);
    assert!(h.state().can_open_project());
}

#[test]
fn a_save_of_the_project_open_before_never_marks_the_next_one_saved() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    h.state_mut().save();
    let late = host.saves.lock().unwrap().pop().unwrap();
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    h.run_steps(2);
    assert!(!h.state().is_saving());
    add_schema(&mut h, "books");
    let _ = late.send(Ok(true));
    h.run_steps(2);
    assert!(dirty(&h), "the old project's save answers for it alone");
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
            "broken" => (*defs.font_data["Hack"].font).as_ref()[..40].to_vec(),
            key => (*defs.font_data[key].font).as_ref().to_vec(),
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
                let covered = f.characters(&egui::FontFamily::Proportional).clone();
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
            let covered = f.characters(&egui::FontFamily::Proportional).clone();
            wanted.iter().copied().filter(|c| !matches!(c, ' ' | '\u{202F}') && !covered.contains_key(c)).collect()
        });
        assert!(missing.is_empty(), "{}: {missing:?}", lang.code);
    }
}

/// The shell with a project open, in the harness at the design's frame size or another.
fn workspace_harness(theme: ThemeKind, size: egui::Vec2) -> Harness<'static, Shell> {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() });
    shell.state.theme = theme;
    shell.open_project(crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into(), ..Default::default() });
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
            "workspace.header.project.0",
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
fn the_open_rail_icon_and_the_collapse_button_fold_a_column_and_any_icon_opens_it_again() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    let rail = rect_of(&h, "workspace.panels.properties").left();
    // The open panel's own icon folds it; the centre takes its width and the rail closes the edge.
    let at = rect_of(&h, "workspace.panels.properties").center();
    click(&mut h, at);
    assert!(h.state().state.panel_collapsed);
    assert!(!ids(&h).iter().any(|i| i == "workspace.panel.collapse"), "the panel is gone");
    assert!(rect_of(&h, "workspace.panels.properties").left() > rail + 200.0);
    // Another icon opens it on its own content.
    let at = rect_of(&h, "workspace.panels.skills").center();
    click(&mut h, at);
    assert!(!h.state().state.panel_collapsed);
    assert_eq!(h.state().state.panel, crate::state::Panel::Skills);
    // The collapse button folds it too, and its own icon opens it again.
    let at = rect_of(&h, "workspace.panel.collapse").center();
    click(&mut h, at);
    assert!(h.state().state.panel_collapsed);
    let at = rect_of(&h, "workspace.panels.skills").center();
    click(&mut h, at);
    assert!(!h.state().state.panel_collapsed && h.state().state.panel == crate::state::Panel::Skills);
    // The activity rail folds and opens the explorer the same way.
    let at = rect_of(&h, "workspace.explorers.models").center();
    click(&mut h, at);
    assert!(h.state().state.explorer_collapsed);
    assert!(!ids(&h).iter().any(|i| i == "workspace.explorer.add"), "the explorer is gone");
    let at = rect_of(&h, "workspace.explorers.diagrams").center();
    click(&mut h, at);
    assert!(!h.state().state.explorer_collapsed && h.state().state.explorer == crate::state::Explorer::Diagrams);
    // Opening a column's editor from the canvas opens a folded panel on Properties.
    h.state_mut().state.panel_collapsed = true;
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    assert!(!h.state().state.panel_collapsed);
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
    assert!(h.state().project.is_none() && h.state().session.is_none(), "Home shows instead of the project");
    assert!(ids(&h).iter().any(|i| i == "home.start.open-project"), "Home is back");
    assert_eq!(h.state().tabs.len(), 1, "the project stays open in its tab");
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
    shell.open_project(crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into(), ..Default::default() });
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
            let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() });
            shell.open_project(crate::Opened {
                name: "A project with a very long name that goes on and on".into(),
                path: "/tmp/long.archforge".into(),
                ..Default::default()
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
                let chip = rect_of(&h, "workspace.header.project.0");
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
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).map(|o| archforge_core::name_of(st, o)).collect()
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
    crate::explorer::add(h.state_mut(), "schema");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.is_none());
    // The new schema is selected: + offers a table first.
    crate::explorer::add(h.state_mut(), "table");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Schema 1", "Table 1"]);
    assert_eq!(rows[1].depth, 1);
    // A column under the table.
    crate::explorer::add(h.state_mut(), "column");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
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
        crate::explorer::add(h.state_mut(), "schema");
        h.run();
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
    }
    crate::explorer::add(h.state_mut(), "schema");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(schema_names(&h).len(), 3);
    h.state_mut().explorer.filter = "schema 2".into();
    h.run();
    let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Schema 2"]);
    h.state_mut().explorer.filter.clear();
    h.run();
    // Down from the selected third schema stays on it; up goes to the second.
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    let selected = h.state().session.as_ref().unwrap().view().selection[0];
    assert_eq!(archforge_core::name_of(h.state().session.as_ref().unwrap().doc(), selected), "Schema 2");
    key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.as_ref().is_some_and(|r| r.text == "Schema 2"));
}

/// A harness with a schema, a table of two columns, the table selected.
#[test]
fn the_models_tree_scrolls_and_follows_the_selection() {
    let (mut h, _) = table_harness();
    let (first, last) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let mut ids = Vec::new();
        for n in 0..60 {
            let out = s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": format!("table_{n:02}") })).unwrap();
            ids.push(out.value["id"].as_str().unwrap().to_string());
        }
        s.execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        (ids[0].clone(), ids[59].clone())
    };
    h.run();
    let row = |id: &str| format!("workspace.explorer.row.{id}");
    assert!(!ids(&h).contains(&row(&last)), "the last table is below the tree's foot");
    // The wheel over the tree brings the rows below into view.
    let tree = rect_of(&h, "workspace.explorer.filter").center() + egui::vec2(0.0, 300.0);
    h.event(egui::Event::PointerMoved(tree));
    for _ in 0..8 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -1200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(30);
    }
    assert!(ids(&h).contains(&row(&last)), "scrolled to the end");
    assert!(h.state().explorer.scroll.offset > 0.0);
    for _ in 0..8 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 1200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(30);
    }
    assert_eq!(h.state().explorer.scroll.offset, 0.0, "and back to the top, never past it");
    // A table selected elsewhere (the canvas, Properties) is brought into view.
    h.event(egui::Event::PointerMoved(egui::pos2(900.0, 500.0)));
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [last] })).unwrap();
    h.run_steps(5);
    assert!(ids(&h).contains(&row(&last)), "the selected table's row is shown");
    let _ = first;
}

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
    let rows: Vec<String> = found.iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).cloned().collect();
    assert_eq!(rows.len(), 2, "two column rows: {found:?}");
    assert!(found.iter().any(|i| i == "workspace.properties.kind"));
    // Open the second column's editor and make it unique and not null.
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
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
    let email_obj = st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap();
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
fn apply_closes_a_new_columns_editor_after_a_type_is_picked_from_the_list() {
    let (mut h, customer) = table_harness();
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.name").center();
    click(&mut h, at);
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.event(egui::Event::Text("id".into()));
    h.run();
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.type.menu.2").center();
    click(&mut h, at);
    assert!(h.state().panels.menu.is_none(), "a pick closes the list");
    // The list lies over the sections below the card; its last entry takes the click, not the
    // section row under it.
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let n = crate::panels::BASE_TYPES.len() - 1;
    let last = rect_of(&h, &format!("workspace.properties.column.type.menu.{n}"));
    let keys_add = rect_of(&h, "workspace.properties.keys.add");
    assert!(last.bottom() > keys_add.top(), "the list reaches over the keys: {last:?} {keys_add:?}");
    click(&mut h, egui::pos2(keys_add.center().x.min(last.right() - 4.0), last.center().y));
    assert_eq!(h.state().panels.column.as_ref().map(|c| c.ty.clone()), Some(crate::panels::BASE_TYPES[n].to_string()));
    assert!(h.state().panels.key.is_none(), "the key editor under the list stayed closed");
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.type.menu.2").center();
    click(&mut h, at);
    // Primary key brings Not null with it.
    let at = rect_of(&h, "workspace.properties.column.primary-key").center();
    click(&mut h, at);
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.primary_key && c.not_null));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    h.run();
    assert!(h.state().panels.column.is_none(), "one Apply closes the editor: {:?}", h.state().panels.column);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 3, "one column added");
    // Not null cannot be unticked under a key: it takes the key with it.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[0])).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.not-null").center();
    click(&mut h, at);
    assert!(h.state().panels.column.as_ref().is_some_and(|c| !c.primary_key && !c.not_null));
}

#[test]
fn a_column_row_selects_its_column_only_its_edit_button_opens_the_editor_and_selecting_elsewhere_closes_it() {
    let (mut h, customer) = table_harness();
    let select = |h: &mut Harness<'static, Shell>, id: &str| {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
    };
    let email = {
        let st = h.state().session.as_ref().unwrap().doc();
        let email = st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap();
        st.uuid(email).unwrap().to_string()
    };
    select(&mut h, &email);
    h.run();
    assert!(h.state().panels.column.is_none(), "selecting a column only marks its row: {:?}", h.state().panels.column);
    // A click on a row selects its column, as the tree and the canvas do, and opens its table in
    // the tree so the row there shows too; it does not open the editor.
    let customer_id = archforge_mof::Uuid::parse(&customer).unwrap();
    select(&mut h, &customer);
    h.state_mut().explorer.expanded.remove(&customer_id);
    h.run();
    let at = rect_of(&h, &format!("workspace.properties.column.{email}")).center();
    click(&mut h, at);
    h.run();
    assert_eq!(selected_name(&h), "email", "the row's click selects its column");
    assert!(h.state().explorer.expanded.contains(&customer_id), "its table opens in the tree");
    assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.row.{email}")), "{:?}", ids(&h));
    assert!(h.state().panels.column.is_none(), "a click on the row does not open it");
    let at = rect_of(&h, &format!("workspace.properties.column.{email}.edit")).center();
    click(&mut h, at);
    h.run();
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.map(|t| t.to_string()) == Some(email.clone())), "the edit button opens it");
    // The edit button of a row while the table is selected selects the column too.
    let at = rect_of(&h, "workspace.properties.column.cancel").center();
    click(&mut h, at);
    select(&mut h, &customer);
    h.run();
    let at = rect_of(&h, &format!("workspace.properties.column.{email}.edit")).center();
    click(&mut h, at);
    h.run();
    assert_eq!(selected_name(&h), "email");
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.map(|t| t.to_string()) == Some(email.clone())), "and opens it");
    // Selecting the table, then the column again: the editor closed and stays closed.
    select(&mut h, &customer);
    h.run();
    assert!(h.state().panels.column.is_none(), "selecting elsewhere closes it");
    select(&mut h, &email);
    h.run();
    assert!(h.state().panels.column.is_none(), "coming back does not reopen it");
    // The + opens an editor for a new column; Cancel closes it and it stays closed.
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    h.run();
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.target.is_none()), "the + opens an editor for a new column");
    let at = rect_of(&h, "workspace.properties.column.cancel").center();
    click(&mut h, at);
    h.run();
    h.run();
    assert!(h.state().panels.column.is_none(), "Cancel stays closed: {:?}", h.state().panels.column);
}

/// The text the store has for skill `id`.
fn st_body(h: &Harness<'static, Shell>, id: archforge_mof::Uuid) -> String {
    let st = h.state().session.as_ref().unwrap().doc();
    st.get(st.by_uuid(id).unwrap(), archforge_core::skill::BODY).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

#[test]
fn the_skills_plus_makes_a_skill_and_opens_it_with_the_name_ready() {
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
    // + makes the skill on the table and opens its tab, the name selected for typing.
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.run();
    let skill = h.state().canvas.active.expect("the new skill's tab is active");
    let name_id = crate::widgets::egui_id("workspace.skill.name");
    assert!(h.ctx.memory(|m| m.has_focus(name_id)), "the name has the keyboard");
    let st = h.state().session.as_ref().unwrap().doc();
    let obj = st.by_uuid(skill).unwrap();
    assert_eq!(archforge_core::skills::name_of(st, obj), "Skill 1");
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(archforge_core::skills::subject_of(st, obj), Some(customer_obj));
    // Typing replaces the name; Enter keeps it.
    h.event(egui::Event::Text("Customer naming".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_core::skills::name_of(st, st.by_uuid(skill).unwrap()), "Customer naming");
    // It started from the template, whose title followed the name into the store.
    h.run();
    let body = st_body(&h, skill);
    assert!(body.starts_with("---\nname: customer-naming\n") && body.contains("\n# Customer naming\n") && body.contains("## Rules"), "{body}");
    // The subject's own skill can be removed from its row without opening it.
    let at = rect_of(&h, &format!("workspace.skills.remove.{skill}")).center();
    click(&mut h, at);
    h.run();
    assert!(h.state().session.as_ref().unwrap().doc().by_uuid(skill).is_none_or(|o| !h.state().session.as_ref().unwrap().doc().is_alive(o)));
    assert!(!h.state().canvas.tabs.contains(&skill), "its tab went with it");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    crate::canvas::open(h.state_mut(), skill);
    h.run();
    // The tab holds the name and the text only.
    let found = ids(&h);
    assert!(!found.iter().any(|i| i.starts_with("workspace.skill.when") || i == "workspace.skill.applies"), "{found:?}");
    let found = ids(&h);
    assert_eq!(found.iter().filter(|i| i.starts_with("workspace.skills.row.")).count(), 2, "own and inherited: {found:?}");
    // A refused name stays as typed and says why: a second skill of the same name on the table.
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.run();
    h.event(egui::Event::Text("Customer naming".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    let doc = h.state().canvas.skills.iter().find(|d| Some(d.skill) == h.state().canvas.active).unwrap();
    assert!(doc.settings_refused.is_some(), "{doc:?}");
    // The bin removes the skill, and its tab goes with it.
    let second = h.state().canvas.active.unwrap();
    let at = rect_of(&h, "workspace.skill.remove").center();
    click(&mut h, at);
    h.run();
    assert!(!h.state().canvas.tabs.contains(&second));
}

#[test]
fn a_skill_opens_in_its_own_tab_and_keeps_its_markdown() {
    let (mut h, customer) = table_harness();
    let skill = {
        let s = h.state_mut().session.as_mut().unwrap();
        let out = s.execute("skill.add", &serde_json::json!({ "subject": customer, "name": "Customer naming", "body": "Use names." })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        out.value["id"].as_str().unwrap().to_string()
    };
    let uuid = archforge_mof::Uuid::parse(&skill).unwrap();
    let body = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.get(st.by_uuid(uuid).unwrap(), archforge_core::skill::BODY).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
    };
    let at = rect_of(&h, "workspace.panels.skills").center();
    click(&mut h, at);
    // A click on the row opens the skill in its tab, beside the diagrams.
    let at = rect_of(&h, &format!("workspace.skills.row.{skill}")).left_center() + egui::vec2(40.0, 0.0);
    click(&mut h, at);
    assert_eq!(h.state().canvas.active, Some(uuid));
    assert!(h.state().canvas.tabs.contains(&uuid));
    let found = ids(&h);
    for id in ["workspace.skill.text", "workspace.skill.format.bold", "workspace.skill.preview", &format!("workspace.tabs.{skill}")] {
        assert!(found.iter().any(|f| f == id), "{id} in {found:?}");
    }
    // Bold wraps the selected word.
    h.state_mut().canvas.skills[0].selection = Some((4, 9));
    let at = rect_of(&h, "workspace.skill.format.bold").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.skills[0].text, "Use **names**.");
    // Preview keeps the text at once, as one undo step.
    let at = rect_of(&h, "workspace.skill.preview").center();
    click(&mut h, at);
    assert!(h.state().canvas.skills[0].preview);
    assert_eq!(body(&h), "Use **names**.");
    // A text changed elsewhere, here by undo, shows in the tab.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(h.state().canvas.skills[0].text, "Use names.");
    // A link clicked in Preview waits for the confirmation; Cancel drops it.
    let at = rect_of(&h, "workspace.skill.preview").center();
    click(&mut h, at);
    h.state_mut().canvas.skills[0].link = Some("https://example.com/naming".into());
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.skill.link.open"));
    let at = rect_of(&h, "workspace.skill.link.cancel").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.skills[0].link, None);
    // A text the store refuses keeps its tab open, saying why, instead of being lost.
    h.state_mut().canvas.skills[0].text = "Use\u{200B}names".into();
    let at = rect_of(&h, &format!("workspace.tabs.{skill}.close")).center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.active, Some(uuid), "the refused text stays open");
    assert!(h.state().canvas.skills[0].refused.is_some());
    // So does closing the project: it stays open on the skill and says why.
    let project = h.state().active.unwrap();
    h.state_mut().close_tab(project);
    // Unsaved, so it asks first; Save (with no host to save to) goes on to close.
    assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::Unsaved { .. })));
    h.state_mut().ask = None;
    h.state_mut().save_and_close(project);
    h.run();
    assert_eq!(h.state().active, Some(project));
    assert!(h.state().tabs[project].close_error.is_some() && !h.state().tabs[project].closing());
    assert_eq!(h.state().canvas.active, Some(uuid));
    // Closing the tab keeps what was typed.
    h.state_mut().canvas.skills[0].text = "# Naming".into();
    let at = rect_of(&h, &format!("workspace.tabs.{skill}.close")).center();
    click(&mut h, at);
    assert!(h.state().canvas.tabs.is_empty());
    assert_eq!(body(&h), "# Naming");
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

/// The look the store keeps for diagram node or edge `o`: fill, stroke, text and width.
fn look(h: &Harness<'static, Shell>, o: archforge_mof::Obj) -> crate::canvas::Look {
    let st = h.state().session.as_ref().unwrap().doc();
    let prop = if st.class_of(o).ok() == Some(archforge_diagram::NODE) { archforge_diagram::node::STYLE } else { archforge_diagram::edge::STYLE };
    crate::canvas::look_of(st, o, prop)
}

#[test]
fn appearance_styles_the_selection_on_this_diagram() {
    let (mut h, customer, diagram) = diagram_harness();
    let (node, edge, places) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let customer = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
        let d = st.by_uuid(archforge_mof::Uuid::parse(&diagram).unwrap()).unwrap();
        let first = |p| st.list(d, p).unwrap().iter().filter_map(archforge_mof::Value::as_ref).next().unwrap();
        let node = st
            .list(d, archforge_diagram::diagram::NODE)
            .unwrap()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .find(|n| st.get(*n, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(customer))
            .unwrap();
        let edge = first(archforge_diagram::diagram::EDGE);
        let places = st.get(edge, archforge_diagram::edge::ELEMENT).unwrap().as_ref().unwrap();
        (node, edge, st.uuid(places).unwrap().to_string())
    };
    let at = rect_of(&h, "workspace.panels.appearance").center();
    click(&mut h, at);
    // Nothing selected: the panel sets how the diagram's lines run, nothing else.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
    h.run();
    let shown: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.appearance.")).collect();
    assert!(!shown.is_empty() && shown.iter().all(|i| i.starts_with("workspace.appearance.routing.")), "{shown:?}");
    let diagram_way = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let e = st
            .list(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap(), archforge_diagram::diagram::EDGE)
            .unwrap()[0]
            .as_ref()
            .unwrap();
        archforge_diagram::route::way_of(st, e)
    };
    let at = rect_of(&h, "workspace.appearance.routing.right-angled").center();
    click(&mut h, at);
    h.run();
    assert_eq!(diagram_way(&h), archforge_diagram::Way::Avoiding, "right angles go around the tables at first");
    let at = rect_of(&h, "workspace.appearance.routing.avoid").center();
    click(&mut h, at);
    h.run();
    assert_eq!(diagram_way(&h), archforge_diagram::Way::RightAngled);
    let at = rect_of(&h, "workspace.appearance.routing.straight").center();
    click(&mut h, at);
    h.run();
    assert_eq!(diagram_way(&h), archforge_diagram::Way::Straight);
    assert!(!ids(&h).iter().any(|i| i == "workspace.appearance.routing.avoid"), "the switch is for right angles only");
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    h.run();
    // A swatch sets the fill; a width keeps it.
    let at = rect_of(&h, "workspace.appearance.fill.2").center();
    click(&mut h, at);
    assert_eq!(look(&h, node).fill, crate::canvas::colour("#EBF0FE"));
    let at = rect_of(&h, "workspace.appearance.border-width.3").center();
    click(&mut h, at);
    assert_eq!((look(&h, node).fill, look(&h, node).width), (crate::canvas::colour("#EBF0FE"), Some(3.0)));
    // The own colour: the picker changes nothing until Apply.
    let at = rect_of(&h, "workspace.appearance.text.own").center();
    click(&mut h, at);
    assert!(h.state().panels.picker.is_some());
    h.state_mut().panels.picker.as_mut().unwrap().hsv = [0.0, 1.0, 0.8];
    h.run();
    assert_eq!(look(&h, node).text, None, "nothing changes before Apply");
    let at = rect_of(&h, "workspace.appearance.picker.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.picker.is_none());
    assert_eq!(look(&h, node).text, crate::canvas::colour("#CC0000"));
    // The first swatch takes the row's own colour away and keeps the others.
    let at = rect_of(&h, "workspace.appearance.text.diagram").center();
    click(&mut h, at);
    assert_eq!((look(&h, node).text, look(&h, node).fill), (None, crate::canvas::colour("#EBF0FE")));
    // Use diagram defaults takes the own look away; undo brings it back.
    let at = rect_of(&h, "workspace.appearance.defaults").center();
    click(&mut h, at);
    assert_eq!(look(&h, node), crate::canvas::Look::default());
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(look(&h, node).width, Some(3.0));
    // A line: its colour and width, and the key status turned off to show the colour.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [places] })).unwrap();
    h.run();
    assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.appearance.fill.")), "a line has no fill");
    let at = rect_of(&h, "workspace.appearance.line.1").center();
    click(&mut h, at);
    assert_eq!(look(&h, edge).stroke, crate::canvas::colour("#C25E0A"));
    let at = rect_of(&h, "workspace.appearance.line-width.2").center();
    click(&mut h, at);
    assert_eq!(look(&h, edge).width, Some(2.0));
    // Its own routing: curved, keeping its colour and width.
    let at = rect_of(&h, "workspace.appearance.routing.curved").center();
    click(&mut h, at);
    assert_eq!(archforge_diagram::route::way_of(h.state().session.as_ref().unwrap().doc(), edge), archforge_diagram::Way::Curved);
    assert_eq!((look(&h, edge).stroke, look(&h, edge).width), (crate::canvas::colour("#C25E0A"), Some(2.0)));
    assert!(h.state().canvas.key_status);
    let at = rect_of(&h, "workspace.appearance.key-status").center();
    click(&mut h, at);
    assert!(!h.state().canvas.key_status, "the line colours show");
}

#[test]
fn the_control_channel_runs_commands_through_the_open_session() {
    use crate::control::answer;
    use serde_json::json;
    // Without a project: the methods say so, the inspection says nothing is open.
    let mut home = harness(ThemeKind::Light);
    assert_eq!(answer(home.state_mut(), "document.inspect", &json!({})).unwrap()["open"], false);
    assert!(answer(home.state_mut(), "engine.commands", &json!({})).unwrap_err().contains("no project is open"));
    let (mut h, _) = table_harness();
    let schema = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string()
    };
    let count = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::ENTITY)).count()
    };
    let before = count(&h);
    // Listing and describing.
    let listed = answer(h.state_mut(), "engine.commands", &json!({ "filter": "entity" })).unwrap();
    assert!(listed["commands"].as_array().unwrap().iter().any(|c| c["id"] == "er.entity.add"));
    let described = answer(h.state_mut(), "engine.describe", &json!({ "command": "er.entity.add" })).unwrap();
    assert!(described["params"].as_array().unwrap().iter().any(|p| p["key"] == "name"));
    // A command runs in the session the window draws, and undo takes it back.
    let out =
        answer(h.state_mut(), "engine.execute", &json!({ "command": "er.entity.add", "params": { "schema": schema, "name": "Invoice" } })).unwrap();
    assert!(out["result"]["id"].is_string());
    assert_eq!(count(&h), before + 1);
    answer(h.state_mut(), "engine.undo", &json!({})).unwrap();
    assert_eq!(count(&h), before);
    // A batch refers to an earlier step's result; a failing step rolls the whole batch back.
    let steps = json!([
        { "command": "er.entity.add", "params": { "schema": schema, "name": "Payment" } },
        { "command": "er.attribute.add", "params": { "entity": "$0.id", "name": "payment_id", "type": "BIGINT" } },
    ]);
    let out = answer(h.state_mut(), "engine.batch", &json!({ "label": "Add payment", "steps": steps })).unwrap();
    assert_eq!(out["results"].as_array().unwrap().len(), 2);
    assert_eq!(count(&h), before + 1);
    let bad = json!([
        { "command": "er.entity.add", "params": { "schema": schema, "name": "Refund" } },
        { "command": "er.attribute.add", "params": { "entity": "$5.id", "name": "x" } },
    ]);
    let why = answer(h.state_mut(), "engine.batch", &json!({ "steps": bad })).unwrap_err();
    assert!(why.contains("rolled back") && why.contains("step 1"), "{why}");
    assert_eq!(count(&h), before + 1, "nothing of the failed batch stays");
    // Unknown methods and keys are refused, naming what there is.
    assert!(answer(h.state_mut(), "engine.nope", &json!({})).unwrap_err().contains("engine.execute"));
    assert!(answer(h.state_mut(), "engine.execute", &json!({ "cmd": "x" })).unwrap_err().contains("command, params"));
    let inspected = answer(h.state_mut(), "document.inspect", &json!({})).unwrap();
    assert_eq!((inspected["open"].clone(), inspected["undo"].clone()), (json!(true), json!("Add payment")));
    assert!(answer(h.state_mut(), "ui.inspect", &json!({})).unwrap()["controls"].as_array().is_some_and(|c| !c.is_empty()));
}

/// Presses at `points[0]`, moves through the rest and lets go at the last.
fn drag_through(h: &mut Harness<'static, Shell>, points: &[egui::Pos2]) {
    h.event(egui::Event::PointerMoved(points[0]));
    h.event(egui::Event::PointerButton { pos: points[0], button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    for p in &points[1..] {
        h.event(egui::Event::PointerMoved(*p));
        h.run();
    }
    let last = points[points.len() - 1];
    h.event(egui::Event::PointerButton { pos: last, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

#[test]
fn the_minimap_viewport_drags_the_view_without_the_map_rescaling_under_it() {
    let (mut h, _, _) = diagram_harness();
    let map = rect_of(&h, "workspace.minimap.view");
    let start = map.center();
    // A click centres the view where it lands.
    click(&mut h, start);
    let centred = h.state().canvas.pan;
    // Dragging from there moves the view the other way, as the viewport follows the pointer.
    let to = start + egui::vec2(20.0, 8.0);
    drag_through(&mut h, &[start, to]);
    let one_step = h.state().canvas.pan;
    assert!(one_step.x < centred.x - 1.0 && one_step.y < centred.y - 1.0, "{centred:?} -> {one_step:?}");
    assert!(h.state().canvas.minimap_drag.is_none(), "letting go ends the drag");
    // The same drag in steps, from the same start, lands in the same place: the map keeps its
    // scale while it lasts.
    let (mut h, _, _) = diagram_harness();
    click(&mut h, start);
    drag_through(&mut h, &[start, start + egui::vec2(7.0, 3.0), start + egui::vec2(14.0, 5.0), to]);
    let stepped = h.state().canvas.pan;
    assert!((stepped - one_step).length() < 0.5, "{one_step:?} vs {stepped:?}");
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
    if st.class_of(o).ok() == Some(archforge_diagram::DIAGRAM) { crate::diagrams::diagram_name(st, o) } else { archforge_core::name_of(st, o) }
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
    assert!(st.alive().any(|o| st.class_of(o).ok() == Some(archforge_er::SCHEMA) && archforge_core::name_of(st, o) == "Billing"));
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
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Place("entity"));
    let canvas = rect_of(&h, "workspace.canvas");
    let before = node_ids(&h).len();
    click(&mut h, canvas.left_top() + egui::vec2(120.0, 320.0));
    let nodes = node_ids(&h);
    assert_eq!(nodes.len(), before + 1, "a node was placed: {nodes:?}");
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select, "placing hands back to Select");
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
    assert!(!st.alive().any(|o| archforge_core::name_of(st, o) == "Entity"), "the table went with the node");
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
    assert!(st.alive().any(|o| archforge_core::name_of(st, o) == "Order"), "the table stays");
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
    assert!(!st.alive().any(|o| archforge_core::name_of(st, o) == "Customer"));
}

#[test]
fn a_table_added_in_models_is_placed_on_the_open_diagram_and_grows_with_its_columns() {
    let (mut h, _customer, _diagram) = diagram_harness();
    let before = node_ids(&h).len();
    // Select the schema in Models and add a table: it lands on the diagram.
    let schema_row = ids(&h).into_iter().find(|i| i.starts_with("workspace.explorer.row.")).unwrap();
    let at = rect_of(&h, &schema_row).center();
    click(&mut h, at);
    crate::explorer::add(h.state_mut(), "table");
    h.run();
    assert_eq!(node_ids(&h).len(), before + 1, "placed on the open diagram");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    // Its node starts at the empty height and grows with the first column.
    let new_node = node_ids(&h).into_iter().max_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap();
    let id = new_node.trim_start_matches("workspace.canvas.node.").to_string();
    assert!((node_bounds(&h, &id).height - crate::editing::ENTITY_EMPTY_HEIGHT).abs() < 0.01);
    crate::explorer::add(h.state_mut(), "column");
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
    assert!(st.alive().any(|o| archforge_core::name_of(st, o) == "Order"), "the table stays in the model");
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
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Connect("relationship"));
    // Drag from Customer to Order: one to many, Customer has a primary key, so the key comes along.
    let (from, to) = (rect_of(&h, &customer_node).center(), rect_of(&h, &order_node).center());
    drag(&mut h, from, to);
    let rels = relationships(&h);
    assert_eq!(rels.len(), 1, "one relationship drawn");
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select, "drawing hands back to Select");
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
    let keys_and_columns = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let count = |c| st.alive().filter(|o| st.class_of(*o).ok() == Some(c)).count();
        (count(archforge_er::FOREIGN_KEY), count(archforge_er::ATTRIBUTE))
    };
    let with_line = keys_and_columns(&h);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(relationships(&h).is_empty(), "the relationship went");
    assert_eq!(keys_and_columns(&h), (with_line.0 - 1, with_line.1 - 1), "its foreign key and its key column went with it");
    assert!(h.state().canvas.toast.is_some(), "the toast offers Undo");
    let at = rect_of(&h, "workspace.toast.undo").center();
    click(&mut h, at);
    assert_eq!(relationships(&h).len(), 1, "Undo brings it back");
    assert_eq!(keys_and_columns(&h), with_line, "with its key and column");
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
    assert!(h.state().canvas.popover && h.state().canvas.tool == crate::canvas::Tool::Connect("relationship"));
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
    assert!(st.alive().any(|o| archforge_core::name_of(st, o) == "CustomerOrder"), "the mapping table exists");
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
        let order = st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap();
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
        let order = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
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
        .map(|c| archforge_core::name_of(st, c))
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
        let order = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
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

#[test]
fn the_key_editor_adds_a_unique_constraint_and_removes_the_primary_key() {
    let (mut h, customer) = table_harness();
    let found = ids(&h);
    assert!(found.iter().any(|i| i == "workspace.properties.keys.add"), "{found:?}");
    let key_rows: Vec<String> = found.iter().filter(|i| i.starts_with("workspace.properties.key.row.") && !i.ends_with(".edit")).cloned().collect();
    assert_eq!(key_rows.len(), 1, "the primary key: {key_rows:?}");
    // + opens the editor for a new unique constraint; a column is added from the menu; Apply.
    let at = rect_of(&h, "workspace.properties.keys.add").center();
    click(&mut h, at);
    assert!(h.state().panels.key.as_ref().is_some_and(|k| k.target.is_none() && !k.primary));
    let at = rect_of(&h, "workspace.properties.key.column.add").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::KeyColumn));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.key.column.add.")).collect();
    assert!(!items.is_empty());
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    assert_eq!(h.state().panels.key.as_ref().unwrap().columns.len(), 1);
    let at = rect_of(&h, "workspace.properties.key.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.key.is_none(), "{:?}", h.state().panels.key);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(archforge_er::rules::unique_constraints(st, customer_obj).len(), 1, "the unique constraint was added");
    assert_eq!(selected_name(&h), "Customer", "the table stays selected");
    // The primary key's row opens its editor; the bin removes the key, and the table gains a problem.
    let pk_row = ids(&h).into_iter().find(|i| i.starts_with("workspace.properties.key.row.") && !i.ends_with(".edit")).unwrap();
    let at = rect_of(&h, &format!("{pk_row}.edit")).center();
    click(&mut h, at);
    assert!(h.state().panels.key.as_ref().is_some_and(|k| k.primary));
    let at = rect_of(&h, "workspace.properties.key.remove").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(archforge_er::rules::primary_key_of(st, customer_obj).is_none(), "{:?}", h.state().panels.notice);
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.problem.0"), "Problems lists the missing key: {:?}", ids(&h));
    let problems = crate::problems::of(crate::notation_ui::UIS, st, &crate::problems::by_element(crate::notation_ui::UIS, st), customer_obj);
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].text, "Customer has no primary key");
}

#[test]
fn problems_badge_the_canvas_and_a_note_is_placed_and_typed_in_place() {
    let (mut h, _customer, _diagram) = diagram_harness();
    // Order's primary key goes: its node gets a badge; the schema's Properties lists and selects.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let (order, schema) = {
            let st = s.doc();
            let order = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "Order").unwrap()).unwrap().to_string();
            let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
            (order, schema)
        };
        s.execute("er.primary-key.remove", &serde_json::json!({ "entity": order })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
    }
    h.run();
    let badges: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.canvas.problem.")).collect();
    assert_eq!(badges.len(), 1, "one badge on Order: {badges:?}");
    let at = rect_of(&h, "workspace.properties.problem.0").center();
    click(&mut h, at);
    assert_eq!(selected_name(&h), "Order", "the problem row selects its table");
    // The Note tool places a note and opens its text; typing and Cmd+Enter set it.
    let at = rect_of(&h, "workspace.tools.note").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Note);
    let canvas = rect_of(&h, "workspace.canvas");
    let before = node_ids(&h).len();
    click(&mut h, canvas.left_top() + egui::vec2(canvas.width() - 120.0, 80.0));
    assert_eq!(node_ids(&h).len(), before + 1, "the note is a node: {:?}", h.state().canvas.notice);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select, "placing hands back to Select");
    assert!(h.state().canvas.note_edit.is_some(), "its text is open");
    assert!(ids(&h).iter().any(|i| i == "workspace.canvas.note"));
    h.state_mut().canvas.note_edit.as_mut().unwrap().text = "Product.sku is unique per supplier".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::COMMAND);
    assert!(h.state().canvas.note_edit.is_none(), "{:?}", h.state().canvas.note_edit);
    let st = h.state().session.as_ref().unwrap().doc();
    let note = st
        .alive()
        .find(|o| {
            st.class_of(*o).ok() == Some(archforge_diagram::NODE)
                && st.get(*o, archforge_diagram::node::ELEMENT).ok().and_then(|v| v.as_ref()).is_none()
        })
        .unwrap();
    assert_eq!(st.get(note, archforge_diagram::node::TEXT).unwrap().as_str(), Some("Product.sku is unique per supplier"));
    // Removing a column with its key says so in a toast.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_id = st
        .uuid(
            st.alive()
                .find(|o| {
                    archforge_core::name_of(st, *o) == "customer_id"
                        && archforge_er::rules::entity_of(st, *o).is_some_and(|e| archforge_core::name_of(st, e) == "Customer")
                })
                .unwrap(),
        )
        .unwrap();
    crate::panels::open_column(h.state_mut(), archforge_mof::Uuid::parse(&_customer).unwrap(), Some(customer_id));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [_customer] })).unwrap();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.column.remove").center();
    click(&mut h, at);
    let toast = h.state().canvas.toast.clone().expect("the toast");
    assert!(toast.text.starts_with("customer_id removed · "), "{}", toast.text);
}

#[test]
fn the_column_editor_sets_the_size_default_and_generation_and_the_heading_takes_a_description() {
    let (mut h, customer) = table_harness();
    // email becomes VARCHAR with a length of 255 and a default; the row shows VARCHAR(255).
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    h.state_mut().panels.column.as_mut().unwrap().ty = "VARCHAR".into();
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.column.length"), "a varchar takes a length: {:?}", ids(&h));
    assert!(!ids(&h).iter().any(|i| i == "workspace.properties.column.scale"));
    {
        let c = h.state_mut().panels.column.as_mut().unwrap();
        c.length = "255".into();
        c.default = "''".into();
    }
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_none(), "{:?}", h.state().panels.column);
    let st = h.state().session.as_ref().unwrap().doc();
    let email = st.alive().find(|o| archforge_core::name_of(st, *o) == "email").unwrap();
    assert_eq!(archforge_er::rules::type_display(st, email), "VARCHAR(255)");
    assert_eq!(archforge_er::rules::text_of(st, email, archforge_er::attribute::DEFAULT), "''");
    // A numeric takes precision and scale; the scale above the precision is refused and the editor stays open.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    {
        let c = h.state_mut().panels.column.as_mut().unwrap();
        c.ty = "NUMERIC".into();
        c.length.clear();
        c.precision = "4".into();
        c.scale = "6".into();
    }
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.column.scale"));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.refused.is_some()), "the refusal stays in the editor");
    h.state_mut().panels.column.as_mut().unwrap().scale = "2".into();
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_none(), "{:?}", h.state().panels.column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_er::rules::type_display(st, email), "NUMERIC(4, 2)");
    // Generation: computed with an expression through the select and the expression field.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    h.state_mut().panels.column.as_mut().unwrap().default.clear();
    let at = rect_of(&h, "workspace.properties.column.generation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.generation.3").center();
    click(&mut h, at);
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.column.expression"), "a computed column takes an expression");
    h.state_mut().panels.column.as_mut().unwrap().expression = "lower(name)".into();
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_none(), "{:?}", h.state().panels.column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_er::rules::generation_of(st, email), archforge_er::Generation::ComputedStored);
    assert_eq!(archforge_er::rules::text_of(st, email, archforge_er::attribute::EXPRESSION), "lower(name)");
    // The heading's description field describes the table.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    }
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.description"));
    let at = rect_of(&h, "workspace.properties.description").center();
    click(&mut h, at);
    h.event(egui::Event::Text("People who buy.".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    assert_eq!(archforge_er::rules::text_of(st, customer_obj, archforge_er::element::DESCRIPTION), "People who buy.");
}

#[test]
fn the_check_and_index_editors_write_through_the_engine_and_a_refusal_changes_nothing() {
    use archforge_er::{NullsOrder, SortDirection};
    let (mut h, customer) = table_harness();
    let found = ids(&h);
    assert!(
        found.iter().any(|i| i == "workspace.properties.checks.add") && found.iter().any(|i| i == "workspace.properties.indexes.add"),
        "{found:?}"
    );
    // + opens the check editor; Apply without an expression is refused and nothing lands; with
    // one, the check is added and the table stays selected.
    let at = rect_of(&h, "workspace.properties.checks.add").center();
    click(&mut h, at);
    assert!(h.state().panels.check.as_ref().is_some_and(|c| c.target.is_none()));
    let at = rect_of(&h, "workspace.properties.check.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.check.as_ref().is_some_and(|c| c.refused.is_some()), "{:?}", h.state().panels.check);
    let customer_obj = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap()
    };
    assert!(archforge_er::rules::checks_of(h.state().session.as_ref().unwrap().doc(), customer_obj).is_empty());
    h.state_mut().panels.check.as_mut().unwrap().expression = "email <> ''".into();
    let at = rect_of(&h, "workspace.properties.check.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.check.is_none(), "{:?}", h.state().panels.check);
    assert_eq!(archforge_er::rules::checks_of(h.state().session.as_ref().unwrap().doc(), customer_obj).len(), 1);
    assert_eq!(ids(&h).iter().filter(|i| i.starts_with("workspace.properties.check.row.") && !i.ends_with(".edit")).count(), 1);
    assert_eq!(selected_name(&h), "Customer");
    // + opens the index editor; two columns come from the menu, the second turns descending, the
    // first takes Nulls last from its right-click menu; Apply writes the index in one command.
    let at = rect_of(&h, "workspace.properties.indexes.add").center();
    click(&mut h, at);
    assert!(h.state().panels.index.as_ref().is_some_and(|i| i.target.is_none()));
    let at = rect_of(&h, "workspace.properties.index.column.add").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::IndexColumn));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.index.column.add.")).collect();
    assert_eq!(items.len(), 3, "two columns and an expression: {items:?}");
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.index.column.add").center();
    click(&mut h, at);
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.index.column.add.")).collect();
    assert_eq!(items.len(), 2, "the column left and an expression: {items:?}");
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    assert_eq!(h.state().panels.index.as_ref().unwrap().columns.len(), 2);
    let at = rect_of(&h, "workspace.properties.index.column.1.direction").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.index.as_ref().unwrap().columns[1].direction, SortDirection::Descending);
    let at = rect_of(&h, "workspace.properties.index.column.0").center();
    right_click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::IndexNulls(0)));
    let at = rect_of(&h, "workspace.properties.index.column.0.nulls.2").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.index.as_ref().unwrap().columns[0].nulls, NullsOrder::Last);
    let at = rect_of(&h, "workspace.properties.index.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.index.is_none(), "{:?}", h.state().panels.index);
    {
        let st = h.state().session.as_ref().unwrap().doc();
        let indexes = archforge_er::rules::indexes_of(st, customer_obj);
        assert_eq!(indexes.len(), 1);
        let specs = archforge_er::rules::index_specs(st, indexes[0]);
        assert_eq!((specs.len(), specs[0].nulls, specs[1].direction), (2, NullsOrder::Last, SortDirection::Descending));
        assert_eq!(archforge_er::rules::display_name(st, indexes[0]), "ix_customer_customer_id_email");
    }
    assert_eq!(selected_name(&h), "Customer");
    // The index row's edit button opens its editor; the bin removes the index.
    let row = ids(&h).into_iter().find(|i| i.starts_with("workspace.properties.index.row.") && !i.ends_with(".edit")).unwrap();
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    assert!(h.state().panels.index.as_ref().is_some_and(|i| i.target.is_some()));
    let at = rect_of(&h, "workspace.properties.index.remove").center();
    click(&mut h, at);
    assert!(h.state().panels.index.is_none());
    assert!(archforge_er::rules::indexes_of(h.state().session.as_ref().unwrap().doc(), customer_obj).is_empty());
}

#[test]
fn key_columns_follow_their_target_until_unlinked_and_a_pure_mapping_table_collapses() {
    let (mut h, customer) = table_harness();
    let (order, rel, mapping) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let order = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Order" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": order, "name": "order_id", "type": "BIGINT", "primary_key": true })).unwrap();
        let rel = id(s
            .execute(
                "er.relationship.add",
                &serde_json::json!({ "from": customer, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
            )
            .unwrap()
            .value);
        let tag = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Tag" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": tag, "name": "tag_id", "type": "INT", "primary_key": true })).unwrap();
        let line = id(s
            .execute("er.relationship.add", &serde_json::json!({ "from": order, "to": tag, "from_end": "zero-or-more", "to_end": "zero-or-more" }))
            .unwrap()
            .value);
        let mapping = id(s.execute("er.relationship.resolve", &serde_json::json!({ "relationship": line })).unwrap().value);
        s.execute("view.select", &serde_json::json!({ "ids": [order] })).unwrap();
        (order, rel, mapping)
    };
    h.run();
    // Order's key column follows Customer.customer_id: the row carries the link mark, the editor
    // names the target and Unlink frees it.
    let marks: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.") && i.ends_with(".link")).collect();
    assert_eq!(marks.len(), 1, "{marks:?}");
    let row = marks[0].trim_end_matches(".link").to_string();
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    assert_eq!(h.state().panels.column.as_ref().and_then(|c| c.linked.as_deref()), Some("Customer.customer_id"));
    let at = rect_of(&h, "workspace.properties.column.unlink").center();
    click(&mut h, at);
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.linked.is_none() && c.unlink), "{:?}", h.state().panels.column);
    // Cancel keeps the link; Unlink then Apply is one step that drops it.
    let at = rect_of(&h, "workspace.properties.column.cancel").center();
    click(&mut h, at);
    assert_eq!(ids(&h).iter().filter(|i| i.ends_with(".link")).count(), 1, "Cancel kept the link");
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.unlink").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_none(), "{:?}", h.state().panels.column);
    assert!(!ids(&h).iter().any(|i| i.ends_with(".link")), "the mark is gone");
    // The relationship's parent end set to zero-or-one makes the key column nullable.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [rel] })).unwrap();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.end.from.0").center();
    click(&mut h, at);
    {
        let st = h.state().session.as_ref().unwrap().doc();
        let order_obj = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
        let column = st
            .list(order_obj, archforge_er::entity::ATTRIBUTE)
            .unwrap()
            .iter()
            .filter_map(archforge_mof::Value::as_ref)
            .find(|a| archforge_core::name_of(st, *a) == "customer_customer_id")
            .unwrap();
        assert!(archforge_er::rules::is_nullable(st, column), "zero-or-one made the key column nullable");
    }
    // The mapping table's Properties offer to collapse it; the click brings the line back.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [mapping] })).unwrap();
    }
    h.run();
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.collapse"), "{:?}", ids(&h));
    let at = rect_of(&h, "workspace.properties.collapse").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(archforge_mof::Uuid::parse(&mapping).unwrap()).is_none_or(|m| !st.is_alive(m)), "{:?}", h.state().panels.notice);
    let relationships = st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_er::RELATIONSHIP)).count();
    assert_eq!(relationships, 2, "Customer to Order and the many-to-many line");
}

#[test]
fn the_schema_lists_its_types_and_a_column_takes_an_enum_type_from_the_type_menu() {
    let (mut h, customer) = table_harness();
    let schema = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        s.execute(
            "er.domain.add",
            &serde_json::json!({ "schema": schema, "name": "email_address", "base_type": "VARCHAR", "length": 255, "checks": ["position('@' in value) > 1"] }),
        )
        .unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
        schema
    };
    h.run();
    // The schema's Properties list the domain; + offers the kinds; the enum editor takes its
    // literals and Apply writes the enum type.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.type.row.") && !i.ends_with(".edit")).collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let at = rect_of(&h, "workspace.properties.types.add").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::TypeKind));
    let at = rect_of(&h, "workspace.properties.types.add.1").center();
    click(&mut h, at);
    assert!(h.state().panels.type_edit.as_ref().is_some_and(|t| t.kind == crate::types::TypeKind::Enum && t.target.is_none()));
    let at = rect_of(&h, "workspace.properties.enum.literal.add").center();
    click(&mut h, at);
    click(&mut h, at);
    {
        let edit = h.state_mut().panels.type_edit.as_mut().unwrap();
        edit.name = "customer_status".into();
        edit.literals = vec!["active".into(), "dormant".into()];
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.type.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.type_edit.is_none(), "{:?}", h.state().panels.type_edit);
    let (schema_obj, enum_id) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let schema_obj = st.by_uuid(archforge_mof::Uuid::parse(&schema).unwrap()).unwrap();
        let enums = archforge_er::rules::enums_of(st, schema_obj);
        assert_eq!(enums.len(), 1);
        assert_eq!(archforge_er::rules::literals_of(st, enums[0]), ["active", "dormant"]);
        (schema_obj, st.uuid(enums[0]).unwrap())
    };
    assert_eq!(selected_name(&h), "Shop", "the schema stays selected");
    assert_eq!(ids(&h).iter().filter(|i| i.starts_with("workspace.properties.type.row.") && !i.ends_with(".edit")).count(), 2);
    // The Models tree lists both types under the schema.
    assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.row.{enum_id}")), "{:?}", ids(&h));
    // A column takes the enum type from its editor's type menu; the row shows the type's name.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
    }
    h.run();
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::ColumnType));
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.type.menu.")).collect();
    assert!(items.len() > 10, "the schema's two types and the base types: {items:?}");
    let at = rect_of(&h, &items[1]).center();
    click(&mut h, at);
    assert_eq!(h.state().panels.column.as_ref().and_then(|c| c.enum_type), Some(enum_id));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_none(), "{:?}", h.state().panels.column);
    let st = h.state().session.as_ref().unwrap().doc();
    let customer_obj = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
    let email = st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().iter().filter_map(archforge_mof::Value::as_ref).nth(1).unwrap();
    assert_eq!(archforge_er::rules::type_display(st, email), "customer_status");
    // A column on a sized domain applies a change of its own (Not null) without touching the size.
    let domain = archforge_er::rules::domains_of(st, schema_obj)[0];
    let domain = st.uuid(domain).unwrap().to_string();
    let email_id = st.uuid(email).unwrap().to_string();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("er.attribute.set-type", &serde_json::json!({ "attribute": email_id, "domain": domain })).unwrap();
    }
    h.run();
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.0") && !i.ends_with(".edit")).collect();
    let at = rect_of(&h, &format!("{}.edit", rows[1])).center();
    click(&mut h, at);
    assert!(!ids(&h).iter().any(|i| i == "workspace.properties.column.default"), "a domain column shows no default of its own");
    h.state_mut().panels.column.as_mut().unwrap().not_null = true;
    h.run();
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.column.is_none(), "{:?}", h.state().panels.column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!archforge_er::rules::is_nullable(st, email));
    assert_eq!(archforge_er::rules::type_display(st, email), "email_address");
    // A new column typed with a default, then put on the domain from the menu, adds cleanly: the
    // domain's default counts and the typed one goes with the pick.
    let at = rect_of(&h, "workspace.properties.columns.add").center();
    click(&mut h, at);
    h.state_mut().panels.column.as_mut().unwrap().default = "'x'".into();
    h.run();
    let at = rect_of(&h, "workspace.properties.column.type.menu").center();
    click(&mut h, at);
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.column.type.menu.")).collect();
    let at = rect_of(&h, &items[0]).center();
    click(&mut h, at);
    assert!(h.state().panels.column.as_ref().is_some_and(|c| c.domain.is_some() && c.default.is_empty()));
    let at = rect_of(&h, "workspace.properties.column.apply").center();
    click(&mut h, at);
    // The add selects the new column, whose editor opens on it without a refusal.
    assert!(h.state().panels.column.as_ref().is_none_or(|c| c.refused.is_none() && c.target.is_some()), "{:?}", h.state().panels.column);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.list(customer_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 3);
}

#[test]
fn a_subtype_shows_in_the_supertypes_properties_and_the_toolbar_draws_no_generalizations() {
    let (mut h, customer, order) = two_tables_harness();
    // ER draws subtypes with the tools it has; the toolbar has no generalization tool.
    assert!(!ids(&h).iter().any(|i| i == "workspace.tools.generalization"));
    let generalization = {
        let s = h.state_mut().session.as_mut().unwrap();
        let out = s.execute("er.generalization.add", &serde_json::json!({ "supertype": customer, "subtype": order })).unwrap();
        let id = out.value.get("id").and_then(|v| v.as_str()).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
        archforge_mof::Uuid::parse(&id).unwrap()
    };
    h.run();
    let (generalization, customer_obj, order_obj) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let g = st.by_uuid(generalization).unwrap();
        let c = st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap();
        let o = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
        assert_eq!(archforge_er::rules::generalization_parts(st, g).0, Some(c));
        assert_eq!(archforge_er::rules::generalization_parts(st, g).1, Some(o));
        assert!(archforge_er::rules::generalization_parts(st, g).2.is_some(), "table per type made the key");
        (g, c, o)
    };
    // The generalization's Properties show the supertype's Subtypes section; Mapping single table
    // removes the shared key in one step.
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.subtype.row.") && !i.ends_with(".remove")).collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let at = rect_of(&h, "workspace.properties.subtypes.mapping").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.subtypes.mapping.1").center();
    click(&mut h, at);
    {
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(archforge_er::rules::mapping_of(st, customer_obj), archforge_er::SubtypeMapping::SingleTable);
        assert!(archforge_er::rules::generalization_parts(st, generalization).2.is_none(), "the key went");
        assert_eq!(st.list(order_obj, archforge_er::entity::ATTRIBUTE).unwrap().len(), 1, "Order keeps its own order_id only");
    }
    // The row's cross removes the subtype; the supertype stays selected.
    let remove = ids(&h).into_iter().find(|i| i.starts_with("workspace.properties.subtype.row.") && i.ends_with(".remove")).unwrap();
    let at = rect_of(&h, &remove).center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(!st.is_alive(generalization));
    assert_eq!(selected_name(&h), "Customer");
}

#[test]
fn the_schema_adds_and_edits_sequences_and_views_and_a_selected_ones_edit_button_opens_its_editor() {
    let (mut h, _) = table_harness();
    let (schema, key) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let key = st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == "customer_id").unwrap()).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
        (schema, key)
    };
    h.run();
    // + opens the sequence editor; Owned by offers the whole-number columns; Apply writes it.
    let at = rect_of(&h, "workspace.properties.sequences.add").center();
    click(&mut h, at);
    assert!(h.state().panels.object_edit.as_ref().is_some_and(|e| e.kind == crate::objects::ObjectKind::Sequence && e.target.is_none()));
    let at = rect_of(&h, "workspace.properties.sequence.owned-by").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::OwnedBy));
    let offered: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.properties.sequence.owned-by.")).collect();
    assert_eq!(offered.len(), 2, "None and Customer.customer_id, not the text column: {offered:?}");
    let at = rect_of(&h, "workspace.properties.sequence.owned-by.1").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.object_edit.as_ref().and_then(|e| e.owned_by), Some(key));
    {
        let edit = h.state_mut().panels.object_edit.as_mut().unwrap();
        edit.name = "customer_id_seq".into();
        edit.start = "one".into();
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.object.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.object_edit.as_ref().is_some_and(|e| e.refused.is_some()), "a start that is no number is refused");
    {
        let edit = h.state_mut().panels.object_edit.as_mut().unwrap();
        edit.start = "1000".into();
        edit.minimum = "1000".into();
    }
    h.run();
    click(&mut h, at);
    assert!(h.state().panels.object_edit.is_none(), "{:?}", h.state().panels.object_edit);
    // The view editor takes the query over several lines.
    let at = rect_of(&h, "workspace.properties.views.add").center();
    click(&mut h, at);
    {
        let edit = h.state_mut().panels.object_edit.as_mut().unwrap();
        assert_eq!(edit.kind, crate::objects::ObjectKind::View);
        edit.name = "recent_customers".into();
        edit.query = "SELECT customer_id\nFROM \"Customer\"\n".into();
        edit.materialized = true;
    }
    h.run();
    let at = rect_of(&h, "workspace.properties.object.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.object_edit.is_none(), "{:?}", h.state().panels.object_edit);
    let (seq, view) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let s = st.by_uuid(archforge_mof::Uuid::parse(&schema).unwrap()).unwrap();
        let (seqs, views) = (archforge_er::rules::sequences_of(st, s), archforge_er::rules::views_of(st, s));
        assert_eq!((seqs.len(), views.len()), (1, 1));
        let n = archforge_er::rules::sequence_numbers(st, seqs[0]);
        assert_eq!((n.start, n.minimum.as_str()), (1000, "1000"));
        assert_eq!(archforge_er::rules::owner_of_sequence(st, seqs[0]).and_then(|a| st.uuid(a).ok()), Some(key));
        assert_eq!(archforge_er::rules::text_of(st, views[0], archforge_er::view::QUERY), "SELECT customer_id\nFROM \"Customer\"");
        (st.uuid(seqs[0]).unwrap(), st.uuid(views[0]).unwrap())
    };
    assert_eq!(selected_name(&h), "Shop", "the schema stays selected");
    // Both show in the Models tree; a click on one selects it under its schema's section, and its
    // row's edit button opens its editor.
    for id in [seq, view] {
        assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.row.{id}")), "{:?}", ids(&h));
    }
    let at = rect_of(&h, &format!("workspace.explorer.row.{view}")).center();
    click(&mut h, at);
    assert!(h.state().panels.object_edit.is_none(), "selecting it does not open its editor");
    let at = rect_of(&h, &format!("workspace.properties.view.row.{view}.edit")).center();
    click(&mut h, at);
    assert!(h.state().panels.object_edit.as_ref().is_some_and(|e| e.target == Some(view) && e.materialized));
    // The bin removes it; the schema is selected again.
    let at = rect_of(&h, "workspace.properties.object.remove").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(view).is_none_or(|v| !st.is_alive(v)));
    assert_eq!(selected_name(&h), "Shop");
}

#[test]
fn a_table_of_another_schema_dragged_from_the_explorer_lands_with_its_line_once() {
    let (mut h, _, order) = two_tables_harness();
    let (account, diagram) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let crm = id(s.execute("er.schema.add", &serde_json::json!({ "name": "CRM" })).unwrap().value);
        let account = id(s.execute("er.entity.add", &serde_json::json!({ "schema": crm, "name": "Account" })).unwrap().value);
        s.execute("er.attribute.add", &serde_json::json!({ "entity": account, "name": "account_id", "type": "BIGINT", "primary_key": true }))
            .unwrap();
        s.execute(
            "er.relationship.add",
            &serde_json::json!({ "from": account, "to": order, "from_end": "exactly-one", "to_end": "zero-or-more", "create_foreign_key": true }),
        )
        .unwrap();
        let st = s.doc();
        let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
        (account, d)
    };
    h.run();
    let shown = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        (st.list(diagram, archforge_diagram::diagram::NODE).unwrap().len(), st.list(diagram, archforge_diagram::diagram::EDGE).unwrap().len())
    };
    assert_eq!(shown(&h), (2, 0));
    // The new schema starts folded; Expand all opens it.
    let fold_all = rect_of(&h, "workspace.explorer.fold-all").center();
    click(&mut h, fold_all);
    h.run();
    let row = rect_of(&h, &format!("workspace.explorer.row.{account}")).center();
    let canvas = rect_of(&h, "workspace.canvas");
    let to = egui::pos2(canvas.center().x, canvas.bottom() - 200.0);
    drag(&mut h, row, to);
    assert_eq!(shown(&h), (3, 1), "the table and its relationship to Order, drawn together");
    assert_eq!(selected_name(&h), "Account");
    assert!(ids(&h).iter().any(|i| i.starts_with("workspace.canvas.node.")));
    // One undo takes both; a second drop of the same table is said and changes nothing.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("edit.undo", &serde_json::Value::Null).unwrap();
        s.execute("edit.redo", &serde_json::Value::Null).unwrap();
    }
    h.run();
    assert_eq!(shown(&h), (3, 1));
    let row = rect_of(&h, &format!("workspace.explorer.row.{account}")).center();
    drag(&mut h, row, to);
    assert_eq!(shown(&h), (3, 1));
    assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("edit.undo", &serde_json::Value::Null).unwrap();
    }
    h.run();
    assert_eq!(shown(&h), (2, 0));
}

#[test]
fn the_properties_body_scrolls_and_an_editor_opened_below_the_fold_comes_into_view() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1440.0, 760.0));
    let customer = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "Shop" })).unwrap().value);
        let customer = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Customer" })).unwrap().value);
        for i in 0..14 {
            s.execute(
                "er.attribute.add",
                &serde_json::json!({ "entity": customer, "name": format!("column_{i}"), "type": "BIGINT", "primary_key": i == 0 }),
            )
            .unwrap();
        }
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        customer
    };
    let _ = customer;
    h.run();
    // The sections past the window's foot are out of reach until the body scrolls.
    assert!(!ids(&h).iter().any(|i| i == "workspace.properties.indexes.add"), "the Indexes section starts below the fold");
    assert!(h.state().panels.content > h.state().panels.body.unwrap().height());
    let body = h.state().panels.body.unwrap();
    h.event(egui::Event::PointerMoved(body.center()));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -2000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    // The wheel scrolls smoothly over several frames.
    h.run_steps(30);
    assert!(h.state().panels.scroll > 0.0);
    let at = rect_of(&h, "workspace.properties.indexes.add").center();
    click(&mut h, at);
    h.run();
    // The index editor opened at the foot is brought into view with its Apply.
    let apply = rect_of(&h, "workspace.properties.index.apply");
    let body = h.state().panels.body.unwrap();
    assert!(body.contains_rect(apply), "{apply:?} in {body:?}");
    // Another table starts at the top again.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
    }
    h.run();
    assert_eq!(h.state().panels.scroll, 0.0);
}

#[test]
fn every_relationship_end_draws_its_two_crows_foot_marks() {
    use archforge_er::Cardinality as C;
    // The shapes an end draws: its line segments and its circles.
    let marks = |c: C| {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            let p = crate::ux::palette(false);
            let stroke = egui::Stroke::new(1.2, p.text);
            crate::canvas::end_glyph(
                ui.painter(),
                p,
                egui::pos2(100.0, 100.0),
                egui::pos2(200.0, 100.0),
                archforge_er::figures::end_mark(Some(c)),
                stroke,
                1.0,
                false,
            );
        });
        out.textures_delta.clear();
        let shapes: Vec<egui::Shape> = out.shapes.into_iter().map(|s| s.shape).collect();
        let lines = shapes.iter().filter(|s| matches!(s, egui::Shape::LineSegment { .. })).count();
        let circles = shapes.iter().filter(|s| matches!(s, egui::Shape::Circle(c) if c.stroke.width > 0.0)).count();
        (lines, circles)
    };
    // The outer mark is the most (a bar, or a crow's foot of two lines), the inner the fewest (a
    // circle for zero, a bar for one).
    assert_eq!(marks(C::ZeroOrOne), (1, 1));
    assert_eq!(marks(C::ExactlyOne), (2, 0), "exactly one is two bars");
    assert_eq!(marks(C::ZeroOrMore), (2, 1));
    assert_eq!(marks(C::OneOrMore), (3, 0), "one or more is a bar and a crow's foot");
}

#[test]
fn a_selected_notes_handle_links_it_to_a_table_or_a_relationships_line_and_delete_removes_a_link() {
    let (mut h, _customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let order_node = nodes.iter().find(|n| **n != customer_node).unwrap().clone();
    // A relationship between the two, and a note below them.
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (from, to) = (rect_of(&h, &customer_node).center(), rect_of(&h, &order_node).center());
    drag(&mut h, from, to);
    let note = {
        let s = h.state_mut().session.as_mut().unwrap();
        let d = s.view().open_diagram.unwrap();
        let d = s.doc().uuid(d).unwrap().to_string();
        let out = s.execute("diagram.note", &serde_json::json!({ "diagram": d, "text": "Why", "x": 380, "y": 400 })).unwrap();
        let id = out.value["id"].as_str().unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
        id
    };
    h.run_steps(2);
    let links = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive()
            .filter(|o| {
                st.class_of(*o).ok() == Some(archforge_diagram::EDGE)
                    && st.get(*o, archforge_diagram::edge::ELEMENT).unwrap().as_ref().is_none()
                    && st.get(*o, archforge_diagram::edge::SOURCE).unwrap().as_ref() == st.by_uuid(archforge_mof::Uuid::parse(&note).unwrap())
            })
            .map(|o| st.class_of(st.get(o, archforge_diagram::edge::TARGET).unwrap().as_ref().unwrap()).unwrap())
            .collect::<Vec<_>>()
    };
    // Its handle dragged onto Order links it to the table.
    let handle = rect_of(&h, "workspace.canvas.adorn.link").center();
    let onto = rect_of(&h, &order_node).center();
    drag(&mut h, handle, onto);
    assert_eq!(links(&h), [archforge_diagram::NODE], "{:?}", h.state().canvas.notice);
    // Selected again, dragged onto the middle of the relationship's line: linked to the line.
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [note] })).unwrap();
    }
    h.run_steps(2);
    let middle = rect_of(&h, &customer_node).center().lerp(rect_of(&h, &order_node).center(), 0.5);
    let handle = rect_of(&h, "workspace.canvas.adorn.link").center();
    drag(&mut h, handle, middle);
    let kinds = links(&h);
    assert_eq!(kinds.len(), 2, "{:?}", h.state().canvas.notice);
    assert!(kinds.contains(&archforge_diagram::EDGE), "one link ends on the relationship's line");
    // The link is selected after drawing; Delete removes it alone.
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(links(&h), [archforge_diagram::NODE]);
    assert_eq!(relationships(&h).len(), 1, "the relationship stays");
}

#[test]
fn the_link_note_tool_drags_a_link_from_a_note_to_a_table_and_hands_back_to_select() {
    let (mut h, _customer, _order) = two_tables_harness();
    let nodes = node_ids(&h);
    let order_node = nodes.iter().max_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let note = {
        let s = h.state_mut().session.as_mut().unwrap();
        let d = s.view().open_diagram.unwrap();
        let d = s.doc().uuid(d).unwrap().to_string();
        let out = s.execute("diagram.note", &serde_json::json!({ "diagram": d, "text": "Why", "x": 380, "y": 400 })).unwrap();
        out.value["id"].as_str().unwrap().to_string()
    };
    h.run_steps(2);
    let note_node = format!("workspace.canvas.node.{note}");
    let at = rect_of(&h, "workspace.tools.link").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Link);
    // A drag that starts off a note draws nothing; one from the note onto Order links them.
    let (from, onto) = (rect_of(&h, &order_node).center(), rect_of(&h, &note_node).center());
    drag(&mut h, onto + egui::vec2(0.0, 200.0), from);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Link, "nothing linked, the tool stays");
    drag(&mut h, onto, from);
    let st = h.state().session.as_ref().unwrap().doc();
    let note_obj = st.by_uuid(archforge_mof::Uuid::parse(&note).unwrap());
    let links = st
        .alive()
        .filter(|o| {
            st.class_of(*o).ok() == Some(archforge_diagram::EDGE) && st.get(*o, archforge_diagram::edge::SOURCE).unwrap().as_ref() == note_obj
        })
        .count();
    assert_eq!(links, 1, "{:?}", h.state().canvas.notice);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select, "the tool hands back to Select");
}

#[test]
fn key_columns_a_line_adds_grow_the_table_to_show_them_in_the_same_step() {
    let (mut h, _customer, order) = two_tables_harness();
    // Order's box just fits its one column.
    let order_node = {
        let st = h.state().session.as_ref().unwrap().doc();
        let e = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
        let n = st
            .alive()
            .find(|n| {
                st.class_of(*n).ok() == Some(archforge_diagram::NODE) && st.get(*n, archforge_diagram::node::ELEMENT).unwrap().as_ref() == Some(e)
            })
            .unwrap();
        st.uuid(n).unwrap().to_string()
    };
    let snug = crate::editing::content_height(1);
    h.state_mut()
        .session
        .as_mut()
        .unwrap()
        .execute("diagram.resize", &serde_json::json!({ "node": order_node, "width": 240, "height": snug }))
        .unwrap();
    h.run_steps(2);
    let nodes = node_ids(&h);
    let customer_node = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (a, b) = (rect_of(&h, &customer_node).center(), rect_of(&h, &format!("workspace.canvas.node.{order_node}")).center());
    drag(&mut h, a, b);
    let rels = relationships(&h);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(crate::connect::has_foreign_key(st, rels[0]), "both tables have columns and Customer its key: the key comes on");
    let grown = node_bounds(&h, &order_node).height;
    assert_eq!(grown, crate::editing::content_height(2), "Order grew to show its new key column");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run_steps(2);
    assert_eq!(node_bounds(&h, &order_node).height, snug, "one undo takes the line and the growth back");
}

#[test]
fn the_models_tree_starts_folded_and_opens_all_at_once() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let (schema, entity, column) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("er.schema.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
        let entity = id(s.execute("er.entity.add", &serde_json::json!({ "schema": schema, "name": "Customer" })).unwrap().value);
        let column = id(s.execute("er.attribute.add", &serde_json::json!({ "entity": entity, "name": "email" })).unwrap().value);
        s.execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        (schema, entity, column)
    };
    h.run();
    let row = |id: &str| format!("workspace.explorer.row.{id}");
    let shows = |h: &Harness<'static, Shell>, id: &str| ids(h).iter().any(|i| *i == row(id));
    assert!(shows(&h, &schema) && !shows(&h, &entity), "only the schema shows: {:?}", ids(&h));
    // Expand all opens every row; then the same button folds them all again.
    let at = rect_of(&h, "workspace.explorer.fold-all").center();
    click(&mut h, at);
    h.run();
    assert!(shows(&h, &entity) && shows(&h, &column));
    click(&mut h, at);
    h.run();
    assert!(shows(&h, &schema) && !shows(&h, &entity));
    // An element selected elsewhere opens its schema and table once, so its row shows.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [column] })).unwrap();
    h.run();
    assert!(shows(&h, &column), "the selected column's row shows");
    h.state_mut().explorer.expanded.remove(&archforge_mof::Uuid::parse(&entity).unwrap());
    h.run();
    assert!(!shows(&h, &column), "folded again, its table stays folded");
}

#[test]
fn a_recent_project_comes_off_the_list_through_its_menu() {
    let (mut h, host) = saving_harness(None, None);
    h.state_mut().state.remember("Shop", std::path::Path::new("/tmp/Shop.archforge"), 2);
    h.state_mut().state.remember("Billing", std::path::Path::new("/tmp/Billing.archforge"), 3);
    h.run_steps(2);
    // The menu button shows on the row under the pointer; its click opens no project.
    let row = rect_of(&h, "home.recent.0").center();
    h.event(egui::Event::PointerMoved(row));
    h.run_steps(2);
    let at = rect_of(&h, "home.recent.0.more").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(host.opens.lock().unwrap().is_empty(), "the menu button opens nothing");
    let at = rect_of(&h, "home.recent.0.menu.1").center();
    click(&mut h, at);
    h.run_steps(2);
    let left: Vec<String> = h.state().state.recent.iter().map(|r| r.name.clone()).collect();
    assert_eq!(left, ["Shop"], "Billing, opened last and listed first, is gone");
    assert!(host.opens.lock().unwrap().is_empty());
    // Open, the menu's first row, opens it.
    h.event(egui::Event::PointerMoved(rect_of(&h, "home.recent.0").center()));
    h.run_steps(2);
    let at = rect_of(&h, "home.recent.0.more").center();
    click(&mut h, at);
    h.run_steps(2);
    let at = rect_of(&h, "home.recent.0.menu.0").center();
    click(&mut h, at);
    h.run_steps(2);
    assert_eq!(host.opens.lock().unwrap().len(), 1);
}

/// Two tables with the relationship Customer places Order drawn between them, Order moved to
/// `order_at`; the diagram, its edge and the two nodes.
fn related_tables(order_at: (f64, f64)) -> (Harness<'static, Shell>, archforge_mof::Obj, archforge_mof::Obj, archforge_mof::Obj, archforge_mof::Obj) {
    let (mut h, customer, order) = two_tables_harness();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.execute(
            "er.relationship.add",
            &serde_json::json!({ "from": customer, "to": order, "name": "places", "from_end": "exactly-one", "to_end": "zero-or-more" }),
        )
        .unwrap();
        let st = s.doc();
        let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
        let did = st.uuid(d).unwrap().to_string();
        s.execute("diagram.connect-all", &serde_json::json!({ "diagram": did })).unwrap();
        let st = s.doc();
        let order_obj = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
        let order_node = archforge_diagram::views_of(st, order_obj).2[0];
        let (x, y) = (archforge_diagram::grid::bounds_of(st, order_node).x, archforge_diagram::grid::bounds_of(st, order_node).y);
        let node_id = st.uuid(order_node).unwrap().to_string();
        s.execute("diagram.move", &serde_json::json!({ "nodes": [node_id], "dx": order_at.0 - x, "dy": order_at.1 - y })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
    }
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
    let e = st.list(d, archforge_diagram::diagram::EDGE).unwrap()[0].as_ref().unwrap();
    let nodes: Vec<archforge_mof::Obj> = st.list(d, archforge_diagram::diagram::NODE).unwrap().iter().filter_map(|v| v.as_ref()).collect();
    (h, d, e, nodes[0], nodes[1])
}

#[test]
fn arrange_diagram_lays_the_tables_out_along_their_line_as_one_step() {
    // Order on top of Customer.
    let (mut h, _d, _e, customer, order) = related_tables((120.0, 120.0));
    let at = rect_of(&h, "workspace.tools.arrange").center();
    click(&mut h, at);
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let (c, o) = (archforge_diagram::grid::bounds_of(st, customer), archforge_diagram::grid::bounds_of(st, order));
    assert!(c.y + c.height <= o.y, "Customer, where the line starts, above Order: {c:?} {o:?}");
    // One undo puts both back.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_diagram::grid::bounds_of(st, order).x, 120.0);
}

#[test]
fn a_cardinality_text_is_dragged_aside_and_a_double_click_puts_it_back() {
    let (mut h, _d, e, ..) = related_tables((500.0, 100.0));
    let labels = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().doc().list(e, archforge_diagram::edge::LABEL).unwrap().len();
    let first = h.state().canvas.ends.iter().find(|t| t.end == 1).cloned().expect("the line's texts are drawn");
    // Between the tables, clear of both.
    let to = first.rect.center() + egui::vec2(-40.0, 60.0);
    drag(&mut h, first.rect.center(), to);
    assert_eq!(labels(&h), 1, "the place is kept");
    h.run();
    let moved = h.state().canvas.ends.iter().find(|t| t.end == 1).cloned().unwrap();
    assert!((moved.rect.center() - to).length() < 2.0, "drawn where it was let go: {:?} {to:?}", moved.rect.center());
    click(&mut h, moved.rect.center());
    click(&mut h, moved.rect.center());
    h.run();
    assert_eq!(labels(&h), 0, "back beside the line");
}

#[test]
fn a_right_angled_lines_segment_slides_and_a_double_click_routes_it_anew() {
    // Order below and right of Customer: around nothing, at right angles.
    let (mut h, d, e, ..) = related_tables((600.0, 500.0));
    let route = |h: &Harness<'static, Shell>| archforge_diagram::Grid::build(h.state().session.as_ref().unwrap().doc(), d).line(e).unwrap().to_vec();
    let before = route(&h);
    assert!(before.len() >= 3, "{before:?}");
    // The segment from the first corner on: an upright one slides left, a level one up.
    let level = (before[1].1 - before[2].1).abs() < 1e-6;
    let shift = if level { egui::vec2(0.0, -40.0) } else { egui::vec2(-40.0, 0.0) };
    let view = crate::canvas::View { origin: rect_of(&h, "workspace.canvas").min, pan: h.state().canvas.pan, zoom: h.state().canvas.zoom };
    let middle = view.to_screen((before[1].0 + before[2].0) / 2.0, (before[1].1 + before[2].1) / 2.0);
    drag(&mut h, middle, middle + shift * view.zoom);
    let after = route(&h);
    let moved = if level { (after[1].1 - (before[1].1 - 40.0)).abs() < 1.0 } else { (after[1].0 - (before[1].0 - 40.0)).abs() < 1.0 };
    assert!(moved && after.windows(2).all(|w| (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6), "{before:?} → {after:?}");
    let waypoints = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().doc().list(e, archforge_diagram::edge::WAYPOINT).unwrap().len();
    assert_eq!(waypoints(&h), before.len() - 2);
    let at = view.to_screen((after[1].0 + after[2].0) / 2.0, (after[1].1 + after[2].1) / 2.0);
    click(&mut h, at);
    click(&mut h, at);
    h.run();
    assert_eq!(waypoints(&h), 0, "the route is worked out anew");
}

#[test]
fn the_top_bars_search_finds_a_projects_tables_and_diagrams() {
    let (mut h, _d, _e, _customer, order_node) = related_tables((900.0, 700.0));
    let results = |h: &Harness<'static, Shell>| ids(h).into_iter().filter(|i| i.starts_with("workspace.header.search.result.")).collect::<Vec<_>>();
    // The command key with K opens it; typing narrows it to the table Order.
    h.event(egui::Event::Key { key: egui::Key::K, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.run();
    assert!(h.state().home_search.open, "the search opens");
    h.event(egui::Event::Text("order".into()));
    h.run();
    assert!(!results(&h).is_empty());
    // Enter goes to the first: Order is selected and its table comes to the canvas's centre.
    h.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.run();
    h.run();
    assert_eq!(selected_name(&h), "Order");
    assert!(!h.state().home_search.open, "the list closes");
    let node = rect_of(&h, &format!("workspace.canvas.node.{}", h.state().session.as_ref().unwrap().doc().uuid(order_node).unwrap()));
    let canvas = rect_of(&h, "workspace.canvas");
    assert!((node.center() - canvas.center()).length() < 4.0, "brought into view: {node:?} in {canvas:?}");
    // A click opens it too; the diagram is found by its name and opens in its tab.
    let at = rect_of(&h, "workspace.header.search").center();
    click(&mut h, at);
    h.event(egui::Event::Text("shop schema".into()));
    h.run();
    let first = rect_of(&h, "workspace.header.search.result.0").center();
    click(&mut h, first);
    h.run();
    assert_eq!(selected_name(&h), "Shop schema", "the diagram is open and selected");
    // Esc closes it with nothing chosen.
    let at = rect_of(&h, "workspace.header.search").center();
    click(&mut h, at);
    h.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.run();
    assert!(!h.state().home_search.open);
}

#[test]
fn the_avatar_shows_the_initials_of_the_name_entered_in_settings() {
    let mut h = harness(ThemeKind::Light);
    let initials = |h: &Harness<'static, Shell>| crate::settings::avatar_initials(h.state());
    assert_eq!(initials(&h), "JD", "without a name: John Doe");
    // The avatar opens Settings; a name typed there shows at once.
    let at = rect_of(&h, "home.header.account").center();
    click(&mut h, at);
    assert!(h.state().settings.is_some());
    h.event(egui::Event::Text("Ada Lovelace".into()));
    h.run();
    assert_eq!(initials(&h), "AL");
    let at = rect_of(&h, "settings.done").center();
    click(&mut h, at);
    assert!(h.state().settings.is_none());
    assert_eq!(h.state().state.user_name, "Ada Lovelace", "kept in the preferences");
    // The command key with comma opens it again; an emptied name goes back to JD; Escape closes.
    h.event(egui::Event::Key { key: egui::Key::Comma, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.run();
    assert!(h.state().settings.is_some());
    h.state_mut().state.user_name.clear();
    h.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.run();
    assert!(h.state().settings.is_none());
    assert_eq!(initials(&h), "JD");
    // A preferences file's name is cleaned when read.
    let mut state: crate::state::UiState = serde_json::from_str(r#"{"user_name":"  Grace Hopper\n"}"#).unwrap();
    state.sanitize();
    assert_eq!(state.user_name, "Grace Hopper");
}

/// A second notation, for the canvas test alone: diagrams of a UML package showing its classes as
/// cards (their attributes, then their operations) and their generalizations as lines with a
/// triangle and a text at the specific end.
struct Sketch;

const SKETCH: &str = "test.sketch";

impl archforge_diagram::Notation for Sketch {
    fn name(&self) -> &'static str {
        "Sketch"
    }
    fn metamodel(&self) -> u16 {
        archforge_uml::INDEX
    }
    fn kinds(&self) -> &'static [&'static str] {
        &[SKETCH]
    }
    fn is_of(&self, _: &str, class: archforge_mof::ClassId) -> bool {
        class == archforge_uml::PACKAGE
    }
    fn shows(&self, _: &str, class: archforge_mof::ClassId) -> bool {
        class == archforge_uml::CLASS
    }
    fn connects(&self, _: &str, class: archforge_mof::ClassId) -> bool {
        class == archforge_uml::GENERALIZATION
    }
    fn ends(&self, store: &archforge_mof::Store, element: archforge_mof::Obj) -> Option<(archforge_mof::Obj, archforge_mof::Obj)> {
        let specific = store.container(element).ok()??.0;
        let general = store.get(element, archforge_uml::generalization::GENERAL).ok()?.as_ref()?;
        Some((specific, general))
    }
    fn card(&self, store: &archforge_mof::Store, element: archforge_mof::Obj, _: archforge_mof::Obj) -> Option<archforge_diagram::figure::Card> {
        use archforge_diagram::figure::{Card, Row, Text};
        let rows = |prop| {
            store
                .list(element, prop)
                .unwrap_or_default()
                .iter()
                .filter_map(archforge_mof::Value::as_ref)
                .map(|o| Row { mark: None, text: archforge_core::name_of(store, o), linked: false, slanted: false })
                .collect::<Vec<_>>()
        };
        Some(Card {
            eyebrow: Text::Plain("Class".into()),
            name: archforge_core::name_of(store, element),
            compartments: vec![rows(archforge_uml::class::OWNED_ATTRIBUTE), rows(archforge_uml::class::OWNED_OPERATION)],
            more: ("+ {n} more column", "+ {n} more columns"),
            elsewhere: false,
            tone: archforge_diagram::figure::Tone::Tinted,
            slanted: false,
        })
    }
    fn line(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<archforge_diagram::figure::Line> {
        use archforge_diagram::figure::{End, EndMark, Line};
        Some(Line {
            ends: [End { mark: EndMark::None, text: Some("specific".into()), slot: "sketch.specific", role: None }, End::marked(EndMark::Triangle)],
            styled: true,
            dashed: false,
            status: None,
            note: None,
            badge: false,
            resolvable: false,
        })
    }
}

static SKETCHED: &[&dyn archforge_diagram::Notation] = &[&archforge_er::ErNotation, &Sketch];

/// Every text the last frame painted.
/// The colours the text `label` is laid out in, where it is painted inside `within`.
fn label_colours(h: &Harness<'static, Shell>, label: &str, within: egui::Rect) -> Vec<egui::Color32> {
    fn walk(s: &egui::Shape, label: &str, within: egui::Rect, out: &mut Vec<egui::Color32>) {
        match s {
            egui::Shape::Text(t) if t.galley.text() == label && within.contains(t.pos) => {
                out.extend(t.galley.job.sections.iter().map(|section| section.format.color));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, label, within, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, label, within, &mut out));
    out
}

fn painted(h: &Harness<'static, Shell>) -> Vec<String> {
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

/// A workspace whose session holds the sketch notation beside ER, with a sketch diagram of a
/// package: Party at the left, Customer at the right, Customer a kind of Party. Gives the diagram,
/// Party's node, Customer's node and the line's edge.
fn sketch_harness() -> (Harness<'static, Shell>, String, String, String, String) {
    use archforge_mof::Value;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    // A package with Party and Customer, Customer a kind of Party, in a session that holds the
    // sketch notation beside ER.
    let mut st = archforge_notations::store(Box::new({
        let ids = test_ids();
        move || ids()
    }))
    .unwrap();
    let model = st.create(archforge_core::MODEL).unwrap();
    let pkg = st.create(archforge_uml::PACKAGE).unwrap();
    archforge_uml::PackageObj::set_name(&mut st, pkg, Some("sales")).unwrap();
    archforge_core::ModelObj::push_member(&mut st, model, Value::Ref(pkg)).unwrap();
    let class = |st: &mut archforge_mof::Store, name: &str, parts: &[(&str, archforge_mof::ClassId, archforge_mof::PropId)]| {
        let c = st.create(archforge_uml::CLASS).unwrap();
        archforge_uml::ClassObj::set_name(st, c, Some(name)).unwrap();
        st.push(pkg, archforge_uml::package::PACKAGED_ELEMENT, Value::Ref(c)).unwrap();
        for (part, kind, prop) in parts {
            let o = st.create(*kind).unwrap();
            st.set(o, archforge_uml::named_element::NAME, Value::str(part)).unwrap();
            st.push(c, *prop, Value::Ref(o)).unwrap();
        }
        c
    };
    let party = class(
        &mut st,
        "Party",
        &[
            ("partyId", archforge_uml::PROPERTY, archforge_uml::class::OWNED_ATTRIBUTE),
            ("rename", archforge_uml::OPERATION, archforge_uml::class::OWNED_OPERATION),
        ],
    );
    let customer = class(&mut st, "Customer", &[("since", archforge_uml::PROPERTY, archforge_uml::class::OWNED_ATTRIBUTE)]);
    let g = st.create(archforge_uml::GENERALIZATION).unwrap();
    st.set(g, archforge_uml::generalization::GENERAL, Value::Ref(party)).unwrap();
    st.push(customer, archforge_uml::classifier::GENERALIZATION, Value::Ref(g)).unwrap();
    let uuid = |st: &archforge_mof::Store, o| st.uuid(o).unwrap().to_string();
    let (pkg_id, party_id, customer_id, g_id) = (uuid(&st, pkg), uuid(&st, party), uuid(&st, customer), uuid(&st, g));
    let mut s = archforge_engine::Session::with_view(
        archforge_notations::registry().unwrap(),
        st,
        1,
        archforge_engine::Options::default(),
        archforge_diagram::ProjectView::new(SKETCHED),
    );
    let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
    let d = id(s.execute("diagram.add", &serde_json::json!({ "kind": SKETCH, "element": pkg_id })).unwrap().value);
    let a = id(s
        .execute("diagram.place", &serde_json::json!({ "diagram": d, "element": party_id, "x": 100, "y": 100, "width": 240, "height": 166 }))
        .unwrap()
        .value);
    let b = id(s
        .execute("diagram.place", &serde_json::json!({ "diagram": d, "element": customer_id, "x": 600, "y": 100, "width": 240, "height": 166 }))
        .unwrap()
        .value);
    let e = id(s.execute("diagram.connect", &serde_json::json!({ "diagram": d, "element": g_id, "source": b, "target": a })).unwrap().value);
    h.state_mut().session = Some(s);
    h.state_mut().kinds = SKETCH_KINDS;
    h.state_mut().notation_uis = SKETCH_UIS;
    crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&d).unwrap());
    h.run();
    (h, d, a, b, e)
}

#[test]
fn a_second_notations_cards_and_lines_draw_route_arrange_and_take_their_look_on_the_same_canvas() {
    let (mut h, d, a, b, e) = sketch_harness();
    // The cards and the line, drawn through the one canvas from the sketch's descriptions.
    let texts = painted(&h);
    for t in ["CLASS", "Party", "partyId", "rename", "Customer", "since", "specific"] {
        assert!(texts.iter().any(|x| x == t), "{t} is drawn: {texts:?}");
    }
    let found = ids(&h);
    assert_eq!(found.iter().filter(|i| i.starts_with("workspace.canvas.node.")).count(), 2);
    assert!(found.iter().any(|i| *i == format!("workspace.canvas.edge.{e}")));
    assert_eq!(h.state().canvas.ends.len(), 1, "the specific end's text, which the user may move");
    // Routed: at right angles around nothing, as a new diagram's lines run.
    let (dobj, eobj, aobj, bobj) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let o = |x: &str| st.by_uuid(archforge_mof::Uuid::parse(x).unwrap()).unwrap();
        (o(&d), o(&e), o(&a), o(&b))
    };
    let route = archforge_diagram::Grid::build(h.state().session.as_ref().unwrap().doc(), dobj).line(eobj).unwrap().to_vec();
    assert!(route.len() >= 2, "{route:?}");
    // Arranged: Customer, where the line starts, above Party.
    let at = rect_of(&h, "workspace.tools.arrange").center();
    click(&mut h, at);
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let (top, bottom) = (archforge_diagram::grid::bounds_of(st, bobj), archforge_diagram::grid::bounds_of(st, aobj));
    assert!(top.y + top.height <= bottom.y, "Customer above Party: {top:?} {bottom:?}");
    // Its look: a fill of its own on Party's card.
    h.state_mut().session.as_mut().unwrap().execute("diagram.style", &serde_json::json!({ "ids": [a], "fill": "#A0C4FF" })).unwrap();
    h.run();
    assert_eq!(look(&h, aobj).fill, crate::canvas::colour("#A0C4FF"));
    let filled = h.output().shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Rect(r) if Some(r.fill) == crate::canvas::colour("#A0C4FF")));
    assert!(filled, "the card is filled in its own colour");
}

/// What the sketch kind's hooks were asked, in order.
static SKETCH_CALLS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// The sketch diagram kind in the shell, for the tests alone: a Class tool that places and an
/// Inherit tool that connects, each hook writing down what it was asked.
struct SketchKind;

static SKETCH_TOOLS: &[crate::kinds::KindTool] = &[
    crate::kinds::KindTool { key: "class", icon: &crate::ux::icons::TOOL_ENTITY, hint: "Entity", use_: crate::kinds::ToolUse::Place, chevron: None },
    crate::kinds::KindTool {
        key: "inherit",
        icon: &crate::ux::icons::TOOL_RELATIONSHIP,
        hint: "Relationship",
        use_: crate::kinds::ToolUse::Connect,
        chevron: None,
    },
];

impl crate::kinds::DiagramKind for SketchKind {
    fn kind(&self) -> &'static str {
        SKETCH
    }
    fn tools(&self) -> &'static [crate::kinds::KindTool] {
        SKETCH_TOOLS
    }
    fn word(&self) -> String {
        "Sketch".into()
    }
    fn owner_word(&self) -> String {
        "Package".into()
    }
    fn new_owner(&self) -> String {
        "New package".into()
    }
    fn owners(&self, store: &archforge_mof::Store) -> Vec<archforge_mof::Obj> {
        archforge_uml::commands::packages(store)
    }
    fn owner_of(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<archforge_mof::Obj> {
        None
    }
    fn add_owner(&self, _: &mut Shell, _: &str) -> Result<String, String> {
        Err("the sketch makes no package".into())
    }
    fn takes_part(&self, store: &archforge_mof::Store, element: archforge_mof::Obj) -> bool {
        store.class_of(element).ok() == Some(archforge_uml::CLASS)
    }
    fn place(&self, _: &mut Shell, tool: &'static str, _: archforge_mof::Obj, _: f64, _: f64) {
        SKETCH_CALLS.lock().unwrap().push(format!("place {tool}"));
    }
    fn connect(&self, shell: &mut Shell, tool: &'static str, _: archforge_mof::Obj, from: archforge_mof::Uuid, to: archforge_mof::Uuid) {
        let name = |id| {
            shell.session.as_ref().and_then(|s| {
                let st = s.doc();
                let e = st.get(st.by_uuid(id)?, archforge_diagram::node::ELEMENT).ok()?.as_ref()?;
                Some(archforge_core::name_of(st, e))
            })
        };
        SKETCH_CALLS.lock().unwrap().push(format!("connect {tool} {} {}", name(from).unwrap_or_default(), name(to).unwrap_or_default()));
    }
    fn connect_to_new(&self, _: &mut Shell, tool: &'static str, _: archforge_mof::Obj, _: archforge_mof::Uuid, _: f64, _: f64) {
        SKETCH_CALLS.lock().unwrap().push(format!("connect to new {tool}"));
    }
    fn rename(&self, _: &mut Shell, _: archforge_mof::Uuid, name: &str) -> Result<(), String> {
        SKETCH_CALLS.lock().unwrap().push(format!("rename {name}"));
        Ok(())
    }
    fn open(&self, _: &mut Shell, _: archforge_mof::Uuid, row: Option<usize>) {
        SKETCH_CALLS.lock().unwrap().push(format!("open {row:?}"));
    }
}

static SKETCH_KINDS: &[&dyn crate::kinds::DiagramKind] = &[&crate::er_kind::ErKind, &SketchKind];

#[test]
fn a_second_kinds_toolbar_shows_its_own_tools_and_its_place_connect_and_rename_hooks_run() {
    let (mut h, _d, a, b, _e) = sketch_harness();
    SKETCH_CALLS.lock().unwrap().clear();
    // ER's tool, left on from an ER diagram, is not the sketch's: the toolbar hands back to Select.
    h.state_mut().canvas.tool = crate::canvas::Tool::Place("entity");
    h.run();
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select);
    let found = ids(&h);
    for shared in ["select", "pan", "class", "inherit", "note", "link", "grid", "arrange"] {
        assert!(found.iter().any(|i| *i == format!("workspace.tools.{shared}")), "{shared} is on the toolbar: {found:?}");
    }
    for er in ["entity", "relationship", "relationship-types", "key-status"] {
        assert!(!found.iter().any(|i| *i == format!("workspace.tools.{er}")), "ER's {er} is not");
    }
    // The toolbar keeps its order: Select, Pan, the kind's tools, Note, Link note, Grid, Arrange.
    let left = |key: &str| rect_of(&h, &format!("workspace.tools.{key}")).left();
    let xs: Vec<f32> = ["select", "pan", "class", "inherit", "note", "link", "grid", "arrange"].iter().map(|k| left(k)).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
    // Class places where the canvas is clicked.
    let at = rect_of(&h, "workspace.tools.class").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Place("class"));
    let canvas = rect_of(&h, "workspace.canvas");
    click(&mut h, canvas.left_top() + egui::vec2(40.0, 40.0));
    // Inherit connects Customer to Party, and dropped on empty canvas asks for a new class.
    let at = rect_of(&h, "workspace.tools.inherit").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Connect("inherit"));
    let (party, customer) = (rect_of(&h, &format!("workspace.canvas.node.{a}")), rect_of(&h, &format!("workspace.canvas.node.{b}")));
    drag(&mut h, customer.center(), party.center());
    let at = rect_of(&h, "workspace.tools.inherit").center();
    click(&mut h, at);
    drag(&mut h, customer.center(), egui::pos2(customer.center().x, canvas.bottom() - 120.0));
    // A double click on Party's header names it in place, and Enter hands the name to the kind.
    for _ in 0..45 {
        h.step();
    }
    double_click(&mut h, party.left_top() + egui::vec2(60.0, 30.0));
    assert!(h.state().canvas.rename.as_ref().is_some_and(|r| r.text == "Party"), "{:?}", h.state().canvas.rename);
    h.state_mut().canvas.rename.as_mut().unwrap().text = "Person".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().canvas.rename.is_none());
    // A double click on a row opens the kind's editor there.
    for _ in 0..45 {
        h.step();
    }
    double_click(&mut h, party.left_top() + egui::vec2(100.0, 77.0 + 6.0));
    assert_eq!(
        *SKETCH_CALLS.lock().unwrap(),
        ["place class", "connect inherit Customer Party", "connect to new inherit", "rename Person", "open Some(0)"]
    );
}

/// The sketch notation in the shell's explorers, search and panels, for the tests alone: a package
/// at the top of the Models tree with its classes under it, each class found by the search as a
/// Class of its package, and a class's Properties a section of its attributes.
struct SketchUi;

fn sketch_classes(store: &archforge_mof::Store, pkg: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
    store
        .list(pkg, archforge_uml::package::PACKAGED_ELEMENT)
        .unwrap_or_default()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter(|o| store.class_of(*o).ok() == Some(archforge_uml::CLASS))
        .collect()
}

impl crate::notation_ui::NotationUi for SketchUi {
    fn metamodel(&self) -> u16 {
        archforge_uml::INDEX
    }
    fn is_top(&self, store: &archforge_mof::Store, member: archforge_mof::Obj) -> bool {
        store.class_of(member).ok() == Some(archforge_uml::PACKAGE)
    }
    fn tree(&self, store: &archforge_mof::Store, pkg: archforge_mof::Obj, t: &crate::explorer::Tree<'_>) -> Vec<crate::explorer::Row> {
        let classes = sketch_classes(store, pkg);
        let under: Vec<crate::explorer::Row> = classes
            .iter()
            .filter_map(|c| {
                let label = archforge_core::name_of(store, *c);
                if t.matches(&label) { t.row(*c, 1, "class", &crate::ux::icons::TABLE, label) } else { None }
            })
            .collect();
        let expandable = !under.is_empty();
        let expanded = expandable && t.open(pkg);
        let mut out: Vec<crate::explorer::Row> = t
            .row(pkg, 0, "package", &crate::ux::icons::SCHEMA, archforge_core::name_of(store, pkg))
            .map(|r| crate::explorer::Row { expandable, expanded, ..r })
            .into_iter()
            .collect();
        if expanded {
            out.extend(under);
        }
        out
    }
    fn offers(&self, _: &archforge_mof::Store, _: Option<archforge_mof::Obj>) -> Vec<crate::notation_ui::Offer> {
        Vec::new()
    }
    fn add(&self, _: &mut Shell, _: &str, _: Option<archforge_mof::Obj>) -> Option<Result<crate::notation_ui::Added, String>> {
        None
    }
    fn rename(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<(&'static str, &'static str)> {
        None
    }
    fn foldable(&self, _: &archforge_mof::Store, pkg: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
        vec![pkg]
    }
    fn parents(&self, store: &archforge_mof::Store, o: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
        store.container(o).ok().flatten().map(|(c, _)| c).into_iter().collect()
    }
    fn entry(&self, store: &archforge_mof::Store, o: archforge_mof::Obj) -> Option<crate::notation_ui::Entry> {
        (store.class_of(o).ok()? == archforge_uml::CLASS).then(|| {
            let pkg = store.container(o).ok().flatten().map(|(c, _)| archforge_core::name_of(store, c)).unwrap_or_default();
            crate::notation_ui::Entry {
                icon: &crate::ux::icons::TABLE,
                order: 2,
                label: archforge_core::name_of(store, o),
                detail: format!("Class · {pkg}"),
            }
        })
    }
    fn words(&self, store: &archforge_mof::Store, o: archforge_mof::Obj) -> Option<crate::notation_ui::Words> {
        Some(crate::notation_ui::Words {
            kind: "Class · sketch".into(),
            word: "Class".into(),
            this: "This class".into(),
            on_this: "On this class · {n}".into(),
            name: archforge_core::name_of(store, o),
        })
    }
    fn body(&self, shell: &mut Shell, ui: &mut egui::Ui, p: &crate::ux::Palette, r: egui::Rect, y: f32, subject: &crate::panels::Subject) {
        let attributes = shell
            .session
            .as_ref()
            .and_then(|s| {
                let store = s.doc();
                Some(
                    store
                        .list(subject.obj?, archforge_uml::class::OWNED_ATTRIBUTE)
                        .ok()?
                        .iter()
                        .filter_map(archforge_mof::Value::as_ref)
                        .map(|a| archforge_core::name_of(store, a))
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        let y = crate::panels::section(shell, ui, p, r, y, "Attributes", None).unwrap_or(y + 50.0);
        for (i, a) in attributes.iter().enumerate() {
            let row = egui::Rect::from_min_size(egui::pos2(r.left() + 16.0, y + i as f32 * 28.0), egui::vec2(r.width() - 32.0, 24.0));
            crate::home::control(shell, ui, &format!("workspace.properties.sketch.attribute.{i}"), "row", row, true);
            crate::ux::text(&ui.painter().clone(), row.min, a, crate::ux::types::UI, p.text, None);
        }
    }
}

static SKETCH_UIS: &[&dyn crate::notation_ui::NotationUi] = &[&crate::er_ui::ErUi, &SketchUi];

#[test]
fn a_second_notations_elements_show_in_the_tree_are_found_and_get_their_own_properties() {
    let (mut h, ..) = sketch_harness();
    let st = h.state().session.as_ref().unwrap().doc();
    let o = |name: &str| st.alive().find(|o| archforge_core::name_of(st, *o) == name).unwrap();
    let (pkg, party) = (o("sales"), o("Party"));
    let (pkg_id, party_id) = (st.uuid(pkg).unwrap(), st.uuid(party).unwrap());
    // The tree: the package at the top, its classes under it once it is open.
    let rows = |h: &Harness<'static, Shell>| {
        crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer)
            .into_iter()
            .map(|r| (r.depth, r.label))
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(&h), [(0, "sales".to_string())]);
    h.state_mut().explorer.expanded.insert(pkg_id);
    h.run();
    assert_eq!(rows(&h), [(0, "sales".to_string()), (1, "Party".to_string()), (1, "Customer".to_string())]);
    assert!(ids(&h).iter().any(|i| *i == format!("workspace.explorer.row.{party_id}")), "the class has a row");
    // The search finds a class as its notation lists it.
    let index = crate::project_search::index(h.state().kinds, h.state().notation_uis, h.state().session.as_ref().unwrap().doc());
    let (_, found) = crate::project_search::hits(&index, "part");
    assert!(found.iter().any(|e| e.label == "Party" && e.detail == "Class · sales"), "{:?}", found.iter().map(|e| &e.label).collect::<Vec<_>>());
    // Properties: the class's own words and its notation's section.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [party_id.to_string()] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "Class · sketch"), "{texts:?}");
    assert!(ids(&h).iter().any(|i| i == "workspace.properties.sketch.attribute.0"), "the class's attributes section");
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

#[test]
fn a_new_project_makes_a_class_diagram_in_three_steps_and_its_class_tool_places_a_class_named_in_place() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    // 1. The tab strip's + opens New diagram; 2. the kind is Class diagram; 3. Create.
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.kind").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.kind.1").center();
    click(&mut h, at);
    let edit = h.state().diagrams.new.clone().expect("the dialog is open");
    assert_eq!(edit.kind, archforge_uml::CLASS_DIAGRAM);
    assert_eq!(edit.schema, None, "a project without a package gets one");
    h.state_mut().diagrams.new.as_mut().unwrap().name = "Domain".into();
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert!(h.state().diagrams.new.is_none(), "{:?}", h.state().diagrams.new);
    assert_eq!(class_diagram_shown(&h), (archforge_uml::CLASS_DIAGRAM.to_string(), Vec::new()), "an empty class diagram is open");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(archforge_uml::commands::packages(st).iter().map(|p| archforge_core::name_of(st, *p)).collect::<Vec<_>>(), ["Domain"]);
    // Its toolbar: Class, Relationship with its kinds, Note and Link note; ER's tools and its key
    // status are not there.
    let found = ids(&h);
    for tool in ["select", "pan", "class", "relationship", "relationship-kinds", "note", "link", "grid", "arrange"] {
        assert!(found.iter().any(|i| *i == format!("workspace.tools.{tool}")), "{tool}");
    }
    for er in ["entity", "relationship-types", "key-status"] {
        assert!(!found.iter().any(|i| *i == format!("workspace.tools.{er}")), "{er}");
    }
    // A click in Select adds nothing; with the Class tool it places a class, named in place.
    let canvas = rect_of(&h, "workspace.canvas");
    click(&mut h, canvas.center());
    assert!(class_diagram_shown(&h).1.is_empty(), "Select adds nothing");
    let at = rect_of(&h, "workspace.tools.class").center();
    click(&mut h, at);
    click(&mut h, canvas.center());
    assert_eq!(class_diagram_shown(&h).1, ["Class 1"]);
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select, "the tool hands back to Select");
    let rename = h.state().canvas.rename.clone().expect("the class's name is open for typing");
    assert_eq!(rename.text, "Class 1");
    h.state_mut().canvas.rename.as_mut().unwrap().text = "Order".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(class_diagram_shown(&h).1, ["Order"]);
    // The tree shows the package with its class; the search finds it.
    let st = h.state().session.as_ref().unwrap().doc();
    let pkg = st.uuid(archforge_uml::commands::packages(st)[0]).unwrap();
    h.state_mut().explorer.expanded.insert(pkg);
    let rows: Vec<String> = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer)
        .into_iter()
        .map(|r| r.label)
        .collect();
    assert_eq!(rows, ["Domain", "Order"]);
    let index = crate::project_search::index(h.state().kinds, h.state().notation_uis, h.state().session.as_ref().unwrap().doc());
    assert!(crate::project_search::hits(&index, "ord").1.iter().any(|e| e.label == "Order"));
    // Saved and opened again, as the project's rows: the class diagram comes back.
    let (tables, junctions) = archforge_notations::storage();
    let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
    let back = archforge_notations::session_from_rows(&rows, Box::new(|| archforge_mof::Uuid::v7(1_761_670_900_000, [9; 10]))).unwrap();
    let st = back.doc();
    let d = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).unwrap();
    assert_eq!(st.get(d, archforge_diagram::diagram::KIND).unwrap().as_str(), Some(archforge_uml::CLASS_DIAGRAM));
    assert_eq!(st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), 1);
}

#[test]
fn an_er_project_makes_a_class_diagram_beside_its_er_diagrams_and_an_agent_makes_one_too() {
    let (mut h, _customer, er_diagram) = diagram_harness();
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    assert_eq!(h.state().diagrams.new.as_ref().unwrap().kind, archforge_er::DIAGRAM_KIND, "an ER project starts at ER");
    let at = rect_of(&h, "workspace.new-diagram.kind").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.new-diagram.kind.1").center();
    click(&mut h, at);
    h.state_mut().diagrams.new.as_mut().unwrap().name = "Domain".into();
    let at = rect_of(&h, "workspace.new-diagram.create").center();
    click(&mut h, at);
    assert_eq!(class_diagram_shown(&h).0, archforge_uml::CLASS_DIAGRAM);
    assert_eq!(h.state().canvas.tabs.len(), 2, "beside the ER diagram's tab");
    assert!(h.state().canvas.tabs.contains(&archforge_mof::Uuid::parse(&er_diagram).unwrap()));
    // An agent, through the control channel's commands, as its MCP tools call them.
    let mut call = |command: &str, params: serde_json::Value| {
        let out = crate::control::answer(h.state_mut(), "engine.execute", &serde_json::json!({ "command": command, "params": params }))
            .unwrap_or_else(|e| panic!("{command}: {e}"));
        out["result"]["id"].as_str().unwrap_or_default().to_string()
    };
    let pkg = call("uml.package.add", serde_json::json!({ "name": "Billing" }));
    let d = call("diagram.add", serde_json::json!({ "kind": "uml.class", "element": pkg, "name": "Billing" }));
    let invoice = call("uml.class.add", serde_json::json!({ "package": pkg, "name": "Invoice" }));
    call("diagram.place", serde_json::json!({ "diagram": d, "element": invoice, "x": 100, "y": 100 }));
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(archforge_mof::Uuid::parse(&d).unwrap()).expect("the agent's class diagram");
    assert_eq!(st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), 1);
}

/// The sample class model open on its class diagram, and the edge of its association `name`.
fn read_class_diagram(name: &str) -> (Harness<'static, Shell>, archforge_mof::Obj, archforge_mof::Obj) {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    crate::uml_ui::open_sample(h.state_mut()).unwrap();
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    let a = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == name).unwrap();
    let e = archforge_diagram::views_of(st, a).1[0];
    (h, d, e)
}

/// Whether the last frame painted a path in the accent colour.
fn painted_accent(h: &Harness<'static, Shell>) -> bool {
    let accent = crate::ux::palette(false).accent;
    fn walk(s: &egui::Shape, accent: egui::Color32) -> bool {
        match s {
            egui::Shape::Path(p) => p.stroke.color == egui::epaint::ColorMode::Solid(accent),
            egui::Shape::Vec(v) => v.iter().any(|s| walk(s, accent)),
            _ => false,
        }
    }
    h.output().shapes.iter().any(|c| walk(&c.shape, accent))
}

#[test]
fn a_class_diagram_draws_every_line_kind_and_its_lines_highlight_and_their_texts_and_segments_drag() {
    let (mut h, d, e) = read_class_diagram("paid by");
    let st = h.state().session.as_ref().unwrap().doc();
    // Every line kind is on the diagram, each drawn as its notation describes it.
    let lines: Vec<archforge_diagram::figure::Line> = st
        .list(d, archforge_diagram::diagram::EDGE)
        .unwrap()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter_map(|edge| st.get(edge, archforge_diagram::edge::ELEMENT).ok()?.as_ref())
        .filter_map(|el| archforge_uml::figures::line(st, el))
        .collect();
    use archforge_diagram::figure::EndMark as M;
    let marks: Vec<[M; 2]> = lines.iter().map(|l| [l.ends[0].mark, l.ends[1].mark]).collect();
    for kind in [M::OpenArrow, M::FilledDiamond, M::HollowDiamond, M::Triangle] {
        assert!(marks.iter().any(|m| m.contains(&kind)), "{kind:?} in {marks:?}");
    }
    assert!(marks.contains(&[M::OpenArrow, M::OpenArrow]), "navigable both ways: an arrow at each end");
    assert!(lines.iter().any(|l| l.dashed), "the dependency is dashed");
    let texts = painted(&h);
    for t in ["CLASS", "Payment", "+ amount : Money", "+ authorise() : Boolean", "+ price : Money [0..1]", "paid by", "1..*", "0..1", "lines"] {
        assert!(texts.iter().any(|x| x == t), "{t} is drawn: {texts:?}");
    }
    // The hovered line turns to the accent.
    assert!(!painted_accent(&h), "nothing is selected or hovered yet");
    let route = |h: &Harness<'static, Shell>| archforge_diagram::Grid::build(h.state().session.as_ref().unwrap().doc(), d).line(e).unwrap().to_vec();
    let before = route(&h);
    assert!(before.len() >= 3, "a right-angled route: {before:?}");
    let view = crate::canvas::View { origin: rect_of(&h, "workspace.canvas").min, pan: h.state().canvas.pan, zoom: h.state().canvas.zoom };
    let middle = view.to_screen((before[1].0 + before[2].0) / 2.0, (before[1].1 + before[2].1) / 2.0);
    h.event(egui::Event::PointerMoved(middle));
    h.run();
    assert!(painted_accent(&h), "the hovered line is highlighted");
    // A multiplicity text drags aside and stays where it was let go.
    let edge = h.state().session.as_ref().unwrap().doc().uuid(e).unwrap();
    let text = h.state().canvas.ends.iter().find(|t| t.edge == edge && t.end == 1).cloned().expect("the line's end text is drawn");
    let to = text.rect.center() + egui::vec2(30.0, 30.0);
    drag(&mut h, text.rect.center(), to);
    h.run();
    let moved = h.state().canvas.ends.iter().find(|t| t.edge == edge && t.end == 1).cloned().unwrap();
    assert!((moved.rect.center() - to).length() < 2.0, "drawn where it was let go: {:?} {to:?}", moved.rect.center());
    // A segment slides across.
    let level = (before[1].1 - before[2].1).abs() < 1e-6;
    let shift = if level { egui::vec2(0.0, -40.0) } else { egui::vec2(-40.0, 0.0) };
    let middle = view.to_screen((before[1].0 + before[2].0) / 2.0, (before[1].1 + before[2].1) / 2.0);
    drag(&mut h, middle, middle + shift * view.zoom);
    let after = route(&h);
    let slid = if level { (after[1].1 - (before[1].1 - 40.0)).abs() < 1.0 } else { (after[1].0 - (before[1].0 - 40.0)).abs() < 1.0 };
    assert!(slid, "{before:?} → {after:?}");
}

/// The sample class model open with `class` selected and Properties showing it; gives its id.
fn class_properties(class: &str) -> (Harness<'static, Shell>, archforge_mof::Uuid) {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    crate::uml_ui::open_sample(h.state_mut()).unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == class).unwrap();
    let id = st.uuid(c).unwrap();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    (h, id)
}

/// A class's attributes or operations as its card lists them.
fn member_rows(h: &Harness<'static, Shell>, class: archforge_mof::Uuid, operations: bool) -> Vec<String> {
    let st = h.state().session.as_ref().unwrap().doc();
    let c = st.by_uuid(class).unwrap();
    if operations {
        archforge_uml::members::operations(st, c).into_iter().map(|o| archforge_uml::figures::operation_text(st, o)).collect()
    } else {
        archforge_uml::members::attributes(st, c).into_iter().map(|a| archforge_uml::figures::attribute_text(st, a)).collect()
    }
}

/// Picks the type labelled `label` from the open editor's type select `id`.
fn pick_type(h: &mut Harness<'static, Shell>, id: &str, label: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    let i = crate::uml_panel::type_choices(h.state().session.as_ref().unwrap().doc()).iter().position(|(l, _)| l == label).expect(label);
    let at = rect_of(h, &format!("{id}.{i}")).center();
    click(h, at);
}

/// Clicks the field `id` and types `text` into it.
fn type_into(h: &mut Harness<'static, Shell>, id: &str, text: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    h.event(egui::Event::Text(text.into()));
    h.run();
}

#[test]
fn a_class_takes_attributes_in_properties_typed_checked_moved_and_removed_each_one_undo_step() {
    let (mut h, customer) = class_properties("Customer");
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "Attributes · 2") && texts.iter().any(|t| t == "Operations · 1"), "{texts:?}");
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- email : String"]);
    // + opens the editor with the name taking the keyboard; a built-in type makes the Types package.
    let at = rect_of(&h, "workspace.properties.attributes.add").center();
    click(&mut h, at);
    assert!(h.state().panels.member.as_ref().is_some_and(|e| e.target.is_none()), "the editor is open for a new attribute");
    h.event(egui::Event::Text("rating".into()));
    h.run();
    pick_type(&mut h, "workspace.properties.member.type", "Real");
    type_into(&mut h, "workspace.properties.member.multiplicity", "0..1");
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.member.is_none(), "{:?}", h.state().panels.member);
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- email : String", "+ rating : Real [0..1]"]);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(
        archforge_uml::commands::packages(st).iter().any(|p| archforge_core::name_of(st, *p) == archforge_uml::members::TYPES),
        "Real is kept in Types"
    );
    h.run();
    assert!(painted(&h).iter().any(|t| t == "+ rating : Real [0..1]"), "the card and the row show it");
    // One undo step takes it back, redo brings it again.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(member_rows(&h, customer, false).len(), 2);
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!(member_rows(&h, customer, false).len(), 3);
    // A click on a row selects the attribute; Properties stays the class's, the row marked.
    let st = h.state().session.as_ref().unwrap().doc();
    let rating = st.uuid(archforge_uml::members::attributes(st, st.by_uuid(customer).unwrap())[2]).unwrap();
    let row = format!("workspace.properties.attribute.{rating}");
    let at = rect_of(&h, &row).left_center() + egui::vec2(20.0, 0.0);
    click(&mut h, at);
    assert_eq!(crate::panels::selected_id(h.state()), Some(rating));
    assert!(painted(&h).iter().any(|t| t == "Attributes · 3"), "the class's panel shows for its attribute");
    // Its edit button opens the editor in its place; a multiplicity that does not read is refused.
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    assert_eq!(h.state().panels.member.as_ref().and_then(|e| e.target), Some(rating));
    assert_eq!(h.state().panels.member.as_ref().unwrap().multiplicity, "0..1");
    h.state_mut().panels.member.as_mut().unwrap().multiplicity = "3..1".into();
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    let refused = h.state().panels.member.as_ref().and_then(|e| e.refused.clone()).expect("refused");
    assert!(refused.contains("multiplicity"), "{refused}");
    assert_eq!(member_rows(&h, customer, false)[2], "+ rating : Real [0..1]", "nothing changed");
    // Private and static, applied.
    h.state_mut().panels.member.as_mut().unwrap().multiplicity = "1..*".into();
    let at = rect_of(&h, "workspace.properties.member.visibility").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.visibility.1").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.static").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, customer, false)[2], "- rating : Real [1..*]");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(st.by_uuid(rating).unwrap(), archforge_uml::feature::IS_STATIC).unwrap().as_bool(), Some(true));
    // Moved up past email, then removed; undo brings it back where it was.
    let at = rect_of(&h, &format!("{row}.edit")).center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.up").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- rating : Real [1..*]", "- email : String"]);
    let at = rect_of(&h, "workspace.properties.member.remove").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- email : String"]);
    assert!(h.state().panels.member.is_none());
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(member_rows(&h, customer, false), ["+ id : Integer", "- rating : Real [1..*]", "- email : String"]);
}

#[test]
fn a_class_takes_operations_with_parameters_and_turns_abstract_in_properties() {
    let (mut h, payment) = class_properties("CardPayment");
    // The class turns abstract: its name slants on its card.
    let at = rect_of(&h, "workspace.properties.class.abstract").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.get(st.by_uuid(payment).unwrap(), archforge_uml::class::IS_ABSTRACT).unwrap().as_bool(), Some(true));
    // An operation with a parameter typed by a class of the model and a return type.
    let at = rect_of(&h, "workspace.properties.operations.add").center();
    click(&mut h, at);
    h.event(egui::Event::Text("refund".into()));
    h.run();
    pick_type(&mut h, "workspace.properties.member.type", "Boolean");
    let at = rect_of(&h, "workspace.properties.member.parameter.add").center();
    click(&mut h, at);
    type_into(&mut h, "workspace.properties.member.parameter.0.name", "to");
    pick_type(&mut h, "workspace.properties.member.parameter.0.type", "Customer");
    let at = rect_of(&h, "workspace.properties.member.parameter.add").center();
    click(&mut h, at);
    type_into(&mut h, "workspace.properties.member.parameter.1.name", "times");
    type_into(&mut h, "workspace.properties.member.parameter.1.multiplicity", "0..*");
    let at = rect_of(&h, "workspace.properties.member.abstract").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert!(h.state().panels.member.is_none(), "{:?}", h.state().panels.member);
    assert_eq!(member_rows(&h, payment, true), ["+ refund(to : Customer, times) : Boolean"]);
    let st = h.state().session.as_ref().unwrap().doc();
    let op = archforge_uml::members::operations(st, st.by_uuid(payment).unwrap())[0];
    assert_eq!(st.get(op, archforge_uml::behavioral_feature::IS_ABSTRACT).unwrap().as_bool(), Some(true));
    assert_eq!(archforge_uml::figures::multiplicity(st, archforge_uml::members::parameters(st, op).0[1]), "*");
    // Opened again it holds what it was given; a parameter's ✕ drops it.
    let op_id = st.uuid(op).unwrap();
    let at = rect_of(&h, &format!("workspace.properties.operation.{op_id}.edit")).center();
    click(&mut h, at);
    let edit = h.state().panels.member.clone().unwrap();
    assert_eq!((edit.name.as_str(), edit.params.len(), edit.is_abstract), ("refund", 2, true));
    // The model's own Boolean, by its id, rather than a built-in one.
    let st = h.state().session.as_ref().unwrap().doc();
    let ty = st.by_uuid(archforge_mof::Uuid::parse(&edit.ty).expect("an id")).unwrap();
    assert_eq!(archforge_core::name_of(st, ty), "Boolean");
    let at = rect_of(&h, "workspace.properties.member.parameter.1.remove").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.member.apply").center();
    click(&mut h, at);
    assert_eq!(member_rows(&h, payment, true), ["+ refund(to : Customer) : Boolean"]);
}

#[test]
fn a_new_class_shows_the_operations_added_to_it_and_a_card_made_short_says_what_it_hides() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let run = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
            let out = h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
            out.value["id"].as_str().map(|i| archforge_mof::Uuid::parse(i).unwrap())
        };
        let pkg = run(&mut h, "uml.package.add", serde_json::json!({ "name": "Billing" })).unwrap();
        let d = run(&mut h, "diagram.add", serde_json::json!({ "kind": "uml.class", "element": pkg.to_string(), "name": "Billing" })).unwrap();
        crate::canvas::open(h.state_mut(), d);
        let class = run(&mut h, "uml.class.add", serde_json::json!({ "package": pkg.to_string(), "name": "ReviewInvoice" })).unwrap();
        let node =
            run(&mut h, "diagram.place", serde_json::json!({ "diagram": d.to_string(), "element": class.to_string(), "x": 200, "y": 200 })).unwrap();
        let height = |h: &Harness<'static, Shell>| {
            let st = h.state().session.as_ref().unwrap().doc();
            archforge_diagram::grid::bounds_of(st, st.by_uuid(node).unwrap()).height
        };
        assert_eq!(height(&h), archforge_uml::CLASS_SIZE.1, "a new class is as tall as a row in each compartment");
        // The same class on two more diagrams: wider and short, and already tall.
        let mut others = Vec::new();
        for (name, width, tall) in [("Wide", 300, 140), ("Tall", 260, 400)] {
            let other = run(&mut h, "diagram.add", serde_json::json!({ "kind": "uml.class", "element": pkg.to_string(), "name": name })).unwrap();
            let n = run(
                &mut h,
                "diagram.place",
                serde_json::json!({ "diagram": other.to_string(), "element": class.to_string(), "x": 0, "y": 0, "width": width, "height": tall }),
            )
            .unwrap();
            others.push(n);
        }
        let size = |h: &Harness<'static, Shell>, n: archforge_mof::Uuid| {
            let st = h.state().session.as_ref().unwrap().doc();
            let b = archforge_diagram::grid::bounds_of(st, st.by_uuid(n).unwrap());
            (b.width, b.height)
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [class.to_string()] })).unwrap();
        h.state_mut().state.show_panel(crate::state::Panel::Properties);
        h.run();
        // The first operation, added in Properties, shows on the card at its default size.
        let add_operation = |h: &mut Harness<'static, Shell>, name: &str| {
            let at = rect_of(h, "workspace.properties.operations.add").center();
            click(h, at);
            h.event(egui::Event::Text(name.into()));
            h.run();
            let at = rect_of(h, "workspace.properties.member.apply").center();
            click(h, at);
            assert!(h.state().panels.member.is_none(), "{:?}", h.state().panels.member);
            h.run();
        };
        add_operation(&mut h, "calculateTotal");
        assert_eq!(height(&h), archforge_uml::CLASS_SIZE.1, "nothing grows while every row fits");
        // Each member is painted twice: on the card and as its row in Properties.
        let on_card = |h: &Harness<'static, Shell>, row: &str| painted(h).iter().filter(|t| *t == row).count() == 2;
        assert!(on_card(&h, "+ calculateTotal()"), "{theme:?}: {:?}", painted(&h));
        assert!(!painted(&h).iter().any(|t| t.contains("more member")));
        // A second one grows the card to show both, in the same undo step; its width stays.
        add_operation(&mut h, "approve");
        let grown = height(&h);
        assert!(grown > archforge_uml::CLASS_SIZE.1, "{grown}");
        assert!(on_card(&h, "+ calculateTotal()") && on_card(&h, "+ approve()"), "{:?}", painted(&h));
        // Every card of the class grows on every diagram, keeping its width; a taller one is left.
        assert_eq!((size(&h, others[0]), size(&h, others[1])), ((300.0, grown), (260.0, 400.0)));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        assert_eq!((member_rows(&h, class, true).len(), height(&h)), (1, archforge_uml::CLASS_SIZE.1), "one undo takes back both");
        assert_eq!((size(&h, others[0]), size(&h, others[1])), ((300.0, 140.0), (260.0, 400.0)), "and every card's height");
        h.state_mut().session.as_mut().unwrap().redo().unwrap();
        assert_eq!((member_rows(&h, class, true).len(), height(&h)), (2, grown));
        assert_eq!(size(&h, others[0]), (300.0, grown));
        // Made short by hand it stays so, and says what it hides rather than looking empty.
        run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": 120 }));
        h.run();
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "+ 2 more members"), "{theme:?}: {texts:?}");
        assert!(!on_card(&h, "+ calculateTotal()") && !on_card(&h, "+ approve()"), "{texts:?}");
        assert_eq!(height(&h), 120.0);
        // Shorter than a row's place under the header, it says so at its foot, then beside its eyebrow.
        for short in [90, 60] {
            run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": short }));
            h.run();
            assert!(painted(&h).iter().any(|t| t == "+ 2 more members"), "{short} tall: {:?}", painted(&h));
        }
        run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": 120 }));
        h.run();
        // Seen at a reduced zoom, the card still says it.
        let zoom = h.state().canvas.zoom;
        h.state_mut().canvas.zoom = 0.6;
        h.run();
        assert!(painted(&h).iter().any(|t| t == "+ 2 more members"), "at 60 %");
        h.state_mut().canvas.zoom = zoom;
        // With an attribute it shows, the hint takes an operation's place and counts the rest.
        run(&mut h, "uml.attribute.add", serde_json::json!({ "class": class.to_string(), "name": "number" }));
        run(&mut h, "diagram.resize", serde_json::json!({ "node": node.to_string(), "width": 240, "height": archforge_uml::CLASS_SIZE.1 }));
        h.run();
        let texts = painted(&h);
        assert!(on_card(&h, "+ number"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "+ 2 more members"), "{texts:?}");
        assert!(!on_card(&h, "+ calculateTotal()") && !on_card(&h, "+ approve()"), "{texts:?}");
        // Editing a member grows it again.
        let st = h.state().session.as_ref().unwrap().doc();
        let op = st.uuid(archforge_uml::members::operations(st, st.by_uuid(class).unwrap())[1]).unwrap();
        h.run();
        let at = rect_of(&h, &format!("workspace.properties.operation.{op}.edit")).center();
        click(&mut h, at);
        h.state_mut().panels.member.as_mut().unwrap().name = "approveAll".into();
        let at = rect_of(&h, "workspace.properties.member.apply").center();
        click(&mut h, at);
        h.run();
        assert_eq!(height(&h), grown);
        assert!(on_card(&h, "+ approveAll()"), "{:?}", painted(&h));
        // Saved and opened again from the project's rows, the card keeps its size.
        let (tables, junctions) = archforge_notations::storage();
        let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
        let back = archforge_notations::session_from_rows(&rows, Box::new(|| archforge_mof::Uuid::v7(1_761_670_900_000, [9; 10]))).unwrap();
        let st = back.doc();
        assert_eq!(archforge_diagram::grid::bounds_of(st, st.by_uuid(node).unwrap()).height, grown);
        assert_eq!(archforge_uml::members::operations(st, st.by_uuid(class).unwrap()).len(), 2);
    }
}

#[test]
fn a_table_box_shorter_than_its_columns_shows_those_that_fit_and_counts_the_rest() {
    let (mut h, customer, diagram) = diagram_harness();
    let s = h.state_mut().session.as_mut().unwrap();
    for name in ["name", "phone", "city"] {
        s.execute("er.attribute.add", &serde_json::json!({ "entity": customer, "name": name, "type": "TEXT" })).unwrap();
    }
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let (e, d) =
        (st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap(), st.by_uuid(archforge_mof::Uuid::parse(&diagram).unwrap()).unwrap());
    let rows: Vec<String> =
        archforge_diagram::Notation::card(&archforge_er::ErNotation, st, e, d).unwrap().compartments[0].iter().map(|r| r.text.clone()).collect();
    assert_eq!(rows.len(), 5);
    // Its box, 166 tall, holds four rows' places: three columns, then the two past it said in one.
    // A column is painted in the Models tree and, when it shows, in its box too.
    let texts = painted(&h);
    let times = |row: &String| texts.iter().filter(|t| *t == row).count();
    assert_eq!(rows.iter().map(times).collect::<Vec<_>>(), [2, 2, 2, 1, 1], "{texts:?}");
    assert!(texts.iter().any(|t| t == "+ 2 more columns"), "{texts:?}");
}

#[test]
fn a_selected_class_takes_attributes_and_operations_typed_on_its_card_each_one_undo_step() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    crate::uml_ui::open_sample(h.state_mut()).unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == "Catalog").unwrap();
    let catalog = st.uuid(c).unwrap();
    let select = |h: &mut Harness<'static, Shell>| {
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [catalog.to_string()] })).unwrap();
        h.run();
    };
    select(&mut h);
    let revision = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().revision();
    // The attributes' + opens a draft row with the name taking the keys; Tab moves to the type.
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    assert!(h.state().canvas.inline.as_ref().is_some_and(|i| i.edit.kind == crate::uml_panel::MemberKind::Attribute && i.edit.target.is_none()));
    h.event(egui::Event::Text("code".into()));
    h.run();
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    h.event(egui::Event::Text("Integer".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(member_rows(&h, catalog, false), ["+ name : String", "+ code : Integer"]);
    assert_eq!(h.state().session.as_ref().unwrap().undo_label(), Some("Add attribute"), "one undo step");
    let written = revision(&h);
    assert!(painted(&h).iter().any(|t| t == "+ code : Integer"), "the card shows it");
    // The next draft is open; a type the model does not have is refused beside it, nothing written.
    assert!(h.state().canvas.inline.as_ref().is_some_and(|i| i.edit.name.is_empty() && i.edit.target.is_none()));
    h.event(egui::Event::Text("rating".into()));
    h.run();
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    h.event(egui::Event::Text("Currency".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let refused = h.state().canvas.inline.as_ref().and_then(|i| i.refused.clone()).expect("refused");
    assert!(refused.contains("Currency"), "{refused}");
    assert!(painted(&h).contains(&refused), "said beside the row");
    assert_eq!(h.state().canvas.inline.as_ref().unwrap().edit.name, "rating", "the text stays");
    assert_eq!((member_rows(&h, catalog, false).len(), revision(&h)), (2, written), "nothing written");
    // Escape drops only the draft; undo takes back the member written.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().canvas.inline.is_none());
    assert_eq!(member_rows(&h, catalog, false).len(), 2);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(member_rows(&h, catalog, false), ["+ name : String"]);
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    // The operations' + and the type menu: the pick fills the return type.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-operation").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.inline.as_ref().map(|i| i.edit.kind), Some(crate::uml_panel::MemberKind::Operation));
    h.event(egui::Event::Text("reprice".into()));
    h.run();
    let at = rect_of(&h, "workspace.canvas.member.type.menu").center();
    click(&mut h, at);
    let real = crate::uml_panel::type_choices(h.state().session.as_ref().unwrap().doc()).iter().position(|(l, _)| l == "Real").unwrap();
    let at = rect_of(&h, &format!("workspace.canvas.member.type.{real}")).center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.inline.as_ref().unwrap().ty, "Real");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(member_rows(&h, catalog, true), ["+ reprice() : Real"]);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // A draft with a name is written when the pointer goes elsewhere; an empty one only goes.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    h.event(egui::Event::Text("notes".into()));
    h.run();
    let canvas = rect_of(&h, "workspace.canvas");
    click(&mut h, canvas.left_top() + egui::vec2(30.0, 30.0));
    assert!(h.state().canvas.inline.is_none());
    assert_eq!(member_rows(&h, catalog, false).last().map(String::as_str), Some("+ notes"));
    // So is one whose type menu was open when the pointer went elsewhere.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    h.event(egui::Event::Text("tags".into()));
    h.run();
    let at = rect_of(&h, "workspace.canvas.member.type.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::InlineType));
    click(&mut h, canvas.left_top() + egui::vec2(30.0, 30.0));
    assert!(h.state().canvas.inline.is_none(), "{:?}", h.state().canvas.inline);
    assert_eq!(member_rows(&h, catalog, false).last().map(String::as_str), Some("+ tags"));
    // ⋯ takes the draft to Properties' editor with what was typed.
    select(&mut h);
    let at = rect_of(&h, "workspace.canvas.class.add-attribute").center();
    click(&mut h, at);
    h.event(egui::Event::Text("reference".into()));
    h.run();
    let at = rect_of(&h, "workspace.canvas.member.details").center();
    click(&mut h, at);
    assert!(h.state().canvas.inline.is_none());
    let edit = h.state().panels.member.clone().expect("Properties' editor");
    assert_eq!((edit.name.as_str(), edit.target, edit.class), ("reference", None, catalog));
    h.state_mut().panels.member = None;
    // ⇧⌘↩ opens an operation's row from the keys.
    select(&mut h);
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Enter, egui::Modifiers { command: true, shift: true, ..Default::default() });
    assert_eq!(h.state().canvas.inline.as_ref().map(|i| i.edit.kind), Some(crate::uml_panel::MemberKind::Operation));
    // An agent removing the class drops the row.
    h.state_mut().session.as_mut().unwrap().execute("element.remove", &serde_json::json!({ "ids": [catalog.to_string()] })).unwrap();
    h.run();
    assert!(h.state().canvas.inline.is_none());
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    // Drawn too small for its rows, the card leaves the member to Properties' editor.
    h.state_mut().canvas.zoom = 0.4;
    select(&mut h);
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Enter, egui::Modifiers { command: true, ..Default::default() });
    assert!(h.state().canvas.inline.is_none());
    assert!(h.state().panels.member.as_ref().is_some_and(|e| e.class == catalog && e.target.is_none()));
}

#[test]
fn an_inline_row_keeps_a_type_by_what_it_is_refuses_a_name_two_types_share_and_keeps_moved_rows_in_sight() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    crate::uml_ui::open_sample(h.state_mut()).unwrap();
    let run = |h: &mut Harness<'static, Shell>, command: &str, params: serde_json::Value| {
        let out = h.state_mut().session.as_mut().unwrap().execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
        out.value["id"].as_str().map(|i| archforge_mof::Uuid::parse(i).unwrap())
    };
    let class = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap()).unwrap()
    };
    let catalog = class(&h, "Catalog");
    // A second Money, in another package; Catalog's fee is of that one.
    let billing = run(&mut h, "uml.package.add", serde_json::json!({ "name": "Billing" })).unwrap();
    let money = run(&mut h, "uml.class.add", serde_json::json!({ "package": billing.to_string(), "name": "Money" })).unwrap();
    let fee =
        run(&mut h, "uml.attribute.add", serde_json::json!({ "class": catalog.to_string(), "name": "fee", "type": money.to_string() })).unwrap();
    let type_of = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let t = st.get(st.by_uuid(fee).unwrap(), archforge_uml::typed_element::TYPE).unwrap().as_ref().unwrap();
        st.uuid(t).unwrap()
    };
    // Renamed in place, it keeps that Money.
    assert!(crate::uml_inline::open(h.state_mut(), catalog, crate::uml_panel::MemberKind::Attribute, Some(fee)));
    h.run();
    h.state_mut().canvas.inline.as_mut().unwrap().edit.name = "charge".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().canvas.inline.is_none(), "{:?}", h.state().canvas.inline);
    assert_eq!(type_of(&h), money, "the type stays the one it was");
    // Typing a name two types share is refused, the type unchanged.
    assert!(crate::uml_inline::open(h.state_mut(), catalog, crate::uml_panel::MemberKind::Attribute, Some(fee)));
    h.run();
    h.state_mut().canvas.inline.as_mut().unwrap().ty = "money".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let refused = h.state().canvas.inline.as_ref().and_then(|i| i.refused.clone()).expect("refused");
    assert!(refused.contains("money"), "{refused}");
    assert_eq!(type_of(&h), money);
    // A type no type has stays with the row when ⋯ is pressed, rather than going over as another.
    h.state_mut().canvas.inline.as_mut().unwrap().ty = "Currency".into();
    let at = rect_of(&h, "workspace.canvas.member.details").center();
    click(&mut h, at);
    assert!(h.state().panels.member.is_none(), "Properties' editor stays closed");
    assert!(h.state().canvas.inline.as_ref().and_then(|i| i.refused.clone()).is_some_and(|r| r.contains("Currency")));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // A draft at the end of Order's attributes moves its operation down, still drawn in the card.
    let order = class(&h, "Order");
    assert!(crate::uml_inline::open(h.state_mut(), order, crate::uml_panel::MemberKind::Attribute, None));
    h.run();
    let canvas = rect_of(&h, "workspace.canvas");
    fn texts(s: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Pos2, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos, clip)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, clip, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    h.output().shapes.iter().for_each(|c| texts(&c.shape, c.clip_rect, &mut found));
    let (_, pos, clip) = found.iter().find(|(t, pos, _)| t == "+ total() : Money" && canvas.contains(*pos)).expect("the operation drawn on the card");
    assert!(clip.contains(*pos + egui::vec2(1.0, 8.0)), "{pos:?} inside {clip:?}");
}

#[test]
fn each_rail_icon_opens_its_own_panel_in_both_appearances() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = class_properties("Order");
        h.state_mut().state.theme = theme;
        h.run();
        for (id, panel) in [
            ("agent", crate::state::Panel::Agent),
            ("appearance", crate::state::Panel::Appearance),
            ("skills", crate::state::Panel::Skills),
            ("properties", crate::state::Panel::Properties),
        ] {
            let at = rect_of(&h, &format!("workspace.panels.{id}")).center();
            click(&mut h, at);
            assert_eq!((h.state().state.panel, h.state().state.panel_collapsed), (panel, false), "{theme:?}: {id}");
        }
    }
    // Four shapes of their own (AR-61), none the drawing another control uses.
    let rail = [&crate::ux::icons::CHAT_BUBBLE, &crate::ux::icons::PALETTE, &crate::ux::icons::PROPERTIES, &crate::ux::icons::SKILLS];
    for (i, a) in rail.iter().enumerate() {
        for b in rail.iter().skip(i + 1) {
            assert!(!std::ptr::eq(*a, *b));
        }
    }
}

#[test]
fn a_class_dragged_from_models_onto_a_second_diagram_stays_one_element_and_leaves_by_delete_or_shift_delete() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    crate::uml_ui::open_sample(h.state_mut()).unwrap();
    let find = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap();
        st.uuid(c).unwrap()
    };
    let (customer, order) = (find(&h, "Customer"), find(&h, "Order"));
    let (pkg, d2) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let pkg = st.uuid(archforge_uml::commands::packages(st)[0]).unwrap();
        let out = s
            .execute("diagram.add", &serde_json::json!({ "kind": archforge_uml::CLASS_DIAGRAM, "element": pkg.to_string(), "name": "Ordering" }))
            .unwrap();
        (pkg, archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap())
    };
    crate::canvas::open(h.state_mut(), d2);
    h.state_mut().explorer.expanded.insert(pkg);
    h.run();
    let shown = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let d = st.by_uuid(d2).unwrap();
        (st.list(d, archforge_diagram::diagram::NODE).unwrap().len(), st.list(d, archforge_diagram::diagram::EDGE).unwrap().len())
    };
    assert_eq!(shown(&h), (0, 0));
    // Customer, then Order: the association between them draws with the second, one undo step.
    let canvas = rect_of(&h, "workspace.canvas");
    let row = rect_of(&h, &format!("workspace.explorer.row.{customer}")).center();
    drag(&mut h, row, canvas.center() - egui::vec2(200.0, 0.0));
    let row = rect_of(&h, &format!("workspace.explorer.row.{order}")).center();
    drag(&mut h, row, canvas.center() + egui::vec2(200.0, 0.0));
    assert_eq!(shown(&h), (2, 1), "notice {:?}", h.state().canvas.notice);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(shown(&h), (1, 0));
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!(shown(&h), (2, 1));
    // One element, two views; the card is as tall as its members.
    let st = h.state().session.as_ref().unwrap().doc();
    let (_, _, nodes) = archforge_diagram::views_of(st, st.by_uuid(order).unwrap());
    assert_eq!(nodes.len(), 2, "Order on both diagrams");
    let row = rect_of(&h, &format!("workspace.explorer.row.{order}")).center();
    drag(&mut h, row, canvas.center());
    assert_eq!(shown(&h), (2, 1));
    assert_eq!(h.state().canvas.notice.as_deref(), Some("Already on this diagram"));
    // The selected class's adorner opens a new attribute's row on the card; Enter writes it and
    // opens the next, which Escape drops.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [order.to_string()] })).unwrap();
    h.run();
    let at = rect_of(&h, "workspace.canvas.adorn.add-attribute").center();
    click(&mut h, at);
    assert!(h.state().canvas.inline.as_ref().is_some_and(|i| i.edit.target.is_none() && i.edit.class == order));
    h.event(egui::Event::Text("status".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(member_rows(&h, order, false), ["+ id : Integer", "+ placedAt : Date", "+ status"]);
    assert!(h.state().canvas.inline.as_ref().is_some_and(|i| i.edit.target.is_none() && i.edit.name.is_empty()), "the next draft");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().canvas.inline.is_none());
    // A double click on the operation's row of the card edits it in place.
    let st = h.state().session.as_ref().unwrap().doc();
    let node = archforge_diagram::views_of(st, st.by_uuid(order).unwrap())
        .2
        .into_iter()
        .find(|n| st.container(*n).ok().flatten().is_some_and(|(d, _)| st.uuid(d).ok() == Some(d2)))
        .unwrap();
    let node_id = st.uuid(node).unwrap().to_string();
    let r = rect_of(&h, &format!("workspace.canvas.node.{node_id}"));
    let z = h.state().canvas.zoom;
    // Two attributes shown when it was placed, the dividing line, then the operation.
    let y = r.top() + (crate::canvas::ENTITY_FIRST_ROW + 3.5 * crate::canvas::ENTITY_ROW + 6.0) * z;
    let at = egui::pos2(r.center().x, y);
    h.event(egui::Event::PointerMoved(at));
    for _ in 0..2 {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let total = st.uuid(archforge_uml::members::operations(st, st.by_uuid(order).unwrap())[0]).unwrap();
    assert_eq!(h.state().canvas.inline.as_ref().and_then(|i| i.edit.target), Some(total), "{:?}", h.state().canvas.inline);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().canvas.inline.is_none());
    // Delete takes it off this diagram only; Shift+Delete asks, then takes it from the model.
    let at = rect_of(&h, &format!("workspace.canvas.node.{node_id}")).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(shown(&h).0, 1);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(order).is_some_and(|o| st.is_alive(o)), "the class stays in the model");
    let node = node_ids(&h)
        .into_iter()
        .find(|i| {
            i.ends_with(&{
                let st = h.state().session.as_ref().unwrap().doc();
                let c = st.by_uuid(customer).unwrap();
                let (_, _, nodes) = archforge_diagram::views_of(st, c);
                nodes
                    .into_iter()
                    .filter_map(|n| st.uuid(n).ok())
                    .find(|n| {
                        let o = st.by_uuid(*n).unwrap();
                        st.container(o).ok().flatten().is_some_and(|(d, _)| st.uuid(d).ok() == Some(d2))
                    })
                    .unwrap()
                    .to_string()
            })
        })
        .unwrap();
    let at = rect_of(&h, &node).center();
    click(&mut h, at);
    key(&mut h, egui::Key::Delete, egui::Modifiers::SHIFT);
    let ask = h.state().explorer.delete.clone().expect("the dialog asks");
    assert_eq!((ask.name.as_str(), ask.diagrams, ask.relationships), ("Customer", 2, 1), "its association goes with it");
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(customer).is_none_or(|c| !st.is_alive(c)), "Customer left the model");
    assert_eq!(shown(&h).0, 0);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    let st = h.state().session.as_ref().unwrap().doc();
    assert!(st.by_uuid(customer).is_some_and(|c| st.is_alive(c)), "undo brings it back");
}

/// The sample class model open, with the node of each class on its diagram by name.
fn related_sample() -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>) {
    related_sample_in(ThemeKind::Light, egui::vec2(1600.0, 1000.0))
}

/// [`related_sample`] in `theme` in a window of `size`.
fn related_sample_in(theme: ThemeKind, size: egui::Vec2) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>) {
    let mut h = workspace_harness(theme, size);
    crate::uml_ui::open_sample(h.state_mut()).unwrap();
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

/// The relationships of the class named `name`, as the agent's tool lists them.
fn related(h: &mut Harness<'static, Shell>, name: &str) -> Vec<serde_json::Value> {
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap();
    let id = st.uuid(c).unwrap().to_string();
    s.execute("uml.relationship.list", &serde_json::json!({ "class": id })).unwrap().value["relationships"].as_array().unwrap().clone()
}

fn edge_count(h: &Harness<'static, Shell>) -> usize {
    let st = h.state().session.as_ref().unwrap().doc();
    st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::EDGE).unwrap().len()
}

#[test]
fn the_relationship_tool_draws_the_kind_its_menu_chose_refuses_a_cycle_and_a_drop_on_empty_canvas_makes_a_class() {
    let (mut h, nodes) = related_sample();
    let node = |h: &Harness<'static, Shell>, name: &str| rect_of(h, &format!("workspace.canvas.node.{}", nodes[name])).center();
    let before = edge_count(&h);
    // The chevron's menu: Composition.
    let at = rect_of(&h, "workspace.tools.relationship-kinds").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.relate.composition").center();
    click(&mut h, at);
    assert_eq!(h.state().canvas.uml_relation, crate::uml_relate::Relate::Composition);
    assert!(!h.state().canvas.popover, "a pick closes the menu");
    // Drawn from the whole, Catalog, to the part, Customer: one step, the line on the diagram.
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (from, to) = (node(&h, "Catalog"), node(&h, "Customer"));
    drag(&mut h, from, to);
    assert_eq!(edge_count(&h), before + 1, "notice {:?}", h.state().canvas.notice);
    let composed =
        related(&mut h, "Catalog").into_iter().find(|r| r["to"].is_string() && r["ends"][1]["aggregation"] == "composite").expect("the composition");
    assert_eq!(composed["kind"], "association");
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select, "the tool hands back to Select");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(edge_count(&h), before);
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!(edge_count(&h), before + 1);
    // A generalization that would make a cycle is refused and said; nothing is drawn.
    h.state_mut().canvas.uml_relation = crate::uml_relate::Relate::Generalization;
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let (from, to) = (node(&h, "Payment"), node(&h, "CardPayment"));
    drag(&mut h, from, to);
    assert_eq!(edge_count(&h), before + 1);
    let notice = h.state().canvas.notice.clone().expect("the refusal is said");
    assert!(notice.contains("cycle"), "{notice}");
    // A dependency dropped on empty canvas: a new class there, its name open for typing.
    h.state_mut().canvas.uml_relation = crate::uml_relate::Relate::Dependency;
    let at = rect_of(&h, "workspace.tools.relationship").center();
    click(&mut h, at);
    let from = node(&h, "Catalog");
    let canvas = rect_of(&h, "workspace.canvas");
    drag(&mut h, from, egui::pos2(from.x, canvas.bottom() - 60.0));
    let rename = h.state().canvas.rename.clone().expect("the new class's name is open");
    assert_eq!(rename.text, "Class 1");
    assert!(related(&mut h, "Class 1").iter().any(|r| r["kind"] == "dependency"));
    assert_eq!(edge_count(&h), before + 2);
}

/// The sample class model in `theme` and a window of `size`, its association "paid by" (Order to
/// Payment) selected and its Properties shown; its id.
fn paid_by(theme: ThemeKind, size: egui::Vec2) -> (Harness<'static, Shell>, archforge_mof::Uuid) {
    let (mut h, _) = related_sample_in(theme, size);
    let paid = {
        let st = h.state().session.as_ref().unwrap().doc();
        let a =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == "paid by").unwrap();
        st.uuid(a).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [paid.to_string()] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    (h, paid)
}

/// The multiplicity of each end of `a` in the model, and as its line writes it at each end.
fn end_multiplicities(h: &Harness<'static, Shell>, a: archforge_mof::Uuid) -> ([String; 2], [String; 2]) {
    let st = h.state().session.as_ref().unwrap().doc();
    let a = st.by_uuid(a).unwrap();
    let model = archforge_uml::figures::member_ends(st, a).unwrap().map(|e| archforge_uml::figures::multiplicity(st, e));
    let line = archforge_uml::figures::line(st, a).unwrap();
    (model, line.ends.map(|e| e.text.unwrap_or_default()))
}

/// The multiplicity field of end `i` as typed.
fn typed_multiplicity(h: &Harness<'static, Shell>, i: usize) -> String {
    h.state().panels.ends.as_ref().unwrap().multiplicity[i].clone()
}

/// Clicks the field `id`, selects what it holds and types `text` over it.
fn retype(h: &mut Harness<'static, Shell>, id: &str, text: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.run();
    if text.is_empty() {
        key(h, egui::Key::Backspace, egui::Modifiers::NONE);
    } else {
        h.event(egui::Event::Text(text.into()));
        h.run();
    }
}

fn revision(h: &Harness<'static, Shell>) -> u64 {
    h.state().session.as_ref().unwrap().revision()
}

#[test]
fn an_association_end_multiplicity_is_picked_or_typed_one_undo_step_each_its_line_following_and_kept_on_reopen() {
    let size = egui::vec2(1600.0, 1000.0);
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (h, _) = paid_by(theme, size);
        let found = ids(&h);
        for i in 0..2 {
            for id in [format!("workspace.properties.end.{i}.multiplicity"), format!("workspace.properties.end.{i}.multiplicity.menu")] {
                assert!(found.contains(&id), "{theme:?}: {id}");
            }
        }
    }
    let (mut h, paid) = paid_by(ThemeKind::Light, size);
    let s = |a: [&str; 2]| a.map(String::from);
    assert_eq!(end_multiplicities(&h, paid), (s(["1", "0..1"]), s(["1", "0..1"])));
    // The chevron opens the presets, the hint and Clear; a pick writes at once, one undo step.
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::EndMultiplicity(0)));
    let found = ids(&h);
    for item in ["0", "1", "2", "3", "4", "clear"] {
        assert!(found.contains(&format!("workspace.properties.end.0.multiplicity.menu.{item}")), "{item} in {found:?}");
    }
    assert!(painted(&h).iter().any(|t| t == "Or type a number or range"));
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu.3").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, None, "a pick closes the menu");
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", "0..1"]), s(["1..*", "0..1"])));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(end_multiplicities(&h, paid).0, s(["1", "0..1"]));
    assert_eq!(typed_multiplicity(&h, 0), "1", "the field follows the undo");
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert_eq!((end_multiplicities(&h, paid).0, typed_multiplicity(&h, 0)), (s(["1..*", "0..1"]), "1..*".to_string()));
    // Typed on the other end and Enter: written there, the first end keeping its own.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "2..5");
    assert_eq!(end_multiplicities(&h, paid).0[1], "0..1", "typing writes nothing until it is committed");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", "2..5"]), s(["1..*", "2..5"])));
    // A reversed range is refused under the field and stays there to correct; nothing is written.
    let before = revision(&h);
    retype(&mut h, "workspace.properties.end.1.multiplicity", "5..2");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(painted(&h).iter().any(|t| t == "The upper bound must be at least 5."), "{:?}", painted(&h));
    assert_eq!(typed_multiplicity(&h, 1), "5..2");
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", "2..5"]), s(["1..*", "2..5"])));
    assert_eq!(revision(&h), before);
    // Escape brings the written value back and the reason goes.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(typed_multiplicity(&h, 1), "2..5");
    assert!(!painted(&h).iter().any(|t| t == "The upper bound must be at least 5."));
    assert_eq!(revision(&h), before);
    // Each malformed text is refused with its reason, never coerced.
    for (bad, why) in [
        ("-1", "Type a whole number, a range such as 2..5, or *."),
        ("1...*", "Type a whole number, a range such as 2..5, or *."),
        ("99999999999", "A bound can be at most 1000000."),
        ("*..1", "The lower bound cannot be *."),
    ] {
        retype(&mut h, "workspace.properties.end.1.multiplicity", bad);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(h.state().panels.ends.as_ref().unwrap().refused[1].as_deref(), Some(why), "{bad}");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
    assert_eq!(revision(&h), before);
    // Saved and opened again, as the project's rows: the ends keep their multiplicities.
    let (tables, junctions) = archforge_notations::storage();
    let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
    let back = archforge_notations::session_from_rows(&rows, Box::new(|| archforge_mof::Uuid::v7(1_761_670_900_000, [9; 10]))).unwrap();
    let st = back.doc();
    let ends = archforge_uml::figures::member_ends(st, st.by_uuid(paid).unwrap()).unwrap();
    assert_eq!(ends.map(|e| archforge_uml::figures::multiplicity(st, e)), s(["1..*", "2..5"]));
    // Clear takes it away, as empty text does; each one undo step.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu.clear").center();
    click(&mut h, at);
    assert_eq!(end_multiplicities(&h, paid), (s(["1..*", ""]), s(["1..*", ""])));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(end_multiplicities(&h, paid).0, s(["1..*", "2..5"]));
    retype(&mut h, "workspace.properties.end.0.multiplicity", "");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(end_multiplicities(&h, paid), (s(["", "2..5"]), s(["", "2..5"])));
    assert!(painted(&h).iter().any(|t| t == "—"), "an empty field shows the placeholder");
}

#[test]
fn an_end_multiplicity_menu_follows_the_keys_and_escape_closes_it_before_it_cancels_the_edit() {
    let (mut h, paid) = paid_by(ThemeKind::Dark, egui::vec2(1600.0, 1000.0));
    let open = |h: &Harness<'static, Shell>| h.state().panels.menu == Some(crate::panels::Menu::EndMultiplicity(1));
    // Alt+Down in the field opens the presets at the current one; the arrows move; Enter picks.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity").center();
    click(&mut h, at);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::ALT);
    assert!(open(&h));
    assert_eq!(h.state().panels.highlight, Some(0), "0..1 is the current one");
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(h.state().panels.highlight, Some(1));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(!open(&h));
    assert_eq!(end_multiplicities(&h, paid).0[1], "1");
    // Typed but not committed: Escape closes the menu first, then brings the written value back.
    let before = revision(&h);
    retype(&mut h, "workspace.properties.end.1.multiplicity", "3");
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::ALT);
    assert!(open(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!open(&h));
    assert_eq!(typed_multiplicity(&h, 1), "3", "closing the menu keeps the typing");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(typed_multiplicity(&h, 1), "1");
    assert_eq!(revision(&h), before, "nothing was written");
    // Leaving the field writes what was typed: `0..*` reads as `*`.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "0..*");
    let at = rect_of(&h, "workspace.properties.end.1.role").center();
    click(&mut h, at);
    assert_eq!(end_multiplicities(&h, paid).0[1], "*");
    assert_eq!(typed_multiplicity(&h, 1), "*");
    // The keys reach the end chosen: the Order end kept its own.
    assert_eq!(end_multiplicities(&h, paid).0[0], "1");
}

#[test]
fn a_multiplicity_refusal_stays_until_its_own_field_is_written_and_reads_in_full() {
    let (mut h, paid) = paid_by(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let refused = |h: &Harness<'static, Shell>, i: usize| h.state().panels.ends.as_ref().unwrap().refused[i].clone();
    // Writing another field of the end keeps the reason while the refused text stays.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "abc");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(refused(&h, 1).is_some());
    // Out of the field first, so the Navigable click writes on its own.
    let at = rect_of(&h, "workspace.properties.relationship.from").center();
    click(&mut h, at);
    assert!(refused(&h, 1).is_some());
    let at = rect_of(&h, "workspace.properties.end.1.navigable").center();
    click(&mut h, at);
    let navigable = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        let a = st.by_uuid(paid).unwrap();
        archforge_uml::figures::navigable(st, a, archforge_uml::figures::member_ends(st, a).unwrap()[1])
    };
    assert!(!navigable(&h), "Navigable was written");
    assert_eq!(end_multiplicities(&h, paid).0[1], "0..1");
    assert_eq!(typed_multiplicity(&h, 1), "abc");
    assert!(refused(&h, 1).is_some(), "the field still holds text that was never written");
    // Picking the value the model has already takes the reason away, though nothing is written.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu.0").center();
    click(&mut h, at);
    assert_eq!((typed_multiplicity(&h, 1), refused(&h, 1)), ("0..1".to_string(), None));
    // A composition's whole allows one at most: the model's refusal on Enter keeps the keyboard
    // in the field and shows its whole reason under it.
    let at = rect_of(&h, "workspace.properties.end.1.aggregation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.aggregation.2").center();
    click(&mut h, at);
    retype(&mut h, "workspace.properties.end.0.multiplicity", "2");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let why = refused(&h, 0).expect("the model refuses");
    assert!(why.contains("composition's whole"), "{why}");
    assert_eq!(end_multiplicities(&h, paid).0[0], "1");
    let id = crate::widgets::egui_id("workspace.properties.end.0.multiplicity");
    assert!(h.ctx.memory(|m| m.has_focus(id)), "the field keeps the keyboard");
    fn galleys(s: &egui::Shape, out: &mut Vec<std::sync::Arc<egui::Galley>>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| galleys(s, out)),
            _ => {}
        }
    }
    let mut all = Vec::new();
    for clipped in h.output().shapes.iter() {
        galleys(&clipped.shape, &mut all);
    }
    let said = all.iter().find(|g| g.text() == why).expect("the reason is drawn");
    assert!(!said.elided && said.rows.len() > 1, "the reason wraps instead of being cut: {:?} rows", said.rows.len());
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!((typed_multiplicity(&h, 0), refused(&h, 0)), ("1".to_string(), None));
}

#[test]
fn the_chevron_keeps_a_typed_multiplicity_a_draft_so_a_pick_is_one_step_and_escape_still_cancels() {
    let (mut h, paid) = paid_by(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let id = crate::widgets::egui_id("workspace.properties.end.0.multiplicity");
    // Typed over the written 1, then the chevron: nothing is written and the field keeps the keyboard.
    let before = revision(&h);
    retype(&mut h, "workspace.properties.end.0.multiplicity", "3");
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu").center();
    click(&mut h, at);
    assert_eq!(h.state().panels.menu, Some(crate::panels::Menu::EndMultiplicity(0)));
    assert_eq!((end_multiplicities(&h, paid).0[0].as_str(), typed_multiplicity(&h, 0)), ("1", "3".to_string()));
    assert_eq!(revision(&h), before);
    assert!(h.ctx.memory(|m| m.has_focus(id)));
    // Escape closes the menu, then brings the 1 back; still nothing written.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!((typed_multiplicity(&h, 0), revision(&h)), ("1".to_string(), before));
    // Typed, then a preset picked: the preset is written as one step, which one undo takes back.
    retype(&mut h, "workspace.properties.end.0.multiplicity", "3");
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.0.multiplicity.menu.3").center();
    click(&mut h, at);
    assert_eq!(end_multiplicities(&h, paid).0[0], "1..*");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!(end_multiplicities(&h, paid).0[0], "1", "one undo goes back past the pick to the 1, not to a 3");
}

#[test]
fn leaving_a_multiplicity_closes_its_menu_and_its_rows_and_refusal_reach_assistive_technology() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let (mut h, paid) = paid_by(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let open = |h: &Harness<'static, Shell>| h.state().panels.menu == Some(crate::panels::Menu::EndMultiplicity(1));
    // The menu's rows and the chevron carry their words.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity.menu").center();
    click(&mut h, at);
    assert!(open(&h));
    assert!(h.query_by_label("End at Payment · Pick a multiplicity").is_some());
    assert!(h.query_by_label("End at Order · Pick a multiplicity").is_some(), "each chevron names its end");
    for word in ["0..1", "1", "0..*", "1..*", "*", "Clear"] {
        assert!(!h.query_all_by_label(word).collect::<Vec<_>>().is_empty(), "{word} has an accessible name");
    }
    // Tab out of the field with the menu open and a row reached: the menu closes, and Enter
    // afterwards picks nothing.
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    assert_eq!(h.state().panels.highlight, Some(2), "from the current 0..1 down to 0..*");
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    assert!(!open(&h), "leaving the field closes its menu");
    assert_eq!(end_multiplicities(&h, paid).0[1], "0..1", "nothing was picked");
    // Tab reached the chevron, whose Enter opens the menu again; Escape closes it.
    let chevron = crate::widgets::egui_id("workspace.properties.end.1.multiplicity.menu");
    assert!(h.ctx.memory(|m| m.has_focus(chevron)));
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(open(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!open(&h));
    // Typing with the menu open takes over from the row reached: Enter writes the typing.
    let at = rect_of(&h, "workspace.properties.end.1.multiplicity").center();
    click(&mut h, at);
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::ALT);
    assert_eq!((open(&h), h.state().panels.highlight), (true, Some(0)));
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.event(egui::Event::Text("2..*".into()));
    h.run();
    assert_eq!(h.state().panels.highlight, None);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(end_multiplicities(&h, paid).0[1], "2..*");
    // A refusal is the field's description, and the field says it is invalid.
    retype(&mut h, "workspace.properties.end.1.multiplicity", "5..2");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let field = h.get_by_label("End at Payment · Multiplicity");
    assert_eq!(field.accesskit_node().description().as_deref(), Some("The upper bound must be at least 5."));
    assert_eq!(field.accesskit_node().data().invalid(), Some(egui::accesskit::Invalid::True));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    let field = h.get_by_label("End at Payment · Multiplicity");
    assert_eq!((field.accesskit_node().description(), field.accesskit_node().data().invalid()), (None, None));
}

#[test]
fn an_end_multiplicity_menu_opens_above_its_field_when_the_window_has_no_room_below() {
    let (mut h, _) = paid_by(ThemeKind::Light, egui::vec2(1600.0, 820.0));
    for (i, above) in [(0, false), (1, true)] {
        let at = rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity.menu")).center();
        click(&mut h, at);
        let field = rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity"));
        let (first, clear) = (
            rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity.menu.0")),
            rect_of(&h, &format!("workspace.properties.end.{i}.multiplicity.menu.clear")),
        );
        if above {
            assert!(clear.bottom() < field.top(), "end {i}: {clear:?} over {field:?}");
        } else {
            assert!(first.top() > field.bottom(), "end {i}: {first:?} under {field:?}");
        }
        assert!(clear.bottom() <= 820.0 && first.top() >= 0.0, "end {i}: the whole menu shows");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
}

#[test]
fn a_new_association_shows_both_ends_without_a_multiplicity() {
    let (mut h, _) = related_sample();
    let class = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let c = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name).unwrap();
        st.uuid(c).unwrap().to_string()
    };
    let (from, to) = (class(&h, "Catalog"), class(&h, "Product"));
    let s = h.state_mut().session.as_mut().unwrap();
    let a = s.execute("uml.association.add", &serde_json::json!({ "from": from, "to": to })).unwrap().value["id"].as_str().unwrap().to_string();
    s.execute("view.select", &serde_json::json!({ "ids": [a.clone()] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    let a = archforge_mof::Uuid::parse(&a).unwrap();
    let none = [String::new(), String::new()];
    assert_eq!(end_multiplicities(&h, a), (none.clone(), none));
    assert_eq!(painted(&h).iter().filter(|t| *t == "—").count(), 2, "each end shows the placeholder, never a value");
}

#[test]
fn a_selected_associations_ends_are_set_in_properties_its_end_drags_to_another_class_and_delete_removes_it() {
    let (mut h, nodes) = related_sample();
    let paid = {
        let st = h.state().session.as_ref().unwrap().doc();
        let a =
            st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::ASSOCIATION) && archforge_core::name_of(st, *o) == "paid by").unwrap();
        st.uuid(a).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [paid.to_string()] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "End at Order") && texts.iter().any(|t| t == "End at Payment"), "{texts:?}");
    // A role typed and Enter: written to the model, one step.
    let at = rect_of(&h, "workspace.properties.end.1.role").center();
    click(&mut h, at);
    h.event(egui::Event::Key { key: egui::Key::A, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
    h.event(egui::Event::Text("settlement".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let ends = |h: &mut Harness<'static, Shell>| related(h, "Payment").into_iter().find(|r| r["name"] == "paid by").unwrap()["ends"].clone();
    assert_eq!(ends(&mut h)[1]["role"], "settlement");
    // Both ends navigable: each shows Navigable.
    assert!(ids(&h).contains(&"workspace.properties.end.0.navigable".to_string()));
    // Composite on the Payment end: the Order end allows one, so it is fine. The Order end is now
    // the whole's, where the diamond sits: it stops being navigable and shows no Navigable
    // (owner decision); the Payment end keeps its own.
    let at = rect_of(&h, "workspace.properties.end.1.aggregation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.1.aggregation.2").center();
    click(&mut h, at);
    assert_eq!(ends(&mut h)[1]["aggregation"], "composite");
    assert_eq!(ends(&mut h)[0]["navigable"], false);
    let found = ids(&h);
    assert!(!found.contains(&"workspace.properties.end.0.navigable".to_string()), "{found:?}");
    assert!(found.contains(&"workspace.properties.end.1.navigable".to_string()));
    // Undo puts the plain association back, both ends navigable and both showing Navigable.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert_eq!((ends(&mut h)[0]["navigable"].clone(), ends(&mut h)[1]["aggregation"].clone()), (serde_json::json!(true), serde_json::json!("none")));
    assert!(ids(&h).contains(&"workspace.properties.end.0.navigable".to_string()));
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert!(!ids(&h).contains(&"workspace.properties.end.0.navigable".to_string()));
    // Shared on the Order end too is refused, and said.
    let at = rect_of(&h, "workspace.properties.end.0.aggregation").center();
    click(&mut h, at);
    let at = rect_of(&h, "workspace.properties.end.0.aggregation.1").center();
    click(&mut h, at);
    assert!(h.state().panels.notice.as_ref().is_some_and(|n| n.contains("aggregates already")), "{:?}", h.state().panels.notice);
    assert_eq!(ends(&mut h)[0]["aggregation"], "none");
    // The line's target end, at Payment, dragged onto Product: the association and its line move.
    let handle = rect_of(&h, "workspace.canvas.line-end.target").center();
    let to = rect_of(&h, &format!("workspace.canvas.node.{}", nodes["Product"])).center();
    drag(&mut h, handle, to);
    assert!(related(&mut h, "Product").iter().any(|r| r["name"] == "paid by"), "notice {:?}", h.state().canvas.notice);
    assert!(!related(&mut h, "Payment").iter().any(|r| r["name"] == "paid by"));
    let st = h.state().session.as_ref().unwrap().doc();
    let edge = archforge_diagram::views_of(st, st.by_uuid(paid).unwrap()).1[0];
    let target = st.get(edge, archforge_diagram::edge::TARGET).unwrap().as_ref().unwrap();
    assert_eq!(st.uuid(target).unwrap().to_string(), nodes["Product"], "the line follows");
    // Delete on the selected line removes it, said with Undo.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [paid.to_string()] })).unwrap();
    h.run();
    let before = edge_count(&h);
    let at = rect_of(&h, "workspace.canvas").center();
    h.event(egui::Event::PointerMoved(at));
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(edge_count(&h), before - 1);
    assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("Association removed"));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(edge_count(&h), before);
}

/// The labels of the Models tree's rows, each indented by its depth.
fn tree_labels(h: &Harness<'static, Shell>) -> Vec<String> {
    crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer)
        .into_iter()
        .map(|r| format!("{}{}{}", "  ".repeat(r.depth as usize), r.label, r.detail.map(|d| format!(" : {d}")).unwrap_or_default()))
        .collect()
}

fn select_named(h: &mut Harness<'static, Shell>, class: archforge_mof::ClassId, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).unwrap();
    let id = st.uuid(o).unwrap();
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
    h.run();
    id
}

#[test]
fn the_models_tree_nests_packages_lists_members_and_moves_a_class_by_drag_or_properties() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    crate::uml_ui::open_sample(h.state_mut()).unwrap();
    h.run();
    // A package added with Commerce selected goes inside it, its name open for typing.
    let commerce = select_named(&mut h, archforge_uml::PACKAGE, "Commerce");
    crate::explorer::add(h.state_mut(), "package");
    let rename = h.state().explorer.rename.clone().expect("named in place");
    assert_eq!(rename.text, "Package 1");
    h.state_mut().explorer.rename.as_mut().unwrap().text = "Billing".into();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let billing = st.alive().find(|o| archforge_core::name_of(st, *o) == "Billing").unwrap();
    assert_eq!(archforge_uml::commands::package_of(st, billing), st.by_uuid(commerce), "nested in Commerce");
    // Expand all opens the packages and the classes: members show with their types.
    let at = rect_of(&h, "workspace.explorer.fold-all").center();
    click(&mut h, at);
    let rows = tree_labels(&h);
    for want in ["Commerce", "  Billing", "  Customer", "    id : Integer", "    email : String", "    rename()"] {
        assert!(rows.iter().any(|r| r == want), "{want} in {rows:?}");
    }
    // Customer dragged onto Billing's row moves there; one undo puts it back.
    let customer = select_named(&mut h, archforge_uml::CLASS, "Customer");
    let billing_id = h.state().session.as_ref().unwrap().doc().uuid(billing).unwrap();
    let from = rect_of(&h, &format!("workspace.explorer.row.{customer}")).center();
    let to = rect_of(&h, &format!("workspace.explorer.row.{billing_id}")).center();
    drag(&mut h, from, to);
    let package_of = |h: &Harness<'static, Shell>, id: archforge_mof::Uuid| {
        let st = h.state().session.as_ref().unwrap().doc();
        archforge_uml::commands::package_of(st, st.by_uuid(id).unwrap()).map(|p| archforge_core::name_of(st, p))
    };
    assert_eq!(package_of(&h, customer).as_deref(), Some("Billing"), "notice {:?}", h.state().explorer.notice);
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(package_of(&h, customer).as_deref(), Some("Commerce"));
    // Properties' Package select moves Order into Billing.
    let order = select_named(&mut h, archforge_uml::CLASS, "Order");
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    let at = rect_of(&h, "workspace.properties.class.package").center();
    click(&mut h, at);
    let i = {
        let st = h.state().session.as_ref().unwrap().doc();
        archforge_uml::commands::all_packages(st).iter().position(|p| *p == billing).unwrap()
    };
    let at = rect_of(&h, &format!("workspace.properties.class.package.{i}")).center();
    click(&mut h, at);
    assert_eq!(package_of(&h, order).as_deref(), Some("Billing"));
    // On the Commerce diagram Order is now an outsider, its package named.
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(h.state().canvas.active.unwrap()).unwrap();
    let card = archforge_uml::figures::card(st, st.by_uuid(order).unwrap(), d).unwrap();
    assert!(card.elsewhere, "an outsider on Commerce's diagram");
    assert_eq!(card.eyebrow, archforge_diagram::figure::Text::Said("Class · {package}", vec![("package", "Billing".into())]));
    // Billing removed: Order moves up to Commerce, said in the toast; undo brings Billing back.
    select_named(&mut h, archforge_uml::PACKAGE, "Billing");
    crate::explorer::remove_selection(h.state_mut());
    assert!(h.state().explorer.delete.is_none(), "no dialog: nothing goes");
    assert_eq!(package_of(&h, order).as_deref(), Some("Commerce"));
    assert_eq!(h.state().canvas.toast.as_ref().map(|t| t.text.as_str()), Some("Billing removed; what it held moved to Commerce"));
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    assert_eq!(package_of(&h, order).as_deref(), Some("Billing"));
}

#[test]
fn quitting_asks_about_each_unsaved_project_in_turn_and_cancel_stops_it() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    h.run_steps(2);
    add_schema(&mut h, "books");
    // Both tabs and the title mark their unsaved changes.
    assert_eq!(h.state().title(), "• Library — Archforge");
    assert!(h.state().tab_unsaved(0) && h.state().tab_unsaved(1));
    assert!(painted(&h).iter().any(|t| t == "• Commerce platform"), "{:?}", painted(&h));
    // Quit: the first unsaved project is asked about; Cancel stops quitting, nothing closes.
    h.state_mut().request_quit();
    h.run_steps(2);
    assert!(h.state().quitting());
    assert!(matches!(&h.state().ask, Some(crate::close_ask::Ask::Unsaved { path }) if path.ends_with("Commerce platform.archforge")));
    assert!(!h.state().quit_confirmed());
    let at = rect_of(&h, "close-ask.cancel").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(!h.state().quitting() && h.state().ask.is_none() && h.state().tabs.len() == 2);
    // Quit again: Save for Commerce (it closes once saved), then Don't save for Library.
    h.state_mut().request_quit();
    h.run_steps(2);
    let at = rect_of(&h, "close-ask.save").center();
    click(&mut h, at);
    h.run_steps(2);
    host.saves.lock().unwrap().pop().expect("Commerce is saved").send(Ok(true)).unwrap();
    h.run_steps(2);
    host.closings.lock().unwrap().pop().expect("then it closes").send(Ok(())).unwrap();
    h.run_steps(3);
    assert!(matches!(&h.state().ask, Some(crate::close_ask::Ask::Unsaved { path }) if path.ends_with("Library.archforge")));
    let at = rect_of(&h, "close-ask.discard").center();
    click(&mut h, at);
    assert_eq!(host.discarded.lock().unwrap().as_slice(), [std::path::PathBuf::from("/tmp/Library.archforge")]);
    host.closings.lock().unwrap().pop().expect("Library closes without writing").send(Ok(())).unwrap();
    // Every project answered: quitting is confirmed and the window closes.
    h.run_steps(3);
    assert!(h.state().quit_confirmed() && !h.state().quitting(), "the window is asked to close once every project is answered");
}

#[test]
fn a_project_whose_changes_cannot_be_written_closes_only_after_saying_its_changes_are_lost() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    // Saved, so closing asks nothing; the host cannot write and refuses.
    let at = rect_of(&h, "workspace.header.project.0.close").center();
    click(&mut h, at);
    h.run_steps(2);
    host.closings.lock().unwrap().pop().expect("the host is asked").send(Err("the project server stopped".into())).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().close_error(), Some("the project server stopped"));
    // Closing again offers Close without writing, which asks once more.
    let at = rect_of(&h, "workspace.header.project.0.close").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::CannotWrite { .. })));
    assert!(painted(&h).iter().any(|t| t == "the project server stopped"));
    let at = rect_of(&h, "close-ask.close-anyway").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::ConfirmLoss { .. })));
    assert!(host.unwritten.lock().unwrap().is_empty(), "nothing is lost before the second answer");
    let at = rect_of(&h, "close-ask.confirm").center();
    click(&mut h, at);
    assert_eq!(host.unwritten.lock().unwrap().len(), 1, "closed without writing, not put back to its last save");
    assert!(host.discarded.lock().unwrap().is_empty());
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(2);
    assert!(h.state().tabs.is_empty());
}

#[test]
fn save_on_close_waits_for_a_running_save_and_asks_again_when_edits_came_in_meanwhile() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    h.state_mut().save();
    h.state_mut().close_tab(0);
    h.run_steps(2);
    let at = rect_of(&h, "close-ask.save").center();
    click(&mut h, at);
    h.run_steps(2);
    assert_eq!(host.saves.lock().unwrap().len(), 1, "the running save is waited for, not a second one");
    assert!(host.closed.lock().unwrap().is_empty(), "nothing closes before the save answers");
    // An edit arrives while the save runs: once it answers, the question comes again.
    add_schema(&mut h, "crm");
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
    h.run_steps(2);
    assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::Unsaved { .. })), "{:?}", h.state().ask);
    assert!(host.closed.lock().unwrap().is_empty());
}

#[test]
fn a_picker_that_cannot_appear_is_said_on_home_and_a_cancel_says_nothing() {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    let unavailable = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let probe = unavailable.clone();
    let services = Services {
        projects: Some(host),
        ids: Some(test_ids()),
        pick_project: Some(std::sync::Arc::new(|_| None)),
        pick_folder: Some(std::sync::Arc::new(|| None)),
        picker_unavailable: Some(std::sync::Arc::new(move || {
            probe.load(std::sync::atomic::Ordering::SeqCst).then(|| "this system has no desktop portal".to_string())
        })),
        ..Services::default()
    };
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        Shell::new(services),
    );
    h.run();
    // A cancel: nothing to say.
    h.state_mut().open_from_picker();
    assert_eq!(h.state().picker_error, None);
    // A picker that could not appear: Home says why, in its own words, for Open and for Pin a
    // folder alike; a later pick that shows a picker clears it.
    unavailable.store(true, std::sync::atomic::Ordering::SeqCst);
    h.state_mut().open_from_picker();
    h.run();
    assert_eq!(h.state().picker_error.as_deref(), Some("this system has no desktop portal"));
    assert!(painted(&h).iter().any(|t| t == "this system has no desktop portal"), "{:?}", painted(&h));
    assert_eq!(h.state().open_error, None, "not said as a failed opening");
    unavailable.store(false, std::sync::atomic::Ordering::SeqCst);
    h.state_mut().pin_from_picker();
    assert_eq!(h.state().picker_error, None, "a cancel with a picker there clears it");
    unavailable.store(true, std::sync::atomic::Ordering::SeqCst);
    h.state_mut().pin_from_picker();
    assert_eq!(h.state().picker_error.as_deref(), Some("this system has no desktop portal"));
}

#[test]
fn a_class_and_an_attribute_take_a_skill_from_the_skills_panel_and_the_tree_lists_the_class_skill() {
    let (mut h, customer) = class_properties("Customer");
    h.state_mut().state.show_panel(crate::state::Panel::Skills);
    h.run();
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.run();
    let on = |h: &Harness<'static, Shell>, subject: archforge_mof::Uuid| {
        let st = h.state().session.as_ref().unwrap().doc();
        let o = st.by_uuid(subject).unwrap();
        archforge_core::skills::skills(st).into_iter().filter(|s| archforge_core::skills::subject_of(st, *s) == Some(o)).collect::<Vec<_>>()
    };
    let skills = on(&h, customer);
    assert_eq!(skills.len(), 1, "the class carries the new skill");
    let skill = h.state().session.as_ref().unwrap().doc().uuid(skills[0]).unwrap();
    assert_eq!(h.state().canvas.active, Some(skill), "it opens in its tab");
    // The class's row in the Models tree opens onto its skill, above its members.
    let package = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.container(st.by_uuid(customer).unwrap()).unwrap().unwrap().0).unwrap()
    };
    h.state_mut().explorer.expanded.extend([package, customer]);
    h.run();
    let found = ids(&h);
    let row = |id: archforge_mof::Uuid| found.iter().position(|i| *i == format!("workspace.explorer.row.{id}"));
    assert!(row(skill).is_some_and(|s| row(customer).is_some_and(|c| s > c)), "{found:?}");
    // An attribute takes one too, and the class's skill shows on it as inherited.
    let attribute = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(archforge_uml::members::attributes(st, st.by_uuid(customer).unwrap())[0]).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [attribute.to_string()] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Skills);
    h.run();
    assert!(ids(&h).contains(&format!("workspace.skills.row.{skill}")), "inherited from the class");
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.run();
    assert_eq!(on(&h, attribute).len(), 1, "the attribute carries its own");
}

#[test]
fn a_diagram_takes_a_skill_from_the_skills_panel_and_shows_its_schemas_as_inherited() {
    let (mut h, _, diagram) = diagram_harness();
    let schema_skill = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_er::SCHEMA)).unwrap()).unwrap().to_string();
        let out = s.execute("skill.add", &serde_json::json!({ "subject": schema, "name": "Layout" })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [diagram] })).unwrap();
        out.value["id"].as_str().unwrap().to_string()
    };
    h.state_mut().state.show_panel(crate::state::Panel::Skills);
    h.run();
    assert!(ids(&h).contains(&format!("workspace.skills.row.{schema_skill}")), "the schema's skill shows as inherited");
    let at = rect_of(&h, "workspace.skills.add").center();
    click(&mut h, at);
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let d = st.by_uuid(archforge_mof::Uuid::parse(&diagram).unwrap()).unwrap();
    let own: Vec<_> = archforge_core::skills::skills(st).into_iter().filter(|s| archforge_core::skills::subject_of(st, *s) == Some(d)).collect();
    assert_eq!(own.len(), 1, "the diagram carries the new skill");
    assert_eq!(h.state().canvas.active, Some(st.uuid(own[0]).unwrap()), "it opens in its tab");
}

#[test]
fn a_class_is_deleted_from_the_model_through_its_menu_and_comes_back_with_undo() {
    let (mut h, d, _) = read_class_diagram("paid by");
    let class_named = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_uml::CLASS) && archforge_core::name_of(st, *o) == name)
    };
    let node_of = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        let c = class_named(h, name).unwrap();
        let n = archforge_diagram::views_of(st, c).2.into_iter().find(|n| st.container(*n).ok().flatten().map(|(p, _)| p) == Some(d)).unwrap();
        format!("workspace.canvas.node.{}", st.uuid(n).unwrap())
    };
    let customer = node_of(&h, "Customer");
    let lines_before = h.state().session.as_ref().unwrap().doc().list(d, archforge_diagram::diagram::EDGE).unwrap().len();
    // A right click on a class opens its menu: Add attribute, Remove from this diagram, Delete from the model.
    let at = rect_of(&h, &customer).center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    assert!(h.state().canvas.menu.is_some(), "the menu opened");
    let found = ids(&h);
    for row in ["open", "add-row", "remove", "delete"] {
        assert!(found.contains(&format!("workspace.canvas.menu.{row}")), "{row}: {found:?}");
    }
    assert!(!found.iter().any(|i| i == "workspace.canvas.menu.related" || i == "workspace.canvas.menu.add-column"), "no table rows");
    // Delete from the model asks first, naming what goes with it, then deletes.
    let at = rect_of(&h, "workspace.canvas.menu.delete").center();
    click(&mut h, at);
    let ask = h.state().explorer.delete.clone().expect("the dialog asks");
    assert_eq!((ask.name.as_str(), ask.relationships, ask.diagrams), ("Customer", 1, 1));
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    assert!(class_named(&h, "Customer").is_none(), "the class left the model");
    assert!(!ids(&h).contains(&customer), "and the diagram");
    let st = h.state().session.as_ref().unwrap().doc();
    assert_eq!(st.list(d, archforge_diagram::diagram::EDGE).unwrap().len(), lines_before - 1, "its line went with it");
    // Undo brings the class, its card and its line back; redo deletes them again.
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.run();
    assert!(class_named(&h, "Customer").is_some());
    assert!(ids(&h).contains(&customer));
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.run();
    assert!(class_named(&h, "Customer").is_none());
    // Remove from this diagram keeps the class in the model.
    let order = node_of(&h, "OrderLine");
    let at = rect_of(&h, &order).center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.run();
    let at = rect_of(&h, "workspace.canvas.menu.remove").center();
    click(&mut h, at);
    assert!(!ids(&h).contains(&order), "the card left the diagram");
    assert!(class_named(&h, "OrderLine").is_some(), "the class stays in the model");
}
