//! The shell's tests: Home drawn headless in both appearances, its controls and its lists, the
//! theme switch and the banner, the UI state, and the installed fonts.

use egui_kittest::Harness;

use crate::testing::{click, ids};
use crate::theme::ThemeKind;
use crate::{Picked, Services, Shell, UiState};
/// The fake notation's screens part, the one the shells the tests make are given (ADR 0005): the
/// sketch notation with its one diagram kind, over the fake notation's model ([`NOTATIONS`]).
pub(crate) static SCREENS: archforge_shell_api::screens::ScreensRegistry =
    archforge_shell_api::screens::ScreensRegistry::new(&[archforge_shell_api::screens::ScreensPart {
        ui: &SketchUi("sketch"),
        kinds: &[&SketchKind(SKETCH)],
    }]);

/// The notations the shells the tests make hold their projects of, composed: the fake notation alone.
pub(crate) static NOTATIONS: &archforge_notations::Notations = &archforge_notations::fake::NOTATIONS;

mod branches;
mod edit_menu;
mod field_keys;
mod model_menu;
mod read_only;
mod skill_chords;
mod switch_ask;
mod version_chip;
mod version_details;
mod versions_keys;
mod versions_menu;
mod view_menu;

/// The real shell in the harness at the design's frame size, given the app's screens parts.
fn harness(theme: ThemeKind) -> Harness<'static, Shell> {
    crate::testing::harness(NOTATIONS, &SCREENS, theme)
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

/// Home lists a pinned folder and a recent project, in both appearances.
#[test]
fn home_lists_its_pinned_folders_and_recent_projects_in_both_appearances() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        crate::i18n::set_language("en");
        let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
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

/// The kinds a New project dialog lists: the sketch alone, the sketch and a second, then the sketch
/// between a second and a third.
static SKETCH_ALONE: &[&dyn crate::kinds::DiagramKind] = &[&SketchKind(SKETCH)];
static SKETCH_PAIR: &[&dyn crate::kinds::DiagramKind] = &[&SketchKind(SKETCH), &SketchKind("test.second")];
static SKETCH_BETWEEN: &[&dyn crate::kinds::DiagramKind] = &[&SketchKind("test.second"), &SketchKind(SKETCH), &SketchKind("test.third")];

/// The New project dialog's line on diagrams lists the kinds the shell was given, each by the name
/// of its group in the Diagrams explorer, as a sentence lists them: the one kind the shell is given
/// alone, naming no other; two joined by "and"; three with a comma before "and". In light and dark.
#[test]
fn the_new_project_line_lists_the_kinds_the_shell_was_given() {
    let line = |kinds: &'static [&'static dyn crate::kinds::DiagramKind], theme| {
        let mut h = harness(theme);
        h.state_mut().kinds = kinds;
        open_new_project(&mut h);
        painted(&h).into_iter().filter(|t| t.ends_with("Every diagram shows the same model.")).collect::<Vec<_>>()
    };
    in_both_appearances(|theme| {
        let alone = line(SKETCH_ALONE, theme);
        assert_eq!(line(SCREENS.kinds(), theme), alone, "{theme:?}");
        assert_eq!(alone, ["Add Sketches as you go. Every diagram shows the same model."], "{theme:?}");
        assert!(!alone.iter().any(|t| t.contains("Second sketches") || t.contains("Third sketches")), "{theme:?}: {alone:?}");
        let two = "Add Sketches and Second sketches as you go. Every diagram shows the same model.";
        assert_eq!(line(SKETCH_PAIR, theme), [two], "{theme:?}");
        let three = "Add Second sketches, Sketches and Third sketches as you go. Every diagram shows the same model.";
        assert_eq!(line(SKETCH_BETWEEN, theme), [three], "{theme:?}");
    });
}

/// Twelve kinds, so the New project line runs past the two rows the design leaves it.
static SKETCH_DOZEN: [&dyn crate::kinds::DiagramKind; 12] = [&SketchKind(SKETCH); 12];

/// A New project line longer than the design's two rows, in a narrow dialog, is drawn whole, in
/// as many rows as it takes, and what every project holds starts below it, not over it. In light
/// and dark.
#[test]
fn a_long_new_project_line_is_drawn_whole_and_moves_the_strip_below_it() {
    in_both_appearances(|theme| {
        crate::i18n::set_language("en");
        let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
        shell.state.theme = theme;
        shell.kinds = &SKETCH_DOZEN;
        let mut h = Harness::builder().with_size(egui::vec2(640.0, 1100.0)).build_ui_state(
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
        /// Collects every text under `s`, with where it is drawn, its rows and whether it was cut.
        fn walk(s: &egui::Shape, out: &mut Vec<(String, egui::Rect, usize, bool)>) {
            match s {
                egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect(), t.galley.rows.len(), t.galley.elided)),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut texts));
        let line = texts.iter().find(|(t, ..)| t.ends_with("Every diagram shows the same model.")).expect("the line");
        assert!(line.0.starts_with("Add Sketches, Sketches,"), "{theme:?}: {}", line.0);
        assert!(line.2 > 2 && !line.3, "{theme:?}: whole, in {} rows, cut: {}", line.2, line.3);
        let strip = texts.iter().find(|(t, ..)| t == "In every project").expect("the strip's heading");
        assert!(strip.1.top() >= line.1.bottom() + 8.0, "{theme:?}: the strip at {:?} starts below the line at {:?}", strip.1, line.1);
    });
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
            for more in NOT_ALONE {
                key(&mut h, egui::Key::Escape, more);
                assert!(h.state().new_project.is_some(), "Escape with {more:?} closes nothing");
            }
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

/// In the smallest window the New project dialog keeps its buttons and its field inside it.
#[test]
fn in_the_smallest_window_the_dialog_keeps_its_buttons_and_field_inside_it() {
    crate::i18n::set_language("en");
    let shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
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

/// A host that keeps no branches refuses a read on another branch's line by a typed reason, which
/// the page says in translated words, never words of the host's own.
#[test]
fn a_host_without_branches_refuses_a_branch_version_by_its_reason() {
    let read = crate::ProjectHost::read_branch_version(&FakeHost::default(), std::path::Path::new("/p"), "side", "c1".into());
    assert!(matches!(read.recv(), Ok(Err(crate::VersionReadError::Unavailable))));
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
        Shell::new(services, NOTATIONS, &SCREENS),
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
fn change_picks_the_folder_the_project_is_made_in_and_a_cancel_keeps_the_last_one() {
    let (mut h, host) = creating_harness();
    let answers = std::sync::Arc::new(std::sync::Mutex::new(vec![None, Some(std::path::PathBuf::from("/Volumes/Work/Clients/Acme"))]));
    let unavailable = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let titles = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let (next, probe, asked) = (answers.clone(), unavailable.clone(), titles.clone());
    h.state_mut().services.pick_folder = Some(std::sync::Arc::new(move |title: &str| {
        asked.lock().unwrap().push(title.to_string());
        Picked::at_once(next.lock().unwrap().pop().flatten())
    }));
    h.state_mut().services.picker_unavailable =
        Some(std::sync::Arc::new(move || probe.load(std::sync::atomic::Ordering::SeqCst).then(|| "this system has no desktop portal".to_string())));
    h.run();
    assert!(painted(&h).iter().any(|t| t == "Documents / Archforge"), "{:?}", painted(&h));
    // A pick: the field shows it, and the project is made there.
    let change = centre_of(&h, "new-project.location.change");
    press(&mut h, change);
    assert!(painted(&h).iter().any(|t| t == "Clients / Acme"), "{:?}", painted(&h));
    // The picker says what the folder is for: the project's location, not a folder to pin.
    assert_eq!(titles.lock().unwrap().as_slice(), ["Choose project location"]);
    // A cancel keeps the name and the folder picked before it, and says nothing.
    press(&mut h, change);
    assert!(painted(&h).iter().any(|t| t == "Clients / Acme"), "{:?}", painted(&h));
    assert!(!painted(&h).iter().any(|t| t == "The folder picker could not open."));
    // A picker that cannot appear says why, under the field.
    unavailable.store(true, std::sync::atomic::Ordering::SeqCst);
    press(&mut h, change);
    assert!(painted(&h).iter().any(|t| t == "The folder picker could not open."), "{:?}", painted(&h));
    assert!(painted(&h).iter().any(|t| t == "this system has no desktop portal"), "{:?}", painted(&h));
    let at = centre_of(&h, "new-project.create");
    press(&mut h, at);
    assert_eq!(host.asked.lock().unwrap().as_slice(), [("/Volumes/Work/Clients/Acme".into(), "Commerce platform".to_string())]);
    // The creation's own failure shows in place of what the picker said before it.
    answer(&mut h, &host, Err(crate::CreateError::FolderUnusable { detail: "read-only volume".into() }));
    assert!(painted(&h).iter().any(|t| t == "The folder cannot hold projects."), "{:?}", painted(&h));
    assert!(!painted(&h).iter().any(|t| t == "The folder picker could not open."), "{:?}", painted(&h));
}

/// A Change… pick belongs to its dialog (AR-737): Create project waits for its answer, a creation
/// running keeps its folder whatever a picker answers late, and closing the dialog lets the pick go,
/// withdrawn from the host (AR-738), so New project is back at once and a later dialog never takes
/// the old answer.
#[test]
fn a_change_pick_belongs_to_its_dialog_and_holds_create_until_it_is_answered() {
    let acme = || std::path::PathBuf::from("/Volumes/Work/Clients/Acme");
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = creating_harness();
        h.state_mut().state.theme = theme;
        let waiting = std::sync::Arc::new(std::sync::Mutex::new(Vec::<std::sync::mpsc::Sender<Picked>>::new()));
        let asked = waiting.clone();
        h.state_mut().services.pick_folder = Some(std::sync::Arc::new(move |_: &str| {
            let (tx, rx) = std::sync::mpsc::channel();
            asked.lock().unwrap().push(tx);
            rx
        }));
        let withdrawn = count_withdrawals(&mut h);
        h.run();
        let (change, create) = (centre_of(&h, "new-project.location.change"), centre_of(&h, "new-project.create"));
        press(&mut h, change);
        assert!(!crate::new_project::can_change(h.state(), false), "{theme:?}: Change… is off while its pick waits");
        press(&mut h, create);
        assert!(host.asked.lock().unwrap().is_empty(), "{theme:?}: Create project waits for the pick");
        waiting.lock().unwrap().pop().expect("a pick was asked for").send(Picked::Folder(acme())).unwrap();
        h.run_steps(4);
        press(&mut h, create);
        assert_eq!(host.asked.lock().unwrap().as_slice(), [(acme(), "Commerce platform".to_string())], "{theme:?}");
        // An answer that comes while the creation runs leaves the folder it is made in.
        let (late, rx) = std::sync::mpsc::channel();
        h.state_mut().ask_pick(crate::Pick::Location, || rx);
        late.send(Picked::Folder("/Users/robin/Elsewhere".into())).unwrap();
        h.run_steps(4);
        assert_eq!(h.state().new_project.as_ref().and_then(|d| d.folder.clone()), Some(acme()), "{theme:?}: the creation keeps its folder");
        answer(&mut h, &host, Err(crate::CreateError::FolderUnusable { detail: "read-only volume".into() }));
        // A pick still waiting when the dialog closes goes with it.
        assert_eq!(withdrawn.load(std::sync::atomic::Ordering::SeqCst), 0, "{theme:?}: picks answered are not withdrawn");
        press(&mut h, change);
        let cancel = centre_of(&h, "new-project.cancel");
        press(&mut h, cancel);
        assert!(crate::actions::available(h.state(), "file.new"), "{theme:?}: New project is back at once");
        assert_eq!(withdrawn.load(std::sync::atomic::Ordering::SeqCst), 1, "{theme:?}: the pick is withdrawn from the host");
        open_new_project(&mut h);
        let _ = waiting.lock().unwrap().pop().expect("a pick was asked for").send(Picked::Folder("/Users/robin/Old".into()));
        h.run_steps(4);
        assert_eq!(h.state().new_project.as_ref().map(|d| d.folder.clone()), Some(None), "{theme:?}: the old answer is not taken");
        assert!(painted(&h).iter().any(|t| t == "Documents / Archforge"), "{theme:?}: {:?}", painted(&h));
    }
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

/// Where a test answers a save: `Ok(true)` stands for a new version, `Ok(false)` for nothing new.
struct SaveAnswer(std::sync::mpsc::Sender<Result<crate::Saved, crate::SaveError>>);

impl SaveAnswer {
    /// Gives the save its answer.
    fn send(&self, answer: Result<bool, crate::SaveError>) -> Result<(), std::sync::mpsc::SendError<Result<crate::Saved, crate::SaveError>>> {
        self.0.send(answer.map(|made| if made { crate::Saved::Version { hash: Some("abc".into()) } } else { crate::Saved::NothingNew }))
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
    saved_by: std::sync::Mutex<Vec<crate::SavedBy>>,
    opens: std::sync::Mutex<Vec<(std::path::PathBuf, std::sync::mpsc::Sender<Result<crate::Opened, crate::CreateError>>)>>,
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
    branch_deletes: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<Vec<archforge_mof::Uuid>, crate::BranchNotDeleted>>>>,
    /// Each branch made, by its stored name, and where the test answers it.
    branch_makes: std::sync::Mutex<Vec<(String, std::sync::mpsc::Sender<Result<(), crate::BranchRefused>>)>>,
    /// The version each branch made starts at, in the same order; none for current work.
    branch_starts: std::sync::Mutex<Vec<Option<String>>>,
    /// Each branch rename, from and to as stored, and where the test answers it.
    branch_renames: std::sync::Mutex<Vec<(String, String, std::sync::mpsc::Sender<Result<(), crate::BranchRefused>>)>>,
    /// Each count of what goes with a branch, by its stored name, and where the test answers it.
    branch_counts: std::sync::Mutex<Vec<(String, std::sync::mpsc::Sender<Result<crate::BranchCounts, String>>)>>,
    /// Each read of a version waiting for the answer the test sends.
    reads: std::sync::Mutex<Vec<(String, ReadAnswer)>>,
    /// The branch each read of another branch's version was asked on, by its stored name.
    branch_version_reads: std::sync::Mutex<Vec<(String, String)>>,
    /// Each read of the versions waiting for the answer the test sends.
    version_reads: std::sync::Mutex<Vec<std::sync::mpsc::Sender<Result<crate::VersionsPage, String>>>>,
    /// The version each read of the versions started after.
    version_asks: std::sync::Mutex<Vec<Option<String>>>,
    /// The branch the project is on, what a read of the branches answers (none for no branches),
    /// whether it says more exist, and how many reads were asked for.
    branch_now: std::sync::Mutex<Option<String>>,
    branch_list: std::sync::Mutex<Option<Result<Vec<crate::BranchEntry>, String>>>,
    branch_cut: std::sync::Mutex<bool>,
    /// While set, each read of the branches waits there for the test to answer it.
    branch_held: std::sync::Mutex<Option<Vec<std::sync::mpsc::Sender<Result<(Vec<crate::BranchEntry>, bool), String>>>>>,
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
struct SwitchEnds {
    branch: String,
    check: ReadAnswer,
    unsaved: std::sync::Arc<std::sync::atomic::AtomicBool>,
    moved: RestoreEnds,
}

/// Where a restore's job is answered, and where its reading back is.
type RestoreEnds = (std::sync::mpsc::Sender<Result<(), crate::replace::JobFailure>>, std::sync::mpsc::Sender<Result<bool, String>>);

/// Where the answer to a read of a version goes.
type ReadAnswer = std::sync::mpsc::Sender<crate::VersionRead>;

impl crate::ProjectHost for SavingHost {
    /// A job the test answers through what is kept in `restores`.
    fn restore_job(
        &self,
        _project: &std::path::Path,
        _commit: String,
        (saved_at, before_at): (String, String),
        _by: crate::SavedBy,
        _repair: crate::Repair,
    ) -> crate::replace::Job {
        let (answer, answers) = std::sync::mpsc::channel();
        let (reading, readings) = std::sync::mpsc::channel();
        self.restore_dates.lock().unwrap().push((saved_at, before_at));
        self.restores.lock().unwrap().push((answer, reading));
        crate::replace::Job { run: Box::new(move || answers), read_back: Box::new(move || readings) }
    }

    /// The branch the test says the project is on.
    fn current_branch(&self, _project: &std::path::Path) -> Option<String> {
        self.branch_now.lock().unwrap().clone()
    }

    /// Answers at once with what the test has set.
    fn branches(&self, _project: &std::path::Path) -> std::sync::mpsc::Receiver<Result<(Vec<crate::BranchEntry>, bool), String>> {
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
    fn begin_branch_switch(&self, _project: &std::path::Path, branch: String) -> Result<crate::BranchSwitch, String> {
        if let Some(why) = self.switch_refused.lock().unwrap().take() {
            return Err(why);
        }
        let (check, read) = std::sync::mpsc::channel();
        let unsaved = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (answer, answers) = std::sync::mpsc::channel();
        let (reading, readings) = std::sync::mpsc::channel();
        self.switches.lock().unwrap().push(SwitchEnds { branch, check, unsaved: unsaved.clone(), moved: (answer, reading) });
        Ok(crate::BranchSwitch { read, unsaved, job: crate::replace::Job { run: Box::new(move || answers), read_back: Box::new(move || readings) } })
    }

    /// Answers nothing yet: the test answers later, through what is kept in `reads`.
    fn read_version(&self, _project: &std::path::Path, commit: String) -> std::sync::mpsc::Receiver<crate::VersionRead> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.reads.lock().unwrap().push((commit, tx));
        rx
    }

    /// A read of a version, answered as [`SavingHost::read_version`]'s are, and kept with its branch.
    fn read_branch_version(&self, project: &std::path::Path, branch: &str, commit: String) -> std::sync::mpsc::Receiver<crate::VersionRead> {
        self.branch_version_reads.lock().unwrap().push((branch.to_string(), commit.clone()));
        self.read_version(project, commit)
    }

    fn create(&self, _folder: &std::path::Path, _name: &str) -> std::sync::mpsc::Receiver<Result<crate::Opened, crate::CreateError>> {
        std::sync::mpsc::channel().1
    }

    /// Keeps the answer for the test to send.
    fn versions(
        &self,
        _project: &std::path::Path,
        after: Option<String>,
        limit: usize,
    ) -> std::sync::mpsc::Receiver<Result<crate::VersionsPage, String>> {
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
    ) -> std::sync::mpsc::Receiver<Result<Vec<archforge_mof::Uuid>, crate::BranchNotDeleted>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_deletes.lock().unwrap().push(tx);
        rx
    }

    /// Stored in lower case with hyphens, which no listed name shares by accident; empty is refused.
    fn branch_name(&self, typed: &str) -> Result<String, String> {
        Some(typed.trim().to_lowercase().replace(' ', "-")).filter(|s| !s.is_empty()).ok_or_else(|| crate::i18n::tr("Enter a name."))
    }

    /// Keeps the answer for the test to send.
    fn create_branch(&self, _path: &std::path::Path, name: &str, start: Option<&str>) -> std::sync::mpsc::Receiver<Result<(), crate::BranchRefused>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_starts.lock().unwrap().push(start.map(str::to_string));
        self.branch_makes.lock().unwrap().push((name.to_string(), tx));
        rx
    }

    /// Keeps the answer for the test to send.
    fn branch_counts(&self, _path: &std::path::Path, branch: &str) -> std::sync::mpsc::Receiver<Result<crate::BranchCounts, String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_counts.lock().unwrap().push((branch.to_string(), tx));
        rx
    }

    /// Keeps the answer for the test to send.
    fn rename_branch(&self, _path: &std::path::Path, from: &str, to: &str) -> std::sync::mpsc::Receiver<Result<(), crate::BranchRefused>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.branch_renames.lock().unwrap().push((from.to_string(), to.to_string(), tx));
        rx
    }

    fn open(&self, path: &std::path::Path) -> std::sync::mpsc::Receiver<Result<crate::Opened, crate::CreateError>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.opens.lock().unwrap().push((path.to_path_buf(), tx));
        rx
    }

    /// Records the stored branch it was asked for, then opens as `open` does.
    fn open_on(&self, path: &std::path::Path, branch: Option<&str>) -> std::sync::mpsc::Receiver<Result<crate::Opened, crate::CreateError>> {
        self.opened_on.lock().unwrap().push(branch.map(str::to_string));
        self.open(path)
    }

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
    fn writing(&self, _project: &std::path::Path) -> crate::Writing {
        crate::Writing { pending: false, error: self.write_error.lock().unwrap().clone() }
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

    /// Keeps the save's answer and its description for the test.
    fn save(
        &self,
        project: &std::path::Path,
        description: Option<String>,
        by: crate::SavedBy,
    ) -> std::sync::mpsc::Receiver<Result<crate::Saved, crate::SaveError>> {
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
fn saving_harness(opened: Option<crate::Opened>, picked: Option<&str>) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    let picked = picked.map(std::path::PathBuf::from);
    let services = Services {
        projects: Some(host.clone()),
        ids: Some(test_ids()),
        pick_project: Some(std::sync::Arc::new(move |_| Picked::at_once(picked.clone()))),
        ..Services::default()
    };
    let mut shell = Shell::new(services, NOTATIONS, &SCREENS);
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

/// Adds a package named `name` to the open project through the fake notation's command, and lets
/// the window settle for two frames.
fn add_schema(h: &mut Harness<'static, Shell>, name: &str) {
    h.state_mut().session.as_mut().unwrap().execute("sketch.package.add", &serde_json::json!({ "name": name })).unwrap();
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
    host.saves.lock().unwrap().pop().unwrap().send(Err(crate::SaveError::Failed("the disk is full".into()))).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().save_error.as_deref(), Some("the disk is full"));
    assert!(dirty(&h));
}

/// A saved project with a UML class and an ER table, the class selected in Properties, in
/// `theme`: (harness, host, class, table).
fn described_project(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>, archforge_mof::Uuid, archforge_mof::Uuid) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().state.theme = theme;
    let (class, table) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let pkg = id(s.execute("sketch.package.add", &serde_json::json!({ "name": "sales" })).unwrap().value);
        let class = id(s.execute("sketch.class.add", &serde_json::json!({ "package": pkg.to_string(), "name": "Order" })).unwrap().value);
        let schema = id(s.execute("sketch.package.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
        let table = id(s.execute("sketch.class.add", &serde_json::json!({ "package": schema.to_string(), "name": "customer" })).unwrap().value);
        (class, table)
    };
    h.run_steps(2);
    h.state_mut().save();
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
    h.run_steps(2);
    assert!(!dirty(&h), "saved before the description is typed");
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    (h, host, class, table)
}

/// The description an element of either notation has, as Properties shows it.
fn description_on(h: &Harness<'static, Shell>, id: archforge_mof::Uuid) -> String {
    let s = h.state().session.as_ref().unwrap();
    let o = s.doc().by_uuid(id).unwrap();
    s.view().of_element(s.doc(), o).map(|n| n.description(s.doc(), o)).unwrap_or_default()
}

/// Types `text` into the Description field of the element `id`, after what it holds, without Enter.
fn type_description(h: &mut Harness<'static, Shell>, id: archforge_mof::Uuid, text: &str) {
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
    h.run();
    let at = rect_of(h, "workspace.properties.description").center();
    click(h, at);
    h.event(egui::Event::Text(text.into()));
    h.run();
    assert!(h.state().panels.description.as_ref().is_some_and(|d| d.text.ends_with(text)), "typed, not yet written");
}

/// A description being typed, on a UML class or an ER table, is written before Cmd+S saves, so the
/// save takes it. In light and dark.
#[test]
fn a_description_being_typed_is_saved_by_cmd_s() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host, class, table) = described_project(theme);
        let unsaved_shown = |h: &mut Harness<'static, Shell>| {
            let inspected = crate::control::answer(h.state_mut(), "document.inspect", &serde_json::json!({})).unwrap();
            painted(h).iter().any(|t| t.starts_with("Unsaved changes")) && !painted(h).iter().any(|t| t == "Saved") && inspected["unsaved"] == true
        };
        for (id, text) in [(class, "One purchase."), (table, "People who buy.")] {
            type_description(&mut h, id, text);
            // Typed but not yet written: an unsaved change, on the status bar, the tab and to an agent.
            assert!(!dirty(&h) && unsaved_shown(&mut h) && h.state().tab_unsaved(0), "a draft being typed is unsaved");
            h.event(egui::Event::Key { key: egui::Key::S, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
            h.run_steps(2);
            assert_eq!(description_on(&h, id), text, "written before the save");
            assert!(h.state().panels.description.is_none(), "the draft is spent");
            assert!(h.state().is_saving(), "the save started");
            host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
            h.run_steps(2);
            assert!(!dirty(&h), "the save took the description");
        }
        // A save already running: Cmd+S waits for it and leaves the draft being typed, which still
        // shows as unsaved, also once that save is done; Enter then writes it.
        h.state_mut().session.as_mut().unwrap().execute("sketch.package.add", &serde_json::json!({ "name": "crm" })).unwrap();
        h.run_steps(2);
        h.state_mut().save();
        assert!(h.state().is_saving());
        type_description(&mut h, class, " Or two.");
        h.event(egui::Event::Key { key: egui::Key::S, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
        h.run_steps(2);
        assert_eq!(host.saves.lock().unwrap().len(), 1, "one save at a time");
        assert!(h.state().panels.description.is_some(), "the draft waits");
        // While the save runs the status bar says Saving…; the tab and an agent count the draft.
        let inspected = crate::control::answer(h.state_mut(), "document.inspect", &serde_json::json!({})).unwrap();
        assert_eq!(inspected["unsaved"], true, "the waiting draft is unsaved while the save runs");
        assert!(h.state().tab_unsaved(0) && painted(&h).iter().any(|t| t == "Saving…"));
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(2);
        assert!(!dirty(&h) && unsaved_shown(&mut h) && h.state().tab_unsaved(0), "and after it is done");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(description_on(&h, class), "One purchase. Or two.");
        h.state_mut().save();
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(2);
        assert!(!dirty(&h) && !unsaved_shown(&mut h), "saved, and Saved shows");
        // A draft the model refuses, on a clean project: the save does not start, the refusal
        // shows, the draft stays, and neither the status bar nor an agent's save says Saved.
        h.state_mut().panels.description = Some(crate::panels::DescriptionDraft { target: table, text: "a\u{7}b".into(), refused: false });
        h.event(egui::Event::Key { key: egui::Key::S, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
        h.run_steps(2);
        assert!(!h.state().is_saving(), "the save did not start");
        assert!(host.saves.lock().unwrap().is_empty(), "the host was not asked");
        assert!(h.state().panels.notice.as_deref().is_some_and(|n| n.contains("control")), "{:?}", h.state().panels.notice);
        assert!(h.state().panels.description.as_ref().is_some_and(|d| d.refused && d.text == "a\u{7}b"), "the draft stays");
        assert!(painted(&h).iter().any(|t| t.starts_with("Unsaved changes")), "not Saved: {:?}", painted(&h));
        assert!(!painted(&h).iter().any(|t| t == "Saved"));
        let refused = crate::control::answer(h.state_mut(), "app.save", &serde_json::json!({})).unwrap_err();
        assert!(refused.contains("description") && refused.contains("control"), "{refused}");
        assert!(host.saves.lock().unwrap().is_empty());
        assert_eq!(crate::control::answer(h.state_mut(), "document.inspect", &serde_json::json!({})).unwrap()["unsaved"], true);
    }
}

/// A description being typed is written before closing takes stock, so the close asks about it;
/// one whose write is refused is kept, its refusal shown, and the close asks rather than closing
/// it away, and Save then keeps the project open, saying why. In light and dark.
#[test]
fn closing_with_a_description_being_typed_keeps_it_or_asks() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host, class, table) = described_project(theme);
        type_description(&mut h, class, "One purchase.");
        h.state_mut().close_project();
        assert_eq!(description_on(&h, class), "One purchase.", "written before the close");
        assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::Unsaved { .. })), "{:?}", h.state().ask);
        h.state_mut().ask = None;
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        h.run_steps(2);
        assert!(!dirty(&h));
        // A draft the store refuses: kept, its refusal the notice, and the clean project asks.
        h.state_mut().panels.description = Some(crate::panels::DescriptionDraft { target: table, text: "a\u{7}b".into(), refused: false });
        h.state_mut().close_project();
        assert!(h.state().panels.description.as_ref().is_some_and(|d| d.refused), "kept");
        assert!(h.state().panels.notice.as_deref().is_some_and(|n| n.contains("control")), "{:?}", h.state().panels.notice);
        assert!(matches!(h.state().ask, Some(crate::close_ask::Ask::Unsaved { .. })), "{:?}", h.state().ask);
        assert!(h.state().tab_unsaved(0));
        // Save from the question: the refused draft stops the save, and the project stays open,
        // its tab saying why, rather than being closed away.
        h.state_mut().ask = None;
        h.state_mut().save_and_close(0);
        h.run_steps(2);
        assert!(host.saves.lock().unwrap().is_empty(), "no save started");
        assert_eq!(h.state().tabs.len(), 1, "not closed away");
        assert!(h.state().tabs[0].closing.is_none());
        assert!(h.state().tabs[0].close_error.as_deref().is_some_and(|e| e.contains("control")), "{:?}", h.state().tabs[0].close_error);
        assert!(h.state().panels.description.is_some(), "the draft is still there");
    }
}

/// The name of the element `id`, as the model has it.
fn name_on(h: &Harness<'static, Shell>, id: archforge_mof::Uuid) -> String {
    let st = h.state().session.as_ref().unwrap().doc();
    archforge_core::name_of(st, st.by_uuid(id).unwrap())
}

/// Selects `id` and replaces what its Properties field `field` holds with `text`, without Enter.
fn retype_on(h: &mut Harness<'static, Shell>, id: archforge_mof::Uuid, field: &str, text: &str) {
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id.to_string()] })).unwrap();
    h.run();
    retype(h, field, text);
}

/// Escape in a Properties field gives the edit up: an emptied name and a typed description come
/// back as the model has them, nothing is sent and no step is added. In light and dark (AR-417).
#[test]
fn escape_gives_up_a_properties_edit_and_sends_nothing() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _host, class, _) = described_project(theme);
        let before = h.state().session.as_ref().unwrap().undo_label().map(str::to_string);
        retype_on(&mut h, class, "workspace.properties.name", "");
        assert!(h.state().panels.name.as_ref().is_some_and(|n| n.text.is_empty()), "{theme:?}: emptied, not yet written");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        retype_on(&mut h, class, "workspace.properties.description", "Draft.");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert_eq!(name_on(&h, class), "Order", "{theme:?}");
        assert_eq!(description_on(&h, class), "", "{theme:?}");
        assert!(h.state().panels.name.is_none() && h.state().panels.description.is_none(), "{theme:?}: the drafts are gone");
        assert_eq!(h.state().panels.notice, None, "{theme:?}: nothing was refused");
        assert_eq!(h.state().session.as_ref().unwrap().undo_label().map(str::to_string), before, "{theme:?}: no step");
    }
}

/// Enter on an empty name, or one of spaces, sends nothing and says so in plain words, read out as
/// a status; another refusal is the engine's reason without its command's id; the notice, and its
/// status, go with another selection. In light and dark (AR-417, AR-420).
#[test]
fn an_empty_name_is_refused_in_plain_words_until_the_selection_moves_on() {
    use egui_kittest::kittest::{NodeT, Queryable};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _host, class, table) = described_project(theme);
        for empty in ["", "   "] {
            retype_on(&mut h, class, "workspace.properties.name", empty);
            // A session that refuses every change in its own words shows whether anything reached it.
            h.state_mut().session.as_mut().unwrap().set_read_only(true).unwrap();
            key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
            h.state_mut().session.as_mut().unwrap().set_read_only(false).unwrap();
            assert_eq!(name_on(&h, class), "Order", "{theme:?}");
            assert_eq!(h.state().panels.notice.as_deref(), Some("A name can't be empty."), "{theme:?}");
            assert!(painted(&h).iter().any(|t| t == "A name can't be empty."), "{theme:?}: said at the panel's foot");
            let status = h.get_by_label("A name can't be empty.").accesskit_node();
            assert_eq!((status.role(), status.live()), (egui::accesskit::Role::Status, egui::accesskit::Live::Polite), "{theme:?}: read out");
            assert!(h.state().panels.name.is_none() && !dirty(&h), "{theme:?}: the old name stays, nothing sent");
        }
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [table.to_string()] })).unwrap();
        h.run();
        assert_eq!(h.state().panels.notice, None, "{theme:?}: gone with the selection");
        assert!(h.query_by_label("A name can't be empty.").is_none(), "{theme:?}: and no longer read out");
        let s = h.state_mut().session.as_mut().unwrap();
        let schema = s.doc().uuid(s.doc().container(s.doc().by_uuid(table).unwrap()).unwrap().unwrap().0).unwrap();
        s.execute("sketch.class.add", &serde_json::json!({ "package": schema.to_string(), "name": "order" })).unwrap();
        retype_on(&mut h, table, "workspace.properties.name", "order");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let notice = h.state().panels.notice.clone().unwrap_or_default();
        assert!(notice.starts_with(char::is_uppercase) && notice.contains("order") && !notice.contains("rename"), "{theme:?}: {notice}");
    }
}

/// Enter with a new name still renames, and a click elsewhere still writes what was typed. In
/// light and dark (AR-417).
#[test]
fn enter_and_a_click_elsewhere_still_rename_in_properties() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _host, class, _) = described_project(theme);
        retype_on(&mut h, class, "workspace.properties.name", "Purchase");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(name_on(&h, class), "Purchase", "{theme:?}");
        retype_on(&mut h, class, "workspace.properties.name", "Sale");
        let at = rect_of(&h, "workspace.properties.description").center();
        click(&mut h, at);
        assert_eq!(name_on(&h, class), "Sale", "{theme:?}: written when left");
    }
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
    host.saves.lock().unwrap().pop().unwrap().send(Err(crate::SaveError::Failed("the disk is full".into()))).unwrap();
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

/// A hold starts alone, on a model, outside a gesture, and leaves read-only what it found.
#[test]
fn a_hold_runs_alone_on_a_model_outside_a_gesture() {
    use crate::hold::HoldRefused;
    assert_eq!(saving_harness(None, None).0.state_mut().begin_hold(), Err(HoldRefused::NoProject));
    let (mut h, _) = saving_harness(Some(commerce()), None);
    let read_only = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().read_only();
    h.state_mut().session.as_mut().unwrap().begin_interaction("sketch.package.add").unwrap();
    assert_eq!(h.state_mut().begin_hold(), Err(HoldRefused::Gesture));
    assert!(!h.state().is_held() && !read_only(&h));
    h.state_mut().session.as_mut().unwrap().cancel().unwrap();
    h.state_mut().begin_hold().unwrap();
    assert!(h.state().is_held() && read_only(&h));
    assert_eq!(h.state_mut().begin_hold(), Err(HoldRefused::Another));
    h.state_mut().end_hold();
    assert!(!h.state().is_held() && !read_only(&h), "the session can change again");
    // A session that was read-only already, as a version being viewed is, stays so.
    h.state_mut().session.as_mut().unwrap().set_read_only(true).unwrap();
    h.state_mut().begin_hold().unwrap();
    h.state_mut().end_hold();
    assert!(read_only(&h));
}

/// While held nothing changes the model or makes a version; afterwards both work again.
#[test]
fn while_held_edits_undo_and_save_are_refused() {
    use archforge_engine::EngineError;
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    h.state_mut().begin_hold().unwrap();
    let session = h.state_mut().session.as_mut().unwrap();
    assert!(matches!(session.execute("sketch.package.add", &serde_json::json!({ "name": "billing" })), Err(EngineError::ReadOnly)));
    assert!(matches!(session.undo(), Err(EngineError::ReadOnly)));
    assert!(!h.state().can_save());
    h.state_mut().save();
    h.state_mut().save_reviewed("A description".into(), 0);
    h.run_steps(2);
    assert!(host.saves.lock().unwrap().is_empty() && !h.state().is_saving(), "no save was asked for");
    h.state_mut().end_hold();
    assert!(h.state().can_save());
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.state_mut().session.as_mut().unwrap().redo().unwrap();
    h.state_mut().save();
    assert_eq!(host.saves.lock().unwrap().len(), 1, "Save works again");
}

/// While held the Save key is left alone, as the menu's Save is disabled, and afterwards it saves.
#[test]
fn the_save_key_is_left_alone_and_saves_nothing_while_held() {
    use std::sync::atomic::{AtomicBool, Ordering};
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    let mut shell = Shell::new(Services { projects: Some(host.clone()), ids: Some(test_ids()), ..Services::default() }, NOTATIONS, &SCREENS);
    shell.open_project(commerce());
    // Whether the key is still there once the shell's logic has run: taken, it is gone.
    let left = std::sync::Arc::new(AtomicBool::new(false));
    let seen = left.clone();
    let frame = move |ui: &mut egui::Ui, shell: &mut Shell| {
        shell.logic(ui.ctx());
        seen.fetch_or(ui.input(|i| i.key_pressed(egui::Key::S)), Ordering::SeqCst);
        shell.ui(ui);
    };
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(frame, shell);
    h.run_steps(4);
    add_schema(&mut h, "shop");
    let press = |h: &mut Harness<'static, Shell>| {
        h.event(egui::Event::Key { key: egui::Key::S, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
        h.run_steps(2);
    };
    h.state_mut().begin_hold().unwrap();
    press(&mut h);
    assert!(left.swap(false, Ordering::SeqCst), "while held the shell does not take the key");
    assert!(host.saves.lock().unwrap().is_empty() && !h.state().is_saving(), "and no save starts");
    h.state_mut().end_hold();
    press(&mut h);
    assert!(!left.load(Ordering::SeqCst), "after the hold it takes the key");
    assert_eq!(host.saves.lock().unwrap().len(), 1, "and saves");
}

/// Whatever puts another session in the place of the one shown moves the generation.
#[test]
fn the_session_generation_moves_with_every_session_shown() {
    let (mut h, _) = saving_harness(Some(commerce()), None);
    let moved = |h: &mut Harness<'static, Shell>, what: &str, last: &mut u64| {
        h.run_steps(3);
        let now = h.state().session_generation();
        assert!(now > *last, "{what}: {now} after {last}");
        *last = now;
    };
    let mut last = 0;
    moved(&mut h, "the first project's model", &mut last);
    h.state_mut().go_home();
    moved(&mut h, "Home", &mut last);
    h.state_mut().activate(0);
    moved(&mut h, "its tab shown from Home", &mut last);
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    moved(&mut h, "another project", &mut last);
    h.state_mut().activate(0);
    moved(&mut h, "back to the first tab", &mut last);
    h.run_steps(3);
    assert_eq!(h.state().session_generation(), last, "frames alone do not move it");
}

/// Rows go over until the hand-over, which sends the latest, and none after it.
#[test]
fn a_hold_hands_the_latest_rows_over_once_and_then_stops_writing() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    let written = || host.written.lock().unwrap().len();
    let edit = |h: &mut Harness<'static, Shell>, name: &str| {
        h.state_mut().session.as_mut().unwrap().execute("sketch.package.add", &serde_json::json!({ "name": name })).unwrap();
    };
    assert!(!h.state_mut().hand_over_held(), "there is no hold to hand over for");
    edit(&mut h, "first");
    let before = written();
    h.state_mut().begin_hold().unwrap();
    assert!(h.state_mut().hand_over_held() && h.state_mut().hand_over_held());
    assert_eq!(written(), before + 1, "the latest rows go over, once");
    // Nothing goes after it, whatever moved: the rows would reach the replaced state.
    h.state_mut().session.as_mut().unwrap().set_read_only(false).unwrap();
    edit(&mut h, "second");
    h.run_steps(3);
    assert_eq!(written(), before + 1);
    h.state_mut().end_hold();
    h.run_steps(3);
    assert_eq!(written(), before + 2, "after the hold the writing goes on");
}

/// A project open with an unsaved change.
fn edited_project() -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    (h, host)
}

/// Ends the hold and lets what waited run.
fn end_hold(h: &mut Harness<'static, Shell>, how: fn(&mut Shell)) {
    how(h.state_mut());
    h.run_steps(3);
}

/// The question a closing asks about the unsaved project at `path`.
fn asked_unsaved(path: &str) -> Option<crate::close_ask::Ask> {
    Some(crate::close_ask::Ask::Unsaved { path: path.into() })
}

const COMMERCE: &str = "/tmp/Commerce platform.archforge";

/// Changing tab, Home and opening wait, in order, and an opening's answer is not looked at.
#[test]
fn showing_another_project_and_opening_wait_for_the_hold() {
    let opened = |name: &str| crate::Opened { name: name.into(), path: format!("/tmp/{name}.archforge").into(), ..Default::default() };
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().open_project(opened("Library"));
    h.run_steps(3);
    h.state_mut().open_path(std::path::Path::new("/tmp/Before.archforge"));
    h.state_mut().begin_hold().unwrap();
    let held = h.state().active;
    h.state_mut().activate(0);
    h.state_mut().go_home();
    h.state_mut().open_path(std::path::Path::new("/tmp/During.archforge"));
    h.state_mut().open_project(opened("Made"));
    host.opens.lock().unwrap().pop().unwrap().1.send(Ok(opened("Before"))).unwrap();
    h.run_steps(3);
    assert_eq!((h.state().active, h.state().tabs.len()), (held, 2), "the project held stays shown, and no answer was taken");
    assert!(host.opens.lock().unwrap().is_empty(), "nothing more was asked to open");
    h.state_mut().end_hold();
    assert_eq!(h.state_mut().begin_hold(), Err(crate::hold::HoldRefused::Another), "what waited runs before the next hold");
    h.run_steps(3);
    let names: Vec<&str> = h.state().tabs.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Commerce platform", "Library", "Before", "Made"], "the answer, then what waited");
    assert_eq!(h.state().active, Some(3), "shown last: the project made");
    // Going back to the held tab is the latest choice, and it stands.
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().open_project(opened("Library"));
    h.run_steps(3);
    through_hold(
        &mut h,
        &host,
        |s| {
            s.activate(0);
            s.activate(1);
        },
        Shell::end_hold,
    );
    assert_eq!(h.state().active, Some(1));
    // With no opening under way, the host is asked only after the hold.
    let (mut h, host) = saving_harness(Some(commerce()), None);
    through_hold(&mut h, &host, |s| s.open_path(std::path::Path::new("/tmp/Later.archforge")), Shell::end_hold);
    assert_eq!(host.opens.lock().unwrap().len(), 1);
}

/// An opening waiting for the hold keeps every folder pick off, Pin a folder and Change… among
/// them: a pick waiting when the hold ends would turn that opening away (AR-737).
#[test]
fn an_opening_waiting_for_the_hold_keeps_folder_picks_off_until_it_runs() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = asked.clone();
    h.state_mut().services.pick_folder = Some(std::sync::Arc::new(move |_: &str| {
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::sync::mpsc::channel().1
    }));
    h.state_mut().begin_hold().unwrap();
    h.state_mut().open_path(std::path::Path::new("/tmp/Later.archforge"));
    let off = |s: &Shell| !crate::actions::available(s, "home.pin-folder") && !crate::new_project::can_change(s, false);
    assert!(off(h.state()), "Pin a folder and Change… off while the opening waits");
    h.state_mut().pin_from_picker();
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0, "no picker asked");
    end_hold(&mut h, Shell::end_hold);
    assert_eq!(host.opens.lock().unwrap().len(), 1, "the opening runs after the hold");
    assert!(!off(h.state()), "and the pickers are back");
}

/// Holds `h`, does `during`, checks that it acted on nothing, and ends the hold as `how`.
fn through_hold(h: &mut Harness<'static, Shell>, host: &SavingHost, during: impl FnOnce(&mut Shell), how: fn(&mut Shell)) {
    h.state_mut().begin_hold().unwrap();
    during(h.state_mut());
    h.run_steps(3);
    let asked = (host.saves.lock().unwrap().len(), host.closed.lock().unwrap().len(), host.opens.lock().unwrap().len());
    assert_eq!((h.state().ask.clone(), asked, h.state().quitting()), (None, (0, 0, 0), false), "nothing runs while held");
    end_hold(h, how);
}

/// Closes and quits wait, those under way before the hold too, then run one question at a time.
#[test]
fn a_close_and_a_quit_wait_for_the_hold_and_then_run() {
    use crate::hold::Waiting;
    let (mut h, host) = edited_project();
    through_hold(&mut h, &host, |s| s.close_tab(0), Shell::end_hold);
    assert_eq!(h.state().ask, asked_unsaved(COMMERCE), "a close asks its usual question after the hold");
    let (mut h, host) = edited_project();
    through_hold(&mut h, &host, |s| s.save_and_close(0), Shell::end_hold);
    assert_eq!(host.saves.lock().unwrap().len(), 1, "Save answered while held is kept");
    // A closing waiting on a save from before the hold waits too.
    let (mut h, host) = edited_project();
    h.state_mut().save_and_close(0);
    let answer = host.saves.lock().unwrap().pop().unwrap();
    through_hold(&mut h, &host, |_| answer.send(Ok(true)).unwrap(), Shell::end_hold);
    assert_eq!(host.closed.lock().unwrap().len(), 1);
    // A quit waits and is kept once: Cancel stops all of it.
    let (mut h, host) = edited_project();
    let twice = |s: &mut Shell| assert!(s.remember(Waiting::Quit) && s.remember(Waiting::Quit));
    through_hold(&mut h, &host, twice, Shell::end_hold);
    assert_eq!(h.state().ask, asked_unsaved(COMMERCE));
    h.state_mut().ask = None;
    h.state_mut().cancel_quit();
    h.run_steps(3);
    assert_eq!(h.state().ask, None, "the second request was the same quit");
    let (mut h, host) = saving_harness(Some(commerce()), None);
    through_hold(&mut h, &host, Shell::request_quit, Shell::end_hold);
    assert!(h.state().quit_confirmed(), "nothing unsaved: the quit goes through");
    // A quit under way asks its next question only after the hold.
    let (mut h, _) = edited_project();
    h.state_mut().request_quit();
    h.run_steps(2);
    h.state_mut().ask = None;
    h.state_mut().begin_hold().unwrap();
    h.run_steps(3);
    assert_eq!(h.state().ask, None);
    end_hold(&mut h, Shell::end_hold);
    assert_eq!(h.state().ask, asked_unsaved(COMMERCE));
    // Two closes ask in turn: the second question does not replace the first.
    let (mut h, host) = edited_project();
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    h.run_steps(3);
    add_schema(&mut h, "books");
    through_hold(
        &mut h,
        &host,
        |s| {
            s.close_tab(0);
            s.close_tab(1);
        },
        Shell::end_hold,
    );
    assert_eq!(h.state().ask, asked_unsaved(COMMERCE));
    h.state_mut().ask = None;
    h.run_steps(3);
    assert_eq!(h.state().ask, asked_unsaved("/tmp/Library.archforge"));
}

/// After a hold that failed, a close that waited asks instead of closing, in both appearances.
#[test]
fn a_close_that_waited_for_a_failed_hold_asks_first() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        through_hold(&mut h, &host, |s| s.close_tab(0), Shell::fail_hold);
        assert_eq!(h.state().ask, Some(crate::close_ask::Ask::HoldFailed { path: COMMERCE.into(), held: "Commerce platform".into() }));
        assert!(host.closed.lock().unwrap().is_empty(), "it did not run on a state nobody saw");
        assert!(painted(&h).iter().any(|t| t == "Close Commerce platform?"), "{:?}", painted(&h));
        let at = rect_of(&h, "close-ask.go-on").center();
        click(&mut h, at);
        h.run_steps(2);
        assert_eq!(host.closed.lock().unwrap().len(), 1, "closed once the person said so");
    }
    // Unsaved work asks the usual question, and answers given while held are asked again.
    let (mut h, host) = edited_project();
    through_hold(
        &mut h,
        &host,
        |s| {
            s.save_and_close(0);
            s.discard_tab(0, true);
        },
        Shell::fail_hold,
    );
    assert_eq!(h.state().ask, asked_unsaved(COMMERCE));
    assert!(host.saves.lock().unwrap().is_empty() && host.discarded.lock().unwrap().is_empty());
    // Closing another tab asks about that tab and names the project whose change failed.
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    h.run_steps(3);
    through_hold(&mut h, &host, |s| s.close_tab(0), Shell::fail_hold);
    assert_eq!(h.state().ask, Some(crate::close_ask::Ask::HoldFailed { path: COMMERCE.into(), held: "Library".into() }));
    assert!(painted(&h).iter().any(|t| t == "Close Commerce platform?"), "{:?}", painted(&h));
    assert!(painted(&h).iter().any(|t| t.starts_with("The last change to Library")), "{:?}", painted(&h));
    // A hold that finishes runs what waited, whatever the last one did.
    let (mut h, host) = edited_project();
    through_hold(&mut h, &host, |s| s.close_tab(0), Shell::fail_hold);
    h.state_mut().ask = None;
    through_hold(&mut h, &host, |s| s.save_and_close(0), Shell::end_hold);
    assert_eq!(host.saves.lock().unwrap().len(), 1);
}

/// After a hold that failed, a quit that waited asks, in both appearances, and Keep open stops it.
#[test]
fn a_quit_that_waited_for_a_failed_hold_asks_first() {
    // With Home shown by then, it still asks, about the project that was held.
    let (mut h, host) = saving_harness(Some(commerce()), None);
    let home_then_quit = |s: &mut Shell| {
        s.go_home();
        assert!(s.remember(crate::hold::Waiting::Quit));
    };
    through_hold(&mut h, &host, home_then_quit, Shell::fail_hold);
    assert_eq!(h.state().ask, Some(crate::close_ask::Ask::HoldFailed { path: COMMERCE.into(), held: "Commerce platform".into() }));
    assert!(!h.state().quit_confirmed());
    let (mut h, host) = saving_harness(Some(commerce()), None);
    for (theme, go_on) in [(ThemeKind::Light, false), (ThemeKind::Dark, true)] {
        h.state_mut().state.theme = theme;
        through_hold(&mut h, &host, Shell::request_quit, Shell::fail_hold);
        assert!(!h.state().quit_confirmed() && painted(&h).iter().any(|t| t == "Quit Archforge?"), "{:?}", painted(&h));
        let at = rect_of(&h, if go_on { "close-ask.go-on" } else { "close-ask.keep" }).center();
        click(&mut h, at);
        h.run_steps(3);
        assert_eq!((h.state().quit_confirmed(), h.state().quitting()), (go_on, false));
    }
}

/// The rows of the model shown, as a host reads them.
fn rows_now(h: &Harness<'static, Shell>) -> std::sync::Arc<archforge_mof::rows::Rows> {
    let (tables, junctions) = NOTATIONS.storage();
    std::sync::Arc::new(archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap())
}

/// A job, whether it has started, where its answer goes, and where its reading back goes.
type TestJob = (
    crate::replace::Job,
    std::sync::Arc<std::sync::atomic::AtomicBool>,
    std::sync::mpsc::Sender<Result<(), crate::replace::JobFailure>>,
    std::sync::mpsc::Sender<Result<bool, String>>,
);

/// A job that tells when it starts and answers when the test says.
fn test_job() -> TestJob {
    let (answer, answers) = std::sync::mpsc::channel();
    let (reading, readings) = std::sync::mpsc::channel();
    let started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = started.clone();
    let run = Box::new(move || {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        answers
    });
    (crate::replace::Job { run, read_back: Box::new(move || readings) }, started, answer, reading)
}

/// Runs a replacement, doing `during` once it holds, whose job's reply and reading back are both
/// lost, which leaves the project held until closed.
fn held_until_closed(h: &mut Harness<'static, Shell>, schema: &str, during: impl FnOnce(&mut Shell)) {
    let empty = rows_now(h);
    add_schema(h, schema);
    let (job, _, answer, reading) = test_job();
    let read = start_replacing(h, Some(job));
    during(h.state_mut());
    read.send(Ok(empty)).unwrap();
    h.run_steps(3);
    drop(answer);
    h.run_steps(2);
    drop(reading);
    h.run_steps(3);
}

/// Starts replacing the session; gives where the model read goes.
fn start_replacing(h: &mut Harness<'static, Shell>, job: Option<crate::replace::Job>) -> std::sync::mpsc::Sender<crate::VersionRead> {
    let (read, answer) = std::sync::mpsc::channel();
    h.state_mut().begin_replacement(answer, job).unwrap();
    h.run_steps(4);
    read
}

/// Frames that wait out a replacement's comparison on another thread.
trait Steps {
    /// `n` frames, not counting those a replacement spends waiting for its comparison.
    fn steps(&mut self, n: usize);
}

impl Steps for Harness<'static, Shell> {
    /// Gives up past a generous bound, so a comparison that never answers fails the test.
    fn steps(&mut self, n: usize) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut left = n;
        while left > 0 {
            assert!(std::time::Instant::now() < until, "still comparing");
            let waiting = self.state_mut().comparing();
            self.run_steps(1);
            left -= usize::from(!waiting);
        }
    }
}

/// A project with unsaved work, and the rows it held before that work: the earlier version.
fn work_and_earlier() -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>, std::sync::Arc<archforge_mof::rows::Rows>) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    let earlier = rows_now(&h);
    add_schema(&mut h, "shop");
    (h, host, earlier)
}

/// Starts viewing `commit`, its read not answered yet.
fn start_viewing(h: &mut Harness<'static, Shell>, commit: &str) {
    h.state_mut().view_version(commit).unwrap();
    h.run_steps(3);
}

/// Answers the oldest read waiting.
fn answer_read(h: &mut Harness<'static, Shell>, host: &SavingHost, answer: crate::VersionRead) {
    host.reads.lock().unwrap().remove(0).1.send(answer).unwrap();
    h.run_steps(4);
}

/// Asserts that the real work shows, editable, and that no version does.
fn the_work_shows(h: &Harness<'static, Shell>) {
    assert_eq!(h.state().viewing(), None);
    assert!(!h.state().is_held());
    assert!(schema_names(h).contains(&"shop".to_string()));
    assert!(!h.state().session.as_ref().unwrap().read_only());
}

/// The version takes the session's place read-only; going back brings every part of the work back.
#[test]
fn a_version_shows_read_only_and_going_back_restores_the_work() {
    let (mut h, host, earlier) = work_and_earlier();
    let (diagram, shop) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let shop = s.execute("sketch.package.add", &serde_json::json!({ "name": "till" })).unwrap().value["id"].as_str().unwrap().to_string();
        let d = s.execute("diagram.add", &serde_json::json!({ "kind": SKETCH, "element": shop, "name": "Till" })).unwrap().value;
        (archforge_mof::Uuid::parse(d["id"].as_str().unwrap()).unwrap(), shop)
    };
    crate::canvas::open(h.state_mut(), diagram);
    h.run_steps(3);
    h.state_mut().canvas.pan = egui::vec2(31.0, 47.0);
    h.state_mut().canvas.zoom = 1.5;
    let s = h.state_mut().session.as_mut().unwrap();
    s.execute("view.select", &serde_json::json!({ "ids": [shop] })).unwrap();
    let selection = s.view().selection.clone();
    let generation = h.state().session_generation();
    let shell = h.state_mut();
    shell.save_error = Some("kept".into());
    (shell.explorer.filter, shell.diagrams.filter) = ("in tree".into(), "in list".into());
    shell.panels.menu_scroll = Some(("menu".into(), 9.0));

    start_viewing(&mut h, "c1");
    assert_eq!(h.state().viewing(), None, "nothing shows before the read answers");
    answer_read(&mut h, &host, Ok(earlier));
    assert_eq!(h.state().viewing(), Some("c1"));
    assert!(!schema_names(&h).contains(&"shop".to_string()));
    assert!(!h.state().is_held());
    assert!(h.state().session.as_ref().unwrap().read_only());
    assert!(h.state().canvas.tabs.is_empty(), "the version starts with its own view");
    assert_eq!(h.state().session_generation(), generation + 1);
    // No `retain_valid` guard is needed: these frames prune the version's own view, which holds
    // nothing of the work, while the tabs and selection of the work, absent from it, stay aside.
    h.run_steps(3);

    assert!(h.state_mut().back_to_current_work());
    h.run_steps(3);
    the_work_shows(&h);
    assert_eq!(h.state().session_generation(), generation + 2);
    let canvas = &h.state().canvas;
    assert_eq!((canvas.tabs.clone(), canvas.active, canvas.pan, canvas.zoom), (vec![diagram], Some(diagram), egui::vec2(31.0, 47.0), 1.5));
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, selection);
    assert!(h.state().session.as_ref().unwrap().is_dirty());
    let shell = h.state();
    assert_eq!(shell.save_error.as_deref(), Some("kept"));
    assert_eq!((shell.explorer.filter.as_str(), shell.diagrams.filter.as_str()), ("in tree", "in list"));
    assert_eq!(shell.panels.menu_scroll, Some(("menu".into(), 9.0)));
    let s = h.state_mut().session.as_mut().unwrap();
    s.undo().unwrap();
    s.undo().unwrap();
    assert!(
        !h.state().session.as_ref().unwrap().doc().alive().any(|o| archforge_core::name_of(h.state().session.as_ref().unwrap().doc(), o) == "till")
    );
    assert!(!h.state_mut().back_to_current_work(), "nothing to go back from");
}

/// A version writes nothing, repairs included, and the real work's answers stay the real work's.
#[test]
fn nothing_is_written_saved_or_kept_while_a_version_shows() {
    let (mut h, host, earlier) = work_and_earlier();
    h.state_mut().canvas.zoom = 2.0;
    h.run_steps(2);
    let path = std::path::PathBuf::from(COMMERCE);
    let view = h.state().state.view_of(&path).cloned().unwrap();
    assert_eq!(view.zoom, 2.0);
    let written = host.written.lock().unwrap().len();
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    assert!(!h.state().session.as_ref().unwrap().is_dirty());
    h.run_steps(4);
    assert_eq!(host.written.lock().unwrap().len(), written);
    assert_eq!(h.state().state.view_of(&path), Some(&view));
    assert!(h.state().tab_unsaved(0), "the set-aside work is what is unsaved");
    assert!(!h.state().can_save());
    // With something handed over, so that only viewing stands in the way.
    h.state_mut().written = Some(h.state().session.as_ref().unwrap().revision());
    h.state_mut().start_save(None, crate::SavedBy::Person("Ada".into()));
    h.state_mut().written = None;
    assert!(host.saved_by.lock().unwrap().is_empty());
    assert!(h.state_mut().back_to_current_work());
    h.run_steps(3);
    assert_eq!(host.written.lock().unwrap().len(), written, "the work counts as handed over still");
}

/// Every way of leaving the project acts on the real work, not on the version shown.
#[test]
fn every_way_out_returns_to_the_work_first() {
    type Check = fn(&mut Harness<'static, Shell>, &SavingHost);
    let asked: Check = |h, _| assert_eq!(h.state().ask, asked_unsaved(COMMERCE));
    let parked: Check = |h, _| {
        h.state_mut().activate(0);
        h.run_steps(3);
        the_work_shows(h);
        assert!(dirty(h));
    };
    let routes: [(fn(&mut Shell), Check); 6] = [
        (Shell::go_home, parked),
        (|s| s.activate(1), parked),
        (|s| s.close_tab(0), asked),
        (Shell::request_quit, asked),
        (|s| s.save_and_close(0), |_, host| assert_eq!(host.saved_by.lock().unwrap().len(), 1)),
        (|s| s.discard_tab(0, true), |_, host| assert_eq!(*host.discarded.lock().unwrap(), vec![std::path::PathBuf::from(COMMERCE)])),
    ];
    for (route, check) in routes {
        let (mut h, host, earlier) = work_and_earlier();
        h.state_mut().open_project(crate::Opened { name: "Other".into(), path: "/tmp/Other.archforge".into(), ..Default::default() });
        h.run_steps(3);
        h.state_mut().activate(0);
        h.run_steps(3);
        start_viewing(&mut h, "c1");
        answer_read(&mut h, &host, Ok(earlier));
        assert_eq!(h.state().viewing(), Some("c1"));
        route(h.state_mut());
        h.run_steps(3);
        assert_eq!(h.state().viewing(), None);
        check(&mut h, &host);
    }
}

/// A read that fails, is refused or stops ends the hold, says why and changes nothing.
#[test]
fn a_read_that_fails_leaves_the_work_and_says_why() {
    for refused in [true, false] {
        let (mut h, host, _) = work_and_earlier();
        start_viewing(&mut h, "c1");
        if refused {
            answer_read(&mut h, &host, Err("Too many versions".to_string().into()));
        } else {
            host.reads.lock().unwrap().clear();
            h.run_steps(4);
        }
        the_work_shows(&h);
        let said = h.state().save_error.clone().unwrap();
        assert_eq!(said.contains("Too many"), refused, "{said}");
        h.state_mut().session.as_mut().unwrap().execute("sketch.package.add", &serde_json::json!({ "name": "after" })).unwrap();
    }
}

/// Going back during the read leaves its answer nothing to replace: it only goes into the cache.
#[test]
fn going_back_during_the_read_lets_no_late_answer_land() {
    let (mut h, host, earlier) = work_and_earlier();
    start_viewing(&mut h, "c1");
    let (_, late) = host.reads.lock().unwrap().remove(0);
    assert!(h.state_mut().back_to_current_work());
    h.run_steps(3);
    the_work_shows(&h);
    late.send(Ok(earlier.clone())).unwrap();
    h.run_steps(3);
    the_work_shows(&h);
    assert_eq!(h.state().loads.kept_commits(), ["c1"], "the late answer lands nowhere but the cache");
    start_viewing(&mut h, "c2");
    answer_read(&mut h, &host, Ok(earlier));
    assert_eq!(h.state().viewing(), Some("c2"));
}

/// A close or a quit asked for during the read waits for it, then asks about the real work.
#[test]
fn closing_or_quitting_during_the_read_asks_about_the_work() {
    let routes: [fn(&mut Shell); 2] = [|s| s.close_tab(0), Shell::request_quit];
    for route in routes {
        let (mut h, host, earlier) = work_and_earlier();
        start_viewing(&mut h, "c1");
        route(h.state_mut());
        h.run_steps(2);
        assert_eq!(h.state().ask, None, "it waits for the read");
        answer_read(&mut h, &host, Ok(earlier));
        h.run_steps(3);
        the_work_shows(&h);
        assert_eq!(h.state().ask, asked_unsaved(COMMERCE));
    }
}

const OTHER: &str = "/tmp/Other.archforge";

/// A project opened at `/tmp/{name}.archforge`.
fn project_named(name: &str) -> crate::Opened {
    crate::Opened { name: name.into(), path: format!("/tmp/{name}.archforge").into(), ..Default::default() }
}

/// The path of the project shown.
fn shown_project(h: &Harness<'static, Shell>) -> Option<std::path::PathBuf> {
    h.state().project.as_ref().map(|p| p.path.clone())
}

/// Commerce platform with `shop` unsaved, shown, and a project at `OTHER` holding `till` in the
/// tab before or after it. Gives both tabs and the rows Commerce platform held before `shop`.
fn beside_another(
    other_first: bool,
) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>, usize, usize, std::sync::Arc<archforge_mof::rows::Rows>) {
    let [first, second] = if other_first { ["Other", "Commerce platform"] } else { ["Commerce platform", "Other"] };
    let (own, other) = if other_first { (1, 0) } else { (0, 1) };
    let (mut h, host) = saving_harness(Some(project_named(first)), None);
    h.state_mut().open_project(project_named(second));
    h.run_steps(3);
    h.state_mut().activate(other);
    h.run_steps(3);
    add_schema(&mut h, "till");
    h.state_mut().activate(own);
    h.run_steps(3);
    let earlier = rows_now(&h);
    add_schema(&mut h, "shop");
    (h, host, own, other, earlier)
}

/// Asserts that the project at `OTHER` shows, with its own work and editable.
fn the_other_shows(h: &Harness<'static, Shell>) {
    assert_eq!(shown_project(h).as_deref(), Some(std::path::Path::new(OTHER)));
    let names = schema_names(h);
    assert!(names.contains(&"till".to_string()) && !names.contains(&"shop".to_string()), "{names:?}");
    assert!(!h.state().session.as_ref().unwrap().read_only());
}

/// Whichever project shows next, the work set aside goes back to the project it came from.
#[test]
fn the_work_goes_back_to_its_own_project_on_every_path() {
    let routes: [(fn(&mut Shell, usize), Option<&str>); 4] = [
        (|s, other| s.activate(other), Some(OTHER)),
        (|s, _| s.go_home(), None),
        (|s, _| s.open_project(project_named("Third")), Some("/tmp/Third.archforge")),
        (|s, _| s.open_project(project_named("Other")), Some(OTHER)),
    ];
    for other_first in [false, true] {
        for (route, shows) in routes {
            let (mut h, host, own, other, earlier) = beside_another(other_first);
            start_viewing(&mut h, "c1");
            answer_read(&mut h, &host, Ok(earlier));
            route(h.state_mut(), other);
            h.run_steps(3);
            assert_eq!(h.state().viewing(), None);
            assert_eq!(shown_project(&h).as_deref(), shows.map(std::path::Path::new));
            if shows == Some(OTHER) {
                the_other_shows(&h);
            }
            assert!(!h.state_mut().back_to_current_work(), "nothing went along to the project shown");
            h.state_mut().activate(own);
            h.run_steps(3);
            assert_eq!(shown_project(&h).as_deref(), Some(std::path::Path::new(COMMERCE)));
            the_work_shows(&h);
            assert!(dirty(&h));
        }
    }
}

/// A project that closes takes what was set aside with it: the next one shown cannot get it back.
#[test]
fn clearing_the_workspace_leaves_nothing_for_the_next_project() {
    for other_first in [false, true] {
        let (mut h, host, own, _, earlier) = beside_another(other_first);
        start_viewing(&mut h, "c1");
        answer_read(&mut h, &host, Ok(earlier));
        h.state_mut().closed(own);
        h.run_steps(3);
        assert_eq!(h.state().viewing(), None);
        assert!(!h.state_mut().back_to_current_work());
        the_other_shows(&h);
    }
}

/// What is asked for during the read acts on the tab it named, not on the project being read.
#[test]
fn a_tab_named_during_the_read_is_the_one_acted_on() {
    for other_first in [false, true] {
        let (mut h, host, own, other, earlier) = beside_another(other_first);
        start_viewing(&mut h, "c1");
        h.state_mut().activate(other);
        answer_read(&mut h, &host, Ok(earlier));
        h.run_steps(3);
        assert_eq!(h.state().viewing(), None);
        the_other_shows(&h);
        h.state_mut().activate(own);
        h.run_steps(3);
        the_work_shows(&h);

        // A tab not shown goes without touching the version, which keeps its own project.
        let (mut h, host, _, other, earlier) = beside_another(other_first);
        start_viewing(&mut h, "c1");
        h.state_mut().discard_tab(other, true);
        answer_read(&mut h, &host, Ok(earlier));
        h.run_steps(3);
        assert_eq!(*host.discarded.lock().unwrap(), vec![std::path::PathBuf::from(OTHER)]);
        assert_eq!(h.state().viewing(), Some("c1"));
        assert!(h.state_mut().back_to_current_work());
        h.run_steps(3);
        assert_eq!(shown_project(&h).as_deref(), Some(std::path::Path::new(COMMERCE)));
        the_work_shows(&h);
    }
}

/// Closing without asking answers for the work: the host closes the work's project, and a write of
/// it that failed keeps it open, said on its tab.
#[test]
fn closing_without_asking_stops_on_the_works_refused_write() {
    let (mut h, host, earlier) = work_and_earlier();
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    h.state_mut().close_tab_now(0);
    h.run_steps(2);
    assert_eq!(h.state().viewing(), None);
    assert_eq!(host.closed.lock().unwrap().as_slice(), [std::path::PathBuf::from(COMMERCE)]);
    host.closings.lock().unwrap().remove(0).send(Err("could not write".into())).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().tabs[0].close_error.as_deref(), Some("could not write"));
}

/// A tab closed while another shows hands its last edit over before the host closes it, the rows
/// built off the interface thread.
#[test]
fn closing_a_parked_tab_hands_its_last_edit_over_off_the_frame() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().open_project(crate::Opened { name: "Other".into(), path: "/tmp/Other.archforge".into(), ..Default::default() });
    h.run_steps(2);
    let parked = h.state_mut().tabs[0].parked.as_mut().and_then(|p| p.session.as_mut()).unwrap();
    parked.execute("sketch.package.add", &serde_json::json!({ "name": "shop" })).unwrap();
    let built = archforge_mof::rows::built_here();
    h.state_mut().close_tab_now(0);
    assert_eq!(archforge_mof::rows::built_here(), built, "no rows were built on the frame");
    let (tables, junctions) = NOTATIONS.storage();
    let session = h.state().tabs[0].parked.as_ref().and_then(|p| p.session.as_ref()).unwrap();
    let edit = archforge_mof::rows::snapshot(session.doc(), &tables, &junctions).unwrap();
    assert_eq!(host.written.lock().unwrap().last(), Some(&(std::path::PathBuf::from(COMMERCE), edit)), "the edit was handed over");
    assert_eq!(host.closed.lock().unwrap().as_slice(), [std::path::PathBuf::from(COMMERCE)]);
}

/// What a refusal while a version is in view starts with, whatever language the sentence is in.
fn viewing_word() -> String {
    format!("{}: ", crate::control::VIEWING)
}

/// Asserts that `answer` is the refusal while a version is in view: its word tells it from the
/// hold's, a read-only token's and every other refusal, whatever the sentence after it says.
#[track_caller]
fn refused_for_viewing(answer: Result<serde_json::Value, String>, what: &str) {
    match answer {
        Err(refused) => assert!(refused.starts_with(&viewing_word()), "{what}: {refused}"),
        Ok(value) => panic!("{what} ran: {value}"),
    }
}

/// A request as the channel brings it, accepted now.
fn accepted(h: &Harness<'static, Shell>, method: &str, params: serde_json::Value, read_only: bool) -> crate::control::ControlRequest {
    crate::control::ControlRequest {
        method: method.into(),
        params,
        origin: None,
        conversation: None,
        reply: std::sync::mpsc::channel().0,
        state: Default::default(),
        read_only,
        live: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        generation: h.state().session_generation(),
    }
}

/// Every request that may change something; a view command counts, as it does for a read-only token.
fn changing_requests() -> Vec<(&'static str, serde_json::Value)> {
    let add = serde_json::json!({ "command": "sketch.package.add", "params": { "name": "x" } });
    let mixed = serde_json::json!({ "steps": [{ "command": "sketch.package.list" }, add] });
    vec![
        ("engine.execute", add.clone()),
        ("engine.execute", serde_json::json!({ "command": "view.select", "params": { "ids": [] } })),
        ("engine.batch", serde_json::json!({ "steps": [add] })),
        ("engine.batch", mixed),
        ("engine.undo", serde_json::json!({})),
        ("engine.redo", serde_json::json!({})),
        ("app.save", serde_json::json!({})),
    ]
}

/// Asserts that each changing request is refused for the version, by either kind of token, by the
/// window's own side too, and that the session shown did not move.
fn every_change_is_refused(h: &mut Harness<'static, Shell>) {
    let revision = h.state().session.as_ref().unwrap().revision();
    for (method, params) in changing_requests() {
        for read_only in [false, true] {
            let request = accepted(h, method, params.clone(), read_only);
            refused_for_viewing(crate::control::answer_request(h.state_mut(), &request), method);
        }
        refused_for_viewing(crate::control::answer(h.state_mut(), method, &params), method);
    }
    assert_eq!(h.state().session.as_ref().unwrap().revision(), revision);
}

/// What `document.inspect` says.
fn inspected(h: &mut Harness<'static, Shell>) -> serde_json::Value {
    let request = accepted(h, "document.inspect", serde_json::json!({}), false);
    crate::control::answer_request(h.state_mut(), &request).unwrap()
}

/// A version in view refuses every change and answers every read about what shows; back at the
/// work the same requests run.
#[test]
fn an_agent_can_read_a_version_and_change_nothing_until_it_goes_back() {
    let (mut h, host, earlier) = work_and_earlier();
    assert_eq!(inspected(&mut h)["viewing"], serde_json::Value::Null);
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    let saves = host.saved_by.lock().unwrap().len();
    every_change_is_refused(&mut h);
    assert_eq!(host.saved_by.lock().unwrap().len(), saves, "no save started");

    let listing = serde_json::json!({ "command": "sketch.package.list" });
    let batch = serde_json::json!({ "steps": [listing.clone(), listing.clone()] });
    for (method, params) in [
        ("engine.commands", serde_json::json!({})),
        ("engine.describe", serde_json::json!({ "command": "sketch.package.add" })),
        ("ui.inspect", serde_json::json!({})),
    ] {
        let request = accepted(&h, method, params, false);
        assert!(crate::control::answer_request(h.state_mut(), &request).is_ok(), "{method}");
    }
    let long = serde_json::json!({ "steps": vec![listing.clone(); crate::control::MAX_STEPS + 1] });
    assert!(crate::control::answer(h.state_mut(), "engine.batch", &long).unwrap_err().contains("1 to 256"), "its own reason");
    for (method, params) in [("engine.execute", listing.clone()), ("engine.batch", batch)] {
        let request = accepted(&h, method, params, true);
        let answer = crate::control::answer_request(h.state_mut(), &request).unwrap().to_string();
        assert!(!answer.contains("shop"), "{method} describes the version: {answer}");
    }
    let saved = accepted(&h, "app.saved", serde_json::json!({ "ticket": 99 }), false);
    assert!(crate::control::answer_request(h.state_mut(), &saved).unwrap_err().contains("no save 99"));
    let shown = inspected(&mut h);
    assert_eq!(shown["viewing"], serde_json::json!({ "version": "c1", "reading": false, "work_unsaved": true }));
    assert_eq!((&shown["unsaved"], &shown["undo"]), (&serde_json::json!(false), &serde_json::Value::Null), "the rest is the version's");

    assert!(h.state_mut().back_to_current_work());
    h.run_steps(3);
    assert_eq!(inspected(&mut h)["viewing"], serde_json::Value::Null);
    let add = accepted(&h, "engine.execute", serde_json::json!({ "command": "sketch.package.add", "params": { "name": "after" } }), false);
    assert!(crate::control::answer_request(h.state_mut(), &add).is_ok());
    assert!(schema_names(&h).contains(&"after".to_string()));
    let save = accepted(&h, "app.save", serde_json::json!({}), false);
    assert!(crate::control::answer_request(h.state_mut(), &save).is_ok());
    assert_eq!(host.saved_by.lock().unwrap().len(), saves + 1);
}

/// While the version is still being read nothing can be written, and the refusal is the viewing one.
#[test]
fn a_version_still_being_read_counts_as_viewing() {
    let (mut h, host, earlier) = work_and_earlier();
    start_viewing(&mut h, "c1");
    every_change_is_refused(&mut h);
    assert_eq!(inspected(&mut h)["viewing"], serde_json::json!({ "version": "c1", "reading": true, "work_unsaved": true }));
    assert!(schema_names(&h).contains(&"shop".to_string()), "the work still shows");
    answer_read(&mut h, &host, Ok(earlier));
    assert_eq!(inspected(&mut h)["viewing"]["reading"], false);
}

/// A request waiting its turn carries the session it was accepted under, so it lands on neither the
/// version nor the work when the other has taken its place, whatever it asks.
#[test]
fn a_request_accepted_before_a_change_of_session_runs_on_neither() {
    let from_another_session = |answer: Result<serde_json::Value, String>| answer.is_err_and(|why| why.contains("after this request was sent"));
    let add = serde_json::json!({ "command": "sketch.package.add", "params": { "name": "x" } });
    let (mut h, host, earlier) = work_and_earlier();
    let before_viewing = accepted(&h, "engine.execute", add.clone(), false);
    let reading_before = accepted(&h, "document.inspect", serde_json::json!({}), false);
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    for request in [&before_viewing, &reading_before] {
        assert!(from_another_session(crate::control::answer_request(h.state_mut(), request)));
    }
    let while_viewing = accepted(&h, "engine.execute", add, false);
    let revision = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().revision();
    assert!(h.state_mut().back_to_current_work());
    h.run_steps(3);
    let work = revision(&h);
    assert!(from_another_session(crate::control::answer_request(h.state_mut(), &while_viewing)));
    assert_eq!(revision(&h), work);
}

/// The word stays the same in every language and the sentence after it is translated; the
/// sentence says how to return.
#[test]
fn the_refusal_keeps_its_word_and_translates_its_sentence() {
    assert_eq!(crate::control::VIEWING, "viewing", "agents rely on the word");
    let (mut h, host, earlier) = work_and_earlier();
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    let mut sentence_in = |language: &str| {
        crate::i18n::set_language(language);
        let refused = crate::control::answer(h.state_mut(), "app.save", &serde_json::json!({})).unwrap_err();
        crate::i18n::set_language("en");
        refused.strip_prefix(&viewing_word()).unwrap_or_else(|| panic!("the word stays as it is: {refused}")).to_string()
    };
    let (english, german) = (sentence_in("en"), sentence_in("de"));
    assert!(english.contains("current work"), "it says how to return: {english}");
    assert!(!german.is_empty());
    assert_ne!(german, english, "the sentence is translated");
}

/// A conversation on a project a version can be read from, and the rows that version holds.
fn chat_and_a_version(
    theme: ThemeKind,
) -> (Harness<'static, Shell>, crate::chat_fixture::Rig, std::sync::Arc<SavingHost>, std::sync::Arc<archforge_mof::rows::Rows>) {
    let (mut h, rig) = chat_harness(theme, true);
    let host = std::sync::Arc::new(SavingHost::default());
    h.state_mut().services.projects = Some(host.clone());
    let earlier = rows_now(&h);
    (h, rig, host, earlier)
}

/// The turns of the conversation shown.
fn chat_turns<'a>(h: &'a Harness<'static, Shell>) -> &'a [crate::chat_panel::Turn] {
    &h.state().chat_panel.chat.as_ref().unwrap().turns
}

/// While a version is being read or shown, neither Send nor Run again does anything: the text
/// stays, nothing is written and no turn starts. Back at the work the same question goes, and the
/// reply is its own.
#[test]
fn no_question_goes_while_a_version_is_in_view() {
    use crate::chat::{ChatEvent, FailureKind, TurnEnd};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig, host, earlier) = chat_and_a_version(theme);
        ask_in_chat(&mut h, "First");
        chat_event(&mut h, &rig, failed_turn(FailureKind::Other));
        let id = h.state().chat_panel.chat.as_ref().unwrap().id;
        let nothing_goes = |h: &mut Harness<'static, Shell>| {
            h.state_mut().chat_panel.draft = "Second".into();
            let before = (chat_turns(h).len(), chat_calls(&rig).0, chat_calls(&rig).1, rig.store.kept(id).unwrap().turns.len());
            crate::chat_panel::send(h.state_mut());
            crate::chat_panel::retry(h.state_mut(), None);
            assert!(!h.state().chat_panel.chat.as_ref().unwrap().step.busy(), "nothing is written");
            h.run_steps(3);
            assert_eq!((chat_turns(h).len(), chat_calls(&rig).0, chat_calls(&rig).1, rig.store.kept(id).unwrap().turns.len()), before);
            assert_eq!(h.state().chat_panel.draft, "Second", "the text stays");
        };
        start_viewing(&mut h, "c1");
        nothing_goes(&mut h);
        answer_read(&mut h, &host, Ok(earlier));
        assert_eq!(h.state().viewing(), Some("c1"));
        nothing_goes(&mut h);

        assert!(h.state_mut().back_to_current_work());
        h.run_steps(3);
        let turns = chat_turns(&h).len();
        crate::chat_panel::send(h.state_mut());
        h.run();
        assert_eq!(chat_turns(&h).len(), turns + 1, "the same question goes");
        chat_event(&mut h, &rig, ChatEvent::Text { delta: "its own reply".into() });
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(TurnEnd::Completed));
        h.run();
        let reply = chat_turns(&h)
            .last()
            .unwrap()
            .items
            .iter()
            .find_map(|i| if let crate::chat_panel::Item::Text { text, .. } = i { Some(text.as_str()) } else { None });
        assert_eq!((chat_turns(&h).last().unwrap().question.as_str(), reply), ("Second", Some("its own reply")));
    }
}

/// A question on its way when a version is entered holds the version back until it is answered,
/// starts no turn, and leaves its saved turn settled, whichever write it was waiting for.
#[test]
fn a_question_on_its_way_when_a_version_is_entered_starts_no_turn() {
    use crate::chat::{ChatEvent, FailureKind, TurnEnd};
    use crate::chat_panel::{Chip, Step, TurnState};
    use std::sync::atomic::Ordering::SeqCst;
    for case in ["Creating", "Asking", "Rerunning", "Keeping"] {
        let (mut h, rig, host, earlier) = chat_and_a_version(ThemeKind::Light);
        if case != "Creating" {
            ask_in_chat(&mut h, "Before");
            let end = if case == "Rerunning" { failed_turn(FailureKind::Other) } else { ChatEvent::TurnEnded(TurnEnd::Completed) };
            chat_event(&mut h, &rig, end);
        }
        if case == "Keeping" {
            h.state_mut().chat_panel.chat.as_mut().unwrap().removed.insert(Chip::Diagram);
        }
        rig.store.hold.store(true, SeqCst);
        if case == "Rerunning" {
            crate::chat_panel::retry(h.state_mut(), None);
        } else {
            ask_in_chat(&mut h, "Just before");
        }
        let waiting = match h.state().chat_panel.chat.as_ref().unwrap().step {
            Step::Creating(..) => "Creating",
            Step::Asking(..) => "Asking",
            Step::Rerunning(..) => "Rerunning",
            Step::Keeping(..) => "Keeping",
            Step::Idle => "Idle",
        };
        assert_eq!(waiting, case);
        let calls = (chat_calls(&rig).0, chat_calls(&rig).1);
        start_viewing(&mut h, "c1");
        answer_read(&mut h, &host, Ok(earlier));
        assert_eq!(h.state().viewing(), None, "{case}: the version waits for the question");
        rig.store.release();
        h.run_steps(8);
        assert_eq!(h.state().viewing(), Some("c1"), "{case}");
        assert_eq!((chat_calls(&rig).0, chat_calls(&rig).1), calls, "{case}: nothing reached the agent");
        let id = h.state().chat_panel.chat.as_ref().unwrap().id;
        assert_eq!(chat_turns(&h).last().unwrap().state, TurnState::Stopped, "{case}");
        assert_eq!(rig.store.kept(id).unwrap().turns.last().unwrap().outcome, crate::chat_store::Outcome::Stopped, "{case}");
    }
}

/// A turn still running when a change is refused has the refusal on its row.
#[test]
fn a_refused_change_shows_as_refused_on_the_turns_rows() {
    let (mut h, rig, host, earlier) = chat_and_a_version(ThemeKind::Light);
    ask_in_chat(&mut h, "First");
    chat_event(&mut h, &rig, crate::chat::ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    // No turn can start here; this one is put back to running, to see the control channel's side.
    let chat = h.state_mut().chat_panel.chat.as_mut().unwrap();
    chat.allow_changes = true;
    chat.turns.last_mut().unwrap().state = crate::chat_panel::TurnState::Running;
    let add = serde_json::json!({ "command": "sketch.package.add", "params": { "name": "x" } });
    refused_for_viewing(crate::chat_fixture::call(h.state_mut(), "engine.execute", add), "the turn's change");
    let rows = chat_turns(&h).last().unwrap().items.iter();
    let rows: Vec<_> = rows.filter_map(|i| if let crate::chat_panel::Item::Tool(t) = i { Some(t.status) } else { None }).collect();
    assert_eq!(rows, [crate::chat_store::ToolStatus::Refused]);
}

/// Starts restoring a version holding `rows`, and has it built and handed to its job.
fn restore_under_way(h: &mut Harness<'static, Shell>, host: &SavingHost, rows: std::sync::Arc<archforge_mof::rows::Rows>) -> RestoreEnds {
    h.state_mut().restore_version("c1", "2026-11-08T14:02:11Z").unwrap();
    h.steps(3);
    answer_read(h, host, Ok(rows));
    host.restores.lock().unwrap().remove(0)
}

/// Asserts that the session is the version: the work gone, its history with it, nothing unsaved.
#[track_caller]
fn the_version_is_installed(h: &Harness<'static, Shell>) {
    assert!(!h.state().is_held());
    assert!(!schema_names(h).contains(&"shop".to_string()));
    let session = h.state().session.as_ref().unwrap();
    assert_eq!((session.undo_label(), session.is_dirty(), session.read_only()), (None, false, false), "undo starts fresh");
}

/// A restore that finishes puts the version in place of the work, with nothing more to write.
#[test]
fn a_finished_restore_installs_the_version_and_undo_starts_fresh() {
    use crate::replace::JobFailure;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host, earlier) = work_and_earlier();
        h.state_mut().state.theme = theme;
        let (answer, _reading) = restore_under_way(&mut h, &host, earlier);
        assert!(h.state().is_held(), "held while the job runs");
        let written = host.written.lock().unwrap().len();
        answer.send(Ok::<(), JobFailure>(())).unwrap();
        h.steps(4);
        the_version_is_installed(&h);
        assert_eq!(host.written.lock().unwrap().len(), written);
    }
}

/// A refused restore changes nothing and starts nothing, the version in view included.
#[test]
fn a_refused_restore_starts_nothing_and_keeps_the_version_in_view() {
    use crate::hold::HoldRefused::{Another, Gesture, NoProject};
    let (mut h, host) = saving_harness(None, None);
    assert_eq!(h.state_mut().restore_version("c1", "d"), Err(NoProject));
    assert!(host.reads.lock().unwrap().is_empty() && host.restores.lock().unwrap().is_empty());
    let (mut h, host, _) = work_and_earlier();
    h.state_mut().session.as_mut().unwrap().begin_interaction("sketch.package.add").unwrap();
    assert_eq!(h.state_mut().restore_version("c1", "d"), Err(Gesture));
    assert!(host.reads.lock().unwrap().is_empty() && host.restores.lock().unwrap().is_empty(), "a gesture under way stops it before the read");
    let (mut h, host, earlier) = work_and_earlier();
    start_viewing(&mut h, "c1");
    answer_read(&mut h, &host, Ok(earlier));
    h.state_mut().begin_hold().unwrap();
    assert_eq!(h.state_mut().restore_version("c2", "d"), Err(Another));
    assert_eq!(h.state().viewing(), Some("c1"));
    assert!(host.reads.lock().unwrap().is_empty() && host.restores.lock().unwrap().is_empty(), "no read and no job");
}

/// A restore from a version in view goes back to the work first, and a failure leaves that as it was.
#[test]
fn a_failed_restore_from_a_version_in_view_leaves_the_work_as_it_was() {
    use crate::replace::JobFailure;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host, earlier) = work_and_earlier();
        h.state_mut().state.theme = theme;
        let s = h.state_mut().session.as_mut().unwrap();
        let shop = s.execute("sketch.package.add", &serde_json::json!({ "name": "till" })).unwrap().value["id"].clone();
        s.execute("view.select", &serde_json::json!({ "ids": [shop] })).unwrap();
        let selection = s.view().selection.clone();
        start_viewing(&mut h, "c1");
        answer_read(&mut h, &host, Ok(earlier.clone()));
        let (answer, _reading) = restore_under_way(&mut h, &host, earlier);
        assert_eq!(h.state().viewing(), None, "current work first");
        answer.send(Err(JobFailure::Failed("told".into()))).unwrap();
        h.run_steps(4);
        the_work_shows(&h);
        assert_eq!(h.state().save_error.as_deref(), Some("told"));
        let session = h.state().session.as_ref().unwrap();
        assert!(session.is_dirty(), "still unsaved");
        assert_eq!(session.view().selection, selection);
        assert!(session.undo_label().is_some(), "undo is as it was");
    }
}

/// A failure after the unsaved work became a version leaves the work, read as saved; a plain one
/// does not.
#[test]
fn only_a_failure_that_saved_the_work_marks_it_saved() {
    use crate::replace::JobFailure;
    let failures: [(fn() -> JobFailure, bool); 2] = [(|| JobFailure::WorkSaved("told".into()), false), (|| JobFailure::Failed("told".into()), true)];
    for (failure, unsaved) in failures {
        let (mut h, host, earlier) = work_and_earlier();
        let (answer, _reading) = restore_under_way(&mut h, &host, earlier);
        answer.send(Err(failure())).unwrap();
        h.run_steps(4);
        the_work_shows(&h);
        assert_eq!((dirty(&h), h.state().save_error.as_deref()), (unsaved, Some("told")));
    }
}

/// An unclear job is read back: found finished, the version goes in; otherwise the work stays and
/// the project is held until it is closed, by an answer that cannot be read or by none at all.
#[test]
fn an_unclear_restore_installs_only_when_read_back_as_finished() {
    use crate::replace::JobFailure;
    let reads: [Option<Result<bool, String>>; 3] = [Some(Ok(true)), Some(Err("could not".into())), None];
    for read in reads {
        let (mut h, host, earlier) = work_and_earlier();
        let (answer, reading) = restore_under_way(&mut h, &host, earlier);
        answer.send(Err(JobFailure::Unclear)).unwrap();
        h.steps(2);
        let finished = read == Some(Ok(true));
        match read {
            Some(said) => reading.send(said).unwrap(),
            None => drop(reading),
        }
        h.steps(4);
        if finished {
            the_version_is_installed(&h);
        } else {
            assert!(h.state().is_held() && schema_names(&h).contains(&"shop".to_string()), "held, with the work");
        }
    }
}

/// Asks for a switch to `elsewhere` and gets as far as its check being asked of the host.
fn switch_asked(h: &mut Harness<'static, Shell>, host: &SavingHost) -> SwitchEnds {
    h.state_mut().switch_branch("elsewhere".to_string()).unwrap();
    h.run_steps(4);
    assert!(h.state().is_held(), "held while the switch runs");
    host.switches.lock().unwrap().remove(0)
}

/// Answers a switch's check with `rows`, reaching its move.
fn switch_checked(h: &mut Harness<'static, Shell>, ends: &SwitchEnds, rows: std::sync::Arc<archforge_mof::rows::Rows>) {
    ends.check.send(Ok(rows)).unwrap();
    h.steps(4);
}

/// Runs `test` on what `fixture` makes, once in each appearance.
fn in_both<A, B>(fixture: impl Fn() -> (Harness<'static, Shell>, A, B), test: impl Fn(Harness<'static, Shell>, A, B)) {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, a, b) = fixture();
        h.state_mut().state.theme = theme;
        test(h, a, b);
    }
}

/// Answers a switch's move as finished.
fn switch_moved(h: &mut Harness<'static, Shell>, ends: &SwitchEnds) {
    ends.moved.0.send(Ok(())).unwrap();
    h.steps(8);
}

/// The status bar's reason, and that the work is as it was and no longer held.
#[track_caller]
fn the_work_stays_saying(h: &Harness<'static, Shell>, why: &str) {
    assert_eq!(h.state().save_error.as_deref(), Some(why));
    assert!(!h.state().is_held() && schema_names(h).contains(&"shop".to_string()));
}

/// A finished switch installs the branch with a fresh undo history, unsaved if the branch is, and
/// writes nothing back; nothing changes before its move has finished.
#[test]
fn a_finished_switch_installs_the_branch_and_undo_starts_fresh() {
    for unsaved in [false, true] {
        in_both(work_and_earlier, |mut h, host, earlier| {
            let ends = switch_asked(&mut h, &host);
            assert_eq!(ends.branch, "elsewhere");
            ends.unsaved.store(unsaved, std::sync::atomic::Ordering::SeqCst);
            switch_checked(&mut h, &ends, earlier);
            assert!(h.state().is_held() && schema_names(&h).contains(&"shop".to_string()), "not before the move");
            let written = host.written.lock().unwrap().len();
            switch_moved(&mut h, &ends);
            assert!(!h.state().is_held() && !schema_names(&h).contains(&"shop".to_string()));
            let session = h.state().session.as_ref().unwrap();
            assert_eq!((session.undo_label(), session.is_dirty(), session.read_only()), (None, unsaved, false));
            assert_eq!(host.written.lock().unwrap().len(), written, "what was read is what the branch holds");
        });
    }
}

/// What the next opening of the project at `path` asks for, once its tab has closed.
fn reopens_on(h: &mut Harness<'static, Shell>, host: &SavingHost, path: &std::path::Path) -> Option<String> {
    h.state_mut().close_tab_now(0);
    h.run_steps(2);
    host.closings.lock().unwrap().pop().expect("the closing").send(Ok(())).unwrap();
    h.run_steps(2);
    h.state_mut().open_path(path);
    host.opened_on.lock().unwrap().pop().expect("opened")
}

/// A finished switch is what the project reopens on, a switch to `main` being no name at all, and
/// the status bar's reason about the branch left goes; not before the move has finished.
#[test]
fn a_finished_switch_is_remembered_and_its_notice_goes() {
    in_both(work_and_earlier, |mut h, host, earlier| {
        let path = h.state().project.as_ref().unwrap().path.clone();
        h.state_mut().project.as_mut().unwrap().notice = Some(crate::BranchNotice::Gone { name: "old".into() });
        let ends = switch_asked(&mut h, &host);
        switch_checked(&mut h, &ends, earlier.clone());
        assert_eq!(h.state().state.branch_of(&path), None, "not before the move");
        assert!(h.state().project.as_ref().unwrap().notice.is_some());
        switch_moved(&mut h, &ends);
        assert_eq!(h.state().state.branch_of(&path), Some("elsewhere"));
        assert_eq!(h.state().project.as_ref().unwrap().notice, None);
        h.state_mut().state.keep_branch(&path, Some("MAIN".to_string()));
        assert_eq!(h.state().state.branch_of(&path), None, "main is no name");
        h.state_mut().state.keep_branch(&path, Some("elsewhere".to_string()));
        assert_eq!(reopens_on(&mut h, &host, &path).as_deref(), Some("elsewhere"));
    });
}

/// A switch that failed, stayed or cannot be told leaves what is stored and the reason shown.
#[test]
fn a_switch_that_did_not_arrive_is_not_remembered() {
    type Then = fn(&SwitchEnds);
    let ways: [Then; 3] = [
        |e| e.moved.0.send(Err(crate::replace::JobFailure::Failed("no".to_string()))).unwrap(),
        |e| {
            e.moved.0.send(Err(crate::replace::JobFailure::Unclear)).unwrap();
            e.moved.1.send(Ok(false)).unwrap();
        },
        |e| {
            e.moved.0.send(Err(crate::replace::JobFailure::Unclear)).unwrap();
            e.moved.1.send(Err("no".to_string())).unwrap();
        },
    ];
    for then in ways {
        in_both(work_and_earlier, |mut h, host, earlier| {
            let path = h.state().project.as_ref().unwrap().path.clone();
            h.state_mut().state.keep_branch(&path, Some("before".to_string()));
            h.state_mut().project.as_mut().unwrap().notice = Some(crate::BranchNotice::Gone { name: "old".into() });
            let ends = switch_asked(&mut h, &host);
            switch_checked(&mut h, &ends, earlier);
            then(&ends);
            h.run_steps(10);
            assert_eq!(h.state().state.branch_of(&path), Some("before"));
            assert!(h.state().project.as_ref().unwrap().notice.is_some());
            if !h.state().is_held() {
                assert_eq!(reopens_on(&mut h, &host, &path).as_deref(), Some("before"));
            }
        });
    }
}

/// A rename follows the branch in use, in any case of its name, and leaves another's alone.
#[test]
fn a_rename_is_remembered_only_for_the_branch_in_use() {
    in_both(work_and_earlier, |mut h, host, _| {
        let path = h.state().project.as_ref().unwrap().path.clone();
        h.state_mut().state.keep_branch(&path, Some("topic".to_string()));
        h.state_mut().branch_renamed(&path, "other", "new");
        assert_eq!(h.state().state.branch_of(&path), Some("topic"), "another branch");
        h.state_mut().branch_renamed(&path, "TOPIC", "renamed");
        assert_eq!(h.state().state.branch_of(&path), Some("renamed"));
        h.state_mut().branch_renamed(std::path::Path::new("/tmp/Other.archforge"), "renamed", "x");
        assert_eq!(reopens_on(&mut h, &host, &path).as_deref(), Some("renamed"), "another project");
    });
}

/// A request accepted before anything that puts another session in its place is refused by its
/// word, and one accepted after it runs: viewing, returning, a branch change, Restore, another tab.
#[test]
fn a_request_from_before_any_change_of_session_is_refused_by_its_word() {
    type Rows = std::sync::Arc<archforge_mof::rows::Rows>;
    type Step = fn(&mut Harness<'static, Shell>, &SavingHost, Rows);
    let ready: Step = |_, _, _| {};
    let view: Step = |h, host, earlier| {
        start_viewing(h, "c1");
        answer_read(h, host, Ok(earlier));
    };
    let ways: [(&str, Step, Step); 5] = [
        ("viewing", ready, view),
        ("returning", view, |h, _, _| assert!(h.state_mut().back_to_current_work())),
        ("a branch change", ready, |h, host, earlier| {
            let ends = switch_asked(h, host);
            switch_checked(h, &ends, earlier);
            switch_moved(h, &ends);
        }),
        ("Restore", ready, |h, host, earlier| {
            restore_under_way(h, host, earlier).0.send(Ok(())).unwrap();
            h.steps(8);
        }),
        (
            "another tab",
            |h, _, _| {
                h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() })
            },
            |h, _, _| h.state_mut().activate(0),
        ),
    ];
    for (what, first, moving) in ways {
        in_both(work_and_earlier, |mut h, host, earlier| {
            first(&mut h, &host, earlier.clone());
            h.steps(3);
            let inspect = |h: &Harness<'static, Shell>| accepted(h, "document.inspect", serde_json::json!({}), false);
            let stale = inspect(&h);
            moving(&mut h, &host, earlier);
            h.steps(3);
            let refused = crate::control::answer_request(h.state_mut(), &stale).unwrap_err();
            assert!(refused.starts_with(&format!("{}: ", crate::control::CHANGED)), "{what}: {refused}");
            let fresh = inspect(&h);
            assert!(crate::control::answer_request(h.state_mut(), &fresh).is_ok(), "{what}");
        });
    }
}

/// An agent is told the branch the host says the project is on, none without a host that keeps
/// branches, and no method changes one.
#[test]
fn an_agent_is_told_the_branch_and_has_no_way_to_change_it() {
    in_both(work_and_earlier, |mut h, host, _| {
        *host.branch_now.lock().unwrap() = Some("topic".to_string());
        let inspected = |h: &mut Harness<'static, Shell>| crate::control::answer(h.state_mut(), "document.inspect", &serde_json::json!({})).unwrap();
        assert_eq!(inspected(&mut h)["branch"], "topic");
        *host.branch_now.lock().unwrap() = None;
        assert_eq!(inspected(&mut h)["branch"], serde_json::Value::Null);
        h.state_mut().session = None;
        assert_eq!(inspected(&mut h).get("branch"), Some(&serde_json::Value::Null), "the field is there with nothing open");
        assert!(crate::control::METHODS.iter().all(|(m, _)| !m.contains("branch")));
    });
}

/// The check is not asked for until a save under way has answered and the latest rows are over.
#[test]
fn a_switch_asks_for_its_check_only_after_the_latest_rows_have_gone_over() {
    in_both(work_and_earlier, |mut h, host, _| {
        h.state_mut().save();
        h.state_mut().switch_branch("elsewhere".to_string()).unwrap();
        h.run_steps(4);
        assert!(host.switches.lock().unwrap().is_empty(), "the save has not answered");
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(6);
        assert_eq!(host.switches.lock().unwrap().len(), 1);
        assert_eq!(host.written.lock().unwrap().last().unwrap().1, *rows_now(&h), "the latest rows went first");
    });
}

/// A failed check says why and the work stays; a lost answer claims only that the project stayed.
#[test]
fn a_failed_check_says_why_and_the_work_stays() {
    in_both(work_and_earlier, |mut h, host, _| {
        host.switch_refused.lock().unwrap().replace("Another switch".to_string());
        h.state_mut().switch_branch("elsewhere".to_string()).unwrap();
        h.run_steps(4);
        the_work_stays_saying(&h, "Another switch");
        let ends = switch_asked(&mut h, &host);
        ends.check.send(Err("The reason".to_string().into())).unwrap();
        h.run_steps(4);
        the_work_stays_saying(&h, "The reason");
        drop(switch_asked(&mut h, &host));
        h.run_steps(4);
        the_work_stays_saying(&h, "The switch did not finish, so the project stayed on its branch.");
        h.state_mut().begin_replacement(std::sync::mpsc::channel().1, None).unwrap();
        h.run_steps(4);
        the_work_stays_saying(&h, "The reading stopped without an answer, so nothing was changed.");
    });
}

/// A move with no clear answer is read back: installed, stayed, or held until closed.
#[test]
fn a_move_with_no_clear_answer_is_read_back() {
    let told: [(Result<bool, String>, fn(&Harness<'static, Shell>)); 3] = [
        (Ok(true), the_version_is_installed),
        (Ok(false), |h| the_work_stays_saying(h, "The switch did not finish, so the project stayed on its branch.")),
        (Err("no".to_string()), |h| {
            assert!(h.state().is_held() && h.state().save_error.as_deref().is_some_and(|w| w.starts_with("Archforge could not tell")))
        }),
    ];
    for (answer, then) in told {
        in_both(work_and_earlier, |mut h, host, earlier| {
            let ends = switch_asked(&mut h, &host);
            switch_checked(&mut h, &ends, earlier);
            ends.moved.0.send(Err(crate::replace::JobFailure::Unclear)).unwrap();
            h.steps(3);
            ends.moved.1.send(answer.clone()).unwrap();
            h.steps(6);
            then(&h);
        });
    }
}

/// From a version in view the switch goes back to current work first, unless it is refused.
#[test]
fn a_switch_from_every_starting_point() {
    use crate::hold::HoldRefused::{Another, NoProject};
    assert_eq!(Shell::new(Services::default(), NOTATIONS, &SCREENS).switch_branch("x".to_string()), Err(NoProject), "this part draws nothing");
    in_both(work_and_earlier, |mut h, host, earlier| {
        start_viewing(&mut h, "c1");
        answer_read(&mut h, &host, Ok(earlier));
        assert_eq!(h.state().viewing(), Some("c1"));
        h.state_mut().begin_hold().unwrap();
        assert_eq!((h.state_mut().switch_branch("x".to_string()), h.state().viewing()), (Err(Another), Some("c1")));
        assert!(host.switches.lock().unwrap().is_empty());
        h.state_mut().end_hold();
        let _ends = switch_asked(&mut h, &host);
        assert_eq!(h.state().viewing(), None, "current work first");
        assert!(schema_names(&h).contains(&"shop".to_string()));
    });
}

/// A close and a quit asked for during either step of a switch wait for it, and then go ahead.
#[test]
fn closing_or_quitting_during_a_switch_waits_for_it() {
    let routes: [fn(&mut Shell); 2] = [|s| s.close_tab(0), Shell::request_quit];
    for (n, route) in routes.into_iter().enumerate() {
        for during_check in [true, false] {
            in_both(work_and_earlier, |mut h, host, earlier| {
                let ends = switch_asked(&mut h, &host);
                if !during_check {
                    switch_checked(&mut h, &ends, earlier.clone());
                }
                route(h.state_mut());
                h.run_steps(3);
                assert!(!h.state().quit_confirmed() && host.closings.lock().unwrap().is_empty() && h.state().is_held(), "it waits");
                if during_check {
                    switch_checked(&mut h, &ends, earlier);
                }
                switch_moved(&mut h, &ends);
                assert!(if n == 0 { !host.closings.lock().unwrap().is_empty() } else { h.state().quit_confirmed() }, "then goes ahead");
            });
        }
    }
}

/// Only a finished switch ends the conversation's program and starts Chat again, the draft kept.
#[test]
fn a_switch_lets_the_conversation_go_only_once_it_has_finished() {
    use crate::chat::{ChatEvent, TurnEnd};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig, host, earlier) = chat_and_a_version(theme);
        ask_in_chat(&mut h, "First");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(TurnEnd::Completed));
        let old = h.state().chat_panel.chat.as_ref().unwrap().id;
        h.state_mut().chat_panel.draft = "Still typing".into();
        h.state_mut().chat_panel.load_error = Some("old".into());
        let ends = switch_asked(&mut h, &host);
        ends.check.send(Err("The reason".to_string().into())).unwrap();
        h.run_steps(4);
        assert_eq!(h.state().chat_panel.chat.as_ref().unwrap().id, old, "{theme:?}: the switch failed");
        let ended = chat_calls(&rig).3;
        let ends = switch_asked(&mut h, &host);
        switch_checked(&mut h, &ends, earlier);
        switch_moved(&mut h, &ends);
        assert_eq!(chat_calls(&rig).3, ended + 1, "{theme:?}: its program ended");
        let panel = &h.state().chat_panel;
        assert!(panel.chat.as_ref().is_none_or(|c| c.id != old && c.turns.is_empty()), "{theme:?}: a new conversation");
        assert_eq!((panel.draft.as_str(), panel.load_error.as_deref()), ("Still typing", None));
        assert_eq!(h.state().save_error, None, "{theme:?}: the failed switch's reason went with it");
    }
}

/// A deleted conversation's toast closes with a finished switch.
#[test]
fn a_switch_settles_the_toasts_of_deleted_conversations() {
    in_both(
        || chat_history(ThemeKind::Light),
        |mut h, _rig, chats| {
            let host = std::sync::Arc::new(SavingHost::default());
            h.state_mut().services.projects = Some(host.clone());
            crate::chat_delete::ask(h.state_mut(), chats[2], 2);
            crate::chat_delete::confirm(h.state_mut());
            h.run_steps(4);
            assert_eq!(h.state().chats.deletions.len(), 1, "its toast shows");
            let rows = rows_now(&h);
            let ends = switch_asked(&mut h, &host);
            switch_checked(&mut h, &ends, rows);
            switch_moved(&mut h, &ends);
            assert!(h.state().chats.deletions.is_empty());
        },
    );
}

/// A close and a quit asked for during a restore wait for it, and then go ahead.
#[test]
fn closing_or_quitting_during_a_restore_waits_for_it() {
    use crate::replace::JobFailure;
    let routes: [fn(&mut Shell); 2] = [|s| s.close_tab(0), Shell::request_quit];
    for (n, route) in routes.into_iter().enumerate() {
        let (mut h, host, earlier) = work_and_earlier();
        let (answer, _reading) = restore_under_way(&mut h, &host, earlier);
        route(h.state_mut());
        h.steps(3);
        assert!(!h.state().quit_confirmed() && host.closings.lock().unwrap().is_empty() && h.state().is_held(), "it waits");
        answer.send(Ok::<(), JobFailure>(())).unwrap();
        h.steps(6);
        assert!(if n == 0 { !host.closings.lock().unwrap().is_empty() } else { h.state().quit_confirmed() }, "then goes ahead");
    }
}

/// A version whose box an opening fits holds what the store does not: it reads as unsaved and the
/// repair is written. The same rows in another order are no difference.
#[test]
fn a_repair_made_while_building_is_unsaved_and_written_but_row_order_is_not_one() {
    use crate::replace::JobFailure;
    for reordered in [false, true] {
        let (mut h, host, _) = work_and_earlier();
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let shop = id(s.execute("sketch.package.add", &serde_json::json!({ "name": "till" })).unwrap().value);
        let table = id(s.execute("sketch.class.add", &serde_json::json!({ "package": shop, "name": "customer" })).unwrap().value);
        s.execute("sketch.attribute.add", &serde_json::json!({ "class": table, "name": "id" })).unwrap();
        s.execute("sketch.attribute.add", &serde_json::json!({ "class": table, "name": "code" })).unwrap();
        if !reordered {
            let diagram = id(s.execute("diagram.add", &serde_json::json!({ "kind": SKETCH, "element": shop, "name": "Shop" })).unwrap().value);
            s.execute("diagram.place", &serde_json::json!({ "diagram": diagram, "element": table, "x": 0, "y": 0, "width": 240, "height": 20 }))
                .unwrap();
        }
        let mut rows = (*rows_now(&h)).clone();
        if reordered {
            rows.tables.iter_mut().for_each(|t| t.reverse());
            rows.junctions.iter_mut().for_each(|j| j.reverse());
            assert!(rows.junctions.iter().any(|j| j.len() > 1), "a list to read in another order");
        }
        h.steps(2);
        let (answer, _reading) = restore_under_way(&mut h, &host, std::sync::Arc::new(rows));
        let written = host.written.lock().unwrap().len();
        answer.send(Ok::<(), JobFailure>(())).unwrap();
        h.steps(5);
        assert_eq!((dirty(&h), host.written.lock().unwrap().len()), (!reordered, written + usize::from(!reordered)));
    }
}

/// A restore and a switch compare what they built with what they read off the interface thread:
/// the frame builds no rows for it, and the version still takes the session's place, saved.
#[test]
fn a_restore_or_a_switch_builds_no_rows_on_the_frame() {
    use crate::replace::JobFailure;
    in_both(work_and_earlier, |mut h, host, earlier| {
        h.steps(2);
        let built = archforge_mof::rows::built_here();
        let (answer, _reading) = restore_under_way(&mut h, &host, earlier);
        answer.send(Ok::<(), JobFailure>(())).unwrap();
        h.steps(5);
        the_version_is_installed(&h);
        assert_eq!(archforge_mof::rows::built_here(), built, "no rows were built on the frame for a restore");
    });
    in_both(work_and_earlier, |mut h, host, earlier| {
        h.steps(2);
        let built = archforge_mof::rows::built_here();
        let ends = switch_asked(&mut h, &host);
        switch_checked(&mut h, &ends, earlier);
        switch_moved(&mut h, &ends);
        the_version_is_installed(&h);
        assert_eq!(archforge_mof::rows::built_here(), built, "no rows were built on the frame for a switch");
    });
}

/// A comparison whose thread ends without an answer counts as a repair: the restored version reads
/// as unsaved and its rows are written again.
#[test]
fn a_comparison_that_ends_without_an_answer_counts_as_a_repair() {
    use crate::replace::JobFailure;
    in_both(work_and_earlier, |mut h, host, earlier| {
        h.run_steps(2);
        crate::replace::COMPARED.with(|c| *c.borrow_mut() = Some(std::sync::mpsc::channel().1));
        let (answer, _reading) = restore_under_way(&mut h, &host, earlier);
        let written = host.written.lock().unwrap().len();
        answer.send(Ok::<(), JobFailure>(())).unwrap();
        h.steps(5);
        assert!(!h.state().is_held() && dirty(&h), "installed, unsaved");
        assert_eq!(host.written.lock().unwrap().len(), written + 1, "its rows are written again");
    });
}

/// A comparison with no answer holds the replacement back until its limit, counted from when it
/// started; then it counts as a repair.
#[test]
fn a_comparison_past_its_limit_counts_as_a_repair() {
    use crate::replace::{COMPARE_LIMIT, JobFailure};
    use std::sync::atomic::Ordering::Relaxed;
    in_both(work_and_earlier, |mut h, host, earlier| {
        let now = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1_000));
        let clock = now.clone();
        h.state_mut().services.clock = Some(std::sync::Arc::new(move || crate::chat_words::Clock { now: clock.load(Relaxed), offset: None }));
        h.run_steps(2);
        let (_silent, answer_of_comparison) = std::sync::mpsc::channel();
        crate::replace::COMPARED.with(|c| *c.borrow_mut() = Some(answer_of_comparison));
        let (answer, _reading) = restore_under_way(&mut h, &host, earlier);
        // The comparison started with the build; the job's time counts towards its limit.
        now.fetch_add(COMPARE_LIMIT - 1, Relaxed);
        answer.send(Ok::<(), JobFailure>(())).unwrap();
        h.run_steps(5);
        assert!(h.state().is_held(), "waiting for the comparison");
        let written = host.written.lock().unwrap().len();
        now.fetch_add(1, Relaxed);
        h.run_steps(3);
        assert!(!h.state().is_held() && dirty(&h), "installed, unsaved");
        assert_eq!(host.written.lock().unwrap().len(), written + 1, "its rows are written again");
    });
}

/// A Chat write that starts while a comparison holds the replacement gets the whole settle time
/// from when it is seen, however long the comparison took.
#[test]
fn a_chat_write_after_a_long_comparison_gets_its_whole_settle_time() {
    use crate::hold::CHAT_SETTLE_LIMIT;
    use std::sync::atomic::Ordering::{Relaxed, SeqCst};
    in_both(
        || chat_history(ThemeKind::Light),
        |mut h, rig, chats| {
            h.state_mut().services.projects = Some(std::sync::Arc::new(SavingHost::default()));
            h.run_steps(2);
            let rows = rows_now(&h);
            let (comparison, answer_of_comparison) = std::sync::mpsc::channel();
            crate::replace::COMPARED.with(|c| *c.borrow_mut() = Some(answer_of_comparison));
            let read = start_replacing(&mut h, None);
            read.send(Ok(rows)).unwrap();
            h.run_steps(3);
            rig.now.fetch_add(crate::replace::COMPARE_LIMIT - 1, Relaxed);
            h.run_steps(2);
            assert!(h.state().is_held(), "the comparison holds it");
            rig.store.hold.store(true, SeqCst);
            crate::chat_delete::ask(h.state_mut(), chats[2], 2);
            crate::chat_delete::confirm(h.state_mut());
            comparison.send(false).unwrap();
            h.run_steps(3);
            rig.now.fetch_add(CHAT_SETTLE_LIMIT - 1, Relaxed);
            h.run_steps(3);
            assert!(h.state().is_held() && h.state().save_error.is_none(), "Chat still has time: {:?}", h.state().save_error);
            rig.store.release();
            h.run_steps(4);
            assert!(!h.state().is_held() && h.state().save_error.is_none(), "then it goes on");
        },
    );
}

/// A refused view starts no read: the hold comes before the first step.
#[test]
fn a_version_is_not_read_over_another_hold_or_without_a_project() {
    let (mut h, host) = saving_harness(None, None);
    assert!(matches!(h.state_mut().view_version("c1"), Err(crate::hold::HoldRefused::NoProject)));
    let (mut h2, host2, earlier) = work_and_earlier();
    h2.state_mut().begin_hold().unwrap();
    assert!(matches!(h2.state_mut().view_version("c1"), Err(crate::hold::HoldRefused::Another)));
    h2.state_mut().end_hold();
    h2.run_steps(2);
    assert!(host.reads.lock().unwrap().is_empty() && host2.reads.lock().unwrap().is_empty());
    start_viewing(&mut h2, "c1");
    answer_read(&mut h2, &host2, Ok(earlier));
    assert!(matches!(h2.state_mut().view_version("c2"), Err(crate::hold::HoldRefused::Another)));
    assert!(host2.reads.lock().unwrap().is_empty());
}

/// A save under way answers first; the latest rows go over, and the replacement then takes over.
#[test]
fn a_replacement_waits_for_a_save_then_takes_the_sessions_place() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    let saved_rows = rows_now(&h);
    add_schema(&mut h, "billing");
    h.state_mut().save();
    let generation = h.state().session_generation();
    let read = start_replacing(&mut h, None);
    let handed = host.written.lock().unwrap().len();
    read.send(Ok(saved_rows)).unwrap();
    h.steps(3);
    assert!(h.state().is_held() && schema_names(&h).len() == 2, "the save has not answered: {:?}", schema_names(&h));
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
    h.steps(4);
    assert!(!h.state().is_held());
    assert_eq!(schema_names(&h), ["shop"]);
    assert!(!h.state().session.as_ref().unwrap().can_undo(), "undo starts fresh");
    assert!(h.state().session_generation() > generation, "requests sent before it are refused");
    assert_eq!(host.written.lock().unwrap().len(), handed, "what was read is not written back");
}

/// One that cannot be read or built ends with nothing written and the session as it was.
#[test]
fn a_replacement_that_cannot_be_read_or_built_runs_no_job() {
    for answer in [Some(Err("the commit is gone".to_string().into())), Some(Ok(std::sync::Arc::new(archforge_mof::rows::Rows::default()))), None] {
        let (mut h, _) = saving_harness(Some(commerce()), None);
        add_schema(&mut h, "shop");
        let (job, started, ..) = test_job();
        let read = start_replacing(&mut h, Some(job));
        match answer {
            Some(a) => read.send(a).unwrap(),
            None => drop(read),
        }
        h.run_steps(3);
        assert!(!h.state().is_held() && !started.load(std::sync::atomic::Ordering::SeqCst), "no job ran");
        assert_eq!(schema_names(&h), ["shop"]);
        assert!(h.state().save_error.is_some(), "it says why");
    }
}

/// Chat settles first: a running turn is stopped, and one that never ends refuses the operation.
#[test]
fn a_replacement_stops_a_running_turn_and_is_refused_when_chat_does_not_settle() {
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "A long question");
    let _read = start_replacing(&mut h, None);
    assert_eq!(chat_calls(&rig).2, 1, "the turn was asked to stop");
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
    ask_in_chat(&mut h, "A question its project never answers");
    let _read = start_replacing(&mut h, None);
    rig.now.fetch_add(crate::hold::CHAT_SETTLE_LIMIT, std::sync::atomic::Ordering::Relaxed);
    h.run_steps(3);
    assert!(!h.state().is_held());
    assert_eq!(h.state().save_error.as_deref(), Some("Chat did not finish in time, so nothing was changed."));
}

/// Rows that cannot reach the host stop it before anything is read or written.
#[test]
fn a_replacement_stops_when_the_latest_rows_cannot_be_handed_over() {
    let (mut h, _) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    h.state_mut().services.projects = None;
    h.state_mut().session.as_mut().unwrap().execute("sketch.package.add", &serde_json::json!({ "name": "billing" })).unwrap();
    let (job, started, ..) = test_job();
    let _read = start_replacing(&mut h, Some(job));
    assert!(!h.state().is_held() && !started.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(schema_names(&h).len(), 2, "the session is as it was");
}

/// The job runs once the replacement is built; only its finishing puts the replacement in place.
#[test]
fn a_job_runs_after_the_build_and_decides_by_its_answer() {
    use crate::replace::JobFailure::Failed;
    use std::sync::atomic::Ordering::SeqCst;
    let (mut h, _) = saving_harness(Some(commerce()), None);
    let empty = rows_now(&h);
    add_schema(&mut h, "shop");
    let (job, started, answer, _) = test_job();
    let read = start_replacing(&mut h, Some(job));
    assert!(!started.load(SeqCst), "nothing is written before the replacement is built");
    read.send(Ok(empty.clone())).unwrap();
    h.steps(3);
    assert!(started.load(SeqCst) && h.state().is_held() && schema_names(&h) == ["shop"]);
    answer.send(Ok(())).unwrap();
    h.steps(3);
    assert!(!h.state().is_held() && schema_names(&h).is_empty(), "{:?}", schema_names(&h));
    // A job that says it failed leaves the session as it was, and nothing is read back.
    add_schema(&mut h, "shop");
    let (job, _, answer, _reading) = test_job();
    let read = start_replacing(&mut h, Some(job));
    read.send(Ok(empty)).unwrap();
    h.steps(3);
    answer.send(Err(Failed("the disk is full".into()))).unwrap();
    h.steps(3);
    assert!(!h.state().is_held() && schema_names(&h) == ["shop"]);
    assert_eq!(h.state().save_error.as_deref(), Some("the disk is full"));
}

/// A lost reply is read back, and what that shows is what happened.
#[test]
fn a_lost_reply_is_read_back_and_that_decides() {
    for finished in [true, false] {
        let (mut h, _) = saving_harness(Some(commerce()), None);
        let empty = rows_now(&h);
        add_schema(&mut h, "shop");
        let (job, _, answer, reading) = test_job();
        let read = start_replacing(&mut h, Some(job));
        read.send(Ok(empty)).unwrap();
        h.steps(3);
        drop(answer);
        h.steps(3);
        assert!(h.state().is_held(), "waiting for the reading back");
        reading.send(Ok(finished)).unwrap();
        h.steps(3);
        assert!(!h.state().is_held());
        assert_eq!(schema_names(&h).is_empty(), finished, "finished {finished}");
    }
}

/// When it cannot be read back either: it says so, writes nothing, and stays held until closed.
#[test]
fn an_outcome_that_cannot_be_read_stays_held_until_the_project_is_closed() {
    use crate::close_ask::Ask;
    let (mut h, host) = saving_harness(Some(commerce()), None);
    held_until_closed(&mut h, "shop", |_| {});
    let written = host.written.lock().unwrap().len();
    let held = |h: &Harness<'static, Shell>| h.state().is_held() && schema_names(h) == ["shop"];
    assert!(held(&h) && h.state().save_error.as_deref().is_some_and(|e| e.contains("could not tell")), "{:?}", h.state().save_error);
    // A quit asks about this project first, and Keep open forgets it.
    h.state_mut().request_quit();
    h.run_steps(3);
    assert!(matches!(h.state().ask, Some(Ask::CannotWrite { .. })) && !h.state().quit_confirmed());
    let at = rect_of(&h, "close-ask.keep").center();
    click(&mut h, at);
    h.run_steps(3);
    assert!(held(&h) && h.state().ask.is_none());
    // Closing offers only to close without writing, and that ends it.
    h.state_mut().close_tab(0);
    assert!(matches!(h.state().ask, Some(Ask::CannotWrite { .. })));
    h.state_mut().ask = None;
    h.state_mut().discard_tab(0, false);
    h.run_steps(2);
    assert!(!h.state().is_held());
    assert_eq!(host.unwritten.lock().unwrap().len(), 1);
    assert!(host.discarded.lock().unwrap().is_empty(), "nothing was put back");
    assert_eq!(host.written.lock().unwrap().len(), written, "nothing was written back");
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(3);
    assert!(!h.state().quit_confirmed() && !h.state().quitting(), "the quit was forgotten");
}

/// Held until closed: a reset is not a way out, a quit follows the closing, and a reading back that
/// never answers counts as unknown.
#[test]
fn a_hold_until_closed_lets_only_closing_without_writing_end_it() {
    use std::sync::atomic::Ordering::Relaxed;
    let (mut h, rig_host) = saving_harness(Some(commerce()), None);
    let empty = rows_now(&h);
    add_schema(&mut h, "shop");
    let now = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1_000));
    let clock = now.clone();
    h.state_mut().services.clock = Some(std::sync::Arc::new(move || crate::chat_words::Clock { now: clock.load(Relaxed), offset: None }));
    let (mut job, _, answer, _reading) = test_job();
    // The time the reading takes to start counts towards its limit.
    let (slow, read_back) = (now.clone(), job.read_back);
    job.read_back = Box::new(move || {
        slow.fetch_add(crate::replace::READ_BACK_LIMIT - 1, Relaxed);
        read_back()
    });
    let read = start_replacing(&mut h, Some(job));
    read.send(Ok(empty)).unwrap();
    h.run_steps(3);
    drop(answer);
    h.run_steps(3);
    assert!(h.state().save_error.is_none(), "still reading back");
    now.fetch_add(1, Relaxed);
    h.run_steps(3);
    assert!(h.state().save_error.as_deref().is_some_and(|e| e.contains("could not tell")), "{:?}", h.state().save_error);
    h.state_mut().discard_tab(0, true);
    assert!(rig_host.discarded.lock().unwrap().is_empty() && h.state().is_held(), "Don't save puts nothing back");
    h.state_mut().request_quit();
    h.run_steps(3);
    h.state_mut().ask = None;
    h.state_mut().discard_tab(0, false);
    rig_host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(4);
    assert!(h.state().quit_confirmed(), "the quit follows the closing");
}

/// A closing refused leaves the project held: still read-only, writing nothing, and closable again.
#[test]
fn a_closing_refused_keeps_the_hold_until_the_project_really_closes() {
    use crate::close_ask::Ask;
    let (mut h, host) = saving_harness(Some(commerce()), None);
    held_until_closed(&mut h, "shop", |_| {});
    let written = host.written.lock().unwrap().len();
    let close = |h: &mut Harness<'static, Shell>| {
        h.state_mut().ask = None;
        h.state_mut().discard_tab(0, false);
        h.run_steps(2);
    };
    h.state_mut().request_quit();
    h.run_steps(2);
    close(&mut h);
    assert_eq!(h.state_mut().begin_hold(), Err(crate::hold::HoldRefused::Another), "its closing is not over yet");
    host.closings.lock().unwrap().pop().unwrap().send(Err("the disk is full".into())).unwrap();
    h.run_steps(3);
    assert!(h.state().is_held() && h.state().session.as_ref().unwrap().read_only() && !h.state().can_save());
    assert_eq!(h.state().active, Some(0), "it shows again");
    assert_eq!(host.written.lock().unwrap().len(), written, "and nothing was written");
    h.state_mut().close_tab(0);
    assert!(matches!(&h.state().ask, Some(Ask::CannotWrite { why, .. }) if why == "the disk is full"), "{:?}", h.state().ask);
    // A closing that stops without an answer is no closing either.
    close(&mut h);
    drop(host.closings.lock().unwrap().pop());
    h.run_steps(3);
    assert!(h.state().is_held() && h.state().session.as_ref().unwrap().read_only());
    close(&mut h);
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(3);
    assert!(!h.state().is_held() && h.state().tabs.is_empty() && !h.state().holding.pending(), "only a real closing ends it");
    assert!(!h.state().quit_confirmed(), "the quit went with the refusal");
}

/// With no host to ask, closing is immediate, and the hold goes with the project.
#[test]
fn a_hold_until_closed_ends_when_there_is_no_host_to_ask() {
    let (mut h, _) = saving_harness(Some(commerce()), None);
    held_until_closed(&mut h, "shop", |_| {});
    h.state_mut().services.projects = None;
    h.state_mut().discard_tab(0, false);
    h.run_steps(2);
    assert!(h.state().tabs.is_empty() && !h.state().is_held() && !h.state().holding.pending());
}

/// Another project's closing refused meanwhile holds nothing: only the held project's does.
#[test]
fn another_projects_closing_refused_does_not_bring_the_hold_back() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    h.run_steps(3);
    h.state_mut().close_tab(0);
    h.run_steps(2);
    held_until_closed(&mut h, "billing", |_| {});
    let closing = h.state().tabs.iter().position(|t| t.path.ends_with("Library.archforge")).unwrap();
    h.state_mut().discard_tab(closing, false);
    h.run_steps(2);
    assert_eq!(h.state_mut().begin_hold(), Err(crate::hold::HoldRefused::Another), "its closing is not over yet");
    let first = host.closings.lock().unwrap().remove(0);
    first.send(Err("the disk is full".into())).unwrap();
    h.run_steps(3);
    assert!(!h.state().is_held(), "the project refused was not the one held");
}

/// A quit asked for while a held project closes waits for that, with nothing unsaved too.
#[test]
fn a_quit_during_a_held_projects_closing_waits_and_goes_with_a_refusal() {
    for refused in [true, false] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        held_until_closed(&mut h, "shop", |_| {});
        h.state_mut().session.as_mut().unwrap().mark_saved().unwrap();
        h.state_mut().discard_tab(0, false);
        h.state_mut().request_quit();
        h.run_steps(3);
        assert!(!h.state().quit_confirmed(), "it waits for the closing");
        let answer = if refused { Err("the disk is full".to_string()) } else { Ok(()) };
        host.closings.lock().unwrap().pop().unwrap().send(answer).unwrap();
        h.run_steps(4);
        assert_eq!((h.state().quit_confirmed(), h.state().is_held()), (!refused, refused));
    }
}

/// Another project closing for real leaves the hold where it is.
#[test]
fn another_project_closing_does_not_end_the_hold() {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    h.run_steps(3);
    h.state_mut().close_tab(0);
    h.run_steps(2);
    held_until_closed(&mut h, "billing", |_| {});
    host.closings.lock().unwrap().remove(0).send(Ok(())).unwrap();
    h.run_steps(3);
    assert!(h.state().is_held() && h.state().tabs.len() == 1);
}

/// What waited behind a hold kept until closed asks again once it has been closed.
#[test]
fn a_close_that_waited_behind_a_hold_until_closed_asks_again() {
    use crate::close_ask::Ask;
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
    h.run_steps(3);
    held_until_closed(&mut h, "billing", |s| s.close_tab(0));
    h.state_mut().discard_tab(1, false);
    h.run_steps(3);
    assert!(h.state().ask.is_none(), "what waited waits for the closing's answer");
    host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
    h.run_steps(3);
    assert!(!h.state().is_held());
    assert!(matches!(&h.state().ask, Some(Ask::HoldFailed { path, .. }) if path.ends_with("Commerce platform.archforge")), "{:?}", h.state().ask);
    assert!(host.closed.lock().unwrap().is_empty());
}

/// Cancelling a question while an ordinary hold runs leaves a quit that waited in place.
#[test]
fn keep_open_forgets_a_waiting_quit_only_when_held_until_closed() {
    let (mut h, _) = saving_harness(Some(commerce()), None);
    add_schema(&mut h, "shop");
    h.state_mut().begin_hold().unwrap();
    h.state_mut().request_quit();
    h.run_steps(2);
    h.state_mut().ask = asked_unsaved(COMMERCE);
    h.run_steps(2);
    let at = rect_of(&h, "close-ask.cancel").center();
    click(&mut h, at);
    assert_eq!(h.state().ask, None);
    h.state_mut().end_hold();
    h.run_steps(3);
    assert_eq!(h.state().ask, asked_unsaved(COMMERCE), "the quit that waited still runs");
}

/// A chat write started after Chat settled is waited for before the job and before the replacement.
#[test]
fn a_chat_write_started_after_settling_holds_the_replacement_back() {
    use std::sync::atomic::Ordering::{Relaxed, SeqCst};
    let deleting = |wrote_first: bool, release: bool| {
        let (mut h, rig, chats) = chat_history(ThemeKind::Light);
        h.state_mut().services.projects = Some(std::sync::Arc::new(SavingHost::default()));
        h.steps(2);
        let rows = rows_now(&h);
        let (job, started, answer, _reading) = test_job();
        let read = start_replacing(&mut h, Some(job));
        if wrote_first {
            read.send(Ok(rows.clone())).unwrap();
            h.steps(3);
        }
        rig.store.hold.store(true, SeqCst);
        crate::chat_delete::ask(h.state_mut(), chats[2], 2);
        crate::chat_delete::confirm(h.state_mut());
        h.steps(2);
        if wrote_first {
            answer.send(Ok(())).unwrap();
        } else {
            read.send(Ok(rows)).unwrap();
        }
        h.steps(3);
        assert_eq!(started.load(SeqCst), wrote_first, "no job starts while Chat is writing");
        let generation = h.state().session_generation();
        assert!(h.state().is_held());
        if release {
            rig.store.release();
            h.steps(3);
            if !wrote_first {
                assert!(started.load(SeqCst));
                answer.send(Ok(())).unwrap();
                h.steps(3);
            }
            assert!(!h.state().is_held() && h.state().session_generation() > generation, "then it goes on");
        } else {
            rig.now.fetch_add(crate::hold::CHAT_SETTLE_LIMIT, Relaxed);
            h.steps(3);
        }
        h
    };
    deleting(false, true);
    deleting(true, true);
    assert!(!deleting(false, false).state().is_held(), "before anything was written the operation just stops");
    let h = deleting(true, false);
    assert!(h.state().is_held() && h.state().save_error.as_deref().is_some_and(|e| e.contains("could not tell")), "written, so it cannot just stop");
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
    let opened = crate::Opened { name: "Shop".into(), path, model: Some(std::sync::Arc::new(rows)), unsaved: true, notice: None };
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

/// The refusal of diagrams of `kinds` as the window words it in German, checked to be translated
/// and to name each kind given, and `template` of the catalogues with it as `{why}`.
fn refused_in_german(kinds: &[&str], template: &str) -> (String, String) {
    let kinds: Vec<String> = kinds.iter().map(|k| k.to_string()).collect();
    crate::i18n::set_language("en");
    let english = crate::unsupported_kinds(&kinds);
    crate::i18n::set_language("de");
    let german = crate::unsupported_kinds(&kinds);
    assert!(german != english && kinds.iter().all(|k| german.contains(k.as_str())), "{german}");
    let around = crate::i18n::fmt(&crate::i18n::tr(template), &[("why", &german)]);
    (german, around)
}

/// AR-548: an opening refused for diagrams of kinds Archforge does not support says so in the
/// user's words, each kind once, in the status bar and on Home; the project shown, the one parked
/// and the model's revision stay as they were.
#[test]
fn an_opening_refused_for_unsupported_diagram_kinds_says_so_and_keeps_the_projects() {
    in_both_appearances(|theme| {
        let (mut h, host) = saving_harness(Some(commerce()), Some("/tmp/Old.archforge"));
        h.state_mut().state.theme = theme;
        h.state_mut().state.language = "de".into();
        h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
        h.run_steps(2);
        add_schema(&mut h, "shop");
        let revision = h.state().session.as_ref().unwrap().revision();
        h.state_mut().open_from_picker();
        h.run_steps(2);
        let (_, tx) = host.opens.lock().unwrap().pop().expect("the host was asked to open");
        let kinds = ["old.class", "", "old.class", "legacy.kind"].map(String::from).to_vec();
        tx.send(Err(crate::CreateError::UnsupportedKinds { kinds })).unwrap();
        h.run_steps(4);
        let (said, line) = refused_in_german(&["old.class", "legacy.kind"], "The project could not be opened: {why}");
        let shell = h.state();
        assert_eq!(shell.open_error.as_deref(), Some(said.as_str()), "{theme:?}");
        assert_eq!(said.matches("old.class").count(), 1, "{theme:?}: each kind once");
        assert!(!shell.is_opening() && shell.tabs.len() == 2 && shell.active == Some(1), "{theme:?}: no tab opened");
        let shown = shell.project.as_ref().map(|o| o.path.clone());
        assert!(shell.tabs[0].parked.is_some() && shown == Some("/tmp/Library.archforge".into()), "{theme:?}");
        assert_eq!(shell.session.as_ref().map(|s| s.revision()), Some(revision), "{theme:?}");
        assert!(painted_has(&h, &line), "{theme:?}: {:?}", painted(&h));
        // Home says it too, with the projects still open in their tabs.
        h.state_mut().go_home();
        h.run_steps(2);
        let start: String = line.chars().take(60).collect();
        assert!(h.state().project.is_none() && painted_has(&h, &start), "{theme:?}: {:?}", painted(&h));
        assert_eq!(h.state().tabs.len(), 2, "{theme:?}");
    });
}

/// The sentence names the first few kinds, each once and cut short, and one with no kind at all.
#[test]
fn the_unsupported_kinds_sentence_names_a_few_kinds_each_once() {
    crate::i18n::set_language("en");
    let said = |kinds: &[&str]| crate::unsupported_kinds(&kinds.iter().map(|k| k.to_string()).collect::<Vec<_>>());
    assert_eq!(said(&["old.class"]), "It holds diagrams whose kind Archforge does not support (old.class); create a new project to carry on.");
    assert_eq!(said(&["", ""]), "It holds diagrams whose kind Archforge does not support; create a new project to carry on.");
    let long = "k".repeat(200);
    assert!(said(&["a", "b", "a", "c", "d", "e", &long]).contains("(a, b, c, …);"), "the first three, each once, and that more were found");
    assert!(said(&["a", "", "b", "a"]).contains("(a, b);"), "nothing more was found");
    assert!(said(&[&long]).contains(&format!("({});", "k".repeat(64))), "a kind is cut short");
}

/// A project with an ER diagram, then a schema "shop" added as unsaved work, and the rows of it
/// before "shop" with the diagram's kind as no notation knows it.
fn work_and_unsupported() -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>, std::sync::Arc<archforge_mof::rows::Rows>) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    let s = h.state_mut().session.as_mut().unwrap();
    let till = s.execute("sketch.package.add", &serde_json::json!({ "name": "till" })).unwrap().value["id"].as_str().unwrap().to_string();
    s.execute("diagram.add", &serde_json::json!({ "kind": SKETCH, "element": till, "name": "Till" })).unwrap();
    h.run_steps(2);
    let mut rows = (*rows_now(&h)).clone();
    let (tables, _) = NOTATIONS.storage();
    let at = tables.iter().position(|t| t.name == "diagram_diagram").unwrap();
    let kind = tables[at].columns.iter().position(|c| c.name == "kind").unwrap();
    rows.tables[at].iter_mut().for_each(|row| row[kind] = archforge_mof::rows::Cell::Text("legacy.kind".into()));
    add_schema(&mut h, "shop");
    (h, host, std::sync::Arc::new(rows))
}

/// AR-548: rows handed over with such diagrams build no model, and the status bar says why in the
/// user's words, never the store's.
#[test]
fn a_model_of_unsupported_diagram_kinds_says_why_in_the_status_bar() {
    let rows = work_and_unsupported().2;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = saving_harness(Some(crate::Opened { model: Some(rows.clone()), ..commerce() }), None);
        h.state_mut().state.theme = theme;
        h.state_mut().state.language = "de".into();
        h.run_steps(2);
        assert!(h.state().session.is_none(), "{theme:?}");
        assert_eq!(h.state().session_error, Some(crate::SessionError::UnsupportedKinds { kinds: vec!["legacy.kind".into()] }), "{theme:?}");
        let said = refused_in_german(&["legacy.kind"], "{why}").0;
        assert!(painted_has(&h, &crate::i18n::tr("The model could not be opened")) && painted_has(&h, &said), "{theme:?}: {:?}", painted(&h));
    }
}

/// AR-548: broken default-diagram bindings are no unsupported kind: the status bar and a refused
/// restore or switch give the store's own English words for them, as before the kind check.
#[test]
fn broken_bindings_keep_the_stores_words() {
    let broken = |n: u8| archforge_notations::BrokenBinding {
        binding: archforge_mof::Uuid([n; 16]),
        end: archforge_notations::BindingEnd::Diagram,
        names: Some(archforge_mof::Uuid([n + 100; 16])),
    };
    for all in [vec![broken(1)], vec![broken(1), broken(2), broken(3)]] {
        let e = archforge_notations::OpenError::BrokenBindings(all);
        let said = e.to_string();
        assert!(!said.is_empty(), "{e:?}");
        assert_eq!(crate::SessionError::from(e.clone()), crate::SessionError::Engine(said.clone()));
        assert_eq!(crate::open_refusal(e), said);
    }
}

/// AR-548: a restore of a version, or a switch to a branch, holding diagrams of kinds Archforge
/// does not support is refused in the user's words, never the store's, and the work stays.
#[test]
fn a_restore_or_a_switch_to_unsupported_diagram_kinds_says_so_and_keeps_the_work() {
    for restore in [true, false] {
        in_both(work_and_unsupported, |mut h, host, rows| {
            h.state_mut().state.language = "de".into();
            let revision = h.state().session.as_ref().unwrap().revision();
            if restore {
                h.state_mut().restore_version("c1", "2026-11-08T14:02:11Z").unwrap();
                h.run_steps(3);
                answer_read(&mut h, &host, Ok(rows));
            } else {
                h.state_mut().switch_branch("old".into()).unwrap();
                h.run_steps(4);
                host.switches.lock().unwrap().remove(0).check.send(Ok(rows)).unwrap();
                h.run_steps(4);
            }
            // A restore adds that nothing was changed, as for any version it can't build.
            let (said, expected) = refused_in_german(&["legacy.kind"], if restore { "{why} Nothing was changed." } else { "{why}" });
            assert_eq!(h.state().save_error.as_deref(), Some(expected.as_str()), "restore {restore}");
            assert!(painted_has(&h, &said), "restore {restore}: {:?}", painted(&h));
            assert!(!h.state().is_held() && schema_names(&h).contains(&"shop".to_string()), "restore {restore}: the work stays");
            assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "restore {restore}");
        });
    }
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
    // It holds unsaved changes, so it asks: Save, its label white on the accent, and once
    // saved it closes.
    let save = rect_of(&h, "close-ask.save");
    assert_eq!(label_colours(&h, "Save", save), [egui::Color32::from(crate::ux::palette(false).on_accent)]);
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

/// The most tabs a window opens stay left of Help at every width, down to the smallest window.
#[test]
fn eight_tabs_stay_left_of_help_at_every_width_down_to_the_smallest_window() {
    for w in [640.0, 900.0, 1600.0] {
        crate::i18n::set_language("en");
        let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() }, NOTATIONS, &SCREENS);
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
    let entry = |name: &str, path: std::path::PathBuf| crate::state::RecentProject { name: name.into(), path, opened: 1, view: None, branch: None };
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

/// The project shown, its tab closed and its Recent row chosen while the host closes it.
fn reopened_while_closing(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().state.theme = theme;
    h.state_mut().close_tab(0);
    h.run_steps(2);
    let at = rect_of(&h, "home.recent.0").center();
    click(&mut h, at);
    h.run_steps(2);
    assert!(h.state().tabs[0].closing() && host.opens.lock().unwrap().is_empty(), "{theme:?}: nothing opens while it closes");
    assert_eq!(h.state().holding.reopen, [std::path::PathBuf::from(COMMERCE)], "{theme:?}: the opening waits");
    (h, host)
}

/// An opening waiting for its tab to close keeps waiting while a folder pick waits, once the closing
/// ends, and runs when the pick is answered: the pick would turn it away (AR-737).
#[test]
fn a_reopen_waits_for_a_folder_pick_answered_after_its_tab_closed() {
    in_both_appearances(|theme| {
        let (mut h, host) = reopened_while_closing(theme);
        let (answer, rx) = std::sync::mpsc::channel();
        h.state_mut().ask_pick(crate::Pick::Pin, || rx);
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(4);
        assert!(host.opens.lock().unwrap().is_empty(), "{theme:?}: nothing opens while the pick waits");
        assert_eq!(h.state().holding.reopen, [std::path::PathBuf::from(COMMERCE)], "{theme:?}: the opening still waits");
        drop(answer);
        h.run_steps(4);
        assert_eq!(host.opens.lock().unwrap().len(), 1, "{theme:?}: it opens once the pick is answered");
    });
}

/// An opening of a project whose tab is closing waits for the closing, then opens it afresh in a
/// tab of its own; asked for twice, it opens once (AR-748).
#[test]
fn a_project_opened_while_its_tab_closes_opens_again_once_closed() {
    in_both_appearances(|theme| {
        let (mut h, host) = reopened_while_closing(theme);
        let at = rect_of(&h, "home.recent.0").center();
        click(&mut h, at);
        h.run_steps(2);
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.step();
        h.step();
        // The frame that took the closing asked for the next, which takes the opening's answer.
        let asked = h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("ui-egui/src/lib.rs"));
        assert!(asked, "{theme:?}: {:?}", h.ctx.repaint_causes());
        h.run_steps(1);
        let (path, tx) = host.opens.lock().unwrap().pop().expect("the host opens it once the closing ends");
        tx.send(Ok(crate::Opened { name: "Commerce platform".into(), path, ..Default::default() })).unwrap();
        h.run_steps(4);
        assert!(host.opens.lock().unwrap().is_empty(), "{theme:?}: the second ask opened nothing more");
        assert_eq!((h.state().tabs.len(), h.state().active, h.state().title().as_str()), (1, Some(0), "Commerce platform — Archforge"));
        assert!(!h.state().tabs[0].closing() && !h.state().holding.pending());
        // Asked twice and the opening failed: the host is not asked again behind the failure.
        let (mut h, host) = reopened_while_closing(theme);
        let at = rect_of(&h, "home.recent.0").center();
        click(&mut h, at);
        h.run_steps(2);
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(3);
        let (_, tx) = host.opens.lock().unwrap().pop().expect("the host opens it once the closing ends");
        tx.send(Err(crate::CreateError::Failed { detail: "the folder is gone".into() })).unwrap();
        h.run_steps(4);
        assert!(host.opens.lock().unwrap().is_empty() && h.state().holding.reopen.is_empty(), "{theme:?}: asked once");
        assert_eq!(h.state().open_error.as_deref(), Some("Commerce platform: the folder is gone"), "{theme:?}: named");
    });
}

/// A closing refused while an opening of its project waits keeps the tab, which shows, saying why.
#[test]
fn a_project_opened_while_its_closing_is_refused_shows_its_tab() {
    in_both_appearances(|theme| {
        let (mut h, host) = reopened_while_closing(theme);
        h.state_mut().open_path(std::path::Path::new("/tmp/Shop.archforge"));
        host.closings.lock().unwrap().pop().unwrap().send(Err("the server would not stop".into())).unwrap();
        h.run_steps(3);
        assert_eq!((h.state().tabs.len(), h.state().active, h.state().close_error()), (1, Some(0), Some("the server would not stop")));
        assert!(h.state().holding.reopen.is_empty(), "{theme:?}: the opening ran on the tab kept, not after the other opening");
        let (shop, tx) = host.opens.lock().unwrap().pop().expect("the other opening");
        tx.send(Ok(crate::Opened { name: "Shop".into(), path: shop, ..Default::default() })).unwrap();
        h.run_steps(3);
        assert_eq!((h.state().tabs.len(), h.state().active), (2, Some(1)), "{theme:?}: the project asked for last shows");
    });
}

/// Once its tab has closed, the waiting opening also waits for another opening or the New project
/// dialog under way, which would drop it (AR-748).
#[test]
fn a_project_opened_while_its_tab_closes_waits_for_another_opening_and_the_new_project_dialog() {
    in_both_appearances(|theme| {
        let (mut h, host) = reopened_while_closing(theme);
        h.state_mut().open_path(std::path::Path::new("/tmp/Shop.archforge"));
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(3);
        let (shop, tx) = host.opens.lock().unwrap().pop().expect("the other opening");
        assert!(h.state().tabs.is_empty() && host.opens.lock().unwrap().is_empty(), "{theme:?}: it waits for that opening");
        tx.send(Ok(crate::Opened { name: "Shop".into(), path: shop, ..Default::default() })).unwrap();
        h.run_steps(3);
        assert_eq!(host.opens.lock().unwrap().pop().map(|(p, _)| p), Some(COMMERCE.into()), "{theme:?}: then it opens");
        let (mut h, host) = reopened_while_closing(theme);
        crate::new_project::open(h.state_mut());
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(3);
        assert!(h.state().tabs.is_empty() && host.opens.lock().unwrap().is_empty(), "{theme:?}: it waits for the dialog");
        h.state_mut().new_project = None;
        h.run_steps(3);
        assert_eq!(host.opens.lock().unwrap().pop().map(|(p, _)| p), Some(COMMERCE.into()), "{theme:?}: then it opens");
    });
}

/// An opening waiting for a tab's closing is no change under way: a hold in another tab starts,
/// and the opening, once the closing ends, waits for that hold, then for an opening queued behind
/// it, so that one does not drop it (AR-748).
#[test]
fn a_project_waiting_for_its_tab_to_close_does_not_hold_up_another_tab() {
    in_both_appearances(|theme| {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
        h.run_steps(3);
        h.state_mut().close_tab(0);
        h.run_steps(2);
        h.state_mut().open_path(std::path::Path::new(COMMERCE));
        assert_eq!(h.state_mut().begin_hold(), Ok(()), "{theme:?}: Library's hold starts");
        h.state_mut().open_path(std::path::Path::new("/tmp/Shop.archforge"));
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(3);
        assert!(h.state().tabs.len() == 1 && host.opens.lock().unwrap().is_empty(), "{theme:?}: it waits for the hold");
        end_hold(&mut h, Shell::end_hold);
        let (shop, tx) = host.opens.lock().unwrap().pop().expect("Shop, asked for during the hold");
        assert!(host.opens.lock().unwrap().is_empty(), "{theme:?}: Commerce waits for Shop's opening");
        tx.send(Ok(crate::Opened { name: "Shop".into(), path: shop, ..Default::default() })).unwrap();
        h.run_steps(3);
        assert_eq!(host.opens.lock().unwrap().pop().map(|(p, _)| p), Some(COMMERCE.into()));
        // Asked for again during the hold, its tab gone by then: still one opening, not retried.
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
        h.run_steps(3);
        h.state_mut().close_tab(0);
        h.run_steps(2);
        h.state_mut().open_path(std::path::Path::new(COMMERCE));
        h.state_mut().begin_hold().unwrap();
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(3);
        h.state_mut().open_path(std::path::Path::new(COMMERCE));
        end_hold(&mut h, Shell::end_hold);
        let (_, tx) = host.opens.lock().unwrap().pop().expect("Commerce's opening");
        tx.send(Err(crate::CreateError::Failed { detail: "the folder is gone".into() })).unwrap();
        h.run_steps(3);
        assert!(host.opens.lock().unwrap().is_empty() && h.state().holding.reopen.is_empty(), "{theme:?}: asked once");
    });
}

/// Two projects reopened after their tabs closed: the first one's failure, named, stays shown
/// while the second opens and after it has opened, its first model handed over too, until the
/// person changes it (AR-748).
#[test]
fn a_reopen_that_failed_stays_said_while_the_next_reopen_opens() {
    in_both_appearances(|theme| {
        for read_back in [true, false] {
            let (mut h, host) = saving_harness(Some(commerce()), None);
            h.state_mut().state.theme = theme;
            let library = std::path::PathBuf::from("/tmp/Library.archforge");
            h.state_mut().open_project(crate::Opened { name: "Library".into(), path: library.clone(), ..Default::default() });
            h.run_steps(3);
            // What Library holds, read back as it opens again, or nothing: a fresh model is handed over.
            let model = Some(rows_now(&h)).filter(|_| read_back);
            h.state_mut().close_tab(1);
            h.state_mut().close_tab(0);
            h.run_steps(2);
            h.state_mut().open_path(std::path::Path::new(COMMERCE));
            h.state_mut().open_path(&library);
            for closing in host.closings.lock().unwrap().drain(..) {
                closing.send(Ok(())).unwrap();
            }
            h.run_steps(3);
            let (_, tx) = host.opens.lock().unwrap().pop().expect("Commerce's opening");
            tx.send(Err(crate::CreateError::Failed { detail: "the folder is gone".into() })).unwrap();
            h.run_steps(3);
            let said = Some("Commerce platform: the folder is gone");
            assert_eq!(h.state().open_error.as_deref(), said, "{theme:?}: said while Library opens");
            let (path, tx) = host.opens.lock().unwrap().pop().expect("Library's opening");
            tx.send(Ok(crate::Opened { name: "Library".into(), path, model, ..Default::default() })).unwrap();
            h.run_steps(3);
            assert_eq!((h.state().tabs.len(), h.state().open_error.as_deref()), (1, said), "{theme:?} {read_back}: and once it has opened");
            add_schema(&mut h, "shop");
            assert_eq!(h.state().open_error, None, "{theme:?} {read_back}: a change of the person's replaces it");
        }
    });
}

/// A reopen refused at once, every tab taken by an opening that went before it, says so with the
/// project's name (AR-748).
#[test]
fn a_reopen_with_no_tab_free_says_which_project() {
    in_both_appearances(|theme| {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        for n in 1..crate::MAX_TABS {
            h.state_mut().open_project(crate::Opened { name: format!("P{n}"), path: format!("/tmp/P{n}.archforge").into(), ..Default::default() });
        }
        h.run_steps(3);
        h.state_mut().close_tab(0);
        h.run_steps(2);
        h.state_mut().open_path(std::path::Path::new(COMMERCE));
        h.state_mut().begin_hold().unwrap();
        h.state_mut().open_path(std::path::Path::new("/tmp/Shop.archforge"));
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(3);
        end_hold(&mut h, Shell::end_hold);
        let (shop, tx) = host.opens.lock().unwrap().pop().expect("Shop, asked for during the hold");
        tx.send(Ok(crate::Opened { name: "Shop".into(), path: shop, ..Default::default() })).unwrap();
        h.run_steps(3);
        assert_eq!(h.state().tabs.len(), crate::MAX_TABS, "{theme:?}: Shop took the last tab");
        assert_eq!(h.state().open_error.as_deref(), Some("Commerce platform: Close a project to open another"), "{theme:?}");
        assert!(h.state().holding.reopen.is_empty() && host.opens.lock().unwrap().is_empty(), "{theme:?}: nothing more is asked");
    });
}

/// Openings that waited for a hold show their open tabs while another project is opening, and
/// what waited behind them runs (AR-748).
#[test]
fn openings_after_a_hold_show_open_tabs_while_another_project_opens() {
    in_both_appearances(|theme| {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
        h.run_steps(3);
        h.state_mut().open_path(std::path::Path::new("/tmp/Shop.archforge"));
        h.state_mut().begin_hold().unwrap();
        h.state_mut().open_path(std::path::Path::new(COMMERCE));
        h.state_mut().close_tab(0);
        end_hold(&mut h, Shell::end_hold);
        assert_eq!(*host.closed.lock().unwrap(), [std::path::PathBuf::from(COMMERCE)], "{theme:?}: shown, then closed");
        assert_eq!(host.opens.lock().unwrap().len(), 1, "{theme:?}: Shop is still opening");
        // The opening alone shows Commerce's tab before Shop has opened.
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
        h.run_steps(3);
        h.state_mut().open_path(std::path::Path::new("/tmp/Shop.archforge"));
        h.state_mut().begin_hold().unwrap();
        h.state_mut().open_path(std::path::Path::new(COMMERCE));
        end_hold(&mut h, Shell::end_hold);
        assert_eq!((h.state().active, host.opens.lock().unwrap().len()), (Some(0), 1), "{theme:?}: Commerce shows, Shop still opening");
    });
}

/// A quit asked for while an opening waits for its tab's closing keeps it from starting; called
/// off, the opening goes on (AR-748).
#[test]
fn a_project_waiting_for_its_tab_to_close_does_not_open_while_quitting() {
    in_both_appearances(|theme| {
        let (mut h, host) = reopened_while_closing(theme);
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.state_mut().request_quit();
        h.run_steps(3);
        assert!(h.state().quit_confirmed() && host.opens.lock().unwrap().is_empty(), "{theme:?}: the window is closing");
        // A quit still asking about another project's changes, then called off.
        let (mut h, host) = edited_project();
        h.state_mut().state.theme = theme;
        h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
        h.run_steps(3);
        h.state_mut().close_tab(1);
        h.run_steps(2);
        h.state_mut().open_path(std::path::Path::new("/tmp/Library.archforge"));
        h.state_mut().request_quit();
        host.closings.lock().unwrap().pop().unwrap().send(Ok(())).unwrap();
        h.run_steps(3);
        assert_eq!(h.state().ask, asked_unsaved(COMMERCE), "{theme:?}: the quit asks about Commerce");
        assert!(host.opens.lock().unwrap().is_empty(), "{theme:?}: Library waits while quitting");
        let ours = |h: &Harness<'static, Shell>| h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("ui-egui/src/lib.rs"));
        h.step();
        assert!(!ours(&h), "{theme:?}: only the person's answer moves it on: {:?}", h.ctx.repaint_causes());
        // Cancel, pressed and let go in one frame: that frame asks for the next.
        let at = rect_of(&h, "close-ask.cancel").center();
        let button =
            |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        h.input_mut().events.extend([egui::Event::PointerMoved(at), button(true), button(false)]);
        h.step();
        assert!(!h.state().quitting() && host.opens.lock().unwrap().is_empty(), "{theme:?}: called off");
        h.step();
        assert!(ours(&h), "{theme:?}: {:?}", h.ctx.repaint_causes());
        h.run_steps(2);
        assert_eq!(host.opens.lock().unwrap().pop().map(|(p, _)| p), Some("/tmp/Library.archforge".into()));
    });
}

/// The folder a test's pick answers with.
const PICKED: &str = "/Users/robin/Picked";

/// Every kind of folder pick.
const PICKS: [crate::Pick; 3] = [crate::Pick::Open, crate::Pick::Pin, crate::Pick::Location];

/// A folder pick for `pick` waiting, its answer for the test to send: a Location pick asked from the
/// New project dialog, opened for it.
fn pick_waiting(h: &mut Harness<'static, Shell>, pick: crate::Pick) -> std::sync::mpsc::Sender<Picked> {
    if pick == crate::Pick::Location {
        crate::new_project::open(h.state_mut());
    }
    let (answer, rx) = std::sync::mpsc::channel();
    h.state_mut().ask_pick(pick, || rx);
    h.run_steps(2);
    assert!(h.state().picking.is_some(), "{pick:?}: the pick waits");
    answer
}

/// Counts the picks the shell withdraws from its host (`Services::withdraw_pick`).
fn count_withdrawals(h: &mut Harness<'static, Shell>) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = count.clone();
    h.state_mut().services.withdraw_pick = Some(std::sync::Arc::new(move || {
        counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    count
}

/// Whether [`PICKED`] was taken for `pick`: a project opening, a folder pinned, or the New project
/// dialog's folder.
fn pick_taken(h: &Harness<'static, Shell>, host: &SavingHost, pick: crate::Pick) -> bool {
    let picked = std::path::Path::new(PICKED);
    match pick {
        crate::Pick::Open => host.opens.lock().unwrap().iter().any(|(p, _)| p == picked),
        crate::Pick::Pin => h.state().state.pinned.iter().any(|p| p == picked),
        crate::Pick::Location => h.state().new_project.as_ref().and_then(|d| d.folder.as_deref()) == Some(picked),
    }
}

/// A quit by `request_quit`, the window's close button, ⌘Q or a field letting go first goes ahead
/// while an Open, a Pin or a Location pick waits, and the pick goes with it, withdrawn from the host
/// once (AR-738): an answer that comes after all, or in the quit's own frame, opens, pins or
/// changes nothing (AR-759).
#[test]
fn a_quit_goes_ahead_while_a_folder_pick_waits_and_lets_the_pick_go() {
    let ways: [(&str, fn(&mut Harness<'static, Shell>)); 4] = [
        ("request_quit", |h| h.state_mut().request_quit()),
        ("the close button", |h| h.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close)),
        ("⌘Q", |h| {
            // In the input of one frame, as the answer is: kittest runs a frame per event it queues.
            let modifiers = egui::Modifiers::MAC_CMD.plus(egui::Modifiers::COMMAND);
            h.input_mut().events.push(egui::Event::Key { key: egui::Key::Q, physical_key: None, pressed: true, repeat: false, modifiers });
        }),
        ("a field letting go", |h| h.state_mut().edit.closing = Some(crate::actions::Closing::Asked(crate::actions::Then::Quit))),
    ];
    in_both_appearances(|theme| {
        for ((pick, (way, quit)), in_its_frame) in PICKS.into_iter().flat_map(|p| ways.map(|w| (p, w))).flat_map(|c| [(c, false), (c, true)]) {
            let what = format!("{theme:?}, {pick:?}, {way}, answered in the quit's frame {in_its_frame}");
            let (mut h, host) = saving_harness(Some(commerce()), None);
            h.state_mut().state.theme = theme;
            let answer = pick_waiting(&mut h, pick);
            let withdrawn = count_withdrawals(&mut h);
            quit(&mut h);
            if in_its_frame {
                answer.send(Picked::Folder(PICKED.into())).unwrap();
            }
            h.run_steps(3);
            assert!(h.state().quit_confirmed(), "{what}: the quit goes ahead");
            assert_eq!(withdrawn.load(std::sync::atomic::Ordering::SeqCst), 1, "{what}: the pick is withdrawn from the host once");
            // The pick was let go: an answer still sent finds nobody waiting for it.
            assert!(answer.send(Picked::Folder(PICKED.into())).is_err(), "{what}: the pick is let go");
            h.run_steps(3);
            assert!(!pick_taken(&h, &host, pick), "{what}: the answer does nothing");
        }
    });
}

/// A quit asking about unsaved changes keeps a waiting pick's answer until the question is
/// answered: called off with Cancel, the pick still waits and its answer, come meanwhile or after,
/// is taken, nothing withdrawn; gone ahead with Don't save, the answer that came meanwhile does
/// nothing (AR-759) and the pick is withdrawn from the host (AR-738).
#[test]
fn a_quit_asking_about_unsaved_changes_holds_a_folder_picks_answer_until_it_is_answered() {
    in_both_appearances(|theme| {
        for (pick, (cancel, meanwhile)) in PICKS.into_iter().flat_map(|p| [(true, false), (true, true), (false, true)].map(|c| (p, c))) {
            let what = format!("{theme:?}, {pick:?}, cancelled {cancel}, answered meanwhile {meanwhile}");
            let (mut h, host) = edited_project();
            h.state_mut().state.theme = theme;
            let answer = pick_waiting(&mut h, pick);
            let withdrawn = count_withdrawals(&mut h);
            h.state_mut().request_quit();
            h.run_steps(2);
            assert_eq!(h.state().ask, asked_unsaved(COMMERCE), "{what}: the quit asks first");
            if meanwhile {
                answer.send(Picked::Folder(PICKED.into())).unwrap();
                h.run_steps(3);
                assert!(!pick_taken(&h, &host, pick), "{what}: the answer waits for the question");
            }
            let at = rect_of(&h, if cancel { "close-ask.cancel" } else { "close-ask.discard" }).center();
            click(&mut h, at);
            h.run_steps(2);
            if cancel {
                assert!(!h.state().quitting() && !h.state().quit_confirmed(), "{what}: the quit is called off");
                if !meanwhile {
                    answer.send(Picked::Folder(PICKED.into())).unwrap();
                }
                h.run_steps(3);
                assert!(pick_taken(&h, &host, pick), "{what}: the answer is taken");
                assert_eq!(withdrawn.load(std::sync::atomic::Ordering::SeqCst), 0, "{what}: nothing withdrawn");
            } else {
                host.closings.lock().unwrap().pop().expect("the project closes without its changes").send(Ok(())).unwrap();
                h.run_steps(3);
                assert!(h.state().quit_confirmed(), "{what}: the quit goes ahead");
                assert!(!pick_taken(&h, &host, pick), "{what}: the answer does nothing");
                assert_eq!(withdrawn.load(std::sync::atomic::Ordering::SeqCst), 1, "{what}: the pick is withdrawn from the host");
            }
        }
    });
}

/// A quit waiting for a hold keeps a waiting pick's answer too, and lets the pick go once it goes
/// ahead after the hold: the answer does nothing (AR-759).
#[test]
fn a_quit_waiting_for_a_hold_holds_a_folder_picks_answer_and_then_lets_it_go() {
    in_both_appearances(|theme| {
        for pick in PICKS {
            let (mut h, host) = saving_harness(Some(commerce()), None);
            h.state_mut().state.theme = theme;
            let answer = pick_waiting(&mut h, pick);
            h.state_mut().begin_hold().unwrap();
            h.state_mut().request_quit();
            h.run_steps(3);
            answer.send(Picked::Folder(PICKED.into())).unwrap();
            h.run_steps(3);
            assert!(!h.state().quit_confirmed() && !pick_taken(&h, &host, pick), "{theme:?}, {pick:?}: both wait for the hold");
            end_hold(&mut h, Shell::end_hold);
            h.run_steps(3);
            assert!(h.state().quit_confirmed(), "{theme:?}, {pick:?}: the quit goes ahead");
            assert!(!pick_taken(&h, &host, pick), "{theme:?}, {pick:?}: the answer does nothing");
        }
    });
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
            Picked::at_once(None)
        })),
        pick_folder: Some(std::sync::Arc::new(|_| Picked::at_once(Some(std::path::PathBuf::from("/Users/robin/Work/Models"))))),
        ..Services::default()
    };
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        Shell::new(services, NOTATIONS, &SCREENS),
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
    // With Shift too, which the field would otherwise take to end its input.
    key(&mut h, egui::Key::Enter, egui::Modifiers::SHIFT);
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
    // With a modifier the arrows and Esc move and close nothing, and Enter runs nothing with Control
    // or the command key (with Shift or Alt it runs, as the field would end its input on them).
    let enter = NOT_ALONE[2..].iter().map(|m| (egui::Key::Enter, *m));
    for (k, more) in [egui::Key::ArrowDown, egui::Key::Escape].into_iter().flat_map(|k| NOT_ALONE.map(|m| (k, m))).chain(enter) {
        key(&mut h, k, more);
        let search = &h.state().home_search;
        assert!(search.open && search.highlight == 1 && host.opens.lock().unwrap().is_empty(), "{k:?} with {more:?}");
    }
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
        pick_project: Some(std::sync::Arc::new(|_| Picked::at_once(None))),
        find_projects: Some(std::sync::Arc::new(move |folders: Vec<std::path::PathBuf>, wake: Box<dyn Fn() + Send>| {
            seen.lock().unwrap().push(folders);
            let (tx, rx) = std::sync::mpsc::channel();
            waiting.lock().unwrap().push((tx, wake));
            rx
        })),
        ..Services::default()
    };
    let mut shell = Shell::new(services, NOTATIONS, &SCREENS);
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
    // The answer comes, and its wake is called: the projects found join the list, which keeps its
    // highlight on the row highlighted, the pinned folder, wherever the rows found put it.
    key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
    assert_eq!(h.state().home_search.highlight, 1);
    let (tx, wake) = pending.lock().unwrap().pop().unwrap();
    tx.send(vec![
        crate::FoundProject { path: "/Users/robin/Work/Client/Billing.archforge".into(), folder: 0, within: vec!["Client".into()] },
        crate::FoundProject { path: "/tmp/Shop.archforge".into(), folder: 0, within: vec![] },
    ])
    .unwrap();
    wake();
    h.run_steps(2);
    assert_eq!(h.state().home_search.highlight, 2, "Shop, Billing, then the pinned folder highlighted");
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
    /// Adds the text of the sources under `dir` but the state and the tests to `sources`; a
    /// listing that fails part way fails the test.
    fn collect(dir: &std::path::Path, sources: &mut String) {
        for entry in std::fs::read_dir(dir).unwrap().map(Result::unwrap) {
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

    /// A font scan that panics leaves no fallback order and nothing in flight to wait for.
    #[test]
    fn a_scan_that_panics_leaves_nothing_to_wait_for() {
        let mut s = Shell::new(Services::default(), NOTATIONS, &SCREENS);
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
        /// The bytes of the face `entry` names, or a refusal when none of that name is served.
        fn read(&self, entry: &FontEntry, _cap: u64) -> Result<Vec<u8>, String> {
            self.0.iter().find(|(f, _)| *f == entry.key).map(|(_, b)| b.clone()).ok_or_else(|| "gone".to_string())
        }
    }

    /// A shell whose installed fonts come from `source`, framed until its scan has finished, with
    /// the context it was framed in.
    fn scripted(source: Scripts) -> (Shell, egui::Context) {
        let mut s = Shell::new(Services::default(), NOTATIONS, &SCREENS);
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
        let mut web = Shell::new(Services::default(), NOTATIONS, &SCREENS);
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
            let text =
                [archforge_shell_api::i18n::CATALOGUES, i18n::CATALOGUES].map(|table| table.iter().find(|c| c.code == code).unwrap().text).concat();
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
        let text = [archforge_shell_api::i18n::CATALOGUES, crate::i18n::CATALOGUES]
            .map(|table| table.iter().find(|c| c.code == lang.code).unwrap().text)
            .concat();
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
    let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() }, NOTATIONS, &SCREENS);
    shell.state.theme = theme;
    shell.open_project(crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into(), ..Default::default() });
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

/// Clicks the in-window bar's File title, by the pointer or else as a screen reader does, with `key`
/// and `modifiers` pressed and let go in the frame of the click, as both can arrive together, and
/// asserts that the click opened File (AR-518).
fn open_file_with(h: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers, pointer: bool) {
    let (at, primary, none) = (rect_of(h, "strip.menu.file").center(), egui::PointerButton::Primary, egui::Modifiers::NONE);
    let click = if pointer {
        h.event(egui::Event::PointerMoved(at));
        h.event(egui::Event::PointerButton { pos: at, button: primary, pressed: true, modifiers: none });
        h.step();
        egui::Event::PointerButton { pos: at, button: primary, pressed: false, modifiers: none }
    } else {
        let (action, target_tree) = (egui::accesskit::Action::Click, egui::accesskit::TreeId::ROOT);
        let target_node = crate::widgets::egui_id("strip.menu.file").accesskit_id();
        egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest { action, target_tree, target_node, data: None })
    };
    // kittest runs a frame per event it queues, so the click and the key go in by hand, together.
    let press = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
    h.input_mut().events.extend([click, press(true), press(false)]);
    h.step();
    assert_eq!(h.state().menu_bar.open, Some("file"), "the click opened the bar's File menu");
}

/// The bar's controls in the frame.
fn menu_ids(h: &Harness<'static, Shell>) -> Vec<String> {
    ids(h).into_iter().filter(|i| i.starts_with("strip.menu.")).collect()
}

/// Told of a native menu, the shell draws no bar; otherwise the groups sit after Open project,
/// centred on the strip, with no Window menu (the window's frame minimises and maximises it there,
/// where macOS has one: AR-463), and File opens under its button at the design's sizes and order.
#[test]
fn the_bar_is_drawn_when_told_and_opens_a_menu_at_the_designs_sizes() {
    for (theme, project) in [(ThemeKind::Light, false), (ThemeKind::Dark, false), (ThemeKind::Light, true), (ThemeKind::Dark, true)] {
        let mut h = menu_harness(theme, project);
        h.state_mut().in_window_menu = false;
        h.run();
        assert!(menu_ids(&h).is_empty(), "{theme:?}: no bar on macOS");
        h.state_mut().in_window_menu = true;
        h.run();
        let (file, help, mid) = (rect_of(&h, "strip.menu.file"), rect_of(&h, "strip.menu.help"), rect_of(&h, "strip.theme").center().y);
        assert_eq!(
            menu_ids(&h),
            ["strip.menu.file", "strip.menu.edit", "strip.menu.view", "strip.menu.model", "strip.menu.versions", "strip.menu.help"],
            "{theme:?}"
        );
        assert!(
            file.left() >= rect_of(&h, "strip.open-project").right() && file.height() == 24.0 && help.left() > file.right() && file.center().y == mid,
            "{theme:?}"
        );
        click_menu(&mut h, "strip.menu.file");
        // File's groups, as on every system, and then Settings… and Exit, each set apart.
        let want = ["file.new", "file.open", "file.close", "file.save", "file.generate", "app.settings", "app.quit"]
            .map(|id| format!("strip.menu.row.{id}"));
        assert_eq!(menu_ids(&h)[6..], want, "{theme:?}");
        let rows = want.each_ref().map(|id| rect_of(&h, id));
        assert!(rows.iter().all(|r| r.width() == 292.0 && r.height() == 30.0 && r.left() == file.left() + 4.0), "{theme:?}");
        assert_eq!(rows[0].top(), mid + 16.0 + 4.0, "{theme:?}: under the strip, inside the padding");
        let gaps: Vec<f32> = rows.windows(2).map(|pair| pair[1].top() - pair[0].bottom()).collect();
        assert_eq!(gaps, [0.0, 0.0, 9.0, 9.0, 9.0, 9.0], "{theme:?}: one divider between groups");
    }
}

/// The bar in the window says the words of Windows and Linux, as `actions::file_title` and the
/// table give them: in German File is Datei, never the native bar's Ablage, and the last row Beenden.
#[test]
fn the_bar_in_the_window_says_its_systems_words() {
    use egui_kittest::kittest::Queryable;
    in_both_appearances(|theme| {
        let mut h = menu_harness(theme, false);
        h.state_mut().state.language = "de".into();
        h.run();
        click_menu(&mut h, "strip.menu.file");
        assert_eq!(crate::actions::file_title(h.state()), "Datei", "{theme:?}");
        assert!(h.query_by_label("Datei").is_some() && h.query_by_label("Ablage").is_none(), "{theme:?}: the title");
        assert!(h.query_by_label("Beenden").is_some() && h.query_by_label("Archforge beenden").is_none(), "{theme:?}: Exit");
    });
}

/// The underlines of a letter drawn inside `rects`: level lines no wider than a letter.
fn underlines(h: &Harness<'static, Shell>, rects: &[egui::Rect]) -> usize {
    let inside = |left: f32, right: f32, y: f32| rects.iter().any(|r| r.contains(egui::pos2((left + right) / 2.0, y)));
    level_lines(h).into_iter().filter(|(left, right, y, ..)| right - left < 12.0 && inside(*left, *right, *y)).count()
}

/// A letter that a label cut short no longer shows is not underlined: neither the ellipsis that
/// stands for it nor anything else.
#[test]
fn a_letter_cut_from_its_label_is_not_underlined() {
    let h = harness(ThemeKind::Light);
    let mut out = h.ctx.run_ui(egui::RawInput::default(), |ui| {
        let (paint, at) = (ui.painter(), egui::pos2(0.0, 0.0));
        let text = crate::ux::galley(paint, "Generate every diagram", crate::ux::types::SMALL, crate::ux::LIGHT.text, Some(60.0));
        let glyphs: Vec<char> = text.rows.iter().flat_map(|r| r.glyphs.iter().map(|g| g.chr)).collect();
        assert_eq!(glyphs.last(), Some(&'…'), "cut: {glyphs:?}");
        for index in [glyphs.len() - 1, glyphs.len() + 3, 0] {
            crate::ux::underline_char(paint, at, &text, index, crate::ux::types::SMALL, crate::ux::LIGHT.text);
        }
    });
    out.textures_delta.clear();
    let lines = out.shapes.iter().filter(|c| matches!(c.shape, egui::Shape::LineSegment { .. })).count();
    assert_eq!(lines, 1, "only the first character's");
}

/// Each title and row carries its access letter on its accessibility node, and underlines it only
/// while Alt alone is held or the keys walk a menu, as on Windows (AR-537).
#[test]
fn the_letters_are_underlined_while_alt_is_held_or_the_keys_walk_a_menu() {
    use egui::Modifiers;
    use egui_kittest::kittest::{NodeT, Queryable};
    in_both_appearances(|theme| {
        let mut h = menu_harness(theme, false);
        let titles =
            |h: &Harness<'static, Shell>| ["file", "edit", "view", "model", "versions", "help"].map(|g| rect_of(h, &format!("strip.menu.{g}")));
        let held = |h: &mut Harness<'static, Shell>, modifiers: Modifiers| {
            h.event(egui::Event::ModifiersChanged(modifiers));
            h.run();
            underlines(h, &titles(h))
        };
        assert_eq!(held(&mut h, Modifiers::NONE), 0, "{theme:?}: hidden until asked for");
        assert_eq!(held(&mut h, Modifiers::ALT), 6, "{theme:?}: Alt shows every title's");
        let alt_gr = Modifiers { alt: true, ctrl: true, command: true, ..Modifiers::NONE };
        assert_eq!(held(&mut h, alt_gr), 0, "{theme:?}: Ctrl+Alt, as AltGr comes, is not Alt");
        assert_eq!(held(&mut h, Modifiers::NONE), 0, "{theme:?}");
        key(&mut h, egui::Key::F10, Modifiers::NONE);
        let rows: Vec<egui::Rect> = menu_ids(&h).iter().filter(|i| i.starts_with("strip.menu.row.")).map(|i| rect_of(&h, i)).collect();
        assert_eq!((underlines(&h, &titles(&h)), underlines(&h, &rows)), (6, rows.len()), "{theme:?}: the keys walk File");
        let key_of = |name: &str| h.get_by_label(name).accesskit_node().data().access_key().map(str::to_string);
        assert_eq!([key_of("File"), key_of("Help"), key_of("Exit")], [Some("F".into()), Some("H".into()), Some("X".into())], "{theme:?}");
        for at in [rows[0].center(), rows[0].center() + egui::vec2(4.0, 0.0)] {
            h.event(egui::Event::PointerMoved(at));
        }
        h.run();
        assert_eq!(underlines(&h, &rows), 0, "{theme:?}: the pointer takes the menu, and the letters go");
    });
}

/// The letters the keys showed go when the menu closes, by any way it closes: Esc, F10, Enter on a
/// row, a press elsewhere or the window losing the focus.
#[test]
fn the_letters_go_when_the_keys_menu_closes() {
    use egui::{Key, Modifiers};
    in_both_appearances(|theme| {
        let closes: [(&str, fn(&mut Harness<'static, Shell>)); 5] = [
            ("Esc", |h| key(h, Key::Escape, Modifiers::NONE)),
            ("F10", |h| key(h, Key::F10, Modifiers::NONE)),
            ("Enter on Help's About", |h| {
                key(h, Key::ArrowLeft, Modifiers::NONE);
                key(h, Key::Enter, Modifiers::NONE);
            }),
            ("a press elsewhere", |h| click(h, egui::pos2(800.0, 500.0))),
            ("the window losing the focus", |h| {
                h.event(egui::Event::WindowFocused(false));
                h.run();
            }),
        ];
        for (how, close) in closes {
            let mut h = menu_harness(theme, false);
            let titles = ["file", "edit", "view", "model", "versions", "help"].map(|g| rect_of(&h, &format!("strip.menu.{g}")));
            key(&mut h, Key::F10, Modifiers::NONE);
            key(&mut h, Key::ArrowDown, Modifiers::NONE);
            assert_eq!(underlines(&h, &titles), 6, "{theme:?}: the keys walk File");
            close(&mut h);
            h.run();
            assert!(h.state().menu_bar.open.is_none(), "{theme:?}: {how} closed the menu");
            assert_eq!(underlines(&h, &titles), 0, "{theme:?}: {how} took the letters away");
        }
    });
}

/// A live row runs on the next frame and closes the menu; a row that is off takes no click.
#[test]
fn a_row_runs_its_action_and_a_row_off_takes_no_click() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        click_menu(&mut h, "strip.menu.file");
        click_menu(&mut h, "strip.menu.row.file.save");
        assert!(h.state().menu_bar.open.is_some() && h.state().settings.is_none(), "{theme:?}: Home has nothing to save");
        click_menu(&mut h, "strip.menu.row.app.settings");
        assert!(h.state().menu_bar.open.is_none() && h.state().settings.is_some(), "{theme:?}: it ran and closed");
    }
}

/// A row chosen acts on the project it was chosen on, though another finishes opening before it runs.
#[test]
fn a_chosen_close_closes_the_project_it_was_chosen_on() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, true);
        let (answer, opening) = std::sync::mpsc::channel();
        answer.send(Ok(crate::Opened { name: "Other".into(), path: "/tmp/Other.archforge".into(), ..Default::default() })).unwrap();
        h.state_mut().menu_bar.chosen = Some(crate::actions::CLOSE);
        h.state_mut().opening = Some(opening);
        h.run();
        assert_eq!(h.state().tabs.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Other"], "{theme:?}");
    }
}

/// A click on another group moves the open menu there; a click on the open one closes it.
#[test]
fn a_click_moves_the_menu_or_closes_it() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, true);
        click_menu(&mut h, "strip.menu.file");
        click_menu(&mut h, "strip.menu.help");
        assert_eq!((h.state().menu_bar.open, &menu_ids(&h)[6..]), (Some("help"), &["strip.menu.row.app.about".to_string()][..]), "{theme:?}");
        click_menu(&mut h, "strip.menu.help");
        assert_eq!(h.state().menu_bar.open, None, "{theme:?}");
    }
}

/// While a menu is open the keys stay from the window behind and the shortcuts, from the frame a click
/// opens it in; Esc closes it and stops there; a press elsewhere and a dialog over the window close it too.
#[test]
fn an_open_menu_holds_the_keys_and_escape_a_press_or_a_dialog_close_it() {
    let press = |h: &mut Harness<'static, Shell>, key, modifiers| {
        h.event(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
        h.run();
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        // Settings' chord as Windows and Linux send it, in the frame of a pointer's or a screen reader's click.
        for pointer in [true, false] {
            open_file_with(&mut h, egui::Key::Comma, egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND), pointer);
            assert!(h.state().settings.is_none(), "{theme:?}: nor the shortcuts in the frame of the click, by the pointer: {pointer}");
            menu_press(&mut h, egui::Key::Escape);
        }
        click_menu(&mut h, "strip.menu.file");
        press(&mut h, egui::Key::K, egui::Modifiers::COMMAND);
        assert!(!h.state().home_search.open, "{theme:?}: nothing behind reads it");
        press(&mut h, egui::Key::Comma, egui::Modifiers::COMMAND);
        assert!(h.state().settings.is_none(), "{theme:?}: nor the shortcuts");
        h.state_mut().home_search.open = true;
        press(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(h.state().menu_bar.open.is_none() && h.state().home_search.open, "{theme:?}: Esc stops at the menu");
        h.state_mut().home_search.open = false;
        click_menu(&mut h, "strip.menu.help");
        click(&mut h, egui::pos2(800.0, 600.0));
        assert_eq!(h.state().menu_bar.open, None, "{theme:?}: a press elsewhere");
        click_menu(&mut h, "strip.menu.help");
        crate::support::open(h.state_mut());
        h.run();
        assert_eq!(h.state().menu_bar.open, None, "{theme:?}: a dialog");
    }
}

/// A key going down with `modifiers`, staying down: egui calls its next press a repeat.
fn menu_hold(h: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers) {
    h.event(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.run();
}

/// A key pressed and let go.
fn menu_tap(h: &mut Harness<'static, Shell>, key: egui::Key, modifiers: egui::Modifiers) {
    h.event(egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    h.event(egui::Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers });
    h.run();
}

/// A key alone, pressed and let go.
fn menu_press(h: &mut Harness<'static, Shell>, key: egui::Key) {
    menu_tap(h, key, egui::Modifiers::NONE);
}

/// The menu open and the row the keys are on.
fn menu_at(h: &Harness<'static, Shell>) -> (Option<&'static str>, Option<&'static str>) {
    (h.state().menu_bar.open, h.state().menu_bar.row)
}

/// Only F10 with no modifier opens the bar, on its first live row, and only where the bar can open.
/// Without a project host New and Open are off, so Settings is that row.
#[test]
fn only_bare_f10_opens_the_bar() {
    use crate::actions::SETTINGS;
    use egui::{Key::F10, Modifiers as M};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        for modifiers in [M::SHIFT, M::ALT, M::CTRL] {
            menu_tap(&mut h, F10, modifiers);
            assert_eq!(menu_at(&h), (None, None), "{theme:?}: {modifiers:?}");
        }
        menu_press(&mut h, F10);
        assert_eq!(menu_at(&h), (Some("file"), Some(SETTINGS)), "{theme:?}");
        menu_press(&mut h, F10);
        assert_eq!(h.state().menu_bar.open, None, "{theme:?}: F10 again closes");
        menu_hold(&mut h, F10, M::NONE);
        assert_eq!(h.state().menu_bar.open, Some("file"), "{theme:?}: down");
        menu_hold(&mut h, F10, M::NONE);
        assert_eq!(h.state().menu_bar.open, Some("file"), "{theme:?}: a held key does not flicker");
        h.event(egui::Event::Key { key: F10, physical_key: None, pressed: false, repeat: false, modifiers: M::NONE });
        menu_press(&mut h, F10);
        assert_eq!(h.state().menu_bar.open, None, "{theme:?}");
        // One frame each: the next one's clean-up would hide a menu that opened here.
        let down = egui::Event::Key { key: F10, physical_key: None, pressed: true, repeat: false, modifiers: M::NONE };
        crate::support::open(h.state_mut());
        h.run();
        h.event(down.clone());
        h.step();
        assert_eq!(h.state().menu_bar.open, None, "{theme:?}: not under a dialog");
        h.event(egui::Event::Key { key: F10, physical_key: None, pressed: false, repeat: false, modifiers: M::NONE });
        h.state_mut().about = None;
        h.state_mut().in_window_menu = false;
        h.run();
        h.event(down);
        h.step();
        assert_eq!(h.state().menu_bar.open, None, "{theme:?}: not where the system has the menu");
    }
}

/// Alt with a title's letter opens its menu on a row, as F10 opens File, and types nothing in the
/// field that has the keyboard; a letter no title has opens nothing and types nothing either. A
/// letter alone, with AltGr (Ctrl+Alt) or with Shift and Alt is the field's text, and so is Alt on
/// macOS, where the system has the menu (AR-538).
#[test]
fn alt_and_a_titles_letter_opens_its_menu() {
    use egui::{Key, Modifiers as M};
    // Key and text in one frame, as a real press arrives.
    let typed = |h: &mut Harness<'static, Shell>, key, modifiers, text: &str| {
        let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
        h.input_mut().events.extend([event(true), egui::Event::Text(text.to_string()), event(false)]);
        h.run();
    };
    let query = |h: &Harness<'static, Shell>| h.state().home_search.query.clone();
    in_both_appearances(|theme| {
        let mut h = menu_harness(theme, false);
        menu_tap(&mut h, Key::K, M::COMMAND);
        typed(&mut h, Key::Q, M::NONE, "q");
        assert_eq!((menu_at(&h), query(&h)), ((None, None), "q".to_string()), "{theme:?}: a letter alone is text");
        typed(&mut h, Key::V, M::ALT, "v");
        assert_eq!((h.state().menu_bar.open, query(&h)), (Some("view"), "q".to_string()), "{theme:?}: Alt+V opens View");
        assert!(h.state().menu_bar.row.is_some(), "{theme:?}: on a row, as from F10");
        menu_press(&mut h, Key::Escape);
        typed(&mut h, Key::Q, M::ALT, "q");
        assert_eq!((h.state().menu_bar.open, query(&h)), (None, "q".to_string()), "{theme:?}: no title has Q");
        for (held, text) in [(M::CTRL | M::COMMAND | M::ALT, "€"), (M::SHIFT | M::ALT, "E")] {
            typed(&mut h, Key::E, held, text);
            assert_eq!(h.state().menu_bar.open, None, "{theme:?}: {held:?}");
        }
        h.state_mut().in_window_menu = false;
        typed(&mut h, Key::F, M::ALT, "ƒ");
        assert_eq!((h.state().menu_bar.open, query(&h)), (None, "q€Eƒ".to_string()), "{theme:?}: each typed its text");
    });
}

/// With a menu open a row's letter runs the row, alone or with Alt, in a menu the keys or the
/// pointer opened, closing it as Enter does. A letter no row has, a row's that is off, one with
/// Shift or Ctrl, another title's with Alt, and a letter held down since before the menu opened do
/// nothing (AR-538).
#[test]
fn a_rows_letter_runs_it_alone_or_with_alt() {
    use crate::actions::SETTINGS;
    use egui::{Key, Modifiers as M};
    in_both_appearances(|theme| {
        let mut h = menu_harness(theme, false);
        menu_press(&mut h, Key::F10);
        // New and Generate are off without a project host and a project.
        let nothing = [(Key::Z, M::NONE), (Key::N, M::NONE), (Key::G, M::ALT), (Key::E, M::SHIFT), (Key::E, M::CTRL | M::COMMAND), (Key::H, M::ALT)];
        for (key, held) in nothing {
            menu_tap(&mut h, key, held);
            assert_eq!((menu_at(&h), h.state().menu_bar.chosen), ((Some("file"), Some(SETTINGS)), None), "{theme:?}: {key:?} with {held:?}");
        }
        menu_press(&mut h, Key::E);
        assert!(h.state().menu_bar.open.is_none() && h.state().settings.is_some(), "{theme:?}: E runs Settings…");
        let mut h = menu_harness(theme, false);
        click_menu(&mut h, "strip.menu.help");
        menu_tap(&mut h, Key::A, M::ALT);
        assert!(h.state().menu_bar.open.is_none() && h.state().about.is_some(), "{theme:?}: Alt+A runs About under the pointer");
        let mut h = menu_harness(theme, false);
        menu_hold(&mut h, Key::E, M::NONE);
        menu_press(&mut h, Key::F10);
        menu_hold(&mut h, Key::E, M::NONE);
        assert_eq!((menu_at(&h).0, h.state().settings.is_some()), (Some("file"), false), "{theme:?}: E held from before");
    });
}

/// Up and Down go through every row, off or not, and wrap at both ends; the row under the keys is marked.
#[test]
fn up_and_down_walk_every_row_and_wrap() {
    use crate::actions::{CLOSE, GENERATE, NEW, OPEN, QUIT, SAVE, SETTINGS};
    use egui::Key::{ArrowDown, ArrowUp, F10};
    use egui_kittest::kittest::{NodeT, Queryable};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        menu_press(&mut h, F10);
        let walk = [(ArrowDown, QUIT), (ArrowDown, NEW), (ArrowUp, QUIT), (ArrowUp, SETTINGS), (ArrowUp, GENERATE), (ArrowDown, SETTINGS)];
        for (key, row) in walk {
            menu_press(&mut h, key);
            assert_eq!(menu_at(&h), (Some("file"), Some(row)), "{theme:?}: {key:?}");
        }
        let selected = |h: &Harness<'static, Shell>, name: &str| h.get_by_label(name).accesskit_node().data().is_selected() == Some(true);
        assert!(selected(&h, "Settings…") && !selected(&h, "Exit"), "{theme:?}");
        menu_hold(&mut h, ArrowDown, egui::Modifiers::NONE);
        assert_eq!(menu_at(&h).1, Some(QUIT), "{theme:?}");
        menu_hold(&mut h, ArrowDown, egui::Modifiers::NONE);
        assert_eq!(menu_at(&h).1, Some(NEW), "{theme:?}: a held arrow goes on");
        h.event(egui::Event::Key { key: ArrowDown, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE });
        menu_tap(&mut h, ArrowDown, egui::Modifiers::SHIFT);
        assert_eq!(menu_at(&h).1, Some(NEW), "{theme:?}: only the bare arrow moves");
        // On a project the menu opens past its rows off, and Up reaches them.
        let mut h = menu_harness(theme, true);
        menu_press(&mut h, F10);
        assert_eq!(menu_at(&h).1, Some(CLOSE), "{theme:?}");
        let walk = [
            (ArrowDown, SAVE),
            (ArrowDown, GENERATE),
            (ArrowUp, SAVE),
            (ArrowUp, CLOSE),
            (ArrowUp, OPEN),
            (ArrowUp, NEW),
            (ArrowUp, QUIT),
            (ArrowDown, NEW),
        ];
        for (key, row) in walk {
            menu_press(&mut h, key);
            assert_eq!(menu_at(&h).1, Some(row), "{theme:?}: {key:?} on a project");
        }
    }
}

/// Left and Right go between the groups, wrapping, each on its first live row, or on its first
/// row where none can act.
#[test]
fn left_and_right_move_between_the_groups_and_wrap() {
    use crate::actions::{ABOUT, FIND, SAVE_VERSION, SETTINGS, UNDO};
    use egui::Key::{ArrowLeft, ArrowRight, F10};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        menu_press(&mut h, F10);
        let (file, edit, model, help) = (("file", Some(SETTINGS)), ("edit", Some(UNDO)), ("model", Some("model.note")), ("help", Some(ABOUT)));
        let (versions, view, r, l) = (("versions", Some(SAVE_VERSION)), ("view", Some(FIND)), ArrowRight, ArrowLeft);
        let rights = [(r, edit), (r, view), (r, model), (r, versions), (r, help), (r, file)];
        let lefts = [(l, help), (l, versions), (l, model), (l, view), (l, edit), (l, file)];
        for (key, at) in rights.into_iter().chain(lefts) {
            menu_press(&mut h, key);
            assert_eq!(menu_at(&h), (Some(at.0), at.1), "{theme:?}: {key:?}");
        }
    }
}

/// Enter and Space each run the row and close the menu; a row gone off, or no row, runs nothing.
#[test]
fn enter_and_space_run_the_row_while_it_can_act() {
    use egui::Key::{ArrowUp, Enter, F10, Space};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for key in [Enter, Space] {
            let mut h = menu_harness(theme, false);
            menu_press(&mut h, F10);
            assert!(h.state().settings.is_none(), "{theme:?}: {key:?}");
            menu_press(&mut h, key);
            assert!(h.state().menu_bar.open.is_none() && h.state().settings.is_some(), "{theme:?}: {key:?}");
            let mut h = menu_harness(theme, false);
            menu_press(&mut h, F10);
            h.state_mut().menu_bar.row = Some(crate::actions::NEW);
            menu_press(&mut h, key);
            assert!(h.state().menu_bar.open.is_some() && h.state().menu_bar.chosen.is_none(), "{theme:?}: {key:?} on a row gone off");
            let mut h = menu_harness(theme, false);
            click_menu(&mut h, "strip.menu.file");
            assert_eq!(menu_at(&h), (Some("file"), None), "{theme:?}");
            menu_press(&mut h, key);
            assert!(h.state().menu_bar.open.is_some() && h.state().settings.is_none(), "{theme:?}: {key:?} on no row");
        }
        let mut h = menu_harness(theme, false);
        click_menu(&mut h, "strip.menu.file");
        menu_press(&mut h, ArrowUp);
        assert_eq!(menu_at(&h).1, Some(crate::actions::QUIT), "{theme:?}: from no row, Up starts at the end");
    }
}

/// A row chosen from the keys closes the menu at once and runs on the next frame, as a click does.
#[test]
fn a_row_chosen_from_the_keys_closes_at_once_and_runs_next_frame() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        menu_press(&mut h, egui::Key::F10);
        h.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        h.step();
        assert_eq!(h.state().menu_bar.chosen, Some(crate::actions::SETTINGS), "{theme:?}");
        assert!(h.state().menu_bar.open.is_none() && h.state().settings.is_none(), "{theme:?}");
        h.run();
        assert!(h.state().settings.is_some(), "{theme:?}");
    }
}

/// A row left under the keys doesn't survive a pointer moving over the menu, or a menu reopened.
#[test]
fn the_pointer_and_a_reopened_menu_clear_the_keys_row() {
    use crate::actions::SETTINGS;
    use egui::Key::F10;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        menu_press(&mut h, F10);
        assert_eq!(menu_at(&h).1, Some(SETTINGS), "{theme:?}");
        click_menu(&mut h, "strip.menu.help");
        assert_eq!(menu_at(&h), (Some("help"), None), "{theme:?}: a click moves the menu without the row");
        click_menu(&mut h, "strip.menu.help");
        menu_press(&mut h, F10);
        assert_eq!(menu_at(&h).1, Some(SETTINGS), "{theme:?}");
        let over = rect_of(&h, "strip.menu.row.app.quit").center();
        h.event(egui::Event::PointerMoved(over));
        h.run();
        assert_eq!(menu_at(&h), (Some("file"), None), "{theme:?}: the pointer has it now");
    }
}

/// The keys never reach what is behind: not in the frame F10 opens, nor while open, nor in the frame
/// the menu closes; the focus stays put under the arrows and Tab and Esc gives it back.
#[test]
fn the_menus_keys_stay_from_the_window_behind_and_esc_returns_the_focus() {
    use egui::Key::{ArrowDown, Escape, F10, K, Space, Tab};
    // Key and text in one frame, as a real press arrives.
    let with_text = |h: &mut Harness<'static, Shell>, key, text: &str| {
        let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers: egui::Modifiers::NONE };
        h.input_mut().events.extend([event(true), egui::Event::Text(text.to_string()), event(false)]);
        h.run();
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        menu_tap(&mut h, K, egui::Modifiers::COMMAND);
        let focus = |h: &Harness<'static, Shell>| h.ctx.memory(egui::Memory::focused);
        let field = focus(&h);
        assert!(field.is_some(), "{theme:?}: the search has the focus");
        with_text(&mut h, F10, "a");
        assert_eq!((h.state().menu_bar.open, h.state().home_search.query.as_str()), (Some("file"), ""), "{theme:?}: opening");
        for key in [ArrowDown, Tab] {
            menu_press(&mut h, key);
            assert_eq!(focus(&h), field, "{theme:?}: {key:?}");
        }
        with_text(&mut h, Escape, "b");
        assert_eq!((h.state().menu_bar.open, h.state().home_search.query.as_str()), (None, ""), "{theme:?}: closing");
        assert_eq!(focus(&h), field, "{theme:?}: Esc gives it back");
        assert!(h.state().home_search.open, "{theme:?}: and stops at the menu");
        menu_press(&mut h, F10);
        with_text(&mut h, Space, " ");
        assert_eq!(h.state().home_search.query, "", "{theme:?}: Space ran the row and typed nothing");
        assert!(h.state().settings.is_some(), "{theme:?}");
    }
}

/// A menu closed by a click on its own title gives the keyboard back to the field it was opened
/// on, as Esc does; a menu opened on no field asks for no focus.
#[test]
fn a_menu_closed_by_its_title_gives_the_keyboard_back() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = table_harness();
        (h.state_mut().state.theme, h.state_mut().in_window_menu) = (theme, true);
        h.run();
        click_id(&mut h, "workspace.properties.name");
        let field = h.ctx.memory(egui::Memory::focused);
        assert!(field.is_some() && h.ctx.text_edit_focused(), "{theme:?}");
        click_menu(&mut h, "strip.menu.view");
        assert_eq!(h.state().menu_bar.open, Some("view"), "{theme:?}");
        click_menu(&mut h, "strip.menu.view");
        assert_eq!((h.state().menu_bar.open, h.ctx.memory(egui::Memory::focused)), (None, field), "{theme:?}");
        assert!(h.ctx.text_edit_focused(), "{theme:?}: the field it was in is still being typed in");
        let mut h = menu_harness(theme, true);
        click_menu(&mut h, "strip.menu.view");
        click_menu(&mut h, "strip.menu.view");
        assert_eq!((h.state().menu_bar.open, h.ctx.memory(egui::Memory::focused)), (None, None), "{theme:?}");
    }
}

/// The groups and rows are named menu items, the open group marked as open; a row off is disabled,
/// and says why only on hover.
#[test]
fn rows_are_named_and_a_row_off_says_why_when_pointed_at() {
    use egui::accesskit::Role::MenuItem;
    use egui_kittest::kittest::{NodeT, Queryable};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        h.state_mut().tabs = (0..crate::MAX_TABS).map(|_| crate::actions::tests::tab()).collect();
        click_menu(&mut h, "strip.menu.file");
        let off = "Close a project to open another";
        let data = h.get_by_label("New project…").accesskit_node().data().clone();
        assert!(data.is_disabled() && data.description() == Some(off), "{theme:?}");
        let said = |name: &str| h.get_by_label(name).accesskit_node().data().clone();
        let roles = ["File", "Help", "New project…", "Exit"].map(|name| said(name).role());
        assert_eq!(roles, [MenuItem; 4], "{theme:?}: titles and rows, on or off");
        assert_eq!((said("File").is_expanded(), said("Help").is_expanded()), (Some(true), Some(false)), "{theme:?}");
        assert!(!said("Exit").is_disabled(), "{theme:?}");
        assert!(h.get_by_label("Exit").accesskit_node().data().description().is_none(), "{theme:?}");
        assert!(
            h.query_by_label("File").is_some() && h.query_by_label("Help").is_some() && h.query_by_label(off).is_none(),
            "{theme:?}: shown only on hover"
        );
        h.event(egui::Event::PointerMoved(rect_of(&h, "strip.menu.row.file.new").center()));
        h.run();
        assert!(h.query_by_label(off).is_some(), "{theme:?}");
    }
}

/// A screen reader finds the titles in a menu bar, and under the open title a menu holding its rows
/// alone: the parts that swallow a press away from the menu are nothing it can read.
#[test]
fn the_bar_is_a_menu_bar_and_the_open_menu_a_menu_of_its_rows_alone() {
    use egui::accesskit::Role::{Menu, MenuBar, MenuItem};
    use egui_kittest::kittest::{NodeT, Queryable, by};
    let away: Vec<_> = (0..4).map(|n| crate::widgets::egui_id(&format!("strip.menu.away.{n}")).accesskit_id()).collect();
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        click_menu(&mut h, "strip.menu.file");
        let titles = ["File", "Edit", "View", "Model", "Versions", "Help"].map(|name| h.get_by_label(name).accesskit_node());
        let bar = titles[0].parent().unwrap();
        assert_eq!((bar.role(), bar.children().count()), (MenuBar, 6), "{theme:?}");
        assert!(titles.iter().all(|title| title.parent().map(|b| b.id()) == Some(bar.id())), "{theme:?}: one bar holds every title");
        let menu = h.get_by_label("Settings…").accesskit_node().parent().unwrap();
        assert_eq!((menu.role(), menu.parent().map(|title| title.id())), (Menu, Some(titles[0].id())), "{theme:?}: the menu is its title's");
        assert_eq!(menu.children().map(|row| row.role()).collect::<Vec<_>>(), [MenuItem; 7], "{theme:?}: the rows and nothing else");
        assert_eq!(h.query_all(by().predicate(|node| away.contains(&node.locate().0))).count(), 0, "{theme:?}: no part around the bar");
    }
}

/// A screen reader's focus is on the row under the keys, in a menu with every row off too, while
/// the keyboard stays with the field the menu was opened on, or with nothing. A pointer taking the
/// highlight takes that focus away, and a closed menu leaves it where the keyboard is.
#[test]
fn a_screen_readers_focus_follows_the_keys_and_the_keyboard_stays_put() {
    use egui::Key::{ArrowDown, ArrowRight, ArrowUp, Escape, F10, K};
    use egui_kittest::kittest::{NodeT, Queryable, by};
    let on = |h: &Harness<'static, Shell>, names: [&str; 2]| names.map(|name| h.get_by_label(name).accesskit_node().is_focused());
    for (theme, field) in [(ThemeKind::Light, false), (ThemeKind::Dark, false), (ThemeKind::Light, true), (ThemeKind::Dark, true)] {
        let mut h = menu_harness(theme, false);
        if field {
            menu_tap(&mut h, K, egui::Modifiers::COMMAND);
        }
        let held = h.ctx.memory(egui::Memory::focused);
        assert_eq!((held.is_some(), h.root().accesskit_node().is_focused()), (field, !field), "{theme:?}: before the menu");
        menu_press(&mut h, F10);
        assert_eq!(on(&h, ["Settings…", "Generate…"]), [true, false], "{theme:?} {field}: the row the menu opens on");
        menu_press(&mut h, ArrowUp);
        assert_eq!(on(&h, ["Settings…", "Generate…"]), [false, true], "{theme:?} {field}: a row off");
        menu_press(&mut h, ArrowRight);
        assert_eq!(on(&h, ["Undo", "Redo"]), [true, false], "{theme:?} {field}: every row off");
        assert_eq!(h.ctx.memory(egui::Memory::focused), held, "{theme:?} {field}: the keyboard never moved");
        // Two places: a pointer that only arrives has not moved yet.
        for row in ["strip.menu.row.edit.undo", "strip.menu.row.edit.redo"] {
            h.event(egui::Event::PointerMoved(rect_of(&h, row).center()));
            h.run();
        }
        assert_eq!(on(&h, ["Undo", "Redo"]), [false, false], "{theme:?} {field}: the pointer's row is not the keys'");
        menu_press(&mut h, ArrowDown);
        assert_eq!(on(&h, ["Undo", "Redo"]), [true, false], "{theme:?} {field}: the keys again");
        menu_press(&mut h, Escape);
        assert_eq!((h.ctx.memory(egui::Memory::focused), h.root().accesskit_node().is_focused()), (held, !field), "{theme:?} {field}: closed");
        let named = h.query_all(by().predicate(|node| node.is_focused() && node.data().active_descendant().is_some())).count();
        assert_eq!(named, 0, "{theme:?} {field}: no row is named once the menu is gone");
        // What has the keyboard without being drawn is nothing a reader knows: the window stands in,
        // and no node is made up for it. One frame: the keyboard leaves what is not drawn as it ends.
        let unseen = egui::Id::unique("unseen");
        h.ctx.memory_mut(|m| m.request_focus(unseen));
        h.event(egui::Event::Key { key: F10, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        h.step();
        let made_up = h.query_all(by().predicate(|node| node.locate().0 == unseen.accesskit_id())).count();
        assert_eq!((on(&h, ["Settings…", "Generate…"]), made_up), ([true, false], 0), "{theme:?} {field}: nothing drawn has the keyboard");
    }
}

/// The arrows reach a row that is off: it is highlighted, shows why as it does under the pointer and
/// says so to a screen reader, and Enter or Space on it does nothing. The reason goes with the keys.
#[test]
fn a_row_off_is_reached_by_the_keys_says_why_and_runs_nothing() {
    use egui::Key::{ArrowDown, ArrowUp, Enter, F10, Space};
    use egui_kittest::kittest::{NodeT, Queryable};
    let why = "Open or go back to a project first";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = menu_harness(theme, false);
        menu_press(&mut h, F10);
        assert!(h.query_by_label(why).is_none(), "{theme:?}: no reason on a row that can act");
        menu_press(&mut h, ArrowUp);
        assert_eq!(menu_at(&h), (Some("file"), Some(crate::actions::GENERATE)), "{theme:?}");
        let row = h.get_by_label("Generate…").accesskit_node().data().clone();
        assert_eq!((row.is_disabled(), row.is_selected(), row.description()), (true, Some(true), Some(why)), "{theme:?}");
        assert!(h.query_by_label(why).is_some(), "{theme:?}: shown without the pointer");
        for key in [Enter, Space] {
            menu_press(&mut h, key);
            assert_eq!((menu_at(&h), h.state().menu_bar.chosen), ((Some("file"), Some(crate::actions::GENERATE)), None), "{theme:?}: {key:?}");
        }
        menu_press(&mut h, ArrowDown);
        assert!(h.query_by_label(why).is_none(), "{theme:?}: the reason left with the keys");
        let row = h.get_by_label("Generate…").accesskit_node().data().clone();
        assert_eq!((row.is_selected(), row.description()), (None, Some(why)), "{theme:?}: still said to a screen reader");
        // A pointer arriving on the row leaves it under the keys, and brings no second reason.
        menu_press(&mut h, ArrowUp);
        h.event(egui::Event::PointerMoved(rect_of(&h, "strip.menu.row.file.generate").center()));
        h.run();
        assert_eq!((menu_at(&h).1, h.query_all_by_label(why).count()), (Some(crate::actions::GENERATE), 1), "{theme:?}");
    }
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

/// The Models explorer of a model with nothing in it says so in the shell's own words, which name
/// no notation's elements, translated; a filter that matches nothing says that instead.
#[test]
fn an_empty_models_explorer_says_the_model_holds_nothing_yet_in_the_language_shown() {
    in_both_appearances(|theme| {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        crate::i18n::set_language("de");
        h.state_mut().state.language = "de".into();
        h.run();
        assert_eq!(h.state().state.explorer, crate::state::Explorer::Models);
        let shown = painted(&h);
        assert!(shown.iter().any(|t| t == "Noch nichts im Modell"), "{theme:?}: {shown:?}");
        assert!(!shown.iter().any(|t| t == "Noch keine Schemas" || t == "Nothing in the model yet"), "{theme:?}: {shown:?}");
        h.state_mut().explorer.filter = "kunde".into();
        h.run();
        let shown = painted(&h);
        assert!(shown.iter().any(|t| t == "Nichts passt zum Filter") && !shown.iter().any(|t| t == "Noch nichts im Modell"), "{theme:?}: {shown:?}");
        crate::i18n::set_language("en");
    });
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

/// The saved interface state names the left side's explorer and the right side's panel by each
/// one's own name, as it always has, and reads every one back as it was saved.
#[test]
fn the_saved_state_names_each_explorer_and_panel_by_its_own_name() {
    use crate::state::{Explorer, Panel, UiState};
    for (explorer, name) in [(Explorer::Models, "Models"), (Explorer::Diagrams, "Diagrams"), (Explorer::SourceControl, "SourceControl")] {
        let state = UiState { explorer, ..UiState::default() };
        let saved = serde_json::to_string(&state).unwrap();
        assert!(saved.contains(&format!(r#""explorer":"{name}""#)), "{saved}");
        assert_eq!(serde_json::from_str::<UiState>(&saved).unwrap(), state);
    }
    for (panel, name) in [(Panel::Agent, "Agent"), (Panel::Appearance, "Appearance"), (Panel::Properties, "Properties"), (Panel::Skills, "Skills")] {
        let state = UiState { panel, ..UiState::default() };
        let saved = serde_json::to_string(&state).unwrap();
        assert!(saved.contains(&format!(r#""panel":"{name}""#)), "{saved}");
        assert_eq!(serde_json::from_str::<UiState>(&saved).unwrap(), state);
    }
}

/// Without an id source the model does not open: the shell keeps no session and holds the reason
/// the status bar says (`SessionError::NoIdSource`).
#[test]
fn without_an_id_source_the_status_bar_says_the_model_could_not_open_once() {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
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

/// A long project name keeps the project chip left of the bar's controls, at every window size and
/// in every language.
#[test]
fn a_long_project_name_keeps_the_chip_left_of_the_bar_controls_at_every_size_and_language() {
    for lang in crate::i18n::LANGUAGES {
        for (w, h_) in [(1600.0, 1120.0), (900.0, 700.0), (480.0, 600.0), (80.0, 80.0)] {
            crate::i18n::set_language(lang.code);
            let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() }, NOTATIONS, &SCREENS);
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

/// What a key meant alone must not act with held: Shift, Alt, Control, and the command key in the
/// two shapes keyboards send it, a Mac's ⌘ and the Ctrl of Windows and Linux.
const NOT_ALONE: [egui::Modifiers; 5] = [
    egui::Modifiers::SHIFT,
    egui::Modifiers::ALT,
    egui::Modifiers::CTRL,
    egui::Modifiers::MAC_CMD.plus(egui::Modifiers::COMMAND),
    egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND),
];

/// The names of the open project's packages of the fake notation.
fn schema_names(h: &Harness<'static, Shell>) -> Vec<String> {
    let s = h.state().session.as_ref().unwrap();
    let st = s.doc();
    st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).map(|o| archforge_core::name_of(st, o)).collect()
}

/// The Models tree's + adds a package, named in place, and Enter renames it.
#[test]
fn the_plus_adds_a_schema_named_in_place_and_enter_renames_it() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    let at = rect_of(&h, "workspace.explorer.add").center();
    click(&mut h, at);
    assert!(h.state().explorer.menu_open);
    let at = rect_of(&h, "workspace.explorer.add.package").center();
    click(&mut h, at);
    h.run();
    assert_eq!(schema_names(&h), ["Package 1"]);
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
    assert_eq!(schema_names(&h), ["Package 1"]);
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT);
    assert_eq!(schema_names(&h), ["Shop"]);
}

/// A name refused in place by Enter keeps the keyboard with the name as typed: Escape then closes
/// it, the old name staying and the tree taking its keys again, and a valid name typed after it is
/// applied by Enter, with no click in between. A click elsewhere still takes the keyboard away from
/// it. The refusal says it in Properties' words, under the field and read out, never the engine's
/// (AR-427). In light and dark (AR-426).
#[test]
fn a_name_refused_in_place_keeps_the_keyboard_for_escape_or_a_correction() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for then in ["escape", "correct", "click"] {
            let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
            crate::explorer::add(h.state_mut(), "package");
            h.run();
            // The default name is selected whole: Backspace empties it, and Enter is refused.
            key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE);
            key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
            let rename = h.state().explorer.rename.clone().unwrap_or_else(|| panic!("{theme:?}: the refused name stays open"));
            let said = crate::i18n::tr("A name can't be empty.");
            assert!(rename.text.is_empty() && rename.refused.as_deref() == Some(said.as_str()), "{theme:?}: {rename:?}");
            read_out(&h, &said, "workspace.explorer.rename.notice");
            assert!(h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id("workspace.explorer.rename"))), "{theme:?}: the field kept the keyboard");
            if then == "correct" {
                h.event(egui::Event::Text("Shop".into()));
                h.run();
                key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
                assert!(h.state().explorer.rename.is_none(), "{theme:?}: {:?}", h.state().explorer.rename);
                assert_eq!(schema_names(&h), ["Shop"], "{theme:?}: the corrected name is applied");
            } else if then == "click" {
                let filter = rect_of(&h, "workspace.explorer.filter").center();
                click(&mut h, filter);
                h.event(egui::Event::Text("x".into()));
                h.run();
                assert_eq!(h.state().explorer.filter, "x", "{theme:?}: the filter has the keyboard, the refused name does not take it back");
            } else {
                key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
                assert!(h.state().explorer.rename.is_none(), "{theme:?}: Escape closes it: {:?}", h.state().explorer.rename);
                assert_eq!(schema_names(&h), ["Package 1"], "{theme:?}: the old name stays");
                // The tree has its keys again: F2 opens the selected schema's name.
                key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
                assert_eq!(h.state().explorer.rename.as_ref().map(|r| r.text.as_str()), Some("Package 1"), "{theme:?}: F2 reaches the tree");
            }
        }
    }
}

/// A name refused in place in Models is said in the chosen language, as Properties says it: the
/// explorer's own wrapper goes through the shared refusal wording. In light and dark (AR-427).
#[test]
fn a_name_refused_in_place_is_said_in_the_chosen_language() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        crate::explorer::add(h.state_mut(), "package");
        h.run();
        key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let said = crate::i18n::tr("A name can't be empty.");
        assert_ne!(said, "A name can't be empty.", "{theme:?}: the catalogue has it in German");
        let refused = h.state().explorer.rename.as_ref().and_then(|r| r.refused.clone());
        assert_eq!(refused.as_deref(), Some(said.as_str()), "{theme:?}");
        read_out(&h, &said, "workspace.explorer.rename.notice");
    }
    crate::i18n::set_language("en");
}

/// A kind word a diagram command refuses is said in the chosen language: a diagram's kind that is
/// no plain word or none the notations know, with the kinds there are, a box's kind, a label's text
/// an association's line lacks, with those it has, and one a generalization's line, which has none,
/// and a style's name; a project without notations said from the reason alone. In light and dark
/// (AR-512).
#[test]
fn a_kind_word_refused_is_said_in_the_chosen_language() {
    use archforge_mof::Value;
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let refused =
            |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| crate::panels::run(h.state_mut(), cmd, p).err().unwrap_or_default();
        let plain = refused(&mut h, "diagram.add", json!({ "kind": "Klasse" }));
        assert_eq!(plain, "Verwenden Sie ein einfaches Wort: Kleinbuchstaben, Ziffern, _, - und ., beginnend mit einem Buchstaben.", "{theme:?}");
        let session = h.state().session.as_ref().unwrap();
        let kinds = archforge_diagram::DiagramView::kinds(session.view()).join(", ");
        let unknown = format!("toy ist keine Diagrammart. Die Arten sind {kinds}.");
        assert_eq!(refused(&mut h, "diagram.add", json!({ "kind": "toy" })), unknown, "{theme:?}");
        let d = h.state().canvas.active.unwrap().to_string();
        let st = h.state().session.as_ref().unwrap().doc();
        let commerce = st.container(st.by_uuid(named(&h, archforge_mof::toy::CLASS, "Order")).unwrap()).unwrap().unwrap().0;
        let p = json!({ "package": st.uuid(commerce).unwrap().to_string(), "name": "Shop" });
        let shop = crate::panels::run(h.state_mut(), "sketch.class.add", p).unwrap()["id"].as_str().unwrap().to_string();
        let role = json!({ "diagram": d, "element": shop, "x": 0, "y": 0, "kind": "Box" });
        assert_eq!(refused(&mut h, "diagram.place", role), plain, "{theme:?}");
        // A line of each kind: an association's, with label texts, and a generalization's, without.
        let st = h.state().session.as_ref().unwrap().doc();
        let lines = st.list(st.by_uuid(h.state().canvas.active.unwrap()).unwrap(), archforge_diagram::diagram::EDGE).unwrap();
        let line_of = |class| {
            let shows = |e: &archforge_mof::Obj| {
                st.get(*e, archforge_diagram::edge::ELEMENT).ok().and_then(|v| v.as_ref()).and_then(|el| st.class_of(el).ok())
            };
            st.uuid(lines.iter().filter_map(Value::as_ref).find(|e| shows(e) == Some(class)).unwrap()).unwrap().to_string()
        };
        let (association, generalization) = (line_of(archforge_mof::toy::ASSOCIATION), line_of(archforge_mof::toy::GENERALIZATION));
        let slots = archforge_diagram::Notation::text_slots(&archforge_notations::fake::Sketch, archforge_mof::toy::ASSOCIATION).join(", ");
        let lacks = format!("Diese Linie hat keinen Beschriftungstext kind. Sie hat {slots}.");
        assert_eq!(refused(&mut h, "diagram.label", json!({ "edge": association, "kind": "kind" })), lacks, "{theme:?}");
        let none = "Diese Linie hat keinen Beschriftungstext name. Sie hat keinen.";
        assert_eq!(refused(&mut h, "diagram.label", json!({ "edge": generalization, "kind": "name" })), none, "{theme:?}");
        assert_eq!(refused(&mut h, "diagram.style", json!({ "ids": [nodes["Order"]], "name": "Akzent" })), plain, "{theme:?}");
        let bare = archforge_engine::NotOneOf { what: archforge_engine::WordPlace::DiagramKind, word: "old.class".into(), known: Vec::new() };
        let no_notation = crate::panels::said(SCREENS.notation_uis(), &archforge_engine::Reason::NotOneOf(Box::new(bare)));
        let no_kind = "Dieses Projekt enthält keine Notation und kennt daher keine Diagrammart.";
        assert_eq!(no_notation.as_deref(), Some(no_kind), "{theme:?}");
    }
    crate::i18n::set_language("en");
}

/// A new diagram's element a diagram command refuses is said in the chosen language: none given for
/// a class diagram, which is always of an element, an id of no element, and a class, which no class
/// diagram is of. In light and dark (AR-513).
#[test]
fn a_new_diagrams_element_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let order = named(&h, archforge_mof::toy::CLASS, "Order").to_string();
        let mut refused = |element: &str| {
            let p = json!({ "kind": SKETCH, "element": element, "name": "Neu" });
            crate::panels::run(h.state_mut(), "diagram.add", p).err().unwrap_or_default()
        };
        assert_eq!(refused(""), "Es ist kein Element angegeben. Geben Sie seine ID an.", "{theme:?}");
        assert_eq!(refused("nope"), "Es gibt kein Element mit der ID nope.", "{theme:?}");
        assert_eq!(refused(&order), "Order kann kein Diagramm dieser Art haben.", "{theme:?}");
    }
    crate::i18n::set_language("en");
}

/// A list of boxes moved or taken off, or of what a style is set on, that a diagram command refuses
/// is said in the chosen language: an empty list, no list of ids, an empty id, an element of the
/// wrong kind and a box named twice. In light and dark (AR-514).
#[test]
fn a_node_list_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let order = named(&h, archforge_mof::toy::CLASS, "Order").to_string();
        let card = nodes["Order"].clone();
        let mut refused = |cmd: &str, list: serde_json::Value| {
            let p = match cmd {
                "diagram.move" => json!({ "nodes": list, "dx": 1, "dy": 1 }),
                "diagram.unplace" => json!({ "nodes": list }),
                _ => json!({ "ids": list }),
            };
            crate::panels::run(h.state_mut(), cmd, p).err().unwrap_or_default()
        };
        assert_eq!(refused("diagram.move", json!([])), "Geben Sie mindestens einen Kasten an.", "{theme:?}");
        assert_eq!(refused("diagram.unplace", json!([7])), "Geben Sie die Kästen als Liste von IDs an.", "{theme:?}");
        assert_eq!(refused("diagram.move", json!([&order])), "Order ist kein Kasten in einem Diagramm.", "{theme:?}");
        let twice = "Ein Element ist zweimal genannt. Jedes Element nur einmal nennen.";
        assert_eq!(refused("diagram.unplace", json!([&card, &card])), twice, "{theme:?}");
        assert_eq!(refused("diagram.style", json!([])), "Geben Sie mindestens ein Diagramm, einen Kasten oder eine Linie an.", "{theme:?}");
        assert_eq!(refused("diagram.style", json!([7])), "Geben Sie die Diagramme, Kästen und Linien als Liste von IDs an.", "{theme:?}");
        let empty = "Es ist kein Diagramm, kein Kasten und keine Linie angegeben. Geben Sie die ID an.";
        assert_eq!(refused("diagram.style", json!([""])), empty, "{theme:?}");
        assert_eq!(refused("diagram.style", json!([&order])), "Order ist weder ein Diagramm noch ein Kasten noch eine Linie.", "{theme:?}");
        assert_eq!(refused("diagram.style", json!([&card, &card])), twice, "{theme:?}");
    }
    crate::i18n::set_language("en");
}

/// A style's colour not of the form #rrggbb, of each part alone, and a diagram arranged that shows
/// no box or would put one beyond a diagram's reach, are said in the chosen language. In light and
/// dark (AR-516).
#[test]
fn a_style_colour_or_an_arrangement_refused_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, nodes) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let run = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| crate::panels::run(h.state_mut(), cmd, p);
        let refused = |h: &mut Harness<'static, Shell>, cmd: &str, p: serde_json::Value| run(h, cmd, p).err().unwrap_or_default();
        let colour = |part: &str| json!({ "ids": [&nodes["Order"]], part: "red" });
        assert_eq!(refused(&mut h, "diagram.style", colour("fill")), "Geben Sie die Füllfarbe als #rrggbb an, etwa #3366cc.", "{theme:?}");
        let stroke = "Geben Sie die Rahmen- oder Linienfarbe als #rrggbb an, etwa #3366cc.";
        assert_eq!(refused(&mut h, "diagram.style", colour("stroke")), stroke, "{theme:?}");
        assert_eq!(refused(&mut h, "diagram.style", colour("text")), "Geben Sie die Textfarbe als #rrggbb an, etwa #3366cc.", "{theme:?}");
        // A new diagram of the classes' package: empty, then two classes at the edge of the reach.
        let (order, customer) = (named(&h, archforge_mof::toy::CLASS, "Order"), named(&h, archforge_mof::toy::CLASS, "Customer"));
        let st = h.state().session.as_ref().unwrap().doc();
        let package = st.uuid(st.container(st.by_uuid(order).unwrap()).unwrap().unwrap().0).unwrap().to_string();
        let added = run(&mut h, "diagram.add", json!({ "kind": SKETCH, "element": package })).unwrap();
        let d = added["id"].as_str().unwrap().to_string();
        let none = "Dieses Diagramm zeigt keinen Kasten, der sich anordnen ließe.";
        assert_eq!(refused(&mut h, "diagram.arrange", json!({ "diagram": d })), none, "{theme:?}");
        for class in [order, customer] {
            let at_the_edge = json!({ "diagram": d, "element": class.to_string(), "x": 1_000_000, "y": 1_000_000 });
            run(&mut h, "diagram.place", at_the_edge).unwrap();
        }
        let far = "Angeordnet läge ein Kasten weiter als 1000000 vom Ursprung entfernt. Rücken Sie die Kästen zuerst näher an ihn heran.";
        assert_eq!(refused(&mut h, "diagram.arrange", json!({ "diagram": d })), far, "{theme:?}");
    }
    crate::i18n::set_language("en");
}

/// An association's end, which the notation does not remove yet, removed with the selection is said
/// in the chosen language, named by its role or, without one, its id. In light and dark (AR-530).
#[test]
fn an_element_not_removed_yet_is_said_in_the_chosen_language() {
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let order = named(&h, archforge_mof::toy::CLASS, "Order").to_string();
        let st = h.state().session.as_ref().unwrap().doc();
        let commerce = st.container(st.by_uuid(named(&h, archforge_mof::toy::CLASS, "Order")).unwrap()).unwrap().unwrap().0;
        let p = json!({ "package": st.uuid(commerce).unwrap().to_string(), "name": "Shop" });
        let mut run = |cmd: &str, p: serde_json::Value| crate::panels::run(h.state_mut(), cmd, p);
        let shop = run("sketch.class.add", p).unwrap()["id"].as_str().unwrap().to_string();
        let sells = run("sketch.association.add", json!({ "from": shop, "to": order, "from_role": "seller" })).unwrap();
        let (seller, unnamed) = (sells["ends"][0].as_str().unwrap().to_string(), sells["ends"][1].as_str().unwrap().to_string());
        let refused = run("element.remove", json!({ "ids": [seller] })).unwrap_err();
        assert_eq!(refused, "seller lässt sich noch nicht entfernen.", "{theme:?}");
        let refused = run("element.remove", json!({ "ids": [unnamed] })).unwrap_err();
        assert_eq!(refused, format!("{unnamed} lässt sich noch nicht entfernen."), "{theme:?}");
        h.run();
    }
    crate::i18n::set_language("en");
}

/// The engine's name and text rules are said in the chosen language, never in the engine's
/// English, as a panel, a canvas or Models says them: a name too long, a control character, a space
/// at an end and a note's empty text; an empty name has its own test. In light and dark (AR-434).
#[test]
fn the_engines_name_rules_are_said_in_the_chosen_language() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        h.state_mut().state.language = "de".into();
        h.run();
        let id = |v: Result<serde_json::Value, String>| v.unwrap()["id"].as_str().unwrap().to_string();
        let schema = id(crate::panels::run(h.state_mut(), "sketch.package.add", serde_json::json!({ "name": "Shop" })));
        let diagram = id(crate::panels::run(h.state_mut(), "diagram.add", serde_json::json!({ "kind": SKETCH, "element": schema, "name": "Shop" })));
        h.run();
        let max = archforge_notations::fake::MAX_NAME;
        let too_long = crate::i18n::fmt(&crate::i18n::tr("It can't be longer than {max} characters."), &[("max", &max.to_string())]);
        for (cmd, params, said) in [
            ("sketch.class.add", serde_json::json!({ "package": schema, "name": "n".repeat(max + 1) }), too_long),
            (
                "sketch.class.add",
                serde_json::json!({ "package": schema, "name": "Auf\u{7}trag" }),
                crate::i18n::tr("It can't hold a control or hidden character."),
            ),
            (
                "sketch.class.add",
                serde_json::json!({ "package": schema, "name": "Auftrag " }),
                crate::i18n::tr("It can't start or end with a space."),
            ),
            ("diagram.note", serde_json::json!({ "diagram": diagram, "text": "", "x": 0, "y": 0 }), crate::i18n::tr("The text can't be empty.")),
        ] {
            assert!(
                !said.contains("character") && !said.contains("space") && !said.contains("text"),
                "{theme:?}: the catalogue has it in German: {said}"
            );
            assert_eq!(crate::panels::run(h.state_mut(), cmd, params).err(), Some(said), "{theme:?}: {cmd}");
        }
        h.run();
    }
    crate::i18n::set_language("en");
}

/// The Models filter narrows the tree to the names that hold what is typed, and the arrow keys walk
/// the rows it leaves.
#[test]
fn the_filter_narrows_the_tree_and_the_keys_walk_it() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    for _ in 0..2 {
        crate::explorer::add(h.state_mut(), "package");
        h.run();
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
    }
    crate::explorer::add(h.state_mut(), "package");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(schema_names(&h).len(), 3);
    h.state_mut().explorer.filter = "package 2".into();
    h.run();
    let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["Package 2"]);
    h.state_mut().explorer.filter.clear();
    h.run();
    // Down from the selected third schema stays on it; up goes to the second.
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    let selected = h.state().session.as_ref().unwrap().view().selection[0];
    assert_eq!(archforge_core::name_of(h.state().session.as_ref().unwrap().doc(), selected), "Package 2");
    key(&mut h, egui::Key::F2, egui::Modifiers::NONE);
    assert!(h.state().explorer.rename.as_ref().is_some_and(|r| r.text == "Package 2"));
}

/// The Models and Diagrams filters ignore capitals, accents and `ß`, and drop the names that
/// don't hold what is typed.
#[test]
fn the_explorers_filters_ignore_capitals_and_accents() {
    in_both_appearances(|theme| {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        for name in ["Bestellungen prüfen", "Straße"] {
            let s = h.state_mut().session.as_mut().unwrap();
            let schema = s.execute("sketch.package.add", &serde_json::json!({ "name": name })).unwrap().value["id"].as_str().unwrap().to_string();
            s.execute("diagram.add", &serde_json::json!({ "kind": SKETCH, "element": schema, "name": name })).unwrap();
        }
        let models = |h: &Harness<'static, Shell>| {
            let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
            rows.into_iter().map(|r| r.label).collect::<Vec<_>>()
        };
        let diagrams = |h: &Harness<'static, Shell>| {
            let rows = crate::diagrams::rows(h.state().session.as_ref().unwrap().doc(), &h.state().diagrams, h.state().kinds).rows;
            rows.into_iter().filter(|r| r.kind == crate::diagrams::Kind::Diagram).map(|r| r.label).collect::<Vec<_>>()
        };
        let invisible = "\u{200B} PRUFEN \u{200B}";
        for (typed, found) in
            [("bestellung", "Bestellungen prüfen"), ("PRUFEN", "Bestellungen prüfen"), ("STRASSE", "Straße"), (invisible, "Bestellungen prüfen")]
        {
            h.state_mut().explorer.filter = typed.into();
            h.state_mut().diagrams.filter = typed.into();
            h.run();
            assert_eq!((models(&h), diagrams(&h)), (vec![found.to_string()], vec![found.to_string()]), "{theme:?}: {typed}");
        }
        // A filter of invisible characters only filters nothing, so Expand all stays on.
        h.state_mut().explorer.filter = "\u{200B}".into();
        h.run();
        let at = rect_of(&h, "workspace.explorer.fold-all").center();
        click(&mut h, at);
        assert!(!h.state().explorer.expanded.is_empty(), "{theme:?}");
    });
}

/// A harness with a schema, a table of two columns, the table selected.
#[test]
fn the_models_tree_scrolls_and_follows_the_selection() {
    let (mut h, _) = table_harness();
    let (first, last) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).unwrap()).unwrap().to_string();
        let mut ids = Vec::new();
        for n in 0..60 {
            let out = s.execute("sketch.class.add", &serde_json::json!({ "package": schema, "name": format!("table_{n:02}") })).unwrap();
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

// The explorer trees and the keyboard (AR-478). A row of Diagrams or Models, and a group's `+`, a
// row's arrow and its drag handle, take the keyboard: Tab and Shift+Tab land on each in turn and the
// tree brings it whole into view, and Page Up, Page Down, Home and End move the keyboard through the
// rows. While Models holds the keys itself, the same keys move its selection, and the tree follows.

/// Whether the control `id` has the keyboard, listed or not.
fn holds(h: &Harness<'static, Shell>, id: &str) -> bool {
    h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(id)))
}

/// Where a tree shows the rows it brings into view: from `top`, its first row's top unscrolled, to
/// the column's foot less 17 points (8 kept clear, and 9 of a 34-point pitch under a row's foot).
fn within(h: &Harness<'static, Shell>, top: f32) -> (f32, f32) {
    (top, rect_of(h, "workspace.explorer.resize").bottom() - 17.0)
}

/// Whether the control `id` is listed and drawn whole between `bounds`.
fn whole(h: &Harness<'static, Shell>, id: &str, (top, bottom): (f32, f32)) -> bool {
    has_id(h, id) && rect_of(h, id).top() >= top && rect_of(h, id).bottom() <= bottom
}

/// Puts the keyboard on the control `id`, as a screen reader may.
fn focus(h: &mut Harness<'static, Shell>, id: &str) {
    h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(id)));
    h.run();
}

/// The last of `rows`, from the first, drawn whole between `bounds`.
fn last_whole(h: &Harness<'static, Shell>, rows: &[String], bounds: (f32, f32)) -> usize {
    (0..rows.len()).take_while(|n| whole(h, &rows[*n], bounds)).last().unwrap()
}

/// From the first of `order` Tab, and then Shift+Tab from its last, lands on each of the rest in
/// turn, none skipped; each is drawn whole between `bounds`, or the row `shown` names for it is.
fn walk(h: &mut Harness<'static, Shell>, order: &[String], bounds: (f32, f32), shown: impl Fn(&str) -> Option<String>) {
    assert!(!has_id(h, order.last().unwrap()), "the tree goes on below the fold");
    for back in [false, true] {
        let order: Vec<&String> = if back { order.iter().rev().collect() } else { order.iter().collect() };
        focus(h, order[0]);
        let mut landed = vec![order[0].clone()];
        while landed.len() < order.len() {
            key(h, egui::Key::Tab, if back { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE });
            let on = order.iter().find(|id| holds(h, id));
            landed.push(on.map_or_else(|| format!("{:?}", h.ctx.memory(|m| m.focused())), |id| (*id).clone()));
            if let Some(row) = on.and_then(|id| shown(id)) {
                assert!(whole(h, &row, bounds), "{row} in view, after {:?}", &landed[..landed.len() - 1]);
            }
        }
        assert_eq!(landed.iter().collect::<Vec<_>>(), order, "every control in turn, back {back}");
    }
}

/// Moves the tree that shows, as the wheel does, so that `row` is cut by its top (`up`) or its
/// foot: the tree follows nothing the wheel does.
fn cut_by_edge(h: &mut Harness<'static, Shell>, row: &str, up: bool, bounds: (f32, f32)) {
    let top = rect_of(h, row).top();
    let by = if up { top - (bounds.0 - 20.0) } else { top - (bounds.1 + 7.0) };
    let s = h.state_mut();
    let tree = if s.state.explorer == crate::state::Explorer::Diagrams { &mut s.diagrams.scroll } else { &mut s.explorer.scroll };
    tree.offset += by;
    h.run();
    assert!(has_id(h, row) && !whole(h, row, bounds), "{row} cut by the edge");
}

/// As the wheel leaves the row `rows[k]`, whose control `from` has the keyboard, cut by the tree's
/// top (`up`) or its foot, Shift+Tab or Tab goes on to `to` past that edge, its row brought whole
/// into view: the row past an edge is laid out.
fn past_the_edge(h: &mut Harness<'static, Shell>, rows: &[String], (k, from): (usize, &str), (to, up): (&str, bool), bounds: (f32, f32)) {
    focus(h, from);
    cut_by_edge(h, &rows[k], up, bounds);
    assert!(holds(h, from), "{from} keeps the keyboard");
    key(h, egui::Key::Tab, if up { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE });
    let next = if up { k - 1 } else { k + 1 };
    assert!(holds(h, to) && whole(h, &rows[next], bounds), "on to {to}, in view");
}

/// From the first of `rows`, Page Down twice moves what `on` says moved (the keyboard, or the
/// selection) by what shows, End to the last row, Page Up twice back by what shows and Home to the
/// first, each brought whole into view between `bounds`; a key that goes nowhere at either end
/// brings back the row the wheel left cut.
fn pages(h: &mut Harness<'static, Shell>, rows: &[String], bounds: (f32, f32), on: impl Fn(&Harness<'static, Shell>, &str) -> bool) {
    let shown = last_whole(h, rows, bounds);
    let press = |h: &mut Harness<'static, Shell>, key_: egui::Key, to: usize| {
        key(h, key_, egui::Modifiers::NONE);
        assert!(on(h, &rows[to]) && whole(h, &rows[to], bounds), "{key_:?} to row {to}, in {bounds:?}");
    };
    press(h, egui::Key::PageDown, shown);
    press(h, egui::Key::PageDown, 2 * shown);
    press(h, egui::Key::End, rows.len() - 1);
    // The wheel leaves the last row cut by the foot; End and Page Down bring it back, though it stays.
    for key_ in [egui::Key::End, egui::Key::PageDown] {
        cut_by_edge(h, &rows[rows.len() - 1], false, bounds);
        press(h, key_, rows.len() - 1);
    }
    let first = (0..rows.len()).find(|n| whole(h, &rows[*n], bounds)).unwrap();
    assert!(first > 30, "End scrolled the tree to its end");
    press(h, egui::Key::PageUp, first);
    press(h, egui::Key::PageUp, first - shown);
    press(h, egui::Key::Home, 0);
    // The wheel leaves the first row cut by the top; Home and Page Up bring it back, though it stays.
    for key_ in [egui::Key::Home, egui::Key::PageUp] {
        cut_by_edge(h, &rows[0], true, bounds);
        press(h, key_, 0);
    }
}

/// Sixty diagrams, each of a schema of its own, in Diagrams at the design's window size; gives the
/// controls of the tree that take the keyboard, in its order: each row, and after a group's its `+`.
fn many_diagrams(theme: ThemeKind) -> (Harness<'static, Shell>, Vec<String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    for n in 0..60 {
        let schema =
            crate::panels::run(h.state_mut(), "sketch.package.add", serde_json::json!({ "name": format!("s{n:02}") })).unwrap()["id"].clone();
        let made = serde_json::json!({ "kind": SKETCH, "element": schema, "name": format!("d{n:02}") });
        crate::panels::run(h.state_mut(), "diagram.add", made).unwrap();
    }
    h.state_mut().state.show_explorer(crate::state::Explorer::Diagrams);
    h.run();
    let shell = h.state();
    let tree = crate::diagrams::rows(shell.session.as_ref().unwrap().doc(), &shell.diagrams, shell.kinds);
    let add = |row: &crate::diagrams::Row| match row.kind {
        crate::diagrams::Kind::Group(kind) if shell.kinds.iter().any(|k| k.kind() == kind) => Some(crate::diagrams::add_id(kind)),
        _ => None,
    };
    let controls = tree.rows.iter().flat_map(|row| std::iter::once(crate::diagrams::row_id(row)).chain(add(row))).collect();
    (h, controls)
}

/// Tab and Shift+Tab land on every row of Diagrams and group's `+` in turn, each brought whole into
/// view, past a row the wheel left cut too; the frame Tab moves the keyboard in asks for the next.
/// The page keys move the keyboard: not with the control key (Windows, Linux) or the menu bar open,
/// also in the frame a click opens it in, and by a row in the smallest window.
#[test]
fn diagrams_follows_the_keyboard() {
    in_both_appearances(|theme| {
        let (mut h, controls) = many_diagrams(theme);
        let bounds = within(&h, rect_of(&h, &controls[0]).top());
        let order = std::iter::once("workspace.diagrams.filter".to_string()).chain(controls.iter().cloned()).collect::<Vec<_>>();
        walk(&mut h, &order, bounds, |id| (!id.ends_with(".filter")).then(|| id.to_string()));
        assert_eq!(h.state().diagrams.scroll.offset, 0.0, "{theme:?}: Shift+Tab took the tree back to its top");
        let rows = controls.into_iter().filter(|id| !id.ends_with(".add")).collect::<Vec<_>>();
        let shown = last_whole(&h, &rows, bounds);
        focus(&mut h, &rows[shown]);
        h.key_down(egui::Key::Tab);
        h.step();
        h.key_up(egui::Key::Tab);
        h.step();
        let asked = h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("explorer.rs"));
        h.run();
        assert!(asked && holds(&h, &rows[shown + 1]) && whole(&h, &rows[shown + 1], bounds), "{theme:?}: {:?}", h.ctx.repaint_causes());
        past_the_edge(&mut h, &rows, (30, &rows[30]), (&rows[29], true), bounds);
        past_the_edge(&mut h, &rows, (31, &rows[31]), (&rows[32], false), bounds);
        focus(&mut h, &rows[0]);
        pages(&mut h, &rows, bounds, holds);
        key(&mut h, egui::Key::PageDown, egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND));
        assert!(holds(&h, &rows[0]), "{theme:?}: not with the control key");
        // The in-window menu bar opened by F10 leaves the keyboard on the row; its keys are its own.
        h.state_mut().in_window_menu = true;
        key(&mut h, egui::Key::F10, egui::Modifiers::NONE);
        key(&mut h, egui::Key::PageDown, egui::Modifiers::NONE);
        assert!(h.state().menu_bar.open.is_some() && holds(&h, &rows[0]), "{theme:?}: with the menu bar open");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // Nor in the frame of the click that opens the bar (AR-518).
        open_file_with(&mut h, egui::Key::End, egui::Modifiers::NONE, true);
        let still = !holds(&h, &rows[rows.len() - 1]) && h.state().diagrams.scroll.offset == 0.0;
        assert!(still, "{theme:?}: with the click that opens the menu bar");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // In the smallest window, over the open diagram's footer, not even a row shows whole: a
        // page key still moves a row.
        h.set_size(egui::vec2(1600.0, 400.0));
        crate::canvas::open(h.state_mut(), rows[10].rsplit('.').next().and_then(archforge_mof::Uuid::parse).unwrap());
        focus(&mut h, &rows[10]);
        assert!(!whole(&h, &rows[10], (0.0, rect_of(&h, "workspace.explorer.resize").bottom() - 128.0)), "{theme:?}: no row shows whole");
        key(&mut h, egui::Key::PageDown, egui::Modifiers::NONE);
        assert!(holds(&h, &rows[11]), "{theme:?}: Page Down moved a row");
        key(&mut h, egui::Key::PageUp, egui::Modifiers::NONE);
        assert!(holds(&h, &rows[10]), "{theme:?}: Page Up moved a row");
    });
}

/// Sixty tables in a schema, open in Models at the design's window size, nothing selected; gives
/// the rows' ids in the tree's order.
fn many_tables(theme: ThemeKind) -> (Harness<'static, Shell>, Vec<String>) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    let schema = crate::panels::run(h.state_mut(), "sketch.package.add", serde_json::json!({ "name": "Shop" })).unwrap()["id"].clone();
    for n in 0..60 {
        crate::panels::run(h.state_mut(), "sketch.class.add", serde_json::json!({ "package": schema, "name": format!("t{n:02}") })).unwrap();
    }
    h.state_mut().explorer.expanded.insert(schema.as_str().and_then(archforge_mof::Uuid::parse).unwrap());
    // The last table added is selected, and the tree followed it: nothing selected, at the top.
    crate::panels::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [] })).unwrap();
    h.state_mut().explorer.scroll = Default::default();
    h.state_mut().state.show_explorer(crate::state::Explorer::Models);
    h.run();
    let shell = h.state();
    let tree = crate::explorer::rows(shell.notation_uis, shell.session.as_ref().unwrap().doc(), &shell.explorer);
    let rows = tree.iter().map(|row| format!("workspace.explorer.row.{}", row.id)).collect::<Vec<_>>();
    assert_eq!(rows.len(), 61, "{theme:?}: {rows:?}");
    (h, rows)
}

/// Whether the row `id` of Models is the selected element's.
fn selected(h: &Harness<'static, Shell>, id: &str) -> bool {
    let s = h.state().session.as_ref().unwrap();
    s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok()).is_some_and(|u| id == format!("workspace.explorer.row.{u}"))
}

/// From the filter field, Tab lands on the tree, then on every row of Models with its arrow or drag
/// handle in turn, and Shift+Tab from the last back to the filter field, each row brought whole into
/// view, past a row the wheel left cut by an edge too; the page keys move the keyboard. While the tree holds the keys, after a click on a row,
/// they move the selection instead, and the arrows still move it a row; not with the control key,
/// nor in the frame a screen reader's click opens the in-window menu bar in.
/// With nothing selected they select the first row, or the last for End.
#[test]
fn models_follows_the_keyboard() {
    in_both_appearances(|theme| {
        let (mut h, rows) = many_tables(theme);
        let bounds = within(&h, rect_of(&h, &rows[0]).top());
        let head = ["workspace.explorer.filter".to_string(), "workspace.explorer.tree".into(), rows[0].clone(), format!("{}.fold", rows[0])];
        let order = head.into_iter().chain(rows[1..].iter().flat_map(|t| [format!("{t}.drag"), t.clone()])).collect::<Vec<_>>();
        walk(&mut h, &order, bounds, |id| {
            let row = id.trim_end_matches(".fold").trim_end_matches(".drag");
            row.starts_with("workspace.explorer.row.").then(|| row.to_string())
        });
        assert_eq!(h.state().explorer.scroll.offset, 0.0, "{theme:?}: Shift+Tab took the tree back to its top");
        past_the_edge(&mut h, &rows, (30, &format!("{}.drag", rows[30])), (&rows[29], true), bounds);
        past_the_edge(&mut h, &rows, (31, &rows[31]), (&format!("{}.drag", rows[32]), false), bounds);
        focus(&mut h, &rows[0]);
        pages(&mut h, &rows, bounds, holds);
        h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id(&rows[0])));
        let at = rect_of(&h, &rows[0]).center();
        click(&mut h, at);
        assert!(h.state().explorer.focused && selected(&h, &rows[0]), "{theme:?}: the tree has the keys");
        pages(&mut h, &rows, bounds, selected);
        key(&mut h, egui::Key::End, egui::Modifiers::CTRL.plus(egui::Modifiers::COMMAND));
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        assert!(selected(&h, &rows[1]) && h.state().explorer.scroll.offset == 0.0, "{theme:?}: not with the control key");
        // Nor in the frame of the click that opens the in-window menu bar (AR-518).
        h.state_mut().in_window_menu = true;
        h.run();
        open_file_with(&mut h, egui::Key::End, egui::Modifiers::NONE, false);
        assert!(selected(&h, &rows[1]) && h.state().explorer.scroll.offset == 0.0, "{theme:?}: with the click that opens the menu bar");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // With nothing selected, End selects the last row and Home, Page Up and Page Down the first.
        for (key_, to) in [(egui::Key::End, 60), (egui::Key::Home, 0), (egui::Key::PageDown, 0), (egui::Key::End, 60), (egui::Key::PageUp, 0)] {
            crate::panels::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [] })).unwrap();
            key(&mut h, key_, egui::Modifiers::NONE);
            assert!(selected(&h, &rows[to]) && whole(&h, &rows[to], bounds), "{theme:?}: {key_:?} with nothing selected");
        }
        // An empty tree takes none of them.
        h.state_mut().explorer.filter = "none of them".into();
        crate::panels::run(h.state_mut(), "view.select", serde_json::json!({ "ids": [] })).unwrap();
        key(&mut h, egui::Key::End, egui::Modifiers::NONE);
        assert!(h.state().session.as_ref().unwrap().view().selection.is_empty(), "{theme:?}: nothing selected in an empty tree");
    });
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
        let schema = id(s.execute("sketch.package.add", &serde_json::json!({ "name": "Shop" })).unwrap().value);
        let customer = id(s.execute("sketch.class.add", &serde_json::json!({ "package": schema, "name": "Customer" })).unwrap().value);
        s.execute("sketch.attribute.add", &serde_json::json!({ "class": customer, "name": "customer_id" })).unwrap();
        s.execute("sketch.attribute.add", &serde_json::json!({ "class": customer, "name": "email" })).unwrap();
        s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        (schema, customer)
    };
    let _ = schema;
    h.run();
    (h, customer)
}

/// The text the store has for skill `id`.
fn st_body(h: &Harness<'static, Shell>, id: archforge_mof::Uuid) -> String {
    let st = h.state().session.as_ref().unwrap().doc();
    st.get(st.by_uuid(id).unwrap(), archforge_core::skill::BODY).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

/// The Skills panel's `+` makes a skill on the selection and opens it with its name ready to type.
#[test]
fn the_skills_plus_makes_a_skill_and_opens_it_with_the_name_ready() {
    let (mut h, customer) = table_harness();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).unwrap();
        let schema_id = st.uuid(schema).unwrap().to_string();
        s.execute("skill.add", &serde_json::json!({ "subject": schema_id, "name": "Class naming", "applies_to": "class", "triggers": ["naming"] }))
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

/// A skill's name refused in its tab says why in plain words and in the chosen language, never
/// with the command's id, the parameter it is about or the engine's English: the name another skill
/// on the same table has. In English and German, in light and dark (AR-431).
#[test]
fn a_skill_name_refused_in_its_tab_says_why_in_plain_words() {
    for (language, theme) in [("en", ThemeKind::Light), ("en", ThemeKind::Dark), ("de", ThemeKind::Light), ("de", ThemeKind::Dark)] {
        let (mut h, customer) = table_harness();
        h.state_mut().state.theme = theme;
        h.state_mut().state.language = language.into();
        h.run();
        {
            let s = h.state_mut().session.as_mut().unwrap();
            s.execute("skill.add", &serde_json::json!({ "subject": customer, "name": "Customer naming" })).unwrap();
            s.execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        }
        let at = rect_of(&h, "workspace.panels.skills").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.skills.add").center();
        click(&mut h, at);
        h.run();
        h.event(egui::Event::Text("Customer naming".into()));
        h.run();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run();
        let doc = h.state().canvas.skills.iter().find(|d| Some(d.skill) == h.state().canvas.active).unwrap();
        let refused = doc.settings_refused.clone().unwrap_or_else(|| panic!("{language} {theme:?}: refused"));
        let what = crate::i18n::tr_ctx("refusal", "a skill");
        if language == "de" {
            assert_ne!(what, "a skill", "{theme:?}: the catalogue has the kind in German");
        }
        let said = crate::i18n::fmt(&crate::i18n::tr("There is {what} named {name} already."), &[("what", &what), ("name", "Customer naming")]);
        assert_eq!(refused, said, "{language} {theme:?}: the kind and the other skill's name, in the chosen language");
    }
    crate::i18n::set_language("en");
}

/// A skill opens in a tab of its own, and its Markdown is kept as typed.
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
    diagram_harness_in(ThemeKind::Light)
}

/// [`diagram_harness`] in `theme`.
fn diagram_harness_in(theme: ThemeKind) -> (Harness<'static, Shell>, String, String) {
    let (mut h, customer) = table_harness_in(theme);
    let diagram = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).unwrap()).unwrap().to_string();
        let order = id(s.execute("sketch.class.add", &serde_json::json!({ "package": schema, "name": "Order" })).unwrap().value);
        s.execute("sketch.attribute.add", &serde_json::json!({ "class": order, "name": "order_id" })).unwrap();
        let places = id(s.execute("sketch.association.add", &serde_json::json!({ "from": customer, "to": order, "name": "places" })).unwrap().value);
        let diagram = id(s.execute("diagram.add", &serde_json::json!({ "kind": SKETCH, "element": schema, "name": "Shop schema" })).unwrap().value);
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

/// A refused change in Appearance is said under Use diagram defaults and read out as a status
/// until the selection moves on. In light and dark (AR-420).
#[test]
fn a_refused_appearance_change_is_read_out_as_a_status() {
    use egui_kittest::kittest::{NodeT, Queryable};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, customer, _) = diagram_harness();
        h.state_mut().state.theme = theme;
        let at = rect_of(&h, "workspace.panels.appearance").center();
        click(&mut h, at);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        h.run();
        // A session that refuses every change makes the panel say why.
        h.state_mut().session.as_mut().unwrap().set_read_only(true).unwrap();
        let at = rect_of(&h, "workspace.appearance.fill.2").center();
        click(&mut h, at);
        h.run();
        h.state_mut().session.as_mut().unwrap().set_read_only(false).unwrap();
        let notice = h.state().panels.notice.clone().unwrap_or_default();
        assert!(!notice.is_empty(), "{theme:?}: refused");
        let status = h.get_by_label(&notice).accesskit_node();
        assert_eq!((status.role(), status.live()), (egui::accesskit::Role::Status, egui::accesskit::Live::Polite), "{theme:?}: read out");
        assert!(rect_of(&h, "workspace.appearance.notice").top() > rect_of(&h, "workspace.appearance.defaults").bottom(), "{theme:?}");
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
        h.run();
        assert_eq!(h.state().panels.notice, None, "{theme:?}: gone with the selection");
        assert!(h.query_by_label(&notice).is_none(), "{theme:?}: and no longer read out");
    }
}

/// A change in Appearance refused says why in plain words, as a sentence, never as the engine words
/// it: a fill picked, and Use diagram defaults, while the model can't be changed. In light and dark
/// (AR-431).
#[test]
fn a_refused_appearance_change_says_why_in_plain_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for control in ["workspace.appearance.fill.2", "workspace.appearance.defaults"] {
            let (mut h, customer, _) = diagram_harness();
            h.state_mut().state.theme = theme;
            let at = rect_of(&h, "workspace.panels.appearance").center();
            click(&mut h, at);
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
            h.run();
            h.state_mut().session.as_mut().unwrap().set_read_only(true).unwrap();
            let at = rect_of(&h, control).center();
            click(&mut h, at);
            h.state_mut().session.as_mut().unwrap().set_read_only(false).unwrap();
            let notice = h.state().panels.notice.clone().unwrap_or_else(|| panic!("{theme:?}, {control}: refused"));
            assert_eq!(notice, crate::i18n::tr("The model cannot be changed at the moment; nothing was changed."), "{theme:?}, {control}");
        }
    }
}

/// Asserts that `words` are read out as a polite live status, inside the area recorded as `id`.
fn read_out(h: &Harness<'static, Shell>, words: &str, id: &str) {
    use egui_kittest::kittest::{NodeT, Queryable};
    let status = h.get_by_label(words).accesskit_node();
    assert_eq!((status.role(), status.live()), (egui::accesskit::Role::Status, egui::accesskit::Live::Polite), "{words}: read out");
    assert!(ids(h).iter().any(|i| i == id), "{words}: recorded as {id}");
}

/// The Appearance panel styles the selected box and line on the open diagram only.
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

/// The control channel runs its methods through the open session; without a project it says none
/// is open.
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
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).unwrap()).unwrap().to_string()
    };
    let count = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::CLASS)).count()
    };
    let before = count(&h);
    // Listing and describing.
    let listed = answer(h.state_mut(), "engine.commands", &json!({ "filter": "class" })).unwrap();
    assert!(listed["commands"].as_array().unwrap().iter().any(|c| c["id"] == "sketch.class.add"));
    let described = answer(h.state_mut(), "engine.describe", &json!({ "command": "sketch.class.add" })).unwrap();
    assert!(described["params"].as_array().unwrap().iter().any(|p| p["key"] == "name"));
    // A command runs in the session the window draws, and undo takes it back.
    let out = answer(h.state_mut(), "engine.execute", &json!({ "command": "sketch.class.add", "params": { "package": schema, "name": "Invoice" } }))
        .unwrap();
    assert!(out["result"]["id"].is_string());
    assert_eq!(count(&h), before + 1);
    answer(h.state_mut(), "engine.undo", &json!({})).unwrap();
    assert_eq!(count(&h), before);
    // A batch refers to an earlier step's result; a failing step rolls the whole batch back.
    let steps = json!([
        { "command": "sketch.class.add", "params": { "package": schema, "name": "Payment" } },
        { "command": "sketch.attribute.add", "params": { "class": "$0.id", "name": "payment_id" } },
    ]);
    let out = answer(h.state_mut(), "engine.batch", &json!({ "label": "Add payment", "steps": steps })).unwrap();
    assert_eq!(out["results"].as_array().unwrap().len(), 2);
    assert_eq!(count(&h), before + 1);
    let bad = json!([
        { "command": "sketch.class.add", "params": { "package": schema, "name": "Refund" } },
        { "command": "sketch.attribute.add", "params": { "class": "$5.id", "name": "x" } },
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

/// A click on the minimap centres the view there, and a drag of its viewport moves the view with the
/// pointer while the map keeps its scale.
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
    if st.class_of(o).ok() == Some(archforge_diagram::DIAGRAM) { crate::diagrams::diagram_name(st, o) } else { archforge_core::name_of(st, o) }
}

/// Enter on New diagram's focused Cancel cancels and makes nothing; on a kind's card it goes on to
/// the details.
#[test]
fn enter_on_new_diagrams_focused_cancel_cancels_and_on_a_card_goes_on() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let before = h.state().session.as_ref().unwrap().doc().alive().count();
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    // Tab brought to Cancel: Enter is its press.
    h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id("workspace.new-diagram.cancel")));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().diagrams.new.is_none(), "Cancel took Enter");
    assert_eq!(h.state().session.as_ref().unwrap().doc().alive().count(), before, "nothing made");
    // Elsewhere, Enter goes on to the details.
    let at = rect_of(&h, "workspace.tabs.add").center();
    click(&mut h, at);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(h.state().diagrams.new.as_ref().map(|n| n.step), Some(crate::diagrams::Step::Details));
}

/// The automation ids of the canvas's nodes the last frame drew.
fn node_ids(h: &Harness<'static, Shell>) -> Vec<String> {
    ids(h).iter().filter(|i| i.starts_with("workspace.canvas.node.")).cloned().collect()
}

/// The bounds of the node `node` in diagram units.
fn node_bounds(h: &Harness<'static, Shell>, node: &str) -> archforge_diagram::Rect {
    let st = h.state().session.as_ref().unwrap().doc();
    let n = st.by_uuid(archforge_mof::Uuid::parse(node).unwrap()).unwrap();
    archforge_diagram::grid::bounds_of(st, n)
}

/// A drag moves a node by the drag in diagram units and a corner handle resizes it, each one undo
/// step; the Arrange bar aligns two selected nodes.
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

/// A drag with `button` from `from` to `to`, a frame at each step.
fn drag_by(h: &mut Harness<'static, Shell>, button: egui::PointerButton, from: egui::Pos2, to: egui::Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.event(egui::Event::PointerButton { pos: from, button, pressed: true, modifiers: egui::Modifiers::NONE });
    h.run();
    h.event(egui::Event::PointerMoved(to));
    h.run();
    h.event(egui::Event::PointerButton { pos: to, button, pressed: false, modifiers: egui::Modifiers::NONE });
    h.run();
}

/// A marquee selects what it crosses; a middle drag over a node pans the view and moves nothing.
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

/// A table's name refused on the canvas says why as Properties says it, in the chosen language,
/// never with the command's id or the parameter it is about. In light and dark (AR-428).
#[test]
fn a_name_refused_on_the_canvas_says_why_in_plain_words() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _customer, _diagram) = diagram_harness();
        h.state_mut().state.theme = theme;
        h.state_mut().state.language = "de".into();
        h.run();
        let nodes = node_ids(&h);
        let customer = nodes.iter().min_by(|a, b| rect_of(&h, a).left().total_cmp(&rect_of(&h, b).left())).unwrap().clone();
        let at = rect_of(&h, &customer).left_top() + egui::vec2(60.0, 30.0);
        double_click(&mut h, at);
        h.state_mut().canvas.rename.as_mut().unwrap_or_else(|| panic!("{theme:?}: the name opened")).text = String::new();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        let said = crate::i18n::tr("A name can't be empty.");
        assert_ne!(said, "A name can't be empty.", "{theme:?}: the catalogue has it in German");
        let refused = h.state().canvas.rename.as_ref().and_then(|r| r.refused.clone());
        assert_eq!(refused.as_deref(), Some(said.as_str()), "{theme:?}");
    }
    crate::i18n::set_language("en");
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
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).unwrap()).unwrap().to_string();
        let order = id(s.execute("sketch.class.add", &serde_json::json!({ "package": schema, "name": "Order" })).unwrap().value);
        s.execute("sketch.attribute.add", &serde_json::json!({ "class": order, "name": "order_id" })).unwrap();
        let diagram = id(s.execute("diagram.add", &serde_json::json!({ "kind": SKETCH, "element": schema, "name": "Shop schema" })).unwrap().value);
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

/// A note's text typed in place commits on ⌘↩ (Ctrl Enter) alone, in both key shapes: with Shift,
/// Alt or a Mac's Control more the press neither commits it nor writes in it.
#[test]
fn a_note_commits_on_its_own_chord_alone() {
    in_both_appearances(|theme| {
        let (mut h, _customer, diagram) = diagram_harness();
        h.state_mut().state.theme = theme;
        let s = h.state_mut().session.as_mut().unwrap();
        let note = s.execute("diagram.note", &serde_json::json!({ "diagram": diagram, "text": "Why", "x": 100, "y": 320 })).unwrap().value;
        let note = archforge_mof::Uuid::parse(note["id"].as_str().unwrap()).unwrap();
        h.run();
        let (mac, shift, alt) = (egui::Modifiers::MAC_CMD | egui::Modifiers::COMMAND, egui::Modifiers::SHIFT, egui::Modifiers::ALT);
        for (shape, extras, typed) in
            [(mac, vec![shift, alt, egui::Modifiers::CTRL], "Later"), (egui::Modifiers::CTRL | egui::Modifiers::COMMAND, vec![shift, alt], "Sooner")]
        {
            let at = rect_of(&h, &format!("workspace.canvas.node.{note}")).center();
            (0..2).for_each(|_| click(&mut h, at));
            h.state_mut().canvas.note_edit.as_mut().expect("its text is open").text = typed.into();
            for more in extras {
                key(&mut h, egui::Key::Enter, shape | more);
                assert_eq!(h.state().canvas.note_edit.as_ref().map(|n| n.text.as_str()), Some(typed), "{theme:?}: {:?}", shape | more);
            }
            key(&mut h, egui::Key::Enter, shape);
            assert!(h.state().canvas.note_edit.is_none(), "{theme:?}: {shape:?} commits");
            let st = h.state().session.as_ref().unwrap().doc();
            assert_eq!(st.get(st.by_uuid(note).unwrap(), archforge_diagram::node::TEXT).unwrap().as_str(), Some(typed), "{theme:?}");
        }
    });
}

/// An element's description typed and left by clicking another element's row in Models is kept on
/// the element it was typed on, and the other element is left as it was. In light and dark.
#[test]
fn a_description_draft_left_by_clicking_another_row_stays_on_its_own() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, customer) = table_harness();
        h.state_mut().state.theme = theme;
        let (order, schema) = {
            let s = h.state_mut().session.as_mut().unwrap();
            let st = s.doc();
            let schema = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).unwrap();
            let schema = st.uuid(schema).unwrap();
            let out = s.execute("sketch.class.add", &serde_json::json!({ "package": schema.to_string(), "name": "Order" })).unwrap();
            (archforge_mof::Uuid::parse(out.value["id"].as_str().unwrap()).unwrap(), schema)
        };
        h.state_mut().explorer.expanded.insert(schema);
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [customer] })).unwrap();
        h.state_mut().state.show_panel(crate::state::Panel::Properties);
        h.run();
        let at = rect_of(&h, "workspace.properties.description").center();
        click(&mut h, at);
        h.event(egui::Event::Text("People who buy.".into()));
        h.run();
        let at = rect_of(&h, &format!("workspace.explorer.row.{order}")).center();
        click(&mut h, at);
        h.run();
        let s = h.state().session.as_ref().unwrap();
        let st = s.doc();
        let (customer, order) = (st.by_uuid(archforge_mof::Uuid::parse(&customer).unwrap()).unwrap(), st.by_uuid(order).unwrap());
        assert_eq!(s.view().selection, vec![order], "the row selected Order");
        let described = |o| s.view().of_element(st, o).map(|n| n.description(st, o)).unwrap_or_default();
        assert_eq!(described(customer), "People who buy.");
        assert_eq!(described(order), "", "Order is unchanged");
        assert!(h.state().panels.description.is_none(), "the draft is spent");
    }
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

/// The Models tree starts folded, Expand all opens every row and folds them again, and an element
/// selected elsewhere opens the rows above it once.
#[test]
fn the_models_tree_starts_folded_and_opens_all_at_once() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let (schema, entity, column) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| v["id"].as_str().unwrap().to_string();
        let schema = id(s.execute("sketch.package.add", &serde_json::json!({ "name": "shop" })).unwrap().value);
        let entity = id(s.execute("sketch.class.add", &serde_json::json!({ "package": schema, "name": "Customer" })).unwrap().value);
        let column = id(s.execute("sketch.attribute.add", &serde_json::json!({ "class": entity, "name": "email" })).unwrap().value);
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
        s.execute("sketch.association.add", &serde_json::json!({ "from": customer, "to": order, "name": "places" })).unwrap();
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

/// The fake notation's diagram kind, the one the sketch kind serves in the shell.
pub(crate) const SKETCH: &str = archforge_notations::fake::KIND;

/// Every text the last frame painted.
/// The colours the text `label` is laid out in, where it is painted inside `within`.
fn label_colours(h: &Harness<'static, Shell>, label: &str, within: egui::Rect) -> Vec<egui::Color32> {
    /// Collects the colours of every text `label` painted inside `within` under `s`.
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

/// The texts painted with their top left inside `r`, each with how many glyphs it was laid out in.
fn texts_in(h: &Harness<'static, Shell>, r: egui::Rect) -> Vec<(String, usize)> {
    fn walk(s: &egui::Shape, r: egui::Rect, out: &mut Vec<(String, usize)>) {
        match s {
            egui::Shape::Text(t) if r.contains(t.pos) => {
                out.push((t.galley.text().to_string(), t.galley.rows.iter().map(|row| row.glyphs.len()).sum()))
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, r, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, r, &mut out));
    out
}

/// Every text painted in the last frame, in the order it was painted.
fn painted<S>(h: &Harness<'_, S>) -> Vec<String> {
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

/// A workspace with a sketch diagram of a package: Party (`partyId`, `rename()`) at the left,
/// Customer (`since`) at the right, Customer a kind of Party. Gives the diagram, Party's node,
/// Customer's node and the line's edge.
fn sketch_harness() -> (Harness<'static, Shell>, String, String, String, String) {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let (d, a, b, e) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut run = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap_or_default().to_string();
        let pkg = run("sketch.package.add", serde_json::json!({ "name": "sales" }));
        let party = run("sketch.class.add", serde_json::json!({ "package": pkg, "name": "Party" }));
        run("sketch.attribute.add", serde_json::json!({ "class": party, "name": "partyId" }));
        run("sketch.operation.add", serde_json::json!({ "class": party, "name": "rename" }));
        let customer = run("sketch.class.add", serde_json::json!({ "package": pkg, "name": "Customer" }));
        run("sketch.attribute.add", serde_json::json!({ "class": customer, "name": "since" }));
        let g = run("sketch.inherit.add", serde_json::json!({ "specific": customer, "general": party }));
        let d = run("diagram.add", serde_json::json!({ "kind": SKETCH, "element": pkg }));
        let a = run("diagram.place", serde_json::json!({ "diagram": d, "element": party, "x": 100, "y": 100, "width": 240, "height": 166 }));
        let b = run("diagram.place", serde_json::json!({ "diagram": d, "element": customer, "x": 600, "y": 100, "width": 240, "height": 166 }));
        let e = run("diagram.connect", serde_json::json!({ "diagram": d, "element": g, "source": b, "target": a }));
        run("view.select", serde_json::json!({ "ids": [] }));
        (d, a, b, e)
    };
    crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&d).unwrap());
    h.run();
    (h, d, a, b, e)
}

/// What drawing the diagram `d` of `session` paints and records, with no shell anywhere, as `kind`
/// draws its own figures, over the area `width` wide and with `overrides` as a drag's preview: the
/// texts, the ids recorded and the badges given back.
fn drawn_alone(
    session: &archforge_notations::Session,
    d: &str,
    kind: Option<&'static dyn crate::kinds::DiagramKind>,
    dark: bool,
    width: f32,
    overrides: &std::collections::BTreeMap<archforge_mof::Uuid, egui::Rect>,
    problems: &std::collections::BTreeMap<archforge_mof::Obj, Vec<String>>,
) -> (Vec<String>, Vec<String>, usize) {
    let (texts, recorded, badges) = drawn_alone_at(session, d, (kind, SCREENS.kinds()), dark, width, overrides, problems);
    (texts, recorded.into_iter().map(|(id, _)| id).collect(), badges)
}

/// [`drawn_alone`], each id recorded with its rectangle, `kinds` the kinds asked after `kind` for
/// a card they draw themselves.
fn drawn_alone_at(
    session: &archforge_notations::Session,
    d: &str,
    (kind, kinds): (Option<&'static dyn crate::kinds::DiagramKind>, &'static [&'static dyn crate::kinds::DiagramKind]),
    dark: bool,
    width: f32,
    overrides: &std::collections::BTreeMap<archforge_mof::Uuid, egui::Rect>,
    problems: &std::collections::BTreeMap<archforge_mof::Obj, Vec<String>>,
) -> (Vec<String>, Vec<(String, egui::Rect)>, usize) {
    let store = session.doc();
    let diagram = store.by_uuid(archforge_mof::Uuid::parse(d).unwrap()).unwrap();
    let grid = archforge_diagram::Grid::build(store, diagram);
    let area = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 700.0));
    let view = crate::canvas::View { origin: area.min, pan: egui::Vec2::ZERO, zoom: 1.0 };
    let badges = std::cell::Cell::new(0);
    let recorded = std::cell::RefCell::new(crate::widgets::Registry::default());
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
    let texts = painted(&h);
    drop(h);
    let at = |r: [f32; 4]| egui::Rect::from_min_max(egui::pos2(r[0], r[1]), egui::pos2(r[2], r[3]));
    let ids = recorded.borrow().entries().iter().map(|e| (e.id.clone(), at(e.rect))).collect();
    (texts, ids, badges.get())
}

/// A store's diagram draws with no shell at all, lines under cards, as it does on the canvas, in
/// both appearances; only what is in view draws, and what a drag has brought into it. A line
/// whose element has a problem gives its badge back to whoever draws it.
#[test]
fn a_stores_diagram_draws_with_no_shell() {
    let (mut h, d, a, b, e) = sketch_harness();
    let session = h.state_mut().session.take().unwrap();
    drop(h);
    let none = std::collections::BTreeMap::new();
    let clean = std::collections::BTreeMap::new();
    let store = session.doc();
    let line = store.by_uuid(archforge_mof::Uuid::parse(&e).unwrap()).unwrap();
    let element = crate::canvas::element_of(store, line, archforge_diagram::edge::ELEMENT).unwrap();
    let flagged = [(element, vec!["A problem".to_string()])].into_iter().collect();
    for dark in [false, true] {
        let (texts, ids, badges) = drawn_alone(&session, &d, Some(&SketchKind(SKETCH)), dark, 1000.0, &none, &clean);
        assert_eq!(badges, 0);
        for t in ["CLASS", "Party", "partyId", "rename", "Customer", "since", "specific"] {
            assert!(texts.iter().any(|x| x == t), "{t} is drawn: {texts:?}");
        }
        let at = |t: &str| texts.iter().position(|x| x == t).unwrap();
        assert!(at("specific") < at("Party") && at("specific") < at("Customer"), "the line is under the cards: {texts:?}");
        for id in [format!("workspace.canvas.node.{a}"), format!("workspace.canvas.node.{b}"), format!("workspace.canvas.edge.{e}")] {
            assert!(ids.contains(&id), "{id}: {ids:?}");
        }
        let (.., badges) = drawn_alone(&session, &d, Some(&SketchKind(SKETCH)), dark, 1000.0, &none, &flagged);
        assert_eq!(badges, 1, "the line's badge");
        let (texts, ..) = drawn_alone(&session, &d, Some(&SketchKind(SKETCH)), dark, 400.0, &none, &clean);
        assert!(texts.iter().any(|x| x == "Party") && !texts.iter().any(|x| x == "Customer"), "out of view: {texts:?}");
        let dragged = [(archforge_mof::Uuid::parse(&b).unwrap(), egui::Rect::from_min_size(egui::pos2(20.0, 300.0), egui::vec2(240.0, 166.0)))];
        let (texts, ..) = drawn_alone(&session, &d, Some(&SketchKind(SKETCH)), dark, 400.0, &dragged.into_iter().collect(), &clean);
        assert!(texts.iter().any(|x| x == "Customer"), "a drag brought it into view: {texts:?}");
    }
}

/// Runs `check` in both appearances.
fn in_both_appearances(check: impl Fn(ThemeKind)) {
    [ThemeKind::Light, ThemeKind::Dark].into_iter().for_each(check);
}

/// The Versions explorer of a project, first drawn with no host, then its read waiting for the test.
fn versions_panel(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().state.theme = theme;
    h.state_mut().services.projects = None;
    h.state_mut().versions = Default::default();
    host.version_reads.lock().unwrap().clear();
    let at = rect_of(&h, "workspace.explorers.source-control").center();
    click(&mut h, at);
    h.state_mut().services.projects = Some(host.clone());
    h.run_steps(2);
    (h, host)
}

/// Answers the read the panel is waiting for.
fn versions_answer(h: &mut Harness<'static, Shell>, host: &SavingHost, answer: Result<crate::VersionsPage, String>) {
    h.run_steps(2);
    let reply = host.version_reads.lock().unwrap().pop().expect("a read was asked for");
    reply.send(answer).unwrap();
    h.run_steps(2);
}

/// Two versions, the older without a description.
fn two_versions() -> crate::VersionsPage {
    let date = "2026-11-08T15:30:00Z".to_string();
    let aa = crate::VersionEntry {
        hash: "aa".into(),
        description: Some("Add order status".into()),
        author: None,
        by_agent: false,
        date,
        previous: None,
        shared: false,
    };
    let bb = crate::VersionEntry { hash: "bb".into(), description: None, ..aa.clone() };
    crate::VersionsPage { versions: vec![aa, bb], more: false }
}

/// Whether the control `id` is drawn.
fn has_id(h: &Harness<'static, Shell>, id: &str) -> bool {
    ids(h).iter().any(|i| i == id)
}

/// The panel says what is true while it loads, when a read fails, on Retry, and with no version yet.
#[test]
fn the_versions_panel_says_it_is_loading_failed_or_empty() {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        let texts = painted(&h);
        assert!(["VERSIONS", "Branches", "Working changes", "History"].iter().all(|t| texts.iter().any(|x| x == t)), "{theme:?}: {texts:?}");
        assert!(has_id(&h, "workspace.versions.loading") && !has_id(&h, "workspace.versions.retry"), "{theme:?}");
        assert!(!painted(&h).iter().any(|t| t == "No versions yet."), "not said before it is known");
        drop(host.version_reads.lock().unwrap().pop());
        h.run_steps(2);
        assert!(painted(&h).iter().any(|t| t == "The versions couldn't be read.") && !has_id(&h, "workspace.versions.loading"), "{theme:?}");
        let at = rect_of(&h, "workspace.versions.retry").center();
        click(&mut h, at);
        assert!(has_id(&h, "workspace.versions.loading") && !has_id(&h, "workspace.versions.failed"), "{theme:?}: Retry reads again");
        versions_answer(&mut h, &host, Ok(crate::VersionsPage::default()));
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "No versions yet."), "{texts:?}");
        assert!(texts.iter().any(|t| t == "No versions yet. Save version records your first one."), "{texts:?}");
        assert!(!has_id(&h, "workspace.versions.failed"), "{theme:?}");
    });
}

/// ⌘S and File ▸ Save on an untouched new project make no version: the host is never asked, and
/// History still says there are none; a save with a description, or after a failed save or write,
/// still asks it (AR-413).
#[test]
fn saving_an_untouched_new_project_makes_no_version() {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        versions_answer(&mut h, &host, Ok(crate::VersionsPage::default()));
        assert!(!dirty(&h) && !h.state().shown_unsaved(), "{theme:?}: the status bar says Saved");
        h.event(egui::Event::Key { key: egui::Key::S, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND });
        h.run_steps(2);
        h.state_mut().save();
        h.run_steps(2);
        assert!(host.saves.lock().unwrap().is_empty() && !h.state().is_saving(), "{theme:?}: no version was asked for");
        assert!(host.version_reads.lock().unwrap().is_empty(), "{theme:?}: History has nothing new to read");
        assert!(painted(&h).iter().any(|t| t == "No versions yet. Save version records your first one."), "{theme:?}: {:?}", painted(&h));
        // A save under a description, an agent's, is still the host's to decide.
        h.state_mut().save_as(crate::SavedBy::Agent(None), Some("Start".into()));
        assert_eq!(host.saves.lock().unwrap().len(), 1, "{theme:?}: a described save goes to the host");
        host.saves.lock().unwrap().pop().unwrap().send(Ok(false)).unwrap();
        h.run_steps(2);
        // A failed save or write is not Saved: Save tries again, as before.
        h.state_mut().save_error = Some("the disk is full".into());
        h.state_mut().save();
        assert_eq!(host.saves.lock().unwrap().len(), 1, "{theme:?}: after a failed save");
        host.saves.lock().unwrap().pop().unwrap().send(Ok(false)).unwrap();
        h.run_steps(2);
        *host.write_error.lock().unwrap() = Some("the disk is full".into());
        h.state_mut().save();
        assert_eq!(host.saves.lock().unwrap().len(), 1, "{theme:?}: after a failed write");
    });
}

/// History lists what was read and keeps it through a failed read; a save, another session or
/// another project read again; "No changes" is never said of unsaved work.
#[test]
fn the_versions_panel_lists_keeps_and_reads_again() {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        let (first, second) = ("workspace.versions.row.aa", "workspace.versions.row.bb");
        versions_answer(&mut h, &host, Ok(two_versions()));
        assert!(has_id(&h, first) && has_id(&h, second), "{theme:?}");
        assert!(painted(&h).iter().any(|t| t == "Add order status") && painted(&h).iter().any(|t| t == "2"), "{:?}", painted(&h));
        add_schema(&mut h, "crm");
        h.state_mut().save();
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        versions_answer(&mut h, &host, Err("the server stopped".into()));
        assert!(has_id(&h, first) && has_id(&h, "workspace.versions.failed"), "{theme:?}: what was shown stays");
    });
}

/// What the panel is about, against whether a read starts: from a list read, each change as the
/// reads then waiting and whether the list stays; and an answer for what was left behind is dropped.
#[test]
fn the_versions_panel_reads_only_for_what_it_is_about() {
    type Step = fn(&mut Shell);
    let no_host: Step = |s| s.services.projects = None;
    let first_draw: Step = |s| s.versions = Default::default();
    let project: Step = |s| s.project.as_mut().unwrap().path = "/tmp/Other.archforge".into();
    let session: Step = |s| s.session_replaced();
    let failed: Step = |s| s.finished_save(0, crate::SaveState::Failed("no".into()));
    let unknown: Step = |s| s.finished_save(0, crate::SaveState::Unknown);
    let cases: [(&str, &[Step], usize, bool); 9] = [
        ("nothing", &[], 0, true),
        ("first draw, host", &[first_draw], 1, false),
        ("first draw, no host", &[no_host, first_draw], 0, false),
        ("project, host", &[project], 1, false),
        ("project, no host", &[no_host, project], 0, false),
        ("session, host", &[session], 1, false),
        ("session, no host", &[no_host, session], 0, false),
        ("save failed, host", &[failed], 1, true),
        ("save unknown, no host", &[no_host, unknown], 0, true),
    ];
    let (first, list) = ("workspace.versions.row.aa", || Ok(two_versions()));
    let waiting = |host: &SavingHost| host.version_reads.lock().unwrap().len();
    in_both_appearances(|theme| {
        for (what, steps, reads, stays) in cases {
            let (mut h, host) = versions_panel(theme);
            versions_answer(&mut h, &host, list());
            steps.iter().for_each(|step| step(h.state_mut()));
            h.run_steps(2);
            assert_eq!((waiting(&host), has_id(&h, first)), (reads, stays), "{theme:?}: {what}");
        }
        let (mut h, host) = versions_panel(theme);
        unknown(h.state_mut());
        h.run_steps(2);
        assert_eq!(waiting(&host), 1, "{theme:?}: a save waits for the read on its way");
        versions_answer(&mut h, &host, list());
        assert_eq!(waiting(&host), 1, "{theme:?}: then reads");
        let left = host.version_reads.lock().unwrap().pop().unwrap();
        project(h.state_mut());
        h.run_steps(2);
        assert!(left.send(list()).is_err() && !has_id(&h, first), "{theme:?}: the old answer is dropped");
        versions_answer(&mut h, &host, Err("no".into()));
        no_host(h.state_mut());
        session(h.state_mut());
        h.run_steps(2);
        assert!(!has_id(&h, "workspace.versions.failed"), "{theme:?}: nor is its error kept");
    });
}

/// A panel whose clock is an hour east of UTC, its first read answered with `versions`.
fn versions_listed(theme: ThemeKind, versions: Vec<crate::VersionEntry>) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = versions_panel(theme);
    let now = crate::chat_words::seconds_of("2026-11-09T12:00:00Z").unwrap();
    h.state_mut().services.clock = Some(std::sync::Arc::new(move || crate::chat_words::Clock { now, offset: Some(3600) }));
    versions_answer(&mut h, &host, Ok(crate::VersionsPage { versions, more: false }));
    h.run_steps(2);
    assert!(h.ctx.has_requested_repaint(), "{theme:?}: the window looks again for the last version's rows");
    let rows = rows_now(&h);
    answer_read(&mut h, &host, Ok(rows));
    compared(&mut h);
    (h, host)
}

/// A version with the things a row says.
fn version_row(hash: &str, description: Option<&str>, by: (Option<&str>, bool), date: &str) -> crate::VersionEntry {
    let (description, author) = (description.map(str::to_string), by.0.map(str::to_string));
    crate::VersionEntry { hash: hash.into(), description, author, by_agent: by.1, date: date.into(), previous: None, shared: false }
}

/// A row's two lines, its date local and never as stored: with and without a description, by a
/// person or an agent, from another year or with a date that cannot be read; in the language shown.
#[test]
fn a_versions_row_says_what_it_holds_and_when() {
    let shown = |theme| {
        let dejan = (Some("Dejan"), false);
        let agent = (Some("Chat 3"), true);
        let (h, host) = versions_listed(
            theme,
            vec![
                version_row("a", Some("Add order status"), dejan, "2026-11-08T15:30:00Z"),
                version_row("b", None, dejan, "2026-11-06T16:05:00Z"),
                version_row("c", None, agent, "2026-11-08T11:40:00Z"),
                version_row("d", Some("By hand"), agent, "2026-11-07T10:00:00Z"),
                version_row("e", Some("Last year"), (None, false), "2025-03-02T08:00:00Z"),
                version_row("f", Some("Odd"), dejan, "yesterday"),
                version_row("g", Some("   "), dejan, "2026-11-05T09:00:00Z"),
            ],
        );
        (h, host)
    };
    in_both_appearances(|theme| {
        let (mut h, _host) = shown(theme);
        let texts = painted(&h);
        for t in [
            "Add order status",
            "8 Nov 16:30 · Dejan",
            "6 Nov 17:05",
            "No description",
            "8 Nov 12:40",
            "Saved by an agent",
            "By hand",
            "7 Nov 11:00 · Saved by an agent",
            "2 Mar 2025 09:00",
            "Date not known · Dejan",
            "5 Nov 10:00",
        ] {
            assert!(texts.iter().any(|x| x == t), "{theme:?}: {t}: {texts:?}");
        }
        assert!(!texts.iter().any(|t| t.contains("T15:30") || t == "yesterday"), "{theme:?}: {texts:?}");
        h.state_mut().state.language = "de".to_string();
        h.run_steps(2);
        let german =
            crate::chat_words::date_and_time(crate::chat_words::seconds_of("2026-11-06T16:05:00Z").unwrap(), crate::chat_panel::clock(h.state()));
        assert!(german.starts_with("6.") && painted(&h).contains(&german), "{german}: {:?}", painted(&h));
        h.state_mut().state.language = "en".to_string();
        h.run_steps(2);
    });
}

/// A restore's kept work named by an earlier Archforge, `Before restoring` and a stored time, shows
/// that time as the list says it; the stored name is left as it is, and other names show as stored,
/// a time not in the stored form or a day its month lacks among them.
#[test]
fn an_earlier_restores_kept_work_shows_its_time_as_the_list_says_it() {
    let earlier = "Before restoring 2026-11-07T16:15:00Z";
    let named = |hash, d| version_row(hash, Some(d), (None, false), "2026-11-07T16:15:00Z");
    in_both_appearances(|theme| {
        let kept = [
            "Before restoring 9 Nov 12:59",
            "Before restoring 2026-11-07",
            "Before restoring 2026/11/07 16:15:00X",
            "Before restoring 2026-02-31T10:00:00Z",
        ];
        let listed = [named("a", earlier)].into_iter().chain(kept.iter().zip(["b", "c", "d", "e"]).map(|(d, hash)| named(hash, d))).collect();
        let (h, _host) = versions_listed(theme, listed);
        let texts = painted(&h);
        for t in ["Before restoring 7 Nov 17:15"].iter().chain(&kept) {
            assert!(texts.iter().any(|x| x == t), "{theme:?}: {t}: {texts:?}");
        }
        assert!(!texts.iter().any(|t| t == earlier), "{theme:?}: {texts:?}");
        assert_eq!(h.state().versions.page.as_ref().unwrap().versions[0].description.as_deref(), Some(earlier), "{theme:?}: as stored");
    });
}

/// Over the first version `main` has too, and only there, History draws a divider a screen reader
/// reads; none while no page read holds one, and one at the top when the branch has no version of
/// its own yet.
#[test]
fn history_marks_where_a_branch_shares_its_versions_with_main() {
    use egui_kittest::kittest::Queryable;
    let words = "Shared with main up to here";
    let row = |hash: &str, shared| crate::VersionEntry { shared, ..version_row(hash, Some(hash), (None, false), "2026-11-08T15:30:00Z") };
    in_both_appearances(|theme| {
        let (mut h, host) = versions_listed(theme, vec![row("a", false), row("b", false)]);
        assert!(!has_id(&h, "workspace.versions.shared") && !painted(&h).iter().any(|t| t == words), "{theme:?}: not on this page");
        h.state_mut().versions.page.as_mut().unwrap().more = true;
        h.run_steps(2);
        let at = rect_of(&h, "workspace.versions.more").center();
        click(&mut h, at);
        versions_answer(&mut h, &host, Ok(crate::VersionsPage { versions: vec![row("c", true), row("d", true)], more: false }));
        let [b, c, d, divider] = ["row.b", "row.c", "row.d", "shared"].map(|id| rect_of(&h, &format!("workspace.versions.{id}")));
        assert!(b.bottom() < divider.top() && divider.bottom() < c.top(), "{theme:?}: {b:?} {divider:?} {c:?}");
        assert_eq!((c.top() - b.top(), d.top() - c.top()), (68.0, 50.0), "{theme:?}: only the rows under it move");
        assert_eq!(painted(&h).iter().filter(|t| *t == words).count(), 1, "{theme:?}");
        assert_eq!(h.query_all_by_label(words).count(), 1, "{theme:?}: read out whole");

        let (h, _host) = versions_listed(theme, vec![row("e", true)]);
        let (divider, e) = (rect_of(&h, "workspace.versions.shared"), rect_of(&h, "workspace.versions.row.e"));
        assert!(divider.bottom() < e.top(), "{theme:?}: over the newest");
    });
}

/// A version that cannot be shown says so, with its reason behind "Why?" and read out by a screen
/// reader; what was shown about it before does not stay, and the reason ends with the project.
#[test]
fn an_unavailable_version_says_so_and_why() {
    use egui_kittest::kittest::Queryable;
    in_both_appearances(|theme| {
        let (mut h, _host) = versions_listed(theme, vec![version_row("a", Some("Add order status"), (None, false), "2026-11-08T15:30:00Z")]);
        let why = "its tables are damaged";
        h.state_mut().versions.unavailable.insert("a".to_string(), Some(why.to_string()));
        h.run_steps(2);
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "This version can't be shown") && !texts.iter().any(|t| t == "Add order status"), "{texts:?}");
        assert_eq!(h.query_all_by_label_contains(why).count(), 2, "{theme:?}: the row and its Why?");
        let at = rect_of(&h, "workspace.versions.row.a.why").center();
        h.event(egui::Event::PointerMoved(at));
        h.run_steps(4);
        assert!(painted(&h).iter().any(|t| t == why), "{theme:?}: on hover");
        h.state_mut().versions.unavailable.insert("a".to_string(), None);
        h.run_steps(2);
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "Made with a newer Archforge.") && !has_id(&h, "workspace.versions.row.a.why"), "{theme:?}: {texts:?}");
        assert!(texts.iter().any(|t| t == "8 Nov 16:30") && !texts.iter().any(|t| t.ends_with('·')), "{theme:?}: its date alone: {texts:?}");
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert_eq!(h.state().versions.unavailable.len(), 1, "{theme:?}: showing a version moves the session, not the reason");
        h.state_mut().project.as_mut().unwrap().path = "/elsewhere".into();
        h.run_steps(2);
        assert!(h.state().versions.unavailable.is_empty(), "{theme:?}: it was about the project left");
    });
}

/// The rail counts the working changes above zero, in its badge and what it says; they end with
/// the session. Sections fold, a long list scrolls, and the narrowest column holds its rows.
#[test]
fn the_rail_counts_changes_sections_fold_and_rows_fit() {
    use egui_kittest::kittest::Queryable;
    in_both_appearances(|theme| {
        let many = (0..30).map(|i| version_row(&format!("v{i}"), Some("A version"), (None, false), "2026-11-08T15:30:00Z")).collect();
        let (mut h, _host) = versions_listed(theme, many);
        let badge = "workspace.explorers.source-control.badge";
        assert!(!has_id(&h, badge) && h.query_by_label("Versions").is_some(), "{theme:?}");
        for (working, shows, says) in [(5, "5", "Versions · 5 changes"), (1, "1", "Versions · 1 change"), (150, "99+", "Versions · 150 changes")] {
            h.state_mut().versions.working = working;
            h.run_steps(2);
            assert!(has_id(&h, badge) && painted(&h).iter().any(|t| t == shows) && h.query_by_label(says).is_some(), "{theme:?}: {says}");
        }
        assert!(!painted(&h).iter().any(|t| t == "No changes since the last version."), "{theme:?}: never beside a count");
        h.state_mut().versions.working = 0;
        h.run_steps(2);
        assert!(!has_id(&h, badge), "{theme:?}: none at zero");
        assert!(!has_id(&h, "workspace.versions.row.v29"), "{theme:?}: below the column");
        h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.versions.row.v0").center()));
        for _ in 0..8 {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -1200.0),
                modifiers: egui::Modifiers::NONE,
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
            });
            h.run_steps(2);
        }
        assert!(has_id(&h, "workspace.versions.row.v29"), "{theme:?}: scrolled to");
        assert!(!has_id(&h, "workspace.versions.row.v0"), "{theme:?}: scrolled past");
        let folds = |h: &mut Harness<'static, Shell>, id: &str| {
            let at = rect_of(h, id).center();
            click(h, at);
        };
        folds(&mut h, "workspace.versions.history");
        assert!(!has_id(&h, "workspace.versions.row.v29"), "{theme:?}: History folds");
        folds(&mut h, "workspace.versions.history");
        assert!(has_id(&h, "workspace.versions.row.v29"), "{theme:?}: and opens");
        let no_changes = |h: &Harness<'static, Shell>| painted(h).iter().any(|t| t == "No changes since the last version.");
        assert!(no_changes(&h), "{theme:?}");
        folds(&mut h, "workspace.versions.working");
        assert!(!no_changes(&h), "{theme:?}: Working changes folds");
        h.state_mut().state.explorer_width = 240.0;
        h.set_size(egui::vec2(760.0, 860.0));
        h.run_steps(3);
        let shown = ids(&h).into_iter().find(|i| i.starts_with("workspace.versions.row.")).expect("rows in the narrowest column");
        assert!(rect_of(&h, &shown).right() <= 52.0 + 240.0 + 8.0, "{theme:?}: the narrowest column");
        h.state_mut().versions.working = 5;
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(!has_id(&h, badge), "{theme:?}: the count ended with the session");
    });
}

/// Frames until the comparison on another thread has answered, at most a few seconds.
fn compared(h: &mut Harness<'static, Shell>) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    h.run_steps(2);
    while h.state().versions.comparing.is_some() {
        assert!(std::time::Instant::now() < until, "the comparison answered");
        std::thread::sleep(std::time::Duration::from_millis(5));
        h.run_steps(1);
    }
    h.run_steps(2);
}

/// A project with one version, the rows of now, its panel read and compared: no changes yet.
fn changes_panel(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    versions_listed(theme, vec![version_row("c1", Some("First"), (None, false), "2026-11-08T15:30:00Z")])
}

/// Whether the panel says nothing has changed.
fn no_changes_said(h: &Harness<'static, Shell>) -> bool {
    painted(h).iter().any(|t| t == "No changes since the last version.")
}

/// An element that changes table has two rows, and each is a control of its own.
#[test]
fn two_rows_of_one_element_have_ids_of_their_own() {
    in_both_appearances(|theme| {
        let (mut h, _host) = changes_panel(theme);
        add_schema(&mut h, "crm");
        compared(&mut h);
        let (row, key) = (h.state().versions.changes[0].clone(), h.state().versions.compared.clone().unwrap());
        let moved = archforge_notations::differences::Difference { class: archforge_mof::toy::CLASS, ..row.clone() };
        let id = |d: &archforge_notations::differences::Difference| format!("workspace.versions.change.{}.{}", d.id, d.class.0);
        let wanted = [id(&row), id(&moved)];
        crate::versions::settle_changes(h.state_mut(), key, Ok(vec![row, moved]));
        h.run_steps(2);
        assert!(wanted.iter().all(|i| has_id(&h, i)), "{theme:?}: {:?}", ids(&h));
        assert_ne!(wanted[0], wanted[1], "{theme:?}");
    });
}

/// What differs from the last version lists by group with a count and fills the rail's badge; a
/// save's own tables list nothing and are not called no changes; a change the model undoes is.
#[test]
fn working_changes_list_what_a_save_would_write() {
    let row_count = |h: &Harness<'static, Shell>| ids(h).iter().filter(|i| i.starts_with("workspace.versions.change.")).count();
    in_both_appearances(|theme| {
        let (mut h, _host) = changes_panel(theme);
        assert!(no_changes_said(&h) && h.state().versions.working == 0, "{theme:?}: {:?}", painted(&h));
        let (_tx, rx) = std::sync::mpsc::channel();
        let key = h.state().versions.compared.clone().unwrap();
        h.state_mut().versions.comparing = Some((key, rx));
        h.run_steps(2);
        assert!(!crate::versions::settled(h.state()) && !no_changes_said(&h), "{theme:?}: not before it has said");
        h.state_mut().versions.comparing = None;
        add_schema(&mut h, "crm");
        compared(&mut h);
        assert_eq!((row_count(&h), h.state().versions.working), (1, 1), "{theme:?}");
        let texts = painted(&h);
        assert!(!no_changes_said(&h) && ["MODEL ELEMENTS", "crm", "A", "1"].iter().all(|t| texts.iter().any(|x| x == t)), "{theme:?}: {texts:?}");
        assert!(h.state_mut().session.as_mut().unwrap().execute("edit.undo", &serde_json::Value::Null).is_ok());
        compared(&mut h);
        assert_eq!(row_count(&h), 0, "{theme:?}: undone");
        h.state_mut().session.as_mut().unwrap().mark_unsaved();
        h.run_steps(2);
        assert!(!no_changes_said(&h) && row_count(&h) == 0, "{theme:?}: only a save's own tables differ");
    });
}

/// A skill's text typed but not yet kept is a change, as a description is: Working changes never
/// says there are none, Save version… says why it is off, and the status bar and the chip call the
/// work unsaved; once the typing pauses the text is kept and listed.
#[test]
fn skill_text_waiting_to_be_kept_is_no_no_changes() {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        let made = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "name": "DDL", "body": "Old." })).unwrap();
        let skill = made["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap();
        h.state_mut().session.as_mut().unwrap().mark_saved().unwrap();
        versions_answer(
            &mut h,
            &host,
            Ok(crate::VersionsPage { versions: vec![version_row("c1", None, (None, false), "2026-11-08T15:30:00Z")], more: false }),
        );
        h.run_steps(2);
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        crate::skill_tab::open(h.state_mut(), skill);
        compared(&mut h);
        let unsaved = |h: &Harness<'static, Shell>| painted(h).iter().any(|t| t.starts_with("Unsaved changes"));
        let off = |h: &Harness<'static, Shell>| crate::save_version::off_because(h.state());
        let none = Some("No changes since the last version.".to_string());
        assert!(no_changes_said(&h) && off(&h) == none && !unsaved(&h) && !h.state().shown_unsaved(), "{theme:?}: {:?}", painted(&h));
        // Typed just now: kept only once the typing pauses.
        let doc = h.state_mut().canvas.skills.iter_mut().find(|d| d.skill == skill).unwrap();
        (doc.text, doc.changed_at) = ("New.".to_string(), f64::MAX);
        h.run_steps(2);
        assert!(crate::skill_tab::waiting(h.state()) && h.state().versions.working == 0, "{theme:?}: still waiting");
        assert!(!no_changes_said(&h), "{theme:?}: {:?}", painted(&h));
        assert!(off(&h).is_some() && off(&h) != none && off(&h).as_deref() != Some("Loading"), "{theme:?}: {:?}", off(&h));
        assert!(unsaved(&h), "{theme:?}: the status bar: {:?}", painted(&h));
        assert!(h.state().shown_unsaved(), "{theme:?}: the chip and the close question");
        h.state_mut().canvas.skills.iter_mut().find(|d| d.skill == skill).unwrap().changed_at = 0.0;
        compared(&mut h);
        assert!(!crate::skill_tab::waiting(h.state()) && h.state().versions.working == 1 && off(&h).is_none(), "{theme:?}: kept and listed");
    });
}

/// A skill's text the store refused is unsaved work, in the tab shown and in a parked one, though
/// the model has nothing unsaved: Save still asks the host, and quitting asks about it instead of
/// closing the window.
#[test]
fn a_refused_skill_text_is_unsaved_shown_or_parked() {
    for parked in [false, true] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        let made = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "name": "DDL", "body": "Old." })).unwrap();
        let skill = made["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap();
        crate::skill_tab::open(h.state_mut(), skill);
        h.state_mut().session.as_mut().unwrap().mark_saved().unwrap();
        h.run_steps(2);
        let first = h.state().active.unwrap();
        assert!(!h.state().tab_unsaved(first), "{parked}: nothing unsaved yet");
        let doc = h.state_mut().canvas.skills.iter_mut().find(|d| d.skill == skill).unwrap();
        (doc.text, doc.changed_at) = ("x".repeat(16_001), f64::MIN);
        h.run_steps(2);
        let doc = h.state().canvas.skills.iter().find(|d| d.skill == skill).unwrap();
        assert!(doc.refused.is_some() && !h.state().session.as_ref().unwrap().is_dirty(), "{parked}: refused, the model untouched");
        if parked {
            h.state_mut().open_project(crate::Opened { name: "Library".into(), path: "/tmp/Library.archforge".into(), ..Default::default() });
            h.run_steps(3);
            assert!(h.state().active != Some(first) && h.state().tabs[first].parked.is_some(), "parked");
        }
        assert!(h.state().tab_unsaved(first), "{parked}: the refused text is unsaved");
        if !parked {
            // Something to save, as the status bar says, so Save still goes to the host (AR-413).
            h.state_mut().save();
            assert_eq!(host.saves.lock().unwrap().len(), 1, "Save asks the host");
            host.saves.lock().unwrap().pop().unwrap().send(Ok(false)).unwrap();
            h.run_steps(2);
        }
        h.state_mut().request_quit();
        h.run_steps(3);
        assert!(!h.state().quit_confirmed(), "{parked}: the window stays");
        assert_eq!(h.state().ask, asked_unsaved(COMMERCE), "{parked}: quitting asks about it");
    }
}

/// The window title marks typed text the work doesn't have yet as its tab does, though the model
/// has nothing unsaved: a description being typed, a skill's text before its typing pauses, and one
/// the store refused.
#[test]
fn the_title_marks_text_typed_and_not_yet_kept() {
    in_both_appearances(|theme| {
        let (mut h, _host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        let made = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "name": "DDL", "body": "Old." })).unwrap();
        let skill = made["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap();
        crate::skill_tab::open(h.state_mut(), skill);
        h.state_mut().session.as_mut().unwrap().mark_saved().unwrap();
        h.run_steps(2);
        let (clean, marked) = ("Commerce platform — Archforge", "• Commerce platform — Archforge");
        assert_eq!(h.state().title(), clean, "{theme:?}: nothing unsaved");
        h.state_mut().panels.description = Some(crate::panels::DescriptionDraft { target: skill, text: "Typing".into(), refused: false });
        assert_eq!(h.state().title(), marked, "{theme:?}: a description being typed");
        h.state_mut().panels.description = None;
        let doc = h.state_mut().canvas.skills.iter_mut().find(|d| d.skill == skill).unwrap();
        (doc.text, doc.changed_at) = ("New.".to_string(), f64::MAX);
        h.run_steps(2);
        assert_eq!(h.state().title(), marked, "{theme:?}: a skill's text before the pause");
        let doc = h.state_mut().canvas.skills.iter_mut().find(|d| d.skill == skill).unwrap();
        (doc.text, doc.changed_at) = ("x".repeat(16_001), f64::MIN);
        h.run_steps(2);
        let refused = h.state().canvas.skills.iter().any(|d| d.skill == skill && d.refused.is_some());
        assert!(refused && !h.state().session.as_ref().unwrap().is_dirty(), "{theme:?}: refused, the model untouched");
        assert_eq!(h.state().title(), marked, "{theme:?}: a refused skill text");
    });
}

/// Past a dozen changes a filter narrows the list by name and the count stays that of them all.
#[test]
fn working_changes_filter_a_long_list() {
    use egui_kittest::kittest::Queryable;
    in_both_appearances(|theme| {
        let (mut h, _host) = changes_panel(theme);
        // Named with an accent, which the filter typed without one still finds.
        (0..13).for_each(|i| add_schema(&mut h, &format!("Schéma{i}")));
        compared(&mut h);
        assert!(has_id(&h, "workspace.versions.filter") && h.state().versions.working == 13, "{theme:?}");
        h.state_mut().versions.filter = "schema7".to_string();
        h.run_steps(2);
        let shown = |h: &Harness<'static, Shell>| ids(h).iter().filter(|i| i.starts_with("workspace.versions.change.")).count();
        assert_eq!((shown(&h), h.state().versions.working), (1, 13), "{theme:?}");
        // Capitals and full-width letters count for nothing either.
        h.state_mut().versions.filter = "SCHEMA７".to_string();
        h.run_steps(2);
        assert_eq!(shown(&h), 1, "{theme:?}");
        use archforge_notations::differences::{Area, ChangeKind, Difference};
        let item = |n: u8, area, kind| Difference {
            id: archforge_mof::Uuid([n; 16]),
            class: archforge_core::SKILL,
            area,
            kind,
            name: Some(format!("item{n}")),
            path: vec![],
        };
        let found = vec![item(1, Area::Model, ChangeKind::Changed), item(2, Area::Diagram, ChangeKind::Removed)];
        let (current, (tx, rx)) = (h.state().versions.compared.clone().expect("compared"), std::sync::mpsc::channel());
        h.state_mut().versions.comparing = Some((current, rx));
        tx.send(Ok(found)).unwrap();
        h.run_steps(1);
        assert!(h.query_by_label("Versions · 2 changes").is_some() && painted(&h).iter().any(|t| t == "2"), "{theme:?}");
        h.run_steps(1);
        let texts = painted(&h);
        assert!(["MODEL ELEMENTS", "DIAGRAMS", "item1", "item2", "C", "R"].iter().all(|t| texts.iter().any(|x| x == t)), "{theme:?}: {texts:?}");
        h.state_mut().versions.filter = "nothing has this".to_string();
        h.run_steps(2);
        let rows = ids(&h).iter().filter(|i| i.starts_with("workspace.versions.change.")).count();
        assert_eq!(rows, 2, "{theme:?}: a filter left from a longer list hides nothing, as nothing could clear it");
        h.state_mut().versions.filter = "gone with the session".to_string();
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(h.state().versions.filter.is_empty(), "{theme:?}");
    });
}

/// A skill changed since the version is a group of its own, at the path Skills, and the Save dialog
/// counts it as a skill after model elements and diagrams, in one, two or three parts.
#[test]
fn a_skill_is_listed_and_counted_on_its_own() {
    use archforge_notations::differences::{Area, ChangeKind, Difference};
    use egui_kittest::kittest::Queryable;
    let summary = |h: &Harness<'static, Shell>| painted(h).into_iter().find(|t| t.contains("changed since")).unwrap_or_default();
    in_both_appearances(|theme| {
        let (mut h, _host) = changes_panel(theme);
        h.state_mut().session.as_mut().unwrap().execute("skill.add", &serde_json::json!({ "name": "Naming", "body": "Singular." })).unwrap();
        compared(&mut h);
        let areas: Vec<_> = h.state().versions.changes.iter().map(|d| (d.class, d.area)).collect();
        assert_eq!(areas, [(archforge_core::SKILL, Area::Skill)], "{theme:?}");
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "SKILLS") && !texts.iter().any(|t| t == "MODEL ELEMENTS"), "{theme:?}: {texts:?}");
        assert!(h.query_by_label("Naming, Skills, Added").is_some(), "{theme:?}: the row's path is Skills");
        let row = h.state().versions.changes[0].clone();
        click_id(&mut h, &format!("workspace.versions.change.{}.{}", row.id, row.class.0));
        // Its card is read as an element's, and the centre says Skills too.
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !painted(&h).iter().any(|t| t == "Singular.") {
            assert!(h.state().versions.element.is_some() && std::time::Instant::now() < until, "{theme:?}: the card: {:?}", painted(&h));
            std::thread::sleep(std::time::Duration::from_millis(5));
            h.run_steps(1);
        }
        let said = ["Naming, Skills, Added, selected", "Naming, Skills, Added"].map(|l| h.query_all_by_label(l).count());
        assert_eq!(said, [1, 1], "{theme:?}: the row, selected, and the centre's title");
        click_id(&mut h, "workspace.versions.save");
        assert!(summary(&h).starts_with("1 skill changed since"), "{theme:?}: {}", summary(&h));
        click_id(&mut h, "workspace.versions.dialog.cancel");
        add_schema(&mut h, "crm");
        compared(&mut h);
        click_id(&mut h, "workspace.versions.save");
        assert!(summary(&h).starts_with("1 model element and 1 skill changed since"), "{theme:?}: {}", summary(&h));
        click_id(&mut h, "workspace.versions.dialog.cancel");
        let item = |n: u8, area| Difference {
            id: archforge_mof::Uuid([n; 16]),
            class: archforge_core::SKILL,
            area,
            kind: ChangeKind::Changed,
            name: Some(format!("item{n}")),
            path: vec![],
        };
        let found = [Area::Skill, Area::Model, Area::Diagram, Area::Model, Area::Model].into_iter().zip(1..).map(|(a, n)| item(n, a)).collect();
        let key_now = h.state().versions.compared.clone().expect("compared");
        crate::versions::settle_changes(h.state_mut(), key_now, Ok(found));
        h.run_steps(2);
        click_id(&mut h, "workspace.versions.save");
        assert!(summary(&h).starts_with("3 model elements, 1 diagram and 1 skill changed since"), "{theme:?}: {}", summary(&h));
    });
}

/// Presses `key` and looks at every frame from the press on until `done`, at most 200; fails
/// saying `what` when it never is, however late another thread answers meanwhile. The frame the key
/// goes down in is looked at on its own: what the press starts there (an opened element's read) can
/// answer by the next frame, the key's release, so a look after both is late on a loaded machine.
fn pressed_until(h: &mut Harness<'static, Shell>, key: egui::Key, done: impl Fn(&Harness<'static, Shell>) -> bool, what: &str) {
    h.key_down(key);
    h.step();
    let pressed = done(h);
    h.key_up(key);
    for _ in 0..200 {
        h.run_steps(1);
        if pressed || done(h) {
            return;
        }
    }
    panic!("{what}: not within 200 frames");
}

/// ↑ and ↓ on a focused change move between the rows of all three groups, out of view too, each whole
/// in the list as it comes, the first bringing its group's heading back; Enter opens the one reached.
#[test]
fn the_keys_reach_every_working_change() {
    use archforge_notations::differences::{Area, ChangeKind, Difference};
    let item = |n: u8| Difference {
        id: archforge_mof::Uuid([n; 16]),
        class: archforge_core::SKILL,
        area: [Area::Model, Area::Diagram, Area::Skill][usize::from(n / 7)],
        kind: ChangeKind::Changed,
        name: Some(format!("item{n}")),
        path: vec![],
    };
    let id = |n: u8| format!("workspace.versions.change.{}.{}", item(n).id, item(n).class.0);
    in_both_appearances(|theme| {
        let (mut h, _host) = changes_panel(theme);
        let key_now = h.state().versions.compared.clone().expect("compared");
        crate::versions::settle_changes(h.state_mut(), key_now, Ok((0..20).map(item).collect()));
        h.run_steps(2);
        assert!(!has_id(&h, &id(19)), "{theme:?}: the last change starts out of view");
        let (field, history) = (rect_of(&h, "workspace.versions.filter").bottom(), rect_of(&h, "workspace.versions.history").top());
        let whole = |h: &Harness<'static, Shell>, n: u8| {
            let row = rect_of(h, &id(n));
            h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(&id(n)))) && row.top() >= field && row.bottom() <= history - 8.0
        };
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(&id(0))));
        h.run_steps(1);
        for n in 1..20 {
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
            assert!(whole(&h, n), "{theme:?}: ↓ reached change {n}");
        }
        for n in (0..19).rev() {
            key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
            assert!(whole(&h, n), "{theme:?}: ↑ reached change {n}");
        }
        let heading = text_boxes(&h).into_iter().find(|(t, _)| t == "MODEL ELEMENTS").map(|(_, b)| b.top());
        assert!(heading.is_some_and(|top| top >= field), "{theme:?}: the heading came back with the first row: {heading:?}");
        (0..16).for_each(|_| key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE));
        // The element's own read finds nothing of these made-up rows and closes it again, so each
        // frame from the press on is looked at, not the state some frames later.
        let opened = |h: &Harness<'static, Shell>| h.state().versions.element.as_ref().map(|o| o.row.id) == Some(item(16).id);
        pressed_until(&mut h, egui::Key::Enter, opened, &format!("{theme:?}: Enter opened the skill reached"));
    });
}

/// An answer for rows that have moved on shows for what it read, a version in view is not compared,
/// a change of session lets go of what was found, and rows that cannot be read say so, not "no changes".
#[test]
fn working_changes_belong_to_the_rows_compared() {
    in_both_appearances(|theme| {
        let (mut h, host) = changes_panel(theme);
        let revision = h.state().session.as_ref().unwrap().revision();
        add_schema(&mut h, "crm");
        compared(&mut h);
        let (tx, rx) = std::sync::mpsc::channel();
        h.state_mut().versions.comparing = Some(((Some("c1".to_string()), revision), rx));
        h.state_mut().session.as_mut().unwrap().execute("sketch.package.add", &serde_json::json!({ "name": "more" })).unwrap();
        h.run_steps(2);
        assert_eq!(h.state().versions.comparing.as_ref().map(|(key, _)| key.1), Some(revision), "{theme:?}: one at a time");
        tx.send(Ok(Vec::new())).unwrap();
        h.run_steps(1);
        assert_eq!(h.state().versions.working, 0, "{theme:?}: what that answer read, shown until the newest rows are compared");
        compared(&mut h);
        assert_eq!(h.state().versions.working, 2, "{theme:?}: the newest rows compared");
        h.state_mut().session.as_mut().unwrap().begin_interaction("sketch.package.add").unwrap();
        h.state_mut().versions.compared = Some((Some("older".to_string()), 0));
        h.run_steps(2);
        assert!(h.state().versions.comparing.is_none() && !crate::versions::settled(h.state()), "{theme:?}: not during a gesture");
        h.state_mut().session.as_mut().unwrap().cancel().unwrap();
        start_viewing(&mut h, "c1");
        let earlier = rows_now(&h);
        answer_read(&mut h, &host, Ok(earlier));
        versions_answer(&mut h, &host, Ok(two_versions()));
        let v = &h.state().versions;
        assert!(v.comparing.is_none() && v.working == 0 && host.reads.lock().unwrap().is_empty(), "{theme:?}: a version in view is not current work");
        assert!(!painted(&h).iter().any(|t| t == "Loading"), "{theme:?}: nor waited for");
        let (mut h, host) = versions_panel(theme);
        let last = version_row("c1", None, (None, false), "2026-11-08T15:30:00Z");
        versions_answer(&mut h, &host, Ok(crate::VersionsPage { versions: vec![last], more: false }));
        h.run_steps(2);
        answer_read(&mut h, &host, Err("unreadable".to_string().into()));
        h.run_steps(2);
        assert!(has_id(&h, "workspace.versions.changes.failed") && !no_changes_said(&h), "{theme:?}");
        assert!(host.reads.lock().unwrap().is_empty(), "{theme:?}: not asked again every frame");
    });
}

/// An agent's edit, through the road the control channel takes: answered in the window's own frame.
fn agent_adds(h: &mut Harness<'static, Shell>, name: &str) {
    let add = accepted(h, "engine.execute", serde_json::json!({ "command": "sketch.package.add", "params": { "name": name } }), false);
    assert!(crate::control::answer_request(h.state_mut(), &add).is_ok(), "{name}");
}

/// The session's revision now, and which rows the comparison on its way is of.
fn revisions(h: &Harness<'static, Shell>) -> (u64, Option<u64>) {
    (h.state().session.as_ref().unwrap().revision(), h.state().versions.comparing.as_ref().map(|(key, _)| key.1))
}

/// A difference that no real comparison of these rows could find.
fn found(n: u8) -> archforge_notations::differences::Difference {
    use archforge_notations::differences::{Area, ChangeKind, Difference};
    Difference {
        id: archforge_mof::Uuid([n; 16]),
        class: archforge_core::SKILL,
        area: Area::Model,
        kind: ChangeKind::Changed,
        name: None,
        path: vec![],
    }
}

/// While an agent keeps editing, the list still follows: an answer for rows that have moved on
/// lands for what it read and is not called current, the newest rows are compared next, and one
/// comparison runs at a time. A failure for moved rows says nothing, and an answer against a version
/// that is no longer the last is dropped.
#[test]
fn working_changes_keep_up_while_an_agent_edits() {
    in_both_appearances(|theme| {
        let (mut h, _host) = changes_panel(theme);
        let ((first, _), (tx, rx)) = (revisions(&h), std::sync::mpsc::channel());
        h.state_mut().versions.comparing = Some(((Some("c1".to_string()), first), rx));
        agent_adds(&mut h, "crm");
        h.run_steps(2);
        assert_eq!(revisions(&h).1, Some(first), "{theme:?}: one at a time, whatever the agent does");
        tx.send(Ok(vec![found(1), found(2), found(3)])).unwrap();
        h.run_steps(1);
        assert_eq!(h.state().versions.working, 3, "{theme:?}: what it read lands, though the rows moved on");
        assert_eq!(h.state().versions.compared, Some((Some("c1".to_string()), first)), "{theme:?}: pinned to the rows it read");
        assert!(!crate::versions::settled(h.state()), "{theme:?}: and it is not called current");
        let (now, comparing) = revisions(&h);
        assert_eq!(comparing, Some(now), "{theme:?}: the newest rows are next, in that same frame");
        assert_ne!(now, first, "{theme:?}");
        compared(&mut h);
        assert!(h.state().versions.working == 1 && crate::versions::settled(h.state()), "{theme:?}: crm");

        let (behind, (tx, rx)) = (revisions(&h).0, std::sync::mpsc::channel());
        h.state_mut().versions.comparing = Some(((Some("c1".to_string()), behind), rx));
        agent_adds(&mut h, "more");
        tx.send(Err("the comparison stopped".to_string())).unwrap();
        h.run_steps(1);
        assert!(!has_id(&h, "workspace.versions.changes.failed"), "{theme:?}: a failure for rows that moved on says nothing of the newest");
        compared(&mut h);
        assert!(h.state().versions.working == 2 && crate::versions::settled(h.state()), "{theme:?}: crm and more");

        let (now, (tx, rx)) = (revisions(&h).0, std::sync::mpsc::channel());
        h.state_mut().versions.comparing = Some(((Some("older".to_string()), now), rx));
        tx.send(Ok(vec![found(9)])).unwrap();
        h.run_steps(1);
        assert_eq!(h.state().versions.working, 2, "{theme:?}: against a version no longer the last it says nothing");
    });
}

/// A page with `newest` over the version the panel was first read with.
fn page_with(newest: &str) -> crate::VersionsPage {
    let row = |hash: &str| version_row(hash, None, (None, false), "2026-11-08T15:30:00Z");
    crate::VersionsPage { versions: vec![row(newest), row("c1")], more: false }
}

/// Makes the Save that follows an edit, and has it answered.
fn saved(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    h.state_mut().save();
    host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
}

/// Ends a comparison of the last version `last` and rows `revision` with three changes that no real
/// comparison could find, for the test to see whether they land.
fn comparison_ends(h: &mut Harness<'static, Shell>, last: &str, revision: u64) {
    let (tx, rx) = std::sync::mpsc::channel();
    h.state_mut().versions.comparing = Some(((Some(last.to_string()), revision), rx));
    tx.send(Ok(vec![found(1), found(2), found(3)])).unwrap();
    h.run_steps(1);
}

/// Until a read begun after a Save has answered, the listed last version may be the one before it:
/// a comparison against it does not land and none starts. Then the new last version is compared.
#[test]
fn working_changes_wait_for_the_version_a_save_made() {
    in_both_appearances(|theme| {
        let (mut h, host) = changes_panel(theme);
        agent_adds(&mut h, "crm");
        compared(&mut h);
        let (behind, shown) = (revisions(&h).0, h.state().versions.compared.clone());
        agent_adds(&mut h, "more");
        saved(&mut h, &host);
        h.run_steps(2);
        assert!(h.state().versions.comparing.is_none(), "{theme:?}: none starts against the page kept meanwhile");
        comparison_ends(&mut h, "c1", behind);
        assert_eq!((h.state().versions.working, &h.state().versions.compared), (1, &shown), "{theme:?}: nor does one land");
        assert!(!crate::versions::settled(h.state()), "{theme:?}");
        versions_answer(&mut h, &host, Ok(page_with("c2")));
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        compared(&mut h);
        let v = &h.state().versions;
        assert_eq!(v.compared.as_ref().map(|key| key.0.as_deref()), Some(Some("c2")), "{theme:?}");
        assert!(v.working == 0 && crate::versions::settled(h.state()), "{theme:?}: what was saved is no working change");
    });
}

/// A read for a Save's version that fails leaves no count from the version before: the list says
/// the changes could not be compared, not "no changes", and Retry brings it back.
#[test]
fn working_changes_say_so_when_the_read_after_a_save_fails() {
    in_both_appearances(|theme| {
        let (mut h, host) = changes_panel(theme);
        agent_adds(&mut h, "crm");
        compared(&mut h);
        saved(&mut h, &host);
        versions_answer(&mut h, &host, Err("the server stopped".to_string()));
        h.run_steps(1);
        let failed = "workspace.versions.changes.failed";
        assert!(has_id(&h, failed) && !no_changes_said(&h) && h.state().versions.working == 0, "{theme:?}: {:?}", painted(&h));
        assert!(h.state().versions.compared.is_none() && !crate::versions::settled(h.state()), "{theme:?}");
        let off = crate::save_version::off_because(h.state());
        assert_eq!(off, Some(crate::i18n::tr("The changes couldn't be compared.")), "{theme:?}: Save version says why it is off, not Loading");
        let now = revisions(&h).0;
        comparison_ends(&mut h, "c1", now);
        assert_eq!(h.state().versions.working, 0, "{theme:?}: the kept page is no last version to count against");
        click_id(&mut h, "workspace.versions.retry");
        versions_answer(&mut h, &host, Ok(page_with("c2")));
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        compared(&mut h);
        assert!(!has_id(&h, failed) && h.state().versions.working == 0 && crate::versions::settled(h.state()), "{theme:?}");
    });
}

/// A read begun before a Save ended may not list its version, however it answers: it is read again.
#[test]
fn a_read_from_before_a_save_ended_is_not_the_last_word() {
    in_both_appearances(|theme| {
        let (mut h, host) = changes_panel(theme);
        h.state_mut().versions.stale = true;
        h.run_steps(2);
        h.state_mut().versions.stale = true;
        let reply = host.version_reads.lock().unwrap().pop().expect("a read was asked for");
        reply.send(Ok(page_with("c1"))).unwrap();
        h.run_steps(1);
        assert!(!crate::versions::settled(h.state()), "{theme:?}");
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "{theme:?}: read again at once");
        versions_answer(&mut h, &host, Ok(page_with("c1")));
        assert!(crate::versions::settled(h.state()), "{theme:?}: a save that made no version leaves the comparison good");
    });
}

/// A restore that does not replace the session may still have kept the work as a version, or
/// not say whether it did: each such ending reads the versions again.
#[test]
fn a_restore_that_ends_without_replacing_reads_the_versions_again() {
    use crate::replace::JobFailure;
    type Ending = fn(RestoreEnds);
    let endings: [Ending; 3] = [
        |(answer, _)| answer.send(Err(JobFailure::WorkSaved("told".into()))).unwrap(),
        |(answer, reading)| {
            answer.send(Err(JobFailure::Unclear)).unwrap();
            reading.send(Ok(false)).unwrap();
        },
        |(answer, reading)| {
            answer.send(Err(JobFailure::Unclear)).unwrap();
            drop(reading);
        },
    ];
    for (ending, theme) in endings.into_iter().flat_map(|e| [(e, ThemeKind::Light), (e, ThemeKind::Dark)]) {
        let (mut h, host, earlier) = work_and_earlier();
        h.state_mut().state.theme = theme;
        versions_answer(&mut h, &host, Ok(page_with("c1")));
        // The last version's own rows, so the oldest read waiting is the restore's.
        answer_read(&mut h, &host, Ok(earlier.clone()));
        ending(restore_under_way(&mut h, &host, earlier));
        h.run_steps(6);
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "{theme:?}: {:?}", h.state().save_error);
    }
}

/// Before any version everything is added but the project's own root.
#[test]
fn before_the_first_version_everything_is_added_but_the_root() {
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        add_schema(&mut h, "crm");
        versions_answer(&mut h, &host, Ok(crate::VersionsPage::default()));
        compared(&mut h);
        assert_eq!(h.state().versions.working, 1, "{theme:?}: the schema");
    });
}

/// Versions `from..to`, those with even numbers described.
fn long_page(from: usize, to: usize, more: bool) -> crate::VersionsPage {
    let entry = |i: usize| version_row(&format!("v{i}"), i.is_multiple_of(2).then_some("Described"), (None, false), "2026-11-08T15:30:00Z");
    crate::VersionsPage { versions: (from..to).map(entry).collect(), more }
}

/// The first page of a long history, read.
fn long_history(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = versions_panel(theme);
    versions_answer(&mut h, &host, Ok(long_page(0, 50, true)));
    (h, host)
}

/// Clicks the control `id`.
fn click_id(h: &mut Harness<'static, Shell>, id: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
}

/// A long history comes a page at a time; Show more asks for what follows the last row, once,
/// and adds it; a list that ends says so by offering no more; a short one has no filter.
#[test]
fn show_more_adds_the_next_page() {
    in_both_appearances(|theme| {
        let (mut h, host) = long_history(theme);
        let (more, all) = ("workspace.versions.more", "workspace.versions.all");
        assert!(has_id(&h, all) && painted(&h).iter().any(|t| t == "50+"), "{theme:?}: {:?}", painted(&h));
        h.state_mut().versions.selected = Some("v0".to_string());
        scrolled_to_end(&mut h);
        assert!(has_id(&h, "workspace.versions.row.v49"), "{theme:?}");
        click_id(&mut h, more);
        assert!(h.ctx.has_requested_repaint(), "{theme:?}: the window looks again for it");
        assert_eq!(host.version_asks.lock().unwrap().last().unwrap().as_deref(), Some("v49"), "{theme:?}");
        click_id(&mut h, more);
        assert_eq!(host.version_reads.lock().unwrap().len(), 1, "{theme:?}: one at a time");
        versions_answer(&mut h, &host, Ok(long_page(50, 60, false)));
        assert!(painted(&h).iter().any(|t| t == "60") && !has_id(&h, more), "{theme:?}: {:?}", painted(&h));
        assert_eq!(h.state().versions.selected.as_deref(), Some("v0"), "{theme:?}: the selection held as the list grew");
        let (mut h, host) = versions_panel(theme);
        versions_answer(&mut h, &host, Ok(two_versions()));
        assert!(!has_id(&h, all) && !has_id(&h, more), "{theme:?}: a short list");
    });
}

/// A next page on its way when the list is read again, or the project's session changes, has
/// nowhere to land; a failed one keeps the list and Retry asks for that page only.
#[test]
fn a_next_page_belongs_to_the_list_it_was_asked_for() {
    in_both_appearances(|theme| {
        let (mut h, host) = long_history(theme);
        let more_id = "workspace.versions.more";
        for change in [|s: &mut Shell| s.versions.stale = true, |s: &mut Shell| s.session_replaced()] {
            click_id_when_shown(&mut h, more_id);
            let next = host.version_reads.lock().unwrap().pop().unwrap();
            change(h.state_mut());
            h.run_steps(2);
            assert!(next.send(Ok(long_page(50, 60, false))).is_err(), "{theme:?}: its answer has nowhere to land");
            versions_answer(&mut h, &host, Ok(long_page(0, 50, true)));
            assert!(painted(&h).iter().any(|t| t == "50+"), "{theme:?}");
        }
        click_id_when_shown(&mut h, more_id);
        versions_answer(&mut h, &host, Err("the server stopped".to_string()));
        assert!(painted(&h).iter().any(|t| t == "50+") && !has_id(&h, "workspace.versions.failed"), "{theme:?}: the list stays");
        scrolled_to_end(&mut h);
        click_id(&mut h, "workspace.versions.more.retry");
        assert_eq!(host.version_asks.lock().unwrap().last().unwrap().as_deref(), Some("v49"), "{theme:?}: that page only");
        versions_answer(&mut h, &host, Ok(long_page(50, 52, true)));
        assert!(painted(&h).iter().any(|t| t == "52+"), "{theme:?}");
        click_id_when_shown(&mut h, more_id);
        let next = host.version_reads.lock().unwrap().pop().unwrap();
        h.state_mut().services.projects = None;
        h.state_mut().session_replaced();
        h.run_steps(2);
        assert!(next.send(Ok(long_page(60, 61, false))).is_err(), "{theme:?}: with no host to read again too");
    });
}

/// ↑ and ↓ on a focused version move between the rows, out of view too and past the divider over
/// what is shared, each whole in the list as it comes; Enter opens the one reached. Behind a dialog
/// the keys stay the dialog's.
#[test]
fn the_keys_reach_every_history_row() {
    let row = |i: usize| format!("workspace.versions.row.v{i}");
    in_both_appearances(|theme| {
        let (mut h, host) = versions_panel(theme);
        let mut page = long_page(0, 50, true);
        page.versions.iter_mut().skip(20).for_each(|v| v.shared = true);
        versions_answer(&mut h, &host, Ok(page));
        assert!(!has_id(&h, &row(30)), "{theme:?}: out of view");
        let (top, bottom) = (rect_of(&h, "workspace.versions.all").bottom(), rect_of(&h, "workspace.explorer.resize").bottom());
        let whole = |h: &Harness<'static, Shell>, i: usize| {
            let at = rect_of(h, &row(i));
            h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(&row(i)))) && at.top() >= top && at.bottom() <= bottom - 8.0
        };
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(&row(0))));
        h.run_steps(1);
        for i in 1..=30 {
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
            assert!(whole(&h, i), "{theme:?}: ↓ reached v{i}");
        }
        assert!(rect_of(&h, "workspace.versions.shared").bottom() <= rect_of(&h, &row(20)).top(), "{theme:?}: a divider over v20");
        // Behind About, ↓ on the last row in view would scroll the list; egui lets go of a focus under
        // a modal layer, and the shell holds the keys back too.
        let before = rect_of(&h, &row(30));
        crate::support::open(h.state_mut());
        h.run_steps(1);
        assert!(h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(&row(30)))), "{theme:?}: the row has the focus behind About");
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        assert!(has_id(&h, &row(30)) && rect_of(&h, &row(30)) == before, "{theme:?}: behind About the keys are the dialog's");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.run_steps(2);
        assert!(h.state().about.is_none() && rect_of(&h, &row(30)) == before, "{theme:?}: About closed");
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(&row(30))));
        h.run_steps(1);
        for i in (5..30).rev() {
            key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
            assert!(whole(&h, i), "{theme:?}: ↑ reached v{i}");
        }
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(&row(5))));
        h.run_steps(1);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        assert!(whole(&h, 6), "{theme:?}: and with it gone they move again");
        let opened = |h: &Harness<'static, Shell>| h.state().versions.selected.as_deref() == Some("v6");
        pressed_until(&mut h, egui::Key::Enter, opened, &format!("{theme:?}: Enter opened the one reached"));
    });
}

/// Scrolls the list to its end.
fn scrolled_to_end(h: &mut Harness<'static, Shell>) {
    let at = rect_of(h, "workspace.versions.history").center() + egui::vec2(0.0, 200.0);
    h.event(egui::Event::PointerMoved(at));
    for _ in 0..12 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -1200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(2);
    }
}

/// Scrolls to Show more and presses it.
fn click_id_when_shown(h: &mut Harness<'static, Shell>, id: &str) {
    scrolled_to_end(h);
    click_id(h, id);
}

/// A row chosen stays chosen by its hash: through a filter that hides it, without another taking
/// its place, and through a read again; a new session lets go of it and of the filter.
#[test]
fn a_selection_is_held_by_its_hash() {
    use egui_kittest::kittest::Queryable;
    in_both_appearances(|theme| {
        let (mut h, host) = long_history(theme);
        let (hidden, described) = ("workspace.versions.row.v1", "workspace.versions.described");
        click_id(&mut h, hidden);
        assert_eq!(h.state().versions.selected.as_deref(), Some("v1"), "{theme:?}");
        assert!(h.query_by_label_contains(", selected").is_some(), "{theme:?}: read out");
        click_id(&mut h, described);
        assert!(!has_id(&h, hidden) && has_id(&h, "workspace.versions.row.v0"), "{theme:?}: filtered");
        assert_eq!(h.state().versions.selected.as_deref(), Some("v1"), "{theme:?}: still chosen, nothing else is");
        h.state_mut().versions.stale = true;
        h.run_steps(2);
        versions_answer(&mut h, &host, Ok(long_page(0, 50, true)));
        assert_eq!(h.state().versions.selected.as_deref(), Some("v1"), "{theme:?}: through a read again");
        click_id(&mut h, "workspace.versions.all");
        assert!(has_id(&h, hidden) && h.query_by_label_contains(", selected").is_some(), "{theme:?}: and shown again");
        click_id(&mut h, described);
        h.state_mut().session_replaced();
        h.run_steps(2);
        versions_answer(&mut h, &host, Ok(long_page(0, 50, true)));
        assert!(h.state().versions.selected.is_none() && has_id(&h, hidden), "{theme:?}: a new session lets go of both");
        click_id(&mut h, described);
        versions_answer_none(&mut h, &host);
        assert!(painted(&h).iter().any(|t| t == "None of these versions has a description."), "{theme:?}");
    });
}

/// A read again that finds no version with a description.
fn versions_answer_none(h: &mut Harness<'static, Shell>, host: &SavingHost) {
    h.state_mut().versions.stale = true;
    let odd = (0..3).map(|i| version_row(&format!("o{i}"), None, (None, false), "2026-11-08T15:30:00Z")).collect();
    h.run_steps(2);
    versions_answer(h, host, Ok(crate::VersionsPage { versions: odd, more: true }));
}

/// A project with a change on top of its last version, Save version open on it.
fn save_dialog(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = changes_panel(theme);
    add_schema(&mut h, "crm");
    compared(&mut h);
    click_id(&mut h, "workspace.versions.save");
    assert!(dialog_open(&h), "{theme:?}: it opens");
    (h, host)
}

/// Whether the dialog is open.
fn dialog_open(h: &Harness<'static, Shell>) -> bool {
    h.state().versions.dialog.is_some()
}

/// Whether the keyboard is on the control `id`.
fn focused_on(h: &Harness<'static, Shell>, id: &str) -> bool {
    h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(id)))
}

/// Save asks for what was reviewed under what was typed, once, and needs a description; on success
/// the dialog goes and the keyboard is back on the button that opened it.
#[test]
fn save_version_saves_what_was_reviewed_once() {
    let save = "workspace.versions.dialog.save";
    in_both_appearances(|theme| {
        let (mut h, host) = save_dialog(theme);
        assert!(focused_on(&h, "workspace.versions.dialog.description"), "{theme:?}: the keyboard starts in the field");
        assert!(painted(&h).iter().any(|t| t.starts_with("1 model element changed since")), "{:?}", painted(&h));
        click_id(&mut h, save);
        assert!(host.saves.lock().unwrap().is_empty(), "{theme:?}: a description is needed");
        click_id(&mut h, "workspace.versions.dialog.description");
        h.event(egui::Event::Text("Added crm".into()));
        click_id(&mut h, save);
        click_id(&mut h, save);
        assert_eq!(host.saves.lock().unwrap().len(), 1, "{theme:?}: once");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(dialog_open(&h), "{theme:?}: a save under way is not called off");
        assert_eq!(*host.descriptions.lock().unwrap(), [Some("Added crm".to_string())], "{theme:?}");
        let rows = rows_now(&h);
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(3);
        assert!(!dialog_open(&h) && h.state().versions.focus_save, "{theme:?}: done, the keyboard waiting for the read after the save");
        add_schema(&mut h, "later");
        versions_answer(&mut h, &host, Ok(page_with("c2")));
        answer_read(&mut h, &host, Ok(rows));
        compared(&mut h);
        assert!(focused_on(&h, "workspace.versions.save"), "{theme:?}: and back on the button once it is on");
    });
}

/// With nothing left to save the button stays off, so the keyboard stops waiting for it.
#[test]
fn save_version_lets_go_of_the_keyboard_when_nothing_is_left() {
    in_both_appearances(|theme| {
        let (mut h, host) = save_dialog(theme);
        h.event(egui::Event::Text("Added crm".into()));
        h.run_steps(1);
        click_id(&mut h, "workspace.versions.dialog.save");
        let rows = rows_now(&h);
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(3);
        versions_answer(&mut h, &host, Ok(page_with("c2")));
        answer_read(&mut h, &host, Ok(rows));
        compared(&mut h);
        assert!(!dialog_open(&h) && !h.state().versions.focus_save && h.state().versions.working == 0, "{theme:?}");
    });
}

/// A model that moves while the dialog is open holds Save back until the summary is of it, and
/// says so; what is then saved is the new state.
#[test]
fn save_version_says_when_the_model_moved_and_waits_for_the_summary() {
    let (save, warning) = ("workspace.versions.dialog.save", "workspace.versions.dialog.warning");
    in_both_appearances(|theme| {
        let (mut h, host) = save_dialog(theme);
        h.event(egui::Event::Text("Two schemas".into()));
        let (_tx, rx) = std::sync::mpsc::channel();
        let key = h.state().versions.compared.clone().unwrap();
        h.state_mut().versions.comparing = Some((key, rx));
        add_schema(&mut h, "billing");
        click_id(&mut h, save);
        assert!(host.saves.lock().unwrap().is_empty() && !has_id(&h, warning), "{theme:?}: not before the summary is of the new rows");
        h.state_mut().versions.comparing = None;
        compared(&mut h);
        assert!(has_id(&h, warning) && painted(&h).iter().any(|t| t.starts_with("2 model elements changed")), "{theme:?}: {:?}", painted(&h));
        click_id(&mut h, save);
        assert_eq!(host.saves.lock().unwrap().len(), 1, "{theme:?}: now it can");
    });
}

/// Changes undone leave nothing to save and say so; a comparison that failed says that instead.
#[test]
fn a_dialog_whose_changes_are_undone_has_nothing_to_save() {
    in_both_appearances(|theme| {
        let (mut h, host) = save_dialog(theme);
        h.event(egui::Event::Text("Gone".into()));
        h.state_mut().session.as_mut().unwrap().undo().unwrap();
        compared(&mut h);
        assert!(no_changes_said(&h) && h.state().versions.working == 0, "{theme:?}: {:?}", painted(&h));
        let key = h.state().versions.compared.clone().unwrap();
        crate::versions::settle_changes(h.state_mut(), key, Err("no reply".into()));
        click_id(&mut h, "workspace.versions.dialog.save");
        assert!(host.saves.lock().unwrap().is_empty() && painted(&h).iter().filter(|t| t.starts_with("The changes c")).count() == 2, "{theme:?}");
        assert!(crate::save_version::off_because(h.state()).is_some_and(|w| w.starts_with("The changes couldn't")), "{theme:?}");
    });
}

/// Enter with no description keeps the field; with the keyboard elsewhere nothing behind the dialog acts.
#[test]
fn the_save_dialog_keeps_the_keys_from_what_is_behind_it() {
    let field = "workspace.versions.dialog.description";
    in_both_appearances(|theme| {
        let (mut h, host) = save_dialog(theme);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(dialog_open(&h) && focused_on(&h, field), "{theme:?}: Enter with no description keeps the field");
        h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id(field)));
        let revision = h.state().session.as_ref().unwrap().revision();
        key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
        key(&mut h, egui::Key::S, egui::Modifiers::COMMAND);
        assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: no undo behind it");
        assert!(host.saves.lock().unwrap().is_empty() && dialog_open(&h), "{theme:?}: no Save behind it");
        assert!(
            ![crate::actions::GENERATE, crate::actions::OPEN, crate::actions::VERSIONS].iter().any(|a| crate::actions::available(h.state(), a)),
            "{theme:?}"
        );
    });
}

/// A save that fails or goes unanswered keeps what was typed and offers Retry.
#[test]
fn a_failed_save_keeps_the_description_and_offers_retry() {
    in_both_appearances(|theme| {
        let (mut h, host) = save_dialog(theme);
        h.event(egui::Event::Text("Kept".into()));
        click_id(&mut h, "workspace.versions.dialog.save");
        drop(host.saves.lock().unwrap().pop());
        h.run_steps(3);
        assert!(has_id(&h, "workspace.versions.dialog.warning"), "{theme:?}");
        assert_eq!(h.state().versions.dialog.as_ref().unwrap().text, "Kept", "{theme:?}");
        assert!(painted(&h).iter().any(|t| t == "Retry"), "{theme:?}");
        click_id(&mut h, "workspace.versions.dialog.description");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(host.descriptions.lock().unwrap().len(), 1, "{theme:?}: it may have made a version, so not before the list has been read");
        versions_answer(&mut h, &host, Ok(page_with("c1")));
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(*host.descriptions.lock().unwrap(), [Some("Kept".to_string()), Some("Kept".to_string())], "{theme:?}");
    });
}

/// The dialog goes with its project, its session or a hold, on Esc pressed alone, which gives the
/// keyboard back to the button, and on Cancel.
#[test]
fn the_save_dialog_goes_with_what_it_reviews() {
    let goes: [(&str, fn(&mut Shell)); 3] = [
        ("project", |s| s.project.as_mut().unwrap().path = "/tmp/Other.archforge".into()),
        ("session", |s| s.session_replaced()),
        ("hold", |s| s.begin_hold().unwrap()),
    ];
    in_both_appearances(|theme| {
        for (what, step) in goes {
            let (mut h, _host) = save_dialog(theme);
            step(h.state_mut());
            h.run_steps(2);
            assert!(!dialog_open(&h), "{theme:?}: {what}");
        }
        let (mut h, _host) = save_dialog(theme);
        for more in NOT_ALONE {
            key(&mut h, egui::Key::Escape, more);
            assert!(dialog_open(&h), "{theme:?}: Esc with {more:?}");
        }
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(!dialog_open(&h) && focused_on(&h, "workspace.versions.save"), "{theme:?}: Esc");
        click_id(&mut h, "workspace.versions.save");
        click_id(&mut h, "workspace.versions.dialog.cancel");
        assert!(!dialog_open(&h) && h.state().versions.working == 1, "{theme:?}: Cancel changes nothing");
    });
}

/// The button is off with nothing to save, before it is known, during a save and with a version in
/// view, and opens nothing then.
#[test]
fn save_version_is_off_unless_there_is_settled_work_to_save() {
    let button = "workspace.versions.save";
    in_both_appearances(|theme| {
        let (mut h, host) = changes_panel(theme);
        click_id(&mut h, button);
        assert!(!dialog_open(&h), "{theme:?}: no changes");
        h.state_mut().versions.compared = None;
        assert_eq!(crate::save_version::off_because(h.state()), Some(crate::i18n::tr("Loading")), "{theme:?}: not yet compared");
        add_schema(&mut h, "crm");
        compared(&mut h);
        assert!(crate::save_version::off_because(h.state()).is_none(), "{theme:?}: on");
        h.state_mut().save();
        h.run_steps(2);
        click_id(&mut h, button);
        assert!(!dialog_open(&h), "{theme:?}: a save under way");
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(3);
        start_viewing(&mut h, "c1");
        let earlier = rows_now(&h);
        answer_read(&mut h, &host, Ok(earlier));
        versions_answer(&mut h, &host, Ok(two_versions()));
        click_id(&mut h, button);
        assert!(!dialog_open(&h), "{theme:?}: a version in view");
        assert!(crate::save_version::off_because(h.state()).is_some_and(|w| w.starts_with("Back to current work")), "{theme:?}");
    });
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
    // Read against the code's own bytes, not through the parser the canvas uses.
    let own = |fill: egui::Color32| [fill.r(), fill.g(), fill.b(), fill.a()] == [0xA0, 0xC4, 0xFF, 0xFF];
    let filled = h.output().shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Rect(r) if own(r.fill)));
    assert!(filled, "the card is filled in its own colour");
}

/// What the sketch kind's hooks were asked, in order.
static SKETCH_CALLS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// The rows the sketch kind was asked to open a class named [`SKETCH_OPENS`] at, in order.
static SKETCH_OPENED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// The sketch diagram kind in the shell, for the tests alone: a Class tool that places and an
/// Inherit tool that connects, each hook writing down what it was asked. It serves the diagram kind
/// it is given ([`SKETCH`] for the canvas's tests), so two of them are told apart.
struct SketchKind(&'static str);

/// What the sketch kind's status marks toggle says on hover.
const SKETCH_MARKS: &str = "Sketch marks";

/// What the sketch kind's line end handles say on hover.
const SKETCH_ENDS: &str = "Drag onto another sketch card";

static SKETCH_TOOLS: &[archforge_shell_api::kinds::KindTool] = &[
    archforge_shell_api::kinds::KindTool {
        key: "class",
        icon: &crate::ux::icons::TABLE,
        hint: "Entity",
        use_: crate::kinds::ToolUse::Place,
        chevron: None,
        wide: false,
    },
    archforge_shell_api::kinds::KindTool {
        key: "inherit",
        icon: &crate::ux::icons::TOOL_RELATIONSHIP,
        hint: "Relationship",
        use_: crate::kinds::ToolUse::Connect,
        chevron: None,
        wide: false,
    },
];

/// The sketch kind's rows of the Model menu, a section for each of its tools.
static SKETCH_ROWS: &[&[crate::kinds::ModelRow]] = &[
    &[crate::kinds::ModelRow { key: "model.sketch-class", word: || "Class".into(), tool: crate::canvas::Tool::Place("class"), choice: None }],
    &[crate::kinds::ModelRow { key: "model.sketch-inherit", word: || "Inherit".into(), tool: crate::canvas::Tool::Connect("inherit"), choice: None }],
];

/// The second sketch's kinds' one row of the Model menu, so a row of another kind is told apart
/// from the sketch kind's.
static SECOND_SKETCH_MENU: &[&[crate::kinds::ModelRow]] = &[&[crate::kinds::ModelRow {
    key: "model.second-sketch-class",
    word: || "Second class".into(),
    tool: crate::canvas::Tool::Place("class"),
    choice: None,
}]];

impl crate::kinds::DiagramKind for SketchKind {
    /// The kind it was given, which no notation of the shell knows.
    fn kind(&self) -> &'static str {
        self.0
    }
    /// Class and Inherit.
    fn tools(&self) -> &'static [archforge_shell_api::kinds::KindTool] {
        SKETCH_TOOLS
    }
    /// The sketch kind's: a row for each of its tools, each in a section of its own; another kind's
    /// (the second sketch's), one row of its own.
    fn menu_rows(&self) -> &'static [&'static [crate::kinds::ModelRow]] {
        if self.0 == SKETCH { SKETCH_ROWS } else { SECOND_SKETCH_MENU }
    }
    /// Its name in the New diagram dialog.
    fn word(&self) -> String {
        "Sketch".into()
    }
    /// Its group in the Diagrams explorer: "Sketches" for the fake notation's kind, and for another
    /// its own, after its name ("Second sketches" for `test.second`), so kinds are told apart.
    fn group_word(&self) -> String {
        match self.0.strip_prefix("test.") {
            Some(own) if self.0 != SKETCH => format!("{}{} sketches", own[..1].to_uppercase(), &own[1..]),
            _ => "Sketches".into(),
        }
    }
    /// Its card's sentence in the New diagram dialog.
    fn description(&self) -> String {
        "A sketch of classes".into()
    }
    /// The New diagram dialog's title once it is chosen.
    fn new_word(&self) -> String {
        "New sketch".into()
    }
    /// What its diagrams are of.
    fn owner_word(&self) -> String {
        "Package".into()
    }
    /// The choice that makes a new one.
    fn new_owner(&self) -> String {
        "New package".into()
    }
    /// The model's packages.
    fn owners(&self, store: &archforge_mof::Store) -> Vec<archforge_mof::Obj> {
        sketch_packages(store)
    }
    /// No owner from the selection: the sketch's diagrams are of a package chosen in the dialog.
    fn owner_of(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<archforge_mof::Obj> {
        None
    }
    /// Refuses: the sketch makes no package for a diagram to be of.
    fn add_owner(&self, _: &mut crate::cx::Ctx, _: &str) -> Result<String, String> {
        Err("the sketch makes no package".into())
    }
    /// Its status marks toggle, in its own words, so a test sees it asked.
    fn status_marks(&self) -> Option<String> {
        Some(SKETCH_MARKS.into())
    }
    /// A UML class takes part in the sketch's gestures.
    fn takes_part(&self, store: &archforge_mof::Store, element: archforge_mof::Obj) -> bool {
        store.class_of(element).ok() == Some(archforge_mof::toy::CLASS)
    }
    /// Writes down the tool it was asked to place with.
    fn place(&self, _: &mut crate::cx::Ctx, tool: &'static str, _: archforge_mof::Obj, _: f64, _: f64) {
        SKETCH_CALLS.lock().unwrap().push(format!("place {tool}"));
    }
    /// Writes down the tool and the names of the two nodes' elements it was asked to connect.
    fn connect(&self, cx: &mut crate::cx::Ctx, tool: &'static str, _: archforge_mof::Obj, from: archforge_mof::Uuid, to: archforge_mof::Uuid) {
        let name = |id| {
            cx.session.as_ref().and_then(|s| {
                let st = s.doc();
                let e = st.get(st.by_uuid(id)?, archforge_diagram::node::ELEMENT).ok()?.as_ref()?;
                Some(archforge_core::name_of(st, e))
            })
        };
        SKETCH_CALLS.lock().unwrap().push(format!("connect {tool} {} {}", name(from).unwrap_or_default(), name(to).unwrap_or_default()));
    }
    /// Writes down the tool of a line dropped on empty canvas.
    fn connect_to_new(&self, _: &mut crate::cx::Ctx, tool: &'static str, _: archforge_mof::Obj, _: archforge_mof::Uuid, _: f64, _: f64) {
        SKETCH_CALLS.lock().unwrap().push(format!("connect to new {tool}"));
    }
    /// Its own popover of kinds, which the arrow keys walk, so a test sees whose kind is asked; no
    /// other.
    fn popover_takes_keys(&self, key: &str) -> bool {
        (self.0 == SKETCH && key == SKETCH_POPOVER) || (self.0 == "test.second" && key == SECOND_KINDS)
    }
    /// Writes down the name typed in place, and gives it to a class through the fake notation's
    /// rename.
    fn rename(&self, cx: &mut crate::cx::Ctx, element: archforge_mof::Uuid, name: &str) -> Result<(), String> {
        SKETCH_CALLS.lock().unwrap().push(format!("rename {name}"));
        let class = cx
            .session
            .as_ref()
            .and_then(|s| s.doc().by_uuid(element))
            .is_some_and(|o| cx.session.as_ref().is_some_and(|s| s.doc().class_of(o).ok() == Some(archforge_mof::toy::CLASS)));
        if !class {
            return Ok(());
        }
        archforge_shell_api::panels::run_cx(cx, "sketch.class.rename", serde_json::json!({ "class": element.to_string(), "name": name })).map(|_| ())
    }
    /// Writes down the row it was asked to open at: for a class named [`SKETCH_OPENS`] apart, so
    /// the tests that read what the other hooks were asked never see it.
    fn open(&self, cx: &mut crate::cx::Ctx, element: archforge_mof::Uuid, row: Option<usize>) {
        let name = cx.session.as_ref().and_then(|s| s.doc().by_uuid(element).map(|e| archforge_core::name_of(s.doc(), e)));
        let calls = if name.as_deref() == Some(SKETCH_OPENS) { &SKETCH_OPENED } else { &SKETCH_CALLS };
        calls.lock().unwrap().push(format!("open {row:?}"));
    }
    /// Its lines' ends take handles, which move nothing and say its own words on hover, so a test
    /// sees it asked.
    fn reconnects(&self) -> Option<String> {
        Some(SKETCH_ENDS.into())
    }
    /// Party's menu: a first row that is off, then one after Open in Models that writes down what
    /// it was picked on; Customer takes none.
    fn menu(&self, store: &archforge_mof::Store, element: archforge_mof::Obj, mac: bool) -> Option<Vec<crate::kinds::MenuRow>> {
        let first = crate::kinds::MenuRow {
            key: "sketch-first",
            label: "Sketch first",
            keys: "F",
            live: false,
            first: true,
            run: |_, _| SKETCH_CALLS.lock().unwrap().push("menu first".into()),
        };
        let own = crate::kinds::MenuRow {
            key: "sketch-own",
            label: "Sketch own",
            keys: if mac { "⌥S" } else { "Alt S" },
            live: true,
            first: false,
            run: |cx, menu| {
                let name = cx.session.as_ref().and_then(|s| s.doc().by_uuid(menu.element).map(|e| archforge_core::name_of(s.doc(), e)));
                SKETCH_CALLS.lock().unwrap().push(format!("menu own {}", name.unwrap_or_default()));
            },
        };
        (archforge_core::name_of(store, element) == "Party").then(|| vec![first, own])
    }
    /// A class named [`SKETCH_OPENS`] opens whole with a double click, as a sketch kind other than
    /// the canvas's tests' own ([`SKETCH`]) answers, so a test sees the kinds asked beside the
    /// diagram's own.
    fn opens_on_double_click(&self, store: &archforge_mof::Store, element: archforge_mof::Obj) -> bool {
        self.0 != SKETCH && archforge_core::name_of(store, element) == SKETCH_OPENS
    }
    /// The name typed on a card the sketch draws itself goes in the card's part over its top left,
    /// in the small style; any other card's in its header.
    fn name_field(
        &self,
        _: &egui::Painter,
        store: &archforge_mof::Store,
        node: archforge_mof::Obj,
        rect: egui::Rect,
        _: &str,
        z: f32,
    ) -> Option<(egui::Rect, crate::ux::Type)> {
        let element = crate::canvas::element_of(store, node, archforge_diagram::node::ELEMENT)?;
        (archforge_core::name_of(store, element) == SKETCH_DRAWN).then(|| (sketch_part(rect, z), crate::ux::types::SMALL.scaled(z)))
    }
    /// Draws the card of a class named [`SKETCH_DRAWN`] itself: a plain box with its own word, a
    /// part over its top left that takes a press and says [`SKETCH_PART_SAYS`] on hover, and a
    /// label under it that says [`SKETCH_LABEL_SAYS`]; any other card is the canvas's.
    fn draw_card(
        &self,
        paint: &egui::Painter,
        d: &crate::canvas::Drawing<'_>,
        card: &crate::canvas::CardDrawn<'_>,
        registry: &mut crate::widgets::Registry,
        ui: &egui::Ui,
    ) -> bool {
        if archforge_core::name_of(d.store, card.element) != SKETCH_DRAWN {
            return false;
        }
        let z = d.view.zoom;
        paint.rect_filled(card.rect, 0.0, d.p.surface);
        crate::ux::text(paint, card.rect.min + egui::vec2(8.0, 8.0) * z, SKETCH_WORD, crate::ux::types::UI.scaled(z), d.p.text, None);
        let Ok(id) = d.store.uuid(card.node) else { return true };
        let part = sketch_part(card.rect, z);
        paint.rect_filled(part, 0.0, d.p.surface);
        crate::widgets::record_visible(ui, registry, &format!("workspace.canvas.node.{id}.sketch-part"), "tab", part);
        d.parts.borrow_mut().push(crate::canvas::CardPart { rect: part, node: id, hover: Some(SKETCH_PART_SAYS.into()) });
        let label = egui::Rect::from_min_size(card.rect.left_bottom() + egui::vec2(0.0, 6.0) * z, egui::vec2(card.rect.width(), 16.0 * z));
        let label_id = format!("workspace.canvas.node.{id}.sketch-label");
        crate::widgets::record_visible(ui, registry, &label_id, "label", label);
        d.labels.borrow_mut().push(crate::canvas::CardLabel {
            rect: label,
            node: id,
            id: label_id,
            shown: SKETCH_LABEL_SAYS.into(),
            whole: SKETCH_LABEL_SAYS.into(),
        });
        true
    }
    /// Draws [`SKETCH_LINKS`] under the first node's card, recorded under its own kind once each
    /// time it is asked.
    fn draw_links(&self, paint: &egui::Painter, d: &crate::canvas::Drawing<'_>, registry: &mut crate::widgets::Registry, ui: &egui::Ui) {
        let Some((n, _)) = d.boxes.first() else { return };
        let at = d.node_rect(*n).left_bottom() + egui::vec2(0.0, 30.0);
        let rect = crate::ux::text(paint, at, SKETCH_LINKS, crate::ux::types::SMALL, d.p.text, None);
        crate::widgets::record_visible(ui, registry, &format!("workspace.canvas.sketch-links.{}", self.0), "label", rect);
    }
    /// Draws [`SKETCH_OVER`] over the top right corner of every card, recorded.
    fn card_extras(
        &self,
        paint: &egui::Painter,
        d: &crate::canvas::Drawing<'_>,
        card: &crate::canvas::CardDrawn<'_>,
        registry: &mut crate::widgets::Registry,
        ui: &egui::Ui,
    ) {
        let at = card.rect.right_top() - egui::vec2(90.0, 8.0);
        let rect = crate::ux::text(paint, at, SKETCH_OVER, crate::ux::types::SMALL, d.p.text, None);
        if let Ok(id) = d.store.uuid(card.node) {
            crate::widgets::record_visible(ui, registry, &format!("workspace.canvas.sketch-over.{id}"), "label", rect);
        }
    }
    /// Moves each line's source end [`SKETCH_END_SHIFT`] on along the line, writing it down as
    /// moved, and gives a square in the line's ink at the moved end, recorded.
    fn line_ends(
        &self,
        _: &egui::Painter,
        d: &crate::canvas::Drawing<'_>,
        line: &crate::canvas::LineEnds<'_>,
        points: &mut [egui::Pos2],
        registry: &mut crate::widgets::Registry,
        ui: &egui::Ui,
    ) -> Vec<egui::Shape> {
        let (Some(start), Some(next)) = (points.first().copied(), points.get(1).copied()) else { return Vec::new() };
        let moved = start + (next - start).normalized() * SKETCH_END_SHIFT;
        d.moved_ends.borrow_mut().push((start, moved));
        points[0] = moved;
        let square = egui::Rect::from_center_size(moved, egui::Vec2::splat(8.0));
        if let Ok(id) = d.store.uuid(line.line) {
            crate::widgets::record_visible(ui, registry, &format!("workspace.canvas.sketch-end.{id}"), "label", square);
        }
        vec![egui::Shape::rect_filled(square, 0.0, line.stroke.color)]
    }
    /// Says its line's name sits over the line's middle, drawing nothing there.
    fn line_extras(
        &self,
        _: &egui::Painter,
        _: &crate::canvas::Drawing<'_>,
        line: &crate::canvas::LineDrawn<'_>,
        _: &mut crate::widgets::Registry,
        _: &egui::Ui,
    ) -> Option<egui::Rect> {
        Some(egui::Rect::from_center_size(crate::canvas::along(line.points, 0.5) - egui::vec2(0.0, 30.0), egui::vec2(60.0, 14.0)))
    }
    /// Draws [`SKETCH_CAPTION`] above where its line's name sits, recorded; nothing for a line
    /// without a name.
    fn line_caption(
        &self,
        paint: &egui::Painter,
        d: &crate::canvas::Drawing<'_>,
        line: &crate::canvas::LineDrawn<'_>,
        name: Option<egui::Rect>,
        registry: &mut crate::widgets::Registry,
        ui: &egui::Ui,
    ) {
        let Some(name) = name else { return };
        let rect = crate::ux::text(paint, name.left_top() - egui::vec2(0.0, 20.0), SKETCH_CAPTION, crate::ux::types::SMALL, d.p.text, None);
        if let Ok(id) = d.store.uuid(line.line) {
            crate::widgets::record_visible(ui, registry, &format!("workspace.canvas.sketch-caption.{id}"), "label", rect);
        }
    }
}

/// The name of a class whose card the sketch kind draws itself, the word it draws on it, and what
/// its part and its label say.
const SKETCH_DRAWN: &str = "Drawn by the sketch";
const SKETCH_WORD: &str = "Sketched";
const SKETCH_PART_SAYS: &str = "The sketch's part";
const SKETCH_LABEL_SAYS: &str = "The sketch's label, whole";
/// What the sketch kind draws over every card, and over a line's name; how far it moves a line's
/// source end on along the line.
const SKETCH_OVER: &str = "Over the card";
const SKETCH_CAPTION: &str = "Over the name";
/// What the sketch kind draws among the links between nodes, and the name of a class whose card
/// another sketch kind opens whole with a double click.
const SKETCH_LINKS: &str = "Among the links";
const SKETCH_OPENS: &str = "Opened whole";
const SKETCH_END_SHIFT: f32 = 30.0;

/// The part over the top left of a card at `rect` that the sketch kind draws itself, at zoom `z`.
fn sketch_part(rect: egui::Rect, z: f32) -> egui::Rect {
    egui::Rect::from_min_size(rect.left_top() - egui::vec2(0.0, 24.0) * z, egui::vec2(90.0, 24.0) * z)
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

/// The element menu builds from what the diagram's kind answers, a kind it never names included:
/// the sketch's first row in a section of its own above Open in Models, its other row after it with
/// its keys, then Generate…, and Remove and Delete in a section of their own; a row that is off
/// runs nothing (its click falls to the canvas, which closes the menu), a row that is on runs what
/// the kind gave it on the menu's element and closes the menu; on what the kind answers no menu
/// for, a right click opens none.
#[test]
fn the_element_menu_builds_from_the_kinds_answer() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _d, a, b, _e) = sketch_harness();
        h.state_mut().state.theme = theme;
        h.run();
        SKETCH_CALLS.lock().unwrap().clear();
        let (party, customer) = (rect_of(&h, &format!("workspace.canvas.node.{a}")), rect_of(&h, &format!("workspace.canvas.node.{b}")));
        right_click(&mut h, customer.center());
        assert!(h.state().canvas.menu.is_none(), "{theme:?}: Customer takes no menu");
        right_click(&mut h, party.center());
        assert!(h.state().canvas.menu.is_some(), "{theme:?}: Party's menu opened");
        let drawn = menu_drawn(&h);
        let expected = [("sketch-first", 0.0), ("open", 1.0), ("sketch-own", 0.0), ("generate", 0.0), ("remove", 1.0), ("delete", 0.0)];
        assert_eq!(drawn, expected.map(|(k, g)| (k.to_string(), g)), "{theme:?}");
        let texts = painted(&h);
        assert!(["Sketch first", "F", "Sketch own", "Alt S"].iter().all(|t| texts.iter().any(|p| p == t)), "{theme:?}: {texts:?}");
        let at = rect_of(&h, "workspace.canvas.menu.sketch-first").center();
        click(&mut h, at);
        assert!(h.state().canvas.menu.is_none() && SKETCH_CALLS.lock().unwrap().is_empty(), "{theme:?}: a row that is off runs nothing");
        right_click(&mut h, party.center());
        let at = rect_of(&h, "workspace.canvas.menu.sketch-own").center();
        click(&mut h, at);
        assert!(h.state().canvas.menu.is_none(), "{theme:?}: the menu closed");
        assert_eq!(*SKETCH_CALLS.lock().unwrap(), ["menu own Party"], "{theme:?}");
    }
}

/// The sketch kind alone, as the shell the tests make is given it.
static SKETCH_KINDS: &[&dyn crate::kinds::DiagramKind] = &[&SketchKind(SKETCH)];

#[test]
fn a_second_kinds_toolbar_shows_its_own_tools_and_its_place_connect_and_rename_hooks_run() {
    let (mut h, _d, a, b, _e) = sketch_harness();
    SKETCH_CALLS.lock().unwrap().clear();
    // ER's tool, left on from an ER diagram, is not the sketch's: the toolbar hands back to Select.
    h.state_mut().canvas.tool = crate::canvas::Tool::Place("entity");
    h.run();
    assert_eq!(h.state().canvas.tool, crate::canvas::Tool::Select);
    let found = ids(&h);
    for shared in ["select", "pan", "class", "inherit", "note", "link", "key-status", "grid", "arrange"] {
        assert!(found.iter().any(|i| *i == format!("workspace.tools.{shared}")), "{shared} is on the toolbar: {found:?}");
    }
    for er in ["entity", "relationship", "relationship-types"] {
        assert!(!found.iter().any(|i| *i == format!("workspace.tools.{er}")), "ER's {er} is not");
    }
    // The toolbar keeps its order: Select, Pan, the kind's tools, Note, Link note, the kind's
    // status marks, Grid, Arrange.
    let left = |key: &str| rect_of(&h, &format!("workspace.tools.{key}")).left();
    let xs: Vec<f32> = ["select", "pan", "class", "inherit", "note", "link", "key-status", "grid", "arrange"].iter().map(|k| left(k)).collect();
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

/// A diagram kind's own words on the shared canvas, a kind the shell never names included: its
/// status marks toggle on the toolbar and a selected line's two end handles say on hover what the
/// kind answers, never ER's or UML's words; in light and dark.
#[test]
fn a_kinds_status_toggle_and_line_ends_say_its_own_words() {
    let hover = |h: &mut Harness<'static, Shell>, id: &str| {
        let at = rect_of(h, id).center();
        h.event(egui::Event::PointerMoved(at));
        for _ in 0..90 {
            h.step();
        }
        painted(h)
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ..) = sketch_harness();
        h.state_mut().state.theme = theme;
        h.run();
        let said = hover(&mut h, "workspace.tools.key-status");
        assert!(said.iter().any(|t| t == SKETCH_MARKS), "{theme:?}: the toggle says the sketch's words: {said:?}");
        assert!(!said.iter().any(|t| t == "Key status"), "{theme:?}: not ER's");
        // Customer's generalization selected, its line shows the two end handles, in the sketch's words.
        let g = {
            let st = h.state().session.as_ref().unwrap().doc();
            let g = st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::GENERALIZATION)).unwrap();
            st.uuid(g).unwrap().to_string()
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [g] })).unwrap();
        h.run();
        for end in ["source", "target"] {
            let said = hover(&mut h, &format!("workspace.canvas.line-end.{end}"));
            assert!(said.iter().any(|t| t == SKETCH_ENDS), "{theme:?}: the {end} handle says the sketch's words: {said:?}");
            assert!(!said.iter().any(|t| t == "Drag onto another class to move this end"), "{theme:?}: not UML's");
        }
    }
}

/// The sketch harness in `theme` with Customer renamed [`SKETCH_DRAWN`], whose card the sketch kind
/// draws itself; gives the diagram, Party's node and Customer's node.
fn sketch_drawn_harness(theme: ThemeKind) -> (Harness<'static, Shell>, String, String, String) {
    let (mut h, d, a, b, _e) = sketch_harness();
    h.state_mut().state.theme = theme;
    let s = h.state_mut().session.as_mut().unwrap();
    let st = s.doc();
    let customer = st
        .uuid(crate::canvas::element_of(st, st.by_uuid(archforge_mof::Uuid::parse(&b).unwrap()).unwrap(), archforge_diagram::node::ELEMENT).unwrap())
        .unwrap();
    s.execute("sketch.class.rename", &serde_json::json!({ "class": customer.to_string(), "name": SKETCH_DRAWN })).unwrap();
    h.run();
    (h, d, a, b)
}

/// The elements selected, by id.
fn selected_ids(h: &Harness<'static, Shell>) -> Vec<archforge_mof::Uuid> {
    let s = h.state().session.as_ref().unwrap();
    s.view().selection.iter().filter_map(|o| s.doc().uuid(*o).ok()).collect()
}

/// A card its diagram's kind draws itself takes the place of the canvas's own (the sketch's box
/// and word, none of the canvas's rows), while the canvas's other cards stay its own; its part over
/// the card takes a press as the card does and says its words on hover; its label is read out whole
/// and shown on hover; and the canvas still draws the selection ring round it and gives its problem
/// badge back. In light and dark.
#[test]
fn a_card_its_kind_draws_replaces_the_canvass_and_keeps_the_ring_badge_parts_and_labels() {
    use egui_kittest::kittest::{NodeT, Queryable};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, d, _a, b) = sketch_drawn_harness(theme);
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == SKETCH_WORD), "{theme:?}: the kind's card: {texts:?}");
        assert!(!texts.iter().any(|t| t == "since"), "{theme:?}: none of the canvas's card");
        assert_eq!(texts.iter().filter(|t| *t == "CLASS").count(), 1, "{theme:?}: Party's eyebrow alone");
        assert!(texts.iter().any(|t| t == "partyId"), "{theme:?}: Party's card is still the canvas's");
        let node = archforge_mof::Uuid::parse(&b).unwrap();
        assert_eq!(h.state().canvas.kind_cards, vec![(node, SKETCH)], "{theme:?}");
        let body = rect_of(&h, &format!("workspace.canvas.node.{b}"));
        let part = rect_of(&h, &format!("workspace.canvas.node.{b}.sketch-part"));
        assert!(!body.contains(part.center()), "{theme:?}: the part lies outside the card");
        // A click on the part selects the card's element.
        click(&mut h, part.center());
        let element = {
            let st = h.state().session.as_ref().unwrap().doc();
            st.uuid(crate::canvas::element_of(st, st.by_uuid(node).unwrap(), archforge_diagram::node::ELEMENT).unwrap()).unwrap()
        };
        assert_eq!(selected_ids(&h), vec![element], "{theme:?}: the part selects its card");
        // The canvas draws the selection ring round the kind's card.
        let accent = egui::Color32::from(crate::ux::palette(theme == ThemeKind::Dark).accent);
        let ring = body.expand(4.0);
        let ringed = h.output().shapes.iter().any(|c| {
            matches!(&c.shape, egui::Shape::Rect(r) if r.stroke.color == accent && r.rect.min.distance(ring.min) < 0.5 && r.rect.max.distance(ring.max) < 0.5)
        });
        assert!(ringed, "{theme:?}: the selection ring");
        // Neither the part's words nor the label's are painted anywhere but in their tooltips: with
        // the pointer on empty canvas, none shows; on the part, its words; on the label, its
        // sentence. Each is moved to and settled before it is read, whatever the click left.
        let shown = |h: &Harness<'static, Shell>, words: &str| painted(h).iter().filter(|t| *t == words).count();
        let settle_at = |h: &mut Harness<'static, Shell>, at: egui::Pos2| {
            h.event(egui::Event::PointerMoved(at));
            for _ in 0..45 {
                h.step();
            }
        };
        let label = rect_of(&h, &format!("workspace.canvas.node.{b}.sketch-label"));
        let off = body.center_bottom() + egui::vec2(0.0, 150.0);
        settle_at(&mut h, off);
        assert_eq!((shown(&h, SKETCH_PART_SAYS), shown(&h, SKETCH_LABEL_SAYS)), (0, 0), "{theme:?}: off the card, no hover text");
        // The pointer on the part shows its words, a gesture under way or not.
        settle_at(&mut h, part.center());
        assert_eq!(shown(&h, SKETCH_PART_SAYS), 1, "{theme:?}: the part's words on hover");
        settle_at(&mut h, off);
        assert_eq!(shown(&h, SKETCH_PART_SAYS), 0, "{theme:?}: gone off the part");
        // The label is read out whole, as a label, and shown whole on hover.
        let said = h.get_by_label(SKETCH_LABEL_SAYS).accesskit_node();
        assert_eq!(said.role(), egui::accesskit::Role::Label, "{theme:?}");
        settle_at(&mut h, label.center());
        assert_eq!(shown(&h, SKETCH_LABEL_SAYS), 1, "{theme:?}: the label's tooltip");
        // Drawn with no shell, the kind's card gives its problem badge back.
        let session = h.state_mut().session.take().unwrap();
        drop(h);
        let st = session.doc();
        let flagged = [(st.by_uuid(element).unwrap(), vec!["A problem".to_string()])].into_iter().collect();
        let (texts, _, badges) =
            drawn_alone(&session, &d, Some(&SketchKind(SKETCH)), theme == ThemeKind::Dark, 1000.0, &Default::default(), &flagged);
        assert!(texts.iter().any(|t| t == SKETCH_WORD), "{theme:?}: drawn by the kind");
        assert_eq!(badges, 1, "{theme:?}: its badge");
    }
}

/// The name typed in place on a card its kind draws goes where the kind says, from the very frame
/// the double click opens it, and stays there while it is typed; a canvas card's goes in its
/// header. In light and dark.
#[test]
fn the_name_typed_on_a_card_its_kind_draws_goes_where_the_kind_says_from_the_first_frame() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _d, a, b) = sketch_drawn_harness(theme);
        let body = rect_of(&h, &format!("workspace.canvas.node.{b}"));
        let part = sketch_part(body, 1.0);
        // The frame the second click lands in opens the name, and the field is in the part.
        let at = rect_of(&h, &format!("workspace.canvas.node.{b}.sketch-part")).center();
        h.event(egui::Event::PointerMoved(at));
        for _ in 0..2 {
            for pressed in [true, false] {
                h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
            }
            h.step();
        }
        assert!(h.state().canvas.rename.as_ref().is_some_and(|r| r.text == SKETCH_DRAWN), "{theme:?}: {:?}", h.state().canvas.rename);
        let field = rect_of(&h, "workspace.canvas.rename");
        assert_eq!(field, part, "{theme:?}: in the part from the first frame");
        h.state_mut().canvas.rename.as_mut().unwrap().text = "Typed".into();
        h.run();
        assert_eq!(rect_of(&h, "workspace.canvas.rename"), part, "{theme:?}: still there while typed");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // Party's card is the canvas's: its name is typed in its header.
        for _ in 0..45 {
            h.step();
        }
        let party = rect_of(&h, &format!("workspace.canvas.node.{a}"));
        double_click(&mut h, party.left_top() + egui::vec2(60.0, 30.0));
        let field = rect_of(&h, "workspace.canvas.rename");
        assert!(party.contains_rect(field) && field.top() > party.top() + 20.0, "{theme:?}: in Party's header: {field:?} {party:?}");
    }
}

/// The colour a line is painted in, by its path's stroke.
fn path_colour(s: &egui::Shape) -> Option<egui::Color32> {
    match s {
        egui::Shape::Path(p) => match p.stroke.color {
            egui::epaint::ColorMode::Solid(c) => Some(c),
            _ => None,
        },
        _ => None,
    }
}

/// The kinds move a line's ends before the canvas draws it: the line starts where the sketch moved
/// its end, the end is written down as moved, and what the kind gave is painted right after the
/// line, in its ink; a selected line's end handle sits at the moved end. What a kind draws by the
/// line's name goes over it, from where the name sits. In light and dark.
#[test]
fn a_lines_ends_move_as_the_kinds_say_and_what_they_draw_goes_over_the_line_and_by_its_name() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _d, _a, _b, e) = sketch_harness();
        h.state_mut().state.theme = theme;
        h.run();
        let end = rect_of(&h, &format!("workspace.canvas.sketch-end.{e}"));
        let moved = end.center();
        let [(from, to)] = h.state().canvas.moved_ends[..] else { panic!("{theme:?}: {:?}", h.state().canvas.moved_ends) };
        assert!(to.distance(moved) < 0.5 && (from.distance(to) - SKETCH_END_SHIFT).abs() < 0.5, "{theme:?}: {from:?} to {to:?}");
        let shapes: Vec<&egui::Shape> = h.output().shapes.iter().map(|c| &c.shape).collect();
        let at = shapes
            .iter()
            .position(|s| matches!(s, egui::Shape::Path(p) if p.points.first().is_some_and(|q| q.distance(moved) < 0.5)))
            .unwrap_or_else(|| panic!("{theme:?}: the line starts at the moved end"));
        let ink = path_colour(shapes[at]);
        assert!(
            matches!(shapes.get(at + 1), Some(egui::Shape::Rect(r)) if r.rect == end && Some(r.fill) == ink),
            "{theme:?}: the kind's square right after the line, in its ink: {:?}",
            shapes.get(at + 1)
        );
        assert!(ids(&h).iter().any(|i| *i == format!("workspace.canvas.sketch-caption.{e}")), "{theme:?}: drawn by the line's name");
        assert!(painted(&h).iter().any(|t| t == SKETCH_CAPTION), "{theme:?}");
        // Selected, the line's source handle sits where its end moved to.
        let element = {
            let st = h.state().session.as_ref().unwrap().doc();
            let edge = st.by_uuid(archforge_mof::Uuid::parse(&e).unwrap()).unwrap();
            st.uuid(crate::canvas::element_of(st, edge, archforge_diagram::edge::ELEMENT).unwrap()).unwrap()
        };
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [element.to_string()] })).unwrap();
        h.run();
        let handle = rect_of(&h, "workspace.canvas.line-end.source").center();
        assert!(handle.distance(moved) < 0.5, "{theme:?}: {handle:?} at {moved:?}");
    }
}

/// What a kind draws over a card goes over the card and its selection ring, for the canvas's own
/// cards and those a kind drew itself alike. In light and dark.
#[test]
fn what_the_kinds_draw_over_a_card_goes_over_every_card_and_its_ring() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _d, a, b) = sketch_drawn_harness(theme);
        for node in [&a, &b] {
            assert!(ids(&h).iter().any(|i| *i == format!("workspace.canvas.sketch-over.{node}")), "{theme:?}: over {node}");
        }
        assert_eq!(painted(&h).iter().filter(|t| *t == SKETCH_OVER).count(), 2, "{theme:?}");
        // Party selected: its ring, then what the kind draws over it.
        let party = rect_of(&h, &format!("workspace.canvas.node.{a}"));
        click(&mut h, party.center());
        h.run();
        let over = rect_of(&h, &format!("workspace.canvas.sketch-over.{a}"));
        let accent = egui::Color32::from(crate::ux::palette(theme == ThemeKind::Dark).accent);
        let ring = party.expand(4.0);
        let shapes: Vec<&egui::Shape> = h.output().shapes.iter().map(|c| &c.shape).collect();
        let ringed = shapes.iter().position(|s| {
            matches!(s, egui::Shape::Rect(r) if r.stroke.color == accent && r.rect.min.distance(ring.min) < 0.5 && r.rect.max.distance(ring.max) < 0.5)
        });
        let written =
            shapes.iter().position(|s| matches!(s, egui::Shape::Text(t) if t.galley.text() == SKETCH_OVER && t.pos.distance(over.min) < 0.5));
        assert!(ringed.is_some() && written > ringed, "{theme:?}: the ring at {ringed:?}, the kind's text at {written:?}");
    }
}

/// What the kinds draw among the links between nodes goes over the lines, after what a kind gave
/// to go over a line's end, and under the cards. In light and dark.
#[test]
fn what_the_kinds_draw_among_the_links_goes_over_the_lines_and_under_the_cards() {
    in_both_appearances(|theme| {
        let (mut h, _d, _a, _b, e) = sketch_harness();
        h.state_mut().state.theme = theme;
        h.run();
        let end = rect_of(&h, &format!("workspace.canvas.sketch-end.{e}"));
        let shapes: Vec<&egui::Shape> = h.output().shapes.iter().map(|c| &c.shape).collect();
        let text = |words: &str| shapes.iter().position(|s| matches!(s, egui::Shape::Text(t) if t.galley.text() == words));
        let square = shapes.iter().position(|s| matches!(s, egui::Shape::Rect(r) if r.rect == end));
        let (links, card) = (text(SKETCH_LINKS), text("Party"));
        assert!(square.is_some() && links > square, "{theme:?}: over the line's end, {square:?} then {links:?}");
        assert!(card.is_some() && links < card, "{theme:?}: under the cards, {links:?} then {card:?}");
    });
}

/// The sketch kind that opens [`SKETCH_OPENS`] whole, beside the diagram's own kind, which does not.
static OPENING_KINDS: &[&dyn crate::kinds::DiagramKind] = &[&SketchKind(SKETCH), &SketchKind("test.opening")];

/// A double click on a card a kind of the shell opens whole opens its element's editor with no row,
/// though the diagram's own kind does not say so; with no kind saying so, a double click on the
/// card's header names it in place. In light and dark.
#[test]
fn a_double_click_on_a_card_a_kind_opens_whole_opens_its_editor() {
    in_both_appearances(|theme| {
        let (mut h, _d, _a, b, _e) = sketch_harness();
        h.state_mut().state.theme = theme;
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let customer = st
            .uuid(
                crate::canvas::element_of(st, st.by_uuid(archforge_mof::Uuid::parse(&b).unwrap()).unwrap(), archforge_diagram::node::ELEMENT)
                    .unwrap(),
            )
            .unwrap();
        s.execute("sketch.class.rename", &serde_json::json!({ "class": customer.to_string(), "name": SKETCH_OPENS })).unwrap();
        h.run();
        let header = rect_of(&h, &format!("workspace.canvas.node.{b}")).left_top() + egui::vec2(60.0, 30.0);
        // No kind of the shell opens it whole: its header names it in place.
        double_click(&mut h, header);
        assert!(h.state().canvas.rename.as_ref().is_some_and(|r| r.text == SKETCH_OPENS), "{theme:?}: {:?}", h.state().canvas.rename);
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        // The other sketch kind opens it whole: the editor opens with no row, and nothing is named.
        h.state_mut().kinds = OPENING_KINDS;
        for _ in 0..45 {
            h.step();
        }
        SKETCH_OPENED.lock().unwrap().clear();
        double_click(&mut h, header);
        assert!(h.state().canvas.rename.is_none(), "{theme:?}: {:?}", h.state().canvas.rename);
        assert_eq!(*SKETCH_OPENED.lock().unwrap(), ["open None"], "{theme:?}");
    });
}

/// The kinds are asked what they draw of a card and a line, and the links between nodes, whatever
/// kind the diagram is of, each once: drawn with no kind of its own, the sketch still draws its
/// links, draws over the cards and moves the line's end; drawn as a sketch, it does each once, its
/// line's name included.
#[test]
fn every_kind_is_asked_once_what_it_draws_of_cards_and_lines() {
    let (mut h, d, a, b, e) = sketch_harness();
    let session = h.state_mut().session.take().unwrap();
    drop(h);
    let count = |recorded: &[(String, egui::Rect)], id: &str| recorded.iter().filter(|(i, _)| i == id).count();
    let wanted = |caption: usize| {
        [
            (format!("workspace.canvas.sketch-over.{a}"), 1),
            (format!("workspace.canvas.sketch-over.{b}"), 1),
            (format!("workspace.canvas.sketch-end.{e}"), 1),
            (format!("workspace.canvas.sketch-caption.{e}"), caption),
            (format!("workspace.canvas.sketch-links.{SKETCH}"), 1),
        ]
    };
    let none = std::collections::BTreeMap::new();
    for (kind, caption) in [(None, 0), (Some(&SketchKind(SKETCH) as &'static dyn crate::kinds::DiagramKind), 1)] {
        let (_, recorded, _) = drawn_alone_at(&session, &d, (kind, SKETCH_KINDS), false, 1000.0, &Default::default(), &none);
        for (id, times) in wanted(caption) {
            assert_eq!(count(&recorded, &id), times, "{id} as {:?}", kind.map(|k| k.kind()));
        }
    }
}

/// No notation's screens: the sketch's package then has no notation in the shell to word it.
static NO_UIS: &[&dyn crate::notation_ui::NotationUi] = &[];

/// The word over a diagram's Properties field for what it is of: the word that element's notation
/// gives it, and when no notation of the shell words it, the word of the diagram's own kind for
/// what its diagrams are of, never a word of the shell's.
#[test]
fn a_diagrams_properties_word_what_it_is_of_by_its_notation_else_by_its_kind() {
    in_both_appearances(|theme| {
        let (mut h, d, ..) = sketch_harness();
        h.state_mut().state.theme = theme;
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [d] })).unwrap();
        h.run();
        // The word painted just above the field that names what the diagram is of.
        let word_over = |h: &Harness<'static, Shell>| {
            let field = rect_of(h, "workspace.properties.schema");
            let above = egui::Rect::from_min_max(egui::pos2(field.left() - 1.0, field.top() - 20.0), egui::pos2(field.right(), field.top() - 1.0));
            texts_in(h, above).into_iter().map(|(t, _)| t).collect::<Vec<_>>()
        };
        // The sketch notation words the package, as it words everything, "Class".
        assert_eq!(word_over(&h), ["Class"], "{theme:?}");
        assert!(painted(&h).iter().any(|t| t == "Diagram · of sales"), "{theme:?}: {:?}", painted(&h));
        // With no notation to word the package, the sketch kind's word for what its diagrams are of.
        h.state_mut().notation_uis = NO_UIS;
        h.run();
        assert_eq!(word_over(&h), ["Package"], "{theme:?}");
    });
}

/// The sketch notation in the shell's explorers, search and panels, for the tests alone: a package
/// at the top of the Models tree with its classes under it, each class found by the search as a
/// Class of its package, and a class's Properties a section of its attributes. Its name (`sketch`)
/// is the one it signs what it says of a removal with, so two of them are told apart.
struct SketchUi(&'static str);

/// The fake notation's packages in `store`.
fn sketch_packages(store: &archforge_mof::Store) -> Vec<archforge_mof::Obj> {
    store.alive().filter(|o| store.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).collect()
}

/// The package `selection` is, or the one the class `selection` sits in.
fn sketch_package_of(store: &archforge_mof::Store, selection: Option<archforge_mof::Obj>) -> Option<archforge_mof::Obj> {
    let o = selection?;
    match store.class_of(o).ok()? {
        c if c == archforge_mof::toy::PACKAGE => Some(o),
        c if c == archforge_mof::toy::CLASS => store.container(o).ok().flatten().map(|(p, _)| p),
        _ => None,
    }
}

/// The classes of the fake notation's package `pkg`.
fn sketch_classes(store: &archforge_mof::Store, pkg: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
    store
        .list(pkg, archforge_mof::toy::PACKAGE_PACKAGED_ELEMENT)
        .unwrap_or_default()
        .iter()
        .filter_map(archforge_mof::Value::as_ref)
        .filter(|o| store.class_of(*o).ok() == Some(archforge_mof::toy::CLASS))
        .collect()
}

/// The sketch notation's state: the selection it was last told of
/// ([`NotationUi::selection_moved`](crate::notation_ui::NotationUi::selection_moved)), so a test
/// sees it asked, and whether an editor card of its own is open or a drawing of its own is under
/// way, which a test sets to see the shell ask whether the notation is editing, whether a menu of
/// its own is open on the canvas, which holds the keys, and a mark a test leaves to see whose state
/// it is reading.
#[derive(Default)]
struct SketchState {
    told: Option<Option<archforge_mof::Uuid>>,
    card: bool,
    drawing: bool,
    menu: bool,
    mark: u8,
}

impl archforge_shell_api::notation_ui::NotationState for SketchState {
    /// While its card is open or its drawing under way.
    fn editing(&self) -> bool {
        self.card || self.drawing
    }
    /// While its menu is open on the canvas.
    fn holds_keys(&self) -> bool {
        self.menu
    }
}

impl crate::notation_ui::NotationUi for SketchUi {
    /// The toy metamodel's, the fake notation's model.
    fn metamodel(&self) -> u16 {
        archforge_mof::toy::INDEX
    }
    /// A package is a top row of the Models tree.
    fn is_top(&self, store: &archforge_mof::Store, member: archforge_mof::Obj) -> bool {
        store.class_of(member).ok() == Some(archforge_mof::toy::PACKAGE)
    }
    /// The package, its classes under it and each class's attributes under the class; a filter
    /// keeps the rows whose names hold it and the rows they sit under.
    fn tree(&self, store: &archforge_mof::Store, pkg: archforge_mof::Obj, t: &crate::explorer::Tree<'_>) -> Vec<crate::explorer::Row> {
        let mut under = Vec::new();
        for c in sketch_classes(store, pkg) {
            let label = archforge_core::name_of(store, c);
            let attributes: Vec<archforge_mof::Obj> = store
                .list(c, archforge_mof::toy::CLASS_OWNED_ATTRIBUTE)
                .unwrap_or_default()
                .iter()
                .filter_map(archforge_mof::Value::as_ref)
                .collect();
            let rows: Vec<crate::explorer::Row> = attributes
                .iter()
                .filter_map(|a| {
                    let name = archforge_core::name_of(store, *a);
                    if t.matches(&name) {
                        t.row(*a, 2, "attribute", &crate::ux::icons::LIST, name).map(|r| crate::explorer::Row { muted: true, ..r })
                    } else {
                        None
                    }
                })
                .collect();
            let expandable = if t.filtering() { !rows.is_empty() } else { !attributes.is_empty() };
            if t.matches(&label) || !rows.is_empty() {
                let expanded = expandable && t.open(c);
                under.extend(t.row(c, 1, "class", &crate::ux::icons::TABLE, label).map(|r| crate::explorer::Row {
                    expandable,
                    expanded,
                    draggable: true,
                    ..r
                }));
                if expanded {
                    under.extend(rows);
                }
            }
        }
        let label = archforge_core::name_of(store, pkg);
        let expandable = if t.filtering() { !under.is_empty() } else { !sketch_classes(store, pkg).is_empty() };
        let mut out = Vec::new();
        if t.matches(&label) || !under.is_empty() {
            let expanded = expandable && t.open(pkg);
            out.extend(t.row(pkg, 0, "package", &crate::ux::icons::SCHEMA, label).map(|r| crate::explorer::Row { expandable, expanded, ..r }));
            if expanded {
                out.extend(under);
            }
        }
        out
    }
    /// A package for Models' `+`, and a class beside it once a package or a class is selected.
    fn offers(&self, store: &archforge_mof::Store, selection: Option<archforge_mof::Obj>) -> Vec<crate::notation_ui::Offer> {
        let offer = |key, word: &str| crate::notation_ui::Offer { key, word: word.into() };
        match sketch_package_of(store, selection) {
            Some(_) => vec![offer("class", "Class"), offer("package", "Package")],
            None => vec![offer("package", "Package")],
        }
    }
    /// A package named `Package 1` or the next free number, or a class of the selection's package
    /// named so, through the fake notation's commands.
    fn add(&self, cx: &mut crate::cx::Ctx, key: &str, sel: Option<archforge_mof::Obj>) -> Option<Result<crate::notation_ui::Added, String>> {
        let store = cx.session.as_ref()?.doc();
        let names = |list: Vec<archforge_mof::Obj>| list.into_iter().map(|o| archforge_core::name_of(store, o)).collect::<Vec<_>>();
        let (out, parents) = match key {
            "package" => {
                let name = archforge_shell_api::explorer::fresh_name("Package", names(sketch_packages(store)).into_iter(), false)?;
                (archforge_shell_api::panels::run_cx(cx, "sketch.package.add", serde_json::json!({ "name": name })), Vec::new())
            }
            "class" => {
                let package = sketch_package_of(store, sel)?;
                let name = archforge_shell_api::explorer::fresh_name("Class", names(sketch_classes(store, package)).into_iter(), false)?;
                let id = store.uuid(package).ok()?;
                (
                    archforge_shell_api::panels::run_cx(cx, "sketch.class.add", serde_json::json!({ "package": id.to_string(), "name": name })),
                    vec![id],
                )
            }
            _ => return None,
        };
        Some(
            out.and_then(|v| archforge_mof::Uuid::parse(v["id"].as_str().unwrap_or_default()).ok_or_else(|| "no id".to_string()))
                .map(|id| crate::notation_ui::Added { id, parents }),
        )
    }
    /// Grows a class's box an earlier version left shorter than a row for its name and one for each
    /// attribute, as a notation whose boxes show their rows does.
    fn open_repair(&self, store: &mut archforge_mof::Store) {
        let mut grow = Vec::new();
        for c in store.alive().filter(|o| store.class_of(*o).ok() == Some(archforge_mof::toy::CLASS)).collect::<Vec<_>>() {
            let rows = store.list(c, archforge_mof::toy::CLASS_OWNED_ATTRIBUTE).unwrap_or_default().len();
            let needed = 24.0 * (1 + rows) as f64;
            for n in archforge_diagram::views_of(store, c).2 {
                let height = match store.get(n, archforge_diagram::node::HEIGHT) {
                    Ok(archforge_mof::Value::Real(h)) => h,
                    _ => needed,
                };
                if height < needed {
                    grow.push((n, needed));
                }
            }
        }
        for (n, height) in grow {
            store.set(n, archforge_diagram::node::HEIGHT, archforge_mof::Value::Real(height)).unwrap();
        }
    }
    /// A package and a class are what File → Generate… can be about.
    fn scope_word(&self, store: &archforge_mof::Store, obj: archforge_mof::Obj) -> Option<String> {
        match store.class_of(obj).ok()? {
            c if c == archforge_mof::toy::PACKAGE => Some("Package".into()),
            c if c == archforge_mof::toy::CLASS => Some("Class".into()),
            _ => None,
        }
    }
    /// A package's or a class's name, through its own rename command.
    fn rename(&self, store: &archforge_mof::Store, obj: archforge_mof::Obj) -> Option<(&'static str, &'static str)> {
        match store.class_of(obj).ok()? {
            c if c == archforge_mof::toy::PACKAGE => Some(("sketch.package.rename", "package")),
            c if c == archforge_mof::toy::CLASS => Some(("sketch.class.rename", "class")),
            _ => None,
        }
    }
    /// Its one rename that may empty a name, which no other notation knows, so a test sees it asked.
    fn may_be_unnamed(&self, cmd: &str) -> bool {
        cmd == SKETCH_UNNAMED
    }
    /// The package and its classes, the rows that open.
    fn foldable(&self, store: &archforge_mof::Store, pkg: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
        std::iter::once(pkg).chain(sketch_classes(store, pkg)).collect()
    }
    /// What an element sits in: an attribute's class and its package, a class's package.
    fn parents(&self, store: &archforge_mof::Store, o: archforge_mof::Obj) -> Vec<archforge_mof::Obj> {
        std::iter::successors(store.container(o).ok().flatten().map(|(c, _)| c), |c| store.container(*c).ok().flatten().map(|(p, _)| p))
            .filter(|c| store.class_of(*c).ok() != Some(archforge_core::MODEL))
            .collect()
    }
    /// None, but for the sketch named `second`: a row of its own, so a test tells two sketches'
    /// tables apart. Only a shell given the sketches' screens parts registers it.
    fn catalogues(&self) -> &'static [crate::i18n::Rows] {
        match self.0 {
            "sketch" => SKETCH_CATALOGUE,
            "second" => SECOND_SKETCH_ROWS,
            _ => &[],
        }
    }
    /// The package and the class, which its Models `+` makes, and the generalization and the
    /// association, which its commands make.
    fn created_classes(&self) -> &'static [archforge_mof::ClassId] {
        &[archforge_mof::toy::PACKAGE, archforge_mof::toy::CLASS, archforge_mof::toy::GENERALIZATION, archforge_mof::toy::ASSOCIATION]
    }
    /// Its one refusal of its own, something that is no sketch, signed with its name, so a test
    /// sees it asked.
    fn said(&self, _: &[&'static dyn crate::notation_ui::NotationUi], reason: &archforge_engine::Reason) -> Option<String> {
        match reason {
            archforge_engine::Reason::NotA { kind: "sketch", name } => Some(format!("{name} is not drawn in the {}.", self.0)),
            _ => None,
        }
    }
    /// Why its one command is not available, signed with its name, so a test sees it asked.
    fn disabled(&self, reason: &str) -> Option<String> {
        (reason == SKETCH_UNAVAILABLE).then(|| format!("The {} has nothing to draw yet.", self.0))
    }
    /// Its kinds with their article, the sketch by its name, so a test sees it asked.
    fn a_kind(&self, kind: &str) -> Option<String> {
        Some(match kind {
            "sketch" => format!("a {}", self.0),
            "package" | "class" => format!("a {kind}"),
            "attribute" | "operation" => format!("an {kind}"),
            _ => return None,
        })
    }
    /// A class's attributes as its fields, by its name, so a test sees it asked.
    fn list_heading(&self, class: archforge_mof::ClassId) -> Option<String> {
        (class == archforge_mof::toy::PROPERTY).then(|| format!("Fields of the {}", self.0))
    }
    /// A skill for every class, signed with its name, so a test sees it asked; none for the other
    /// kinds of its metamodel, which leaves the shell's words.
    fn skill_target(&self, kind: archforge_mof::ClassId, _: &str) -> Option<String> {
        (kind == archforge_mof::toy::CLASS).then(|| format!("every class of the {}", self.0))
    }
    /// Chat counts its class as a kind of its own, its attribute as a line and its operation as a
    /// part of its class, unlike UML, so a test sees each asked; a description's comment it leaves
    /// to be counted as an element.
    fn chat_kind(&self, table: &str) -> Option<crate::notation_ui::ChatKind> {
        Some(match table {
            "sketch_class" => crate::notation_ui::ChatKind::Own("sketched"),
            "sketch_property" => crate::notation_ui::ChatKind::Line,
            "sketch_operation" => crate::notation_ui::ChatKind::Part,
            _ => return None,
        })
    }
    /// Its classes in Chat's count, signed with its name, so a test sees it asked.
    fn chat_count(&self, kind: &str, n: u64) -> Option<String> {
        (kind == "sketched").then(|| format!("{n} drawn in the {}", self.0))
    }
    /// Shorter than any other notation's, so a field that takes it is told apart.
    fn name_limit(&self) -> usize {
        SKETCH_NAME_LIMIT
    }
    /// Shorter than any other notation's, so a field that takes it is told apart.
    fn description_limit(&self) -> usize {
        SKETCH_DESCRIPTION_LIMIT
    }
    /// The sketch keeps nothing of its own between frames but the selection it was last told of.
    fn new_state(&self) -> Box<dyn archforge_shell_api::notation_ui::NotationState> {
        Box::new(SketchState::default())
    }
    /// The sketch removes nothing its own way: Delete goes on to the shared removals.
    fn canvas_delete(&self, _: &mut crate::cx::Ctx, _: bool) -> bool {
        false
    }
    /// A class's attributes go with it, counted in its own words and signed with its name, so a
    /// test sees it asked and the dialog say it; nothing goes with anything else.
    fn consequences(&self, _: &[&'static dyn crate::notation_ui::NotationUi], store: &archforge_mof::Store, obj: archforge_mof::Obj) -> Vec<String> {
        let attributes = store.list(obj, archforge_mof::toy::CLASS_OWNED_ATTRIBUTE).map(|l| l.len()).unwrap_or_default();
        let class = store.class_of(obj).ok() == Some(archforge_mof::toy::CLASS);
        (class && attributes > 0).then(|| format!("{attributes} attributes go with it in the {}.", self.0)).into_iter().collect()
    }
    /// Names each element it is asked about and counts them, signed with its name, so a test sees
    /// what reached it and which of two answered.
    fn before_removal(&self, store: &archforge_mof::Store, objs: &[archforge_mof::Obj]) -> crate::notation_ui::Removal {
        crate::notation_ui::Removal {
            went: objs.iter().map(|o| format!("{} in the {}", archforge_core::name_of(store, *o), self.0)).collect(),
            after: Some(format!("the {} saw {} go", self.0, objs.len())),
        }
    }
    /// Says the word and how many names its removal gave, in its own words.
    fn removed_text(&self, what: &str, removal: &crate::notation_ui::Removal) -> String {
        format!("{what} gone from the {} with {} named", self.0, removal.went.len())
    }
    /// The sketch draws nothing over the canvas.
    fn canvas_overlay(&self, _: &mut crate::cx::Ctx, _: &mut egui::Ui, _: crate::notation_ui::Layer, _: &crate::notation_ui::Overlaid<'_>) {}
    /// The sketch notation's search entries: a class, under its package, and an attribute, under its class.
    fn entry(&self, store: &archforge_mof::Store, o: archforge_mof::Obj) -> Option<archforge_shell_api::notation_ui::Entry> {
        let owner = store.container(o).ok().flatten().map(|(c, _)| archforge_core::name_of(store, c)).unwrap_or_default();
        let (icon, order, kind) = match store.class_of(o).ok()? {
            c if c == archforge_mof::toy::CLASS => (&crate::ux::icons::TABLE, 2, "Class"),
            c if c == archforge_mof::toy::PROPERTY => (&crate::ux::icons::LIST, 3, "Attribute"),
            _ => return None,
        };
        Some(archforge_shell_api::notation_ui::Entry { icon, order, label: archforge_core::name_of(store, o), detail: format!("{kind} · {owner}") })
    }
    /// A class's words, as Properties names it.
    fn words(&self, store: &archforge_mof::Store, o: archforge_mof::Obj) -> Option<crate::notation_ui::Words> {
        Some(crate::notation_ui::Words {
            kind: "Class · sketch".into(),
            word: "Class".into(),
            this: "This class".into(),
            on_this: "On this class · {n}".into(),
            name: archforge_core::name_of(store, o),
        })
    }
    /// Keeps the selection it is told of, so a test sees it asked.
    fn selection_moved(&self, cx: &mut crate::cx::Ctx, subject: Option<archforge_mof::Uuid>) {
        if let Some(state) = cx.notation_states.own_mut::<SketchState>() {
            state.told = Some(subject);
        }
    }
    /// With two or more elements selected, all of them its classes, a row naming how many, in
    /// place of the selection's panel.
    fn properties_in_place(&self, cx: &mut crate::cx::Ctx, ui: &mut egui::Ui, p: &crate::ux::Palette, r: egui::Rect, y: f32) -> bool {
        let Some(session) = cx.session.as_ref() else { return false };
        let (store, selection) = (session.doc(), &session.view().selection);
        if selection.len() < 2 || !selection.iter().all(|o| store.class_of(*o).ok() == Some(archforge_mof::toy::CLASS)) {
            return false;
        }
        let row = egui::Rect::from_min_size(egui::pos2(r.left() + 16.0, r.top() + y), egui::vec2(r.width() - 32.0, 24.0));
        let text = format!("{} classes", selection.len());
        crate::panels::control_cx(cx, ui, "workspace.properties.sketch.selection", "row", row, true);
        crate::ux::text(&ui.painter().clone(), row.min, &text, crate::ux::types::UI, p.text, None);
        true
    }
    /// A class's attributes in a section of the shared controls, a row each, then the shared name
    /// field.
    fn body(&self, cx: &mut crate::cx::Ctx, ui: &mut egui::Ui, p: &crate::ux::Palette, r: egui::Rect, y: f32, subject: &crate::panels::Subject) {
        let attributes = cx
            .session
            .as_ref()
            .and_then(|s| {
                let store = s.doc();
                Some(
                    store
                        .list(subject.obj?, archforge_mof::toy::CLASS_OWNED_ATTRIBUTE)
                        .ok()?
                        .iter()
                        .filter_map(archforge_mof::Value::as_ref)
                        .map(|a| archforge_core::name_of(store, a))
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        let y = crate::panels::section_cx(cx, ui, p, r, y, "Attributes", None).unwrap_or(y + 50.0);
        for (i, a) in attributes.iter().enumerate() {
            let row = egui::Rect::from_min_size(egui::pos2(r.left() + 16.0, y + i as f32 * 28.0), egui::vec2(r.width() - 32.0, 24.0));
            crate::panels::control_cx(cx, ui, &format!("workspace.properties.sketch.attribute.{i}"), "row", row, true);
            crate::ux::text(&ui.painter().clone(), row.min, a, crate::ux::types::UI, p.text, None);
        }
        // The shared name field, under the attributes.
        let at = egui::pos2(r.left() + 16.0, y + attributes.len() as f32 * 28.0 + 8.0);
        crate::panels::name_field_cx(cx, ui, p, at, r.width() - 32.0, subject, "sketch.class.rename", "class");
    }
}

/// The sketch notation's screens alone, as the shell the tests make is given them.
static SKETCH_UIS: &[&dyn crate::notation_ui::NotationUi] = &[&SketchUi("sketch")];

/// Two sketch notations told apart by name, both answering for UML's metamodel, so a test sees
/// which of them answers and in what order.
static FIRST_SKETCH: SketchUi = SketchUi("first");
static SECOND_SKETCH: SketchUi = SketchUi("second");
static TWO_SKETCH_UIS: &[&dyn crate::notation_ui::NotationUi] = &[&FIRST_SKETCH, &SECOND_SKETCH];

/// The sketch notation's catalogue table, one catalogue for each language in the languages' order
/// with its word and the access letters of its kind's Model menu rows, so the shell's tests see a
/// notation's table registered and looked up beside the shell's own.
static SKETCH_CATALOGUE: &[crate::i18n::Rows] = &[
    crate::i18n::Rows {
        code: "en",
        catalogue: "sketch.tsv",
        text: "# The sketch notation's catalogue, for the shell's tests.\nstr\t\tSketch\tSketch\nstr\taccess\tmodel.sketch-class\tC\nstr\taccess\tmodel.sketch-inherit\tI\n",
    },
    crate::i18n::Rows {
        code: "de",
        catalogue: "sketch-de.tsv",
        text: "# The sketch notation's catalogue, for the shell's tests.\n# Provenance: translated by an AI model, Claude Opus 5.5 (claude-opus-5-5), on 2026-10-27, from the meaning of the English source in en.tsv; screened by the catalogue screen test; not yet reviewed by a native speaker. Rows added later carry their provenance in their own commit. The words for what the metamodel names (AR-361) were translated by the same model on 2026-11-10, likewise from the English source; they are not yet reviewed by a native speaker either.\nstr\t\tSketch\tSkizze\nstr\taccess\tmodel.sketch-class\tC\nstr\taccess\tmodel.sketch-inherit\tI\n",
    },
    crate::i18n::Rows {
        code: "es",
        catalogue: "sketch-es.tsv",
        text: "# The sketch notation's catalogue, for the shell's tests.\n# Provenance: translated by an AI model, Claude Opus 5.5 (claude-opus-5-5), on 2026-10-27, from the meaning of the English source in en.tsv; screened by the catalogue screen test; not yet reviewed by a native speaker. Rows added later carry their provenance in their own commit. The words for what the metamodel names (AR-361) were translated by the same model on 2026-11-10, likewise from the English source; they are not yet reviewed by a native speaker either.\nstr\t\tSketch\tBoceto\nstr\taccess\tmodel.sketch-class\tC\nstr\taccess\tmodel.sketch-inherit\tI\n",
    },
    crate::i18n::Rows {
        code: "fr",
        catalogue: "sketch-fr.tsv",
        text: "# The sketch notation's catalogue, for the shell's tests.\n# Provenance: translated by an AI model, Claude Opus 5.5 (claude-opus-5-5), on 2026-10-27, from the meaning of the English source in en.tsv; screened by the catalogue screen test; not yet reviewed by a native speaker. Rows added later carry their provenance in their own commit. The words for what the metamodel names (AR-361) were translated by the same model on 2026-11-10, likewise from the English source; they are not yet reviewed by a native speaker either.\nstr\t\tSketch\tCroquis\nstr\taccess\tmodel.sketch-class\tC\nstr\taccess\tmodel.sketch-inherit\tI\n",
    },
    crate::i18n::Rows {
        code: "pt-PT",
        catalogue: "sketch-pt-PT.tsv",
        text: "# The sketch notation's catalogue, for the shell's tests.\n# Provenance: translated by an AI model, Claude Opus 5.5 (claude-opus-5-5), on 2026-10-27, from the meaning of the English source in en.tsv; screened by the catalogue screen test; not yet reviewed by a native speaker. Rows added later carry their provenance in their own commit. The words for what the metamodel names (AR-361) were translated by the same model on 2026-11-10, likewise from the English source; they are not yet reviewed by a native speaker either.\nstr\t\tSketch\tEsboço\nstr\taccess\tmodel.sketch-class\tC\nstr\taccess\tmodel.sketch-inherit\tI\n",
    },
    crate::i18n::Rows {
        code: "ja",
        catalogue: "sketch-ja.tsv",
        text: "# The sketch notation's catalogue, for the shell's tests.\n# Provenance: translated by an AI model, Claude Opus 5.5 (claude-opus-5-5), on 2026-10-27, from the meaning of the English source in en.tsv; screened by the catalogue screen test; not yet reviewed by a native speaker. Rows added later carry their provenance in their own commit. The words for what the metamodel names (AR-361) were translated by the same model on 2026-11-10, likewise from the English source; they are not yet reviewed by a native speaker either.\nstr\t\tSketch\tスケッチ\nstr\taccess\tmodel.sketch-class\tC\nstr\taccess\tmodel.sketch-inherit\tI\n",
    },
    crate::i18n::Rows {
        code: "zh-Hans",
        catalogue: "sketch-zh-Hans.tsv",
        text: "# The sketch notation's catalogue, for the shell's tests.\n# Provenance: translated by an AI model, Claude Opus 5.5 (claude-opus-5-5), on 2026-10-27, from the meaning of the English source in en.tsv; screened by the catalogue screen test; not yet reviewed by a native speaker. Rows added later carry their provenance in their own commit. The words for what the metamodel names (AR-361) were translated by the same model on 2026-11-10, likewise from the English source; they are not yet reviewed by a native speaker either.\nstr\t\tSketch\t草图\nstr\taccess\tmodel.sketch-class\tC\nstr\taccess\tmodel.sketch-inherit\tI\n",
    },
];

/// The second sketch's catalogue, a row only it says, in English and in German.
static SECOND_SKETCH_ROWS: &[crate::i18n::Rows] = &[
    crate::i18n::Rows { code: "en", catalogue: "second-sketch.tsv", text: "str\t\tSecond sketch\tSecond sketch\n" },
    crate::i18n::Rows { code: "de", catalogue: "second-sketch-de.tsv", text: "str\t\tSecond sketch\tZweite Skizze\n" },
];

/// The popover of kinds the second sketch's kind walks with the keys.
const SECOND_KINDS: &str = "second.kinds";

/// The popover of kinds the sketch kind walks with the keys.
const SKETCH_POPOVER: &str = "sketch.kinds";

/// Two more sketch kinds, both of the second sketch's part, so a part with more than one kind
/// lists them in its order.
static SECOND_SKETCH_KINDS: [SketchKind; 2] = [SketchKind("test.second"), SketchKind("test.third")];

/// A screens registry of two parts: the sketch with its kind, then the second sketch with two
/// kinds of its own.
static SKETCH_SCREENS: archforge_shell_api::screens::ScreensRegistry = archforge_shell_api::screens::ScreensRegistry::new(&[
    archforge_shell_api::screens::ScreensPart { ui: &FIRST_SKETCH, kinds: &[&SketchKind(SKETCH)] },
    archforge_shell_api::screens::ScreensPart { ui: &SECOND_SKETCH, kinds: &[&SECOND_SKETCH_KINDS[0], &SECOND_SKETCH_KINDS[1]] },
]);

/// A screens registry gives the shell its notations, its diagram kinds and the notations'
/// catalogue tables from the parts it was given: every part's notation in the parts' order, every
/// part's kinds part after part, each part's in its own order, and each part's table in the
/// parts' order. The lists are made once: asked again, the same lists come back.
#[test]
fn a_screens_registry_lists_its_parts_notations_kinds_and_catalogues_in_order() {
    let parts = SKETCH_SCREENS.parts();
    assert_eq!(parts.len(), 2);
    let uis = SKETCH_SCREENS.notation_uis();
    let first: &dyn crate::notation_ui::NotationUi = &FIRST_SKETCH;
    let second: &dyn crate::notation_ui::NotationUi = &SECOND_SKETCH;
    assert_eq!(uis.len(), 2, "one notation for each part");
    assert!(std::ptr::addr_eq(uis[0], first) && std::ptr::addr_eq(uis[1], second), "the parts' notations, in the parts' order");
    assert!(std::ptr::addr_eq(parts[0].ui, first) && std::ptr::addr_eq(parts[1].ui, second));
    let kinds: Vec<&str> = SKETCH_SCREENS.kinds().iter().map(|k| k.kind()).collect();
    assert_eq!(kinds, [SKETCH, "test.second", "test.third"]);
    let tables: Vec<Vec<&str>> = SKETCH_SCREENS.catalogues().map(|t| t.iter().map(|r| r.catalogue).collect()).collect();
    assert_eq!(tables, [vec![], vec!["second-sketch.tsv", "second-sketch-de.tsv"]]);
    assert!(std::ptr::eq(SKETCH_SCREENS.notation_uis(), uis), "made once");
    assert!(std::ptr::eq(SKETCH_SCREENS.kinds(), SKETCH_SCREENS.kinds()), "made once");
}

/// A shell takes its notations, its diagram kinds, its notations' states and its catalogues from
/// the screens parts it is given, not from any fixed list: given the sketches' parts, it knows the
/// two sketches and their three kinds, keeps the sketch's state, and finds the second sketch's row
/// in German, which no other shell registers.
#[test]
fn a_shell_takes_its_notations_kinds_states_and_catalogues_from_the_screens_parts_it_is_given() {
    let shell = Shell::new(Services::default(), NOTATIONS, &SKETCH_SCREENS);
    assert!(std::ptr::eq(shell.notation_uis, SKETCH_SCREENS.notation_uis()), "the parts' notations");
    let kinds: Vec<&str> = shell.kinds.iter().map(|k| k.kind()).collect();
    assert_eq!(kinds, [SKETCH, "test.second", "test.third"], "the parts' kinds");
    assert!(shell.notation_states.own::<SketchState>().is_some(), "the sketch keeps its state");
    let row = "Second sketch";
    crate::i18n::set_language("de");
    let said = crate::i18n::tr(row);
    crate::i18n::set_language("en");
    assert_eq!(said, "Zweite Skizze", "the second sketch's catalogue is registered");
}

/// The sketch notation's limits, shorter than any other notation's, so a field that takes them is
/// told apart from one that takes another's.
const SKETCH_NAME_LIMIT: usize = 40;
const SKETCH_DESCRIPTION_LIMIT: usize = 100;

/// Why the sketch's one command is not available, as its availability check would give it.
const SKETCH_UNAVAILABLE: &str = "nothing to draw";

/// The sketch's rename that may leave its element unnamed.
const SKETCH_UNNAMED: &str = "sketch.link.rename";

/// The refusal words ask the notations the shell registered, not the app's own list: the sketch,
/// which the shell knows here and the app does not, says its own refusal, names its own kind in the
/// shell's sentence and says why its command is not available, each through an engine's refusal as
/// the shell says it; a shell given no notation, which does not know the sketch, leaves the
/// engine's English.
#[test]
fn the_refusal_words_ask_the_notations_the_shell_registered() {
    use archforge_engine::{CommandError, EngineError, Reason};
    let (mut h, ..) = sketch_harness();
    let failed = |reason| EngineError::Failed { id: "sketch.class.add".into(), error: CommandError::new("refused in English").because(reason) };
    let refusals = || {
        [
            failed(Reason::NotA { kind: "sketch", name: "Party".into() }),
            failed(Reason::NameTaken { kind: "sketch", name: "Party".into() }),
            EngineError::Disabled { id: "sketch.draw".into(), reason: SKETCH_UNAVAILABLE },
        ]
    };
    let said = h.state_mut().cx(|cx| refusals().map(|e| crate::panels::refusal(cx.notation_uis, &e)));
    assert_eq!(said, ["Party is not drawn in the sketch.", "There is a sketch named Party already.", "The sketch has nothing to draw yet."]);
    let none: &[&'static dyn crate::notation_ui::NotationUi] = &[];
    let alone = refusals().map(|e| crate::panels::refusal(none, &e));
    assert_eq!(alone, ["Refused in English.", "Refused in English.", "Nothing to draw."]);
}

/// Whether a name of nothing but spaces is refused is asked of the notations the shell registered,
/// not of a fixed list: the sketch may leave its element unnamed, and its classes, which it never
/// lets go unnamed, are refused; a shell given no notation refuses the sketch's rename too, as the
/// sketch is not its own.
#[test]
fn an_empty_name_is_refused_by_the_notations_the_shell_registered() {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
    assert!(!shell.cx(|cx| archforge_shell_api::panels::refuse_empty_name_cx(cx, SKETCH_UNNAMED, "  ")), "the sketch may leave its element unnamed");
    assert_eq!(shell.panels.notice, None);
    assert!(shell.cx(|cx| archforge_shell_api::panels::refuse_empty_name_cx(cx, "sketch.class.rename", "  ")), "a class is never unnamed");
    assert_eq!(shell.panels.notice.take().as_deref(), Some("A name can't be empty."));
    shell.notation_uis = &[];
    assert!(shell.cx(|cx| archforge_shell_api::panels::refuse_empty_name_cx(cx, SKETCH_UNNAMED, "  ")), "the sketch is not this shell's");
    assert_eq!(shell.panels.notice.take().as_deref(), Some("A name can't be empty."));
}

/// Chat counts what a tool row names as the notations the shell registered say, not as a fixed
/// list: the sketch counts a class as a kind of its own in its words, an attribute as a line and an
/// operation as a part of its class, which is not counted, where a shell given no notation counts
/// each as an element; a kind it does not know is an element. The tool rows the window draws and the changes a turn keeps ask the shell's notations.
#[test]
fn chats_counts_ask_the_notations_the_shell_registered() {
    use crate::chat_store::{Anchor, AnchorRole, ToolFacts, ToolStatus};
    use crate::chat_words::{counted, tool_words};
    crate::i18n::set_language("en");
    let at =
        |n: u8, role: AnchorRole, metaclass: &str| Anchor { element: archforge_mof::Uuid([n; 16]), role, metaclass: metaclass.into(), name: None };
    let read = |anchors: Vec<Anchor>| ToolFacts {
        tool: "engine.execute".into(),
        command: Some("sketch.class.list".into()),
        detail: None,
        status: ToolStatus::Done,
        changed_model: false,
        element_count: None,
        anchors,
    };
    let none: &[&'static dyn crate::notation_ui::NotationUi] = &[];
    let mut named: Vec<Anchor> = (1..=3).map(|n| at(n, AnchorRole::Read, "sketch_class")).collect();
    named.extend((4..=5).map(|n| at(n, AnchorRole::Read, "sketch_property")));
    named.extend((6..=9).map(|n| at(n, AnchorRole::Read, "sketch_operation")));
    assert_eq!(tool_words(SKETCH_UIS, &read(named.clone())).0, "Read 3 drawn in the sketch, 2 lines");
    assert_eq!(tool_words(none, &read(named)).0, "Read 4 elements, 3 elements");
    assert!(!counted(SKETCH_UIS, "sketch_operation") && counted(none, "sketch_operation"));
    // What a row is about, unnamed: one of its kind, several of one kind, or elements of several.
    let mut added = read(vec![at(1, AnchorRole::Target, "sketch_class")]);
    (added.command, added.changed_model) = (Some("sketch.class.add".into()), true);
    assert_eq!(tool_words(SKETCH_UIS, &added).0, "Added 1 drawn in the sketch");
    added.anchors.push(at(2, AnchorRole::Target, "sketch_class"));
    assert_eq!(tool_words(SKETCH_UIS, &added).0, "Added 2 drawn in the sketch");
    added.anchors.push(at(3, AnchorRole::Target, "sketch_property"));
    assert_eq!(tool_words(SKETCH_UIS, &added).0, "Added 3 elements");
    // The window's tool row asks the shell's notations.
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "What is on the diagram?");
    let diagram = h.state().canvas.active.unwrap();
    crate::chat_fixture::call(
        h.state_mut(),
        "engine.execute",
        serde_json::json!({ "command": "diagram.get", "params": { "diagram": diagram.to_string() } }),
    )
    .unwrap();
    h.run();
    assert!(painted(&h).iter().any(|t| t.contains("drawn in the sketch")), "{:?}", painted(&h));
    // What a read's answer named is kept as the sketch counts it: Customer's operation, a part of
    // its class to the sketch, is neither kept nor in the row's count.
    let customer = crate::chat_fixture::element(h.state(), archforge_mof::toy::CLASS, "Customer").unwrap();
    crate::chat_fixture::call(
        h.state_mut(),
        "engine.execute",
        serde_json::json!({ "command": "sketch.class.get", "params": { "class": customer.to_string() } }),
    )
    .unwrap();
    h.run();
    let facts = kept_chat(&h, &rig)
        .messages
        .iter()
        .rev()
        .find_map(|m| if let crate::chat_store::StoredBody::Tool(f) = &m.body { Some(f.clone()) } else { None })
        .unwrap();
    let kept = facts.anchors.iter().filter(|a| a.role == AnchorRole::Read).count();
    assert!(kept > 0 && facts.anchors.iter().all(|a| a.metaclass != "sketch_operation"), "{facts:?}");
    assert_eq!(facts.element_count, Some(u32::try_from(kept).unwrap()), "{facts:?}");
    chat_event(&mut h, &rig, crate::chat::ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    // A turn's changes keep what the sketch counts: a description's comment is listed beside the
    // class described.
    let at = rect_of(&h, "workspace.chat.allow-changes").center();
    click(&mut h, at);
    ask_in_chat(&mut h, "Describe Customer");
    crate::chat_fixture::call(
        h.state_mut(),
        "engine.execute",
        serde_json::json!({ "command": "element.describe", "params": { "element": customer.to_string(), "text": "A buyer" } }),
    )
    .unwrap();
    h.run();
    chat_event(&mut h, &rig, crate::chat::ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    let changes = kept_chat(&h, &rig).ends.last().unwrap().1.clone().unwrap();
    let mut listed: Vec<String> = changes.changed.iter().map(|e| e.metaclass.clone()).collect();
    listed.sort();
    assert_eq!(listed, ["sketch_class", "sketch_comment"]);
}

/// A notation that answers for no element, as no class is of its metamodel, and only says words of
/// its own: an association with its article, a sentence for an enum type without literals, a row
/// of its own catalogue, and a repair that leaves a tag in the model. Registered before ER and UML,
/// it is asked before them, so a test sees that the words a notation says inside its own sentences,
/// a model's kind words, the catalogues and an opening's repair ask the notations the shell
/// registered, not the app's own list.
struct WordsUi;

/// The worded notation's word for an association with its article.
const WORDED_ASSOCIATION: &str = "a worded association";

/// The worded notation's sentence for an enum type without literals.
const WORDED_LITERALS: &str = "The words want a literal.";

/// The worded notation's own catalogue row, the same in every language but German.
const WORDED_ROW: &str = "A worded row";

/// The worded notation's catalogues: its one row in every language, German's own text.
static WORDED_CATALOGUES: &[crate::i18n::Rows] = &[
    crate::i18n::Rows { code: "en", catalogue: "words/en.tsv", text: "str\t\tA worded row\tA worded row\n" },
    crate::i18n::Rows { code: "de", catalogue: "words/de.tsv", text: "str\t\tA worded row\tEine Wortzeile\n" },
    crate::i18n::Rows { code: "es", catalogue: "words/es.tsv", text: "str\t\tA worded row\tA worded row\n" },
    crate::i18n::Rows { code: "fr", catalogue: "words/fr.tsv", text: "str\t\tA worded row\tA worded row\n" },
    crate::i18n::Rows { code: "pt-PT", catalogue: "words/pt-PT.tsv", text: "str\t\tA worded row\tA worded row\n" },
    crate::i18n::Rows { code: "ja", catalogue: "words/ja.tsv", text: "str\t\tA worded row\tA worded row\n" },
    crate::i18n::Rows { code: "zh-Hans", catalogue: "words/zh-Hans.tsv", text: "str\t\tA worded row\tA worded row\n" },
];

impl crate::notation_ui::NotationUi for WordsUi {
    /// A metamodel no class is of, so it answers for no element.
    fn metamodel(&self) -> u16 {
        u16::MAX
    }
    /// No element, so no top of Models.
    fn is_top(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> bool {
        false
    }
    /// No rows in Models.
    fn tree(&self, _: &archforge_mof::Store, _: archforge_mof::Obj, _: &crate::explorer::Tree<'_>) -> Vec<crate::explorer::Row> {
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
    fn before_removal(&self, _: &archforge_mof::Store, _: &[archforge_mof::Obj]) -> crate::notation_ui::Removal {
        crate::notation_ui::Removal::default()
    }
    /// The word alone, as none of its elements is removed.
    fn removed_text(&self, what: &str, _: &crate::notation_ui::Removal) -> String {
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
    fn entry(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<archforge_shell_api::notation_ui::Entry> {
        None
    }
    /// No words for Properties.
    fn words(&self, _: &archforge_mof::Store, _: archforge_mof::Obj) -> Option<crate::notation_ui::Words> {
        None
    }
    /// Draws nothing, as Properties shows none of its elements.
    fn body(&self, _: &mut crate::cx::Ctx, _: &mut egui::Ui, _: &crate::ux::Palette, _: egui::Rect, _: f32, _: &crate::panels::Subject) {}
    /// Its one row ([`WORDED_CATALOGUES`]).
    fn catalogues(&self) -> &'static [crate::i18n::Rows] {
        WORDED_CATALOGUES
    }
    /// Creates nothing.
    fn created_classes(&self) -> &'static [archforge_mof::ClassId] {
        &[]
    }
    /// None of its own, so the longest of the shell's is another notation's.
    fn name_limit(&self) -> usize {
        0
    }
    /// None of its own, so the longest of the shell's is another notation's.
    fn description_limit(&self) -> usize {
        0
    }
    /// Deletes nothing on the canvas.
    fn canvas_delete(&self, _: &mut crate::cx::Ctx, _: bool) -> bool {
        false
    }
    /// Draws nothing over the canvas.
    fn canvas_overlay(&self, _: &mut crate::cx::Ctx, _: &mut egui::Ui, _: crate::notation_ui::Layer, _: &crate::notation_ui::Overlaid<'_>) {}
    /// The sketch's state, unused.
    fn new_state(&self) -> Box<dyn archforge_shell_api::notation_ui::NotationState> {
        Box::new(SketchState::default())
    }
    /// Its sentence for an enum type without literals ([`WORDED_LITERALS`]), so a test sees it asked
    /// before ER.
    fn said(&self, _: &[&'static dyn crate::notation_ui::NotationUi], reason: &archforge_engine::Reason) -> Option<String> {
        matches!(reason, archforge_engine::Reason::TooFew { kind: "enum literal", min: 1 }).then(|| WORDED_LITERALS.to_string())
    }
    /// Its word for an association ([`WORDED_ASSOCIATION`]), so a test sees it asked before UML.
    fn a_kind(&self, kind: &str) -> Option<String> {
        (kind == "association").then(|| WORDED_ASSOCIATION.to_string())
    }
    /// Leaves a tag in the model, so a test sees an opening's repair ask it.
    fn open_repair(&self, store: &mut archforge_mof::Store) {
        let _ = store.create(archforge_core::TAG);
    }
}

/// The worded notation before the sketch.
static WORDS_UIS: &[&dyn crate::notation_ui::NotationUi] = &[&WordsUi, &SketchUi("sketch")];

/// The catalogues registered are those of the notations a shell is given: the worded notation's
/// row, which no notation of the app holds, is found in German once its catalogues are registered
/// with the notations it comes with, as a shell registers its own.
#[test]
fn the_catalogues_of_the_notations_a_shell_is_given_are_registered() {
    crate::i18n::register_notations(WORDS_UIS.iter().map(|ui| ui.catalogues()));
    crate::i18n::set_language("de");
    let row = WORDED_ROW;
    assert_eq!(crate::i18n::tr(row), "Eine Wortzeile");
    crate::i18n::set_language("en");
}

/// An opening repairs a model with the shell's notations, not the app's own list: the shell's
/// repair over the app's notations leaves an empty model as it is; with the worded notation
/// registered, its repair runs too and leaves its tag.
#[test]
fn an_opening_repairs_a_model_with_the_notations_the_shell_registered() {
    let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
    let repaired = |shell: &Shell| {
        let mut st = NOTATIONS
            .store(Box::new({
                let ids = test_ids();
                move || ids()
            }))
            .unwrap();
        st.create(archforge_core::MODEL).unwrap();
        (shell.open_repair())(&mut st);
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_core::TAG)).count()
    };
    assert_eq!(repaired(&shell), 0, "the app's notations");
    shell.notation_uis = WORDS_UIS;
    assert_eq!(repaired(&shell), 1, "the worded notation's repair runs too");
}

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

/// Every notation the shell registered is told of the selection on each frame the right panel
/// draws, whichever panel it shows, before that panel, so what it had typed for the element selected
/// before ends as leaving its field would: the sketch, which the app does not know, is told under
/// Chat, Appearance and Skills as under Properties.
#[test]
fn the_notations_are_told_of_the_selection_whichever_panel_shows() {
    use crate::state::Panel;
    let (mut h, ..) = sketch_harness();
    // The states of the notations the shell registered, the sketch's among them.
    h.state_mut().notation_states = crate::notation_ui::NotationStates::new(SKETCH_UIS);
    let (party, customer) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let o = |name: &str| st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == name).unwrap()).unwrap();
        (o("Party"), o("Customer"))
    };
    for (panel, picked) in [(Panel::Agent, party), (Panel::Appearance, customer), (Panel::Skills, party), (Panel::Properties, customer)] {
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [picked.to_string()] })).unwrap();
        h.state_mut().state.show_panel(panel);
        h.state_mut().notation_states.own_mut::<SketchState>().unwrap().told = None;
        h.run();
        assert_eq!(h.state().notation_states.own::<SketchState>().unwrap().told, Some(Some(picked)), "{panel:?}");
    }
}

/// A notation that draws its own panel in place of the selection's takes Properties: with two of
/// the sketch's classes selected, the sketch's row for them is drawn, and neither the selection's
/// heading and description nor the first class's body; with one selected, the selection's panel is
/// drawn as before.
#[test]
fn a_notations_own_panel_takes_properties_in_place_of_the_selections() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ..) = sketch_harness();
        h.state_mut().state.theme = theme;
        let (party, customer) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = |name: &str| st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == name).unwrap()).unwrap();
            (o("Party"), o("Customer"))
        };
        let select = |h: &mut Harness<'static, Shell>, picked: &[archforge_mof::Uuid]| {
            let picked: Vec<String> = picked.iter().map(ToString::to_string).collect();
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": picked })).unwrap();
            h.state_mut().state.show_panel(crate::state::Panel::Properties);
            h.run();
        };
        select(&mut h, &[party, customer]);
        let (drawn, texts) = (ids(&h), painted(&h));
        assert!(drawn.iter().any(|i| i == "workspace.properties.sketch.selection"), "{theme:?}: {drawn:?}");
        assert!(texts.iter().any(|t| t == "2 classes"), "{theme:?}: {texts:?}");
        assert!(!texts.iter().any(|t| t == "Class · sketch"), "{theme:?}: no heading of the selection: {texts:?}");
        assert!(
            !drawn.iter().any(|i| i == "workspace.properties.description" || i == "workspace.properties.sketch.attribute.0"),
            "{theme:?}: {drawn:?}"
        );
        select(&mut h, &[party]);
        let (drawn, texts) = (ids(&h), painted(&h));
        assert!(!drawn.iter().any(|i| i == "workspace.properties.sketch.selection"), "{theme:?}: {drawn:?}");
        assert!(texts.iter().any(|t| t == "Class · sketch"), "{theme:?}: {texts:?}");
        assert!(drawn.iter().any(|i| i == "workspace.properties.description"), "{theme:?}: {drawn:?}");
    }
}

/// A name typed in place takes as many characters as its element's notation lets it have
/// (AR-785): a class of the sketch notation, whose names are shorter than any other's, on the
/// canvas, in the Models tree and in Properties; a diagram's name the diagrams' rules allow, and
/// the filter of Models the most any name has. A description takes what its notation's does.
#[test]
fn a_name_typed_in_place_takes_what_its_notation_allows() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        names_typed_in_place(theme);
    }
}

/// [`a_name_typed_in_place_takes_what_its_notation_allows`] in one appearance.
fn names_typed_in_place(theme: ThemeKind) {
    use crate::notation_ui::{description_limit, longest_name, name_limit_of};
    let (mut h, d, a, ..) = sketch_harness();
    h.state_mut().state.theme = theme;
    h.run();
    let (pkg, party) = {
        let st = h.state().session.as_ref().unwrap().doc();
        let o = |name: &str| st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == name).unwrap()).unwrap();
        (o("sales"), o("Party"))
    };
    let longest = archforge_diagram::rules::MAX_NAME.max(SKETCH_NAME_LIMIT);
    assert!(SKETCH_NAME_LIMIT < longest, "the sketch's limit is told apart from the others'");
    assert_eq!(name_limit_of(h.state(), party), SKETCH_NAME_LIMIT);
    // A diagram's name takes the diagrams' own limit, never another notation's, so its field takes
    // no name `diagram.rename` refuses. Every limit is 128 today, so this names the constant
    // rather than telling the values apart.
    assert_eq!(name_limit_of(h.state(), archforge_mof::Uuid::parse(&d).unwrap()), archforge_diagram::rules::MAX_NAME, "a diagram");
    assert_eq!(name_limit_of(h.state(), archforge_mof::Uuid([7; 16])), longest, "an element not in the model");
    assert_eq!(longest_name(SKETCH_UIS), longest);
    {
        let st = h.state().session.as_ref().unwrap().doc();
        assert_eq!(description_limit(SKETCH_UIS, st, st.by_uuid(party).unwrap()), SKETCH_DESCRIPTION_LIMIT);
    }
    let typed = "y".repeat(longest + 20);
    // On the canvas: the name is selected whole, so typing replaces it, up to the sketch's limit.
    h.state_mut().canvas.rename = Some(crate::editing::NodeRename {
        node: archforge_mof::Uuid::parse(&a).unwrap(),
        element: party,
        text: "Party".into(),
        opened: true,
        refused: None,
    });
    h.run();
    h.event(egui::Event::Text(typed.clone()));
    h.run();
    assert_eq!(h.state().canvas.rename.as_ref().map(|r| r.text.chars().count()), Some(SKETCH_NAME_LIMIT), "on the canvas");
    h.state_mut().canvas.rename = None;
    // In the Models tree.
    h.state_mut().explorer.expanded.insert(pkg);
    h.state_mut().explorer.rename = Some(crate::explorer::Rename::fresh(party, "Party".into()));
    h.run();
    h.run();
    h.event(egui::Event::Text(typed.clone()));
    h.run();
    assert_eq!(h.state().explorer.rename.as_ref().map(|r| r.text.chars().count()), Some(SKETCH_NAME_LIMIT), "in the Models tree");
    h.state_mut().explorer.rename = None;
    h.run();
    // In Properties, the name and the description, each as the sketch's limits allow.
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [party.to_string()] })).unwrap();
    h.state_mut().state.show_panel(crate::state::Panel::Properties);
    h.run();
    for (field, limit) in [("workspace.properties.name", SKETCH_NAME_LIMIT), ("workspace.properties.description", SKETCH_DESCRIPTION_LIMIT)] {
        let at = rect_of(&h, field).center();
        click(&mut h, at);
        h.event(egui::Event::Text(typed.repeat(40)));
        h.run();
        let kept = match field {
            "workspace.properties.name" => h.state().panels.name.as_ref().map(|n| n.text.chars().count()),
            _ => h.state().panels.description.as_ref().map(|d| d.text.chars().count()),
        };
        assert_eq!(kept, Some(limit), "{field}");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    }
    // The filter takes any name whole.
    let at = rect_of(&h, "workspace.explorer.filter").center();
    click(&mut h, at);
    h.event(egui::Event::Text(typed));
    h.run();
    assert_eq!(h.state().explorer.filter.chars().count(), longest, "the filter");
}

/// The narrow context reaches the shell's own parts, each borrowed apart: two of them are held at
/// once, what is changed through it lands on the shell, what it only reads is the shell's, and its
/// read-only twin, from the shell or from the context, reads the same parts.
#[test]
fn the_narrow_context_reaches_the_shells_own_parts() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let shell = h.state_mut();
    let held = crate::widgets::egui_id("cx.held");
    shell.frames = 41;
    let (session, problems, uis): (*const _, *const _, *const _) = (&shell.session, &shell.problems, shell.notation_uis);
    let centre: *const _ = &shell.canvas;
    {
        let twin = shell.cx_ref();
        assert!(std::ptr::eq(twin.session, session) && std::ptr::eq(twin.notation_uis, uis) && !twin.looking_only, "the twin over the shell");
        assert!(std::ptr::eq(twin.canvas, centre), "the twin reads the shell's canvas");
    }
    let made = shell.cx(|cx| {
        assert_eq!((cx.frames, cx.looking_only), (41, false));
        assert!(std::ptr::eq(cx.problems, problems) && std::ptr::eq(cx.notation_uis, uis), "the problems and the notations are the shell's");
        let (panels, canvas, registry) = (&mut *cx.panels, &mut *cx.canvas, &mut *cx.registry);
        panels.notice = Some("said in Properties".into());
        canvas.notice = Some("said on the canvas".into());
        registry.record("cx.part", "button", egui::Rect::from_min_size(egui::pos2(1.0, 2.0), egui::vec2(3.0, 4.0)));
        *cx.menu_bar.held = Some(held);
        let made = crate::panels::run_cx(cx, "sketch.package.add", serde_json::json!({ "name": "sales" })).unwrap();
        let twin = cx.cx_ref();
        assert!(std::ptr::eq(twin.session, session) && std::ptr::eq(twin.notation_uis, uis) && !twin.looking_only, "the twin over the context");
        assert_eq!(twin.canvas.notice.as_deref(), Some("said on the canvas"), "the twin reads the context's canvas");
        assert!(std::ptr::eq(twin.canvas, centre), "the twin's canvas is the shell's");
        made
    });
    assert_eq!(shell.panels.notice.as_deref(), Some("said in Properties"));
    assert_eq!(shell.canvas.notice.as_deref(), Some("said on the canvas"));
    assert!(shell.registry.entries().iter().any(|e| e.id == "cx.part" && e.rect == [1.0, 2.0, 4.0, 6.0]), "recorded in the shell's registry");
    assert_eq!(shell.menu_bar.held, Some(held), "the bar holds the editor");
    let id = archforge_mof::Uuid::parse(made["id"].as_str().unwrap()).unwrap();
    assert!(shell.session.as_ref().unwrap().doc().by_uuid(id).is_some(), "the package is in the shell's model");
}

/// What the narrow context asks of the rest of the shell is carried out after the call returns, in
/// the order asked, once: the rows a selected element sits under open in Models only then, and
/// the next call does not open them again.
#[test]
fn the_narrow_contexts_requests_run_in_order_once_after_the_call() {
    use crate::cx::Request;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let (pkg, class) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let pkg = id(s.execute("sketch.package.add", &serde_json::json!({ "name": "sales" })).unwrap().value);
        (pkg, id(s.execute("sketch.class.add", &serde_json::json!({ "package": pkg.to_string(), "name": "Order" })).unwrap().value))
    };
    let other = archforge_mof::Uuid([7; 16]);
    let shell = h.state_mut();
    shell.explorer.expanded.clear();
    let asked = shell.cx(|cx| {
        cx.requests.push(Request::ExpandRows(vec![other]));
        archforge_shell_api::panels::select_element_cx(cx, class);
        cx.requests.push(Request::HideVersionDetails);
        cx.requests.clone()
    });
    let [Request::ExpandRows(first), Request::ExpandRows(under), Request::HideVersionDetails] = &asked[..] else {
        panic!("kept in the order asked: {asked:?}")
    };
    assert!(first == &vec![other] && under.contains(&pkg), "the rows asked first, then the rows the class sits under: {asked:?}");
    assert!(under.iter().chain([&other]).all(|row| shell.explorer.expanded.contains(row)), "carried out once the call returned");
    let s = shell.session.as_ref().unwrap();
    assert_eq!(s.view().selection.first().and_then(|o| s.doc().uuid(*o).ok()), Some(class), "the selection ran at once");
    shell.explorer.expanded.clear();
    shell.cx(|cx| assert!(cx.requests.is_empty(), "a new call starts with nothing asked"));
    assert!(shell.explorer.expanded.is_empty(), "a request runs once");
    // Asked alone in a context of its own, the rows open as that call returns.
    shell.cx(|cx| archforge_shell_api::panels::select_element_cx(cx, class));
    assert!(shell.explorer.expanded.contains(&pkg), "a context of its own carries the request out too");
}

/// The narrow context reaches each notation's own state, the shell's: what is changed through it
/// lands on the shell, and its read-only twin, from the shell or from the context, reads the same
/// states.
#[test]
fn the_narrow_context_reaches_each_notations_own_state() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let shell = h.state_mut();
    let states: *const _ = &shell.notation_states;
    assert!(std::ptr::eq(shell.cx_ref().notation_states, states), "the twin over the shell");
    assert_eq!(shell.notation_states.own::<SketchState>().unwrap().mark, 0);
    shell.cx(|cx| {
        assert!(std::ptr::eq(&*cx.notation_states, states), "the context's states are the shell's");
        cx.notation_states.own_mut::<SketchState>().expect("the shell's notations hold the sketch").mark = 1;
        assert!(std::ptr::eq(cx.cx_ref().notation_states, states), "the twin over the context");
    });
    assert_eq!(shell.notation_states.own::<SketchState>().unwrap().mark, 1, "changed through the context, changed on the shell");
}

/// A panel the narrow context asks for shows, unfolded, once the call returns; of two asked in one
/// call the one asked last shows, as the requests run in the order asked, and a new call asks
/// nothing again.
#[test]
fn the_narrow_context_shows_the_panel_asked_last() {
    use crate::cx::Request;
    use crate::state::Panel;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let shell = h.state_mut();
    (shell.state.panel, shell.state.panel_collapsed) = (Panel::Skills, true);
    shell.cx(|cx| cx.requests.push(Request::ShowPanel(Panel::Properties)));
    assert_eq!((shell.state.panel, shell.state.panel_collapsed), (Panel::Properties, false), "Properties shows, unfolded");
    shell.cx(|cx| {
        cx.requests.push(Request::ShowPanel(Panel::Agent));
        cx.requests.push(Request::ShowPanel(Panel::Skills));
    });
    assert_eq!(shell.state.panel, Panel::Skills, "the panel asked last shows");
    shell.state.panel_collapsed = true;
    shell.cx(|_| ());
    assert!(shell.state.panel_collapsed, "a request runs once");
}

/// An explorer the narrow context asks for shows, unfolded, once the call returns; of two asked in
/// one call the one asked last shows, as the requests run in the order asked, and a new call asks
/// nothing again.
#[test]
fn the_narrow_context_shows_the_explorer_asked_last() {
    use crate::cx::Request;
    use crate::state::Explorer;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let shell = h.state_mut();
    (shell.state.explorer, shell.state.explorer_collapsed) = (Explorer::Diagrams, true);
    shell.cx(|cx| cx.requests.push(Request::ShowExplorer(Explorer::Models)));
    assert_eq!((shell.state.explorer, shell.state.explorer_collapsed), (Explorer::Models, false), "Models shows, unfolded");
    shell.cx(|cx| {
        cx.requests.push(Request::ShowExplorer(Explorer::SourceControl));
        cx.requests.push(Request::ShowExplorer(Explorer::Diagrams));
    });
    assert_eq!(shell.state.explorer, Explorer::Diagrams, "the explorer asked last shows");
    shell.state.explorer_collapsed = true;
    shell.cx(|_| ());
    assert!(shell.state.explorer_collapsed, "a request runs once");
}

/// A name the narrow context asks Models to open for typing opens once the call returns: the filter
/// goes, so the row is listed, the name is selected whole, so typing replaces it, and Models has
/// the keyboard; a new call asks nothing again.
#[test]
fn the_narrow_context_opens_a_new_name_in_models() {
    use crate::cx::Request;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let shell = h.state_mut();
    let made = archforge_mof::Uuid([9; 16]);
    (shell.explorer.filter, shell.explorer.rename, shell.explorer.focused) = ("order".into(), None, false);
    shell.cx(|cx| cx.requests.push(Request::NameInModels(made, "Enumeration 1".into())));
    assert_eq!(shell.explorer.filter, "", "the filter goes");
    assert_eq!(shell.explorer.rename, Some(crate::explorer::Rename::fresh(made, "Enumeration 1".into())), "the name is open, selected whole");
    assert!(shell.explorer.focused, "Models has the keyboard");
    (shell.explorer.filter, shell.explorer.rename, shell.explorer.focused) = ("order".into(), None, false);
    shell.cx(|_| ());
    assert_eq!((shell.explorer.filter.as_str(), &shell.explorer.rename, shell.explorer.focused), ("order", &None, false), "a request runs once");
}

/// A removal the narrow context asks the delete dialog about opens the dialog once the call
/// returns; of two asked in one call the one asked last is asked, as the requests run in the order
/// asked, and a new call asks nothing again.
#[test]
fn the_narrow_context_opens_the_delete_dialog_asked_last() {
    use crate::cx::Request;
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1000.0));
    let shell = h.state_mut();
    let asked = |name: &str, id: u8| crate::explorer::Delete { ids: vec![archforge_mof::Uuid([id; 16])], name: name.into(), ..Default::default() };
    shell.explorer.delete = None;
    shell.cx(|cx| cx.requests.push(Request::AskDelete(asked("Order", 1))));
    assert_eq!(shell.explorer.delete, Some(asked("Order", 1)), "the dialog asks about Order");
    shell.cx(|cx| {
        cx.requests.push(Request::AskDelete(asked("Customer", 2)));
        cx.requests.push(Request::AskDelete(asked("Payment", 3)));
    });
    assert_eq!(shell.explorer.delete, Some(asked("Payment", 3)), "the removal asked last is asked");
    shell.explorer.delete = None;
    shell.cx(|_| ());
    assert_eq!(shell.explorer.delete, None, "a request runs once");
}

/// The shared removals ask the removed element's notation what to say (AR-789): Delete on a line
/// toasts the sketch's own words for it, then what the sketch worked out before the removal; the
/// Models tree's dialog says the same after its own lead; each notation is asked of its own
/// elements alone, a diagram of none, in the order of the notations. In light and dark.
#[test]
fn the_shared_removals_say_what_the_elements_notation_says() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        shared_removals_ask_the_notation(theme);
    }
}

/// [`the_shared_removals_say_what_the_elements_notation_says`] in one appearance: the sample class
/// diagram, its UML elements answered by the sketch notation.
fn shared_removals_ask_the_notation(theme: ThemeKind) {
    let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
    h.state_mut().notation_uis = SKETCH_UIS;
    h.run();
    {
        let st = h.state().session.as_ref().unwrap().doc();
        let o = |id: archforge_mof::Uuid| st.by_uuid(id).unwrap();
        let (order, payment) = (o(named(&h, archforge_mof::toy::CLASS, "Order")), o(named(&h, archforge_mof::toy::CLASS, "Payment")));
        let diagram = o(h.state().canvas.active.unwrap());
        let said = crate::notation_ui::before_removal(SKETCH_UIS, st, &[diagram, order, payment]);
        let want = crate::notation_ui::Removal {
            went: vec!["Order in the sketch".into(), "Payment in the sketch".into()],
            after: Some("the sketch saw 2 go".into()),
        };
        assert_eq!(said, want, "{theme:?}: the sketch is asked of its own two, the diagram of none");
        let none = crate::notation_ui::before_removal(SKETCH_UIS, st, &[diagram]);
        assert_eq!(none, crate::notation_ui::Removal::default(), "{theme:?}: no notation is asked of nothing of its own");
        let said = crate::notation_ui::before_removal(TWO_SKETCH_UIS, st, &[order]);
        assert_eq!(said.went, ["Order in the first", "Order in the second"], "{theme:?}: each notation's names, in their order");
        assert_eq!(said.after.as_deref(), Some("the first saw 1 go · the second saw 1 go"), "{theme:?}");
    }
    // Delete on a generalization's line: the sketch's words for it, then what it saw.
    let g = {
        let st = h.state().session.as_ref().unwrap().doc();
        let drawn = |o: &archforge_mof::Obj| !archforge_diagram::views_of(st, *o).1.is_empty();
        st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::GENERALIZATION) && drawn(o)).unwrap()).unwrap()
    };
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [g.to_string()] })).unwrap();
    h.run();
    let at = rect_of(&h, "workspace.canvas").center();
    h.event(egui::Event::PointerMoved(at));
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    let toast = h.state().canvas.toast.as_ref().map(|t| t.text.clone());
    assert_eq!(toast.as_deref(), Some("Class gone from the sketch with 1 named · the sketch saw 1 go"), "{theme:?}");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.state_mut().canvas.toast = None;
    // With two notations for the line's metamodel, the one that gives the toast is the one asked
    // what goes: the toast names its one answer, not both notations'.
    h.state_mut().notation_uis = TWO_SKETCH_UIS;
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [g.to_string()] })).unwrap();
    h.run();
    h.state_mut().canvas.focused = true;
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    let toast = h.state().canvas.toast.as_ref().map(|t| t.text.clone());
    assert_eq!(toast.as_deref(), Some("Class gone from the first with 1 named · the first saw 1 go"), "{theme:?}");
    h.state_mut().session.as_mut().unwrap().undo().unwrap();
    h.state_mut().canvas.toast = None;
    h.state_mut().notation_uis = SKETCH_UIS;
    h.run();
    // Order removed in the Models tree, through its dialog: the lead, then what the sketch saw.
    let order = named(&h, archforge_mof::toy::CLASS, "Order");
    h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [order.to_string()] })).unwrap();
    h.run();
    crate::explorer::remove_selection(h.state_mut());
    h.run();
    assert!(h.state().explorer.delete.is_some(), "{theme:?}: a diagram shows Order, so the dialog asks");
    let at = rect_of(&h, "workspace.explorer.delete.confirm").center();
    click(&mut h, at);
    let toast = h.state().canvas.toast.as_ref().map(|t| t.text.clone());
    assert_eq!(toast.as_deref(), Some("Order deleted · the sketch saw 1 go"), "{theme:?}");
}

/// The delete dialog says what goes with an element as the element's notation says it, in its own
/// words, and asks only when something goes: a sketch class shown on no diagram goes at once from
/// Models when the sketch says nothing goes with it, and asks with the sketch's sentence when it
/// has an attribute; the canvas's Delete from the model says the same. In light and dark.
#[test]
fn the_delete_dialog_says_what_the_notation_says_goes_with_it() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = related_sample_in(theme, egui::vec2(1600.0, 1000.0));
        h.run();
        let order = named(&h, archforge_mof::toy::CLASS, "Order").to_string();
        let pkg = {
            let st = h.state().session.as_ref().unwrap().doc();
            let p = st.container(st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap()).unwrap().unwrap().0;
            st.uuid(p).unwrap().to_string()
        };
        let s = h.state_mut().session.as_mut().unwrap();
        let mut id = |cmd: &str, p: serde_json::Value| s.execute(cmd, &p).unwrap().value["id"].as_str().unwrap().to_string();
        let lone = id("sketch.class.add", serde_json::json!({ "package": pkg, "name": "Lone" }));
        let held = id("sketch.class.add", serde_json::json!({ "package": pkg, "name": "Held" }));
        id("sketch.attribute.add", serde_json::json!({ "class": held, "name": "kept" }));
        let remove = |h: &mut Harness<'static, Shell>, id: &str| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [id] })).unwrap();
            h.run();
            crate::explorer::remove_selection(h.state_mut());
            h.run();
        };
        let alive = |h: &Harness<'static, Shell>, id: &str| {
            let st = h.state().session.as_ref().unwrap().doc();
            st.by_uuid(archforge_mof::Uuid::parse(id).unwrap()).is_some_and(|o| st.is_alive(o))
        };
        remove(&mut h, &lone);
        assert!(h.state().explorer.delete.is_none(), "{theme:?}: nothing goes with Lone, so nothing is asked");
        assert!(!alive(&h, &lone), "{theme:?}: Lone went at once");
        remove(&mut h, &held);
        let asked = h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: the sketch says its attribute goes with it"));
        assert_eq!(asked.consequences, ["1 attributes go with it in the sketch."], "{theme:?}");
        assert!(alive(&h, &held), "{theme:?}: nothing removed unasked");
        assert!(painted_has(&h, "1 attributes go with it in the sketch."), "{theme:?}: {:?}", painted(&h));
        h.state_mut().explorer.delete = None;
        // Order on the canvas, through Delete from the model: its attributes, as the sketch says.
        let (attributes, order, node) = {
            let st = h.state().session.as_ref().unwrap().doc();
            let o = st.by_uuid(archforge_mof::Uuid::parse(&order).unwrap()).unwrap();
            let node = archforge_diagram::views_of(st, o).2[0];
            (st.list(o, archforge_mof::toy::CLASS_OWNED_ATTRIBUTE).unwrap().len(), st.uuid(o).unwrap(), st.uuid(node).unwrap())
        };
        crate::editing::ask_delete(h.state_mut(), node, order);
        h.run();
        let asked = h.state().explorer.delete.clone().unwrap_or_else(|| panic!("{theme:?}: Delete from the model asks"));
        assert_eq!(asked.consequences, [format!("{attributes} attributes go with it in the sketch.")], "{theme:?}");
    }
}

/// A row Properties selects opens the rows its element sits under in Models, as its notation names
/// them (AR-785): exactly the rows of what holds it (an attribute's and a generalization's class
/// and package, an association's package), and a class's package.
#[test]
fn a_row_properties_selects_opens_the_rows_its_notation_names() {
    use std::collections::BTreeSet;
    let find = |h: &Harness<'static, Shell>, name: &str| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.alive().find(|o| archforge_core::name_of(st, *o) == name).unwrap()).unwrap()
    };
    let (mut h, customer) = table_harness();
    let (customer, email) = (archforge_mof::Uuid::parse(&customer).unwrap(), find(&h, "email"));
    let schema = {
        let st = h.state().session.as_ref().unwrap().doc();
        st.uuid(st.container(st.by_uuid(customer).unwrap()).unwrap().unwrap().0).unwrap()
    };
    let (association, generalization) = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = |v: serde_json::Value| archforge_mof::Uuid::parse(v["id"].as_str().unwrap()).unwrap();
        let order = id(s.execute("sketch.class.add", &serde_json::json!({ "package": schema.to_string(), "name": "Order" })).unwrap().value);
        let a = serde_json::json!({ "from": customer.to_string(), "to": order.to_string(), "name": "places" });
        let association = id(s.execute("sketch.association.add", &a).unwrap().value);
        let g = serde_json::json!({ "specific": customer.to_string(), "general": order.to_string() });
        (association, id(s.execute("sketch.inherit.add", &g).unwrap().value))
    };
    for (what, element, rows) in [
        ("an attribute", email, BTreeSet::from([customer, schema])),
        ("a generalization", generalization, BTreeSet::from([customer, schema])),
        ("an association", association, BTreeSet::from([schema])),
    ] {
        h.state_mut().explorer.expanded.clear();
        h.state_mut().cx(|cx| archforge_shell_api::panels::select_element_cx(cx, element));
        assert_eq!(h.state().explorer.expanded.iter().copied().collect::<BTreeSet<_>>(), rows, "{what}");
    }
    let (mut h, ..) = sketch_harness();
    let (sales, party) = (find(&h, "sales"), find(&h, "Party"));
    h.state_mut().explorer.expanded.clear();
    h.state_mut().cx(|cx| archforge_shell_api::panels::select_element_cx(cx, party));
    assert_eq!(h.state().explorer.expanded.iter().copied().collect::<Vec<_>>(), [sales], "the class's package");
    assert_eq!(h.state().session.as_ref().unwrap().view().selection.len(), 1, "and it is selected");
}

/// Clicks the field `id` and types `text` into it.
fn type_into(h: &mut Harness<'static, Shell>, id: &str, text: &str) {
    let at = rect_of(h, id).center();
    click(h, at);
    h.event(egui::Event::Text(text.into()));
    h.run();
}

/// Every text the last frame painted, with the box its galley took.
fn text_boxes(h: &Harness<'static, Shell>) -> Vec<(String, egui::Rect)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), egui::Rect::from_min_size(t.pos, t.galley.size()))),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Every rectangle the last frame painted, with its fill and its stroke's colour.
fn rect_shapes(h: &Harness<'static, Shell>) -> Vec<(egui::Rect, egui::Color32, egui::Color32)> {
    /// Collects every rectangle under `s` with its fill and its stroke's colour.
    fn walk(s: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32, egui::Color32)>) {
        match s {
            egui::Shape::Rect(r) => out.push((r.rect, r.fill, r.stroke.color)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// Every level line segment the last frame painted: its left and right ends, its height, its
/// thickness and its colour.
fn level_lines(h: &Harness<'static, Shell>) -> Vec<(f32, f32, f32, f32, egui::Color32)> {
    /// Collects every level line segment under `s`.
    fn walk(s: &egui::Shape, out: &mut Vec<(f32, f32, f32, f32, egui::Color32)>) {
        match s {
            egui::Shape::LineSegment { points: [a, b], stroke } if (a.y - b.y).abs() < 0.01 => {
                out.push((a.x.min(b.x), a.x.max(b.x), a.y, stroke.width, stroke.color));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

/// The sample class model with the Chat panel shown, over the fake agents host and conversations
/// kept in memory (`crate::chat_fixture`), Claude set up when `ready`.
fn chat_harness(theme: ThemeKind, ready: bool) -> (Harness<'static, Shell>, crate::chat_fixture::Rig) {
    crate::i18n::set_language("en");
    let mut shell = Shell::new(Services { ids: Some(test_ids()), ..Services::default() }, NOTATIONS, &SCREENS);
    shell.state.theme = theme;
    shell.open_project(crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into(), ..Default::default() });
    let rig = crate::chat_fixture::sample(&mut shell, ready, open_sample, (archforge_mof::toy::CLASS, "Order")).unwrap();
    let h = harness_over(shell);
    (h, rig)
}

/// Types `text` into the composer and presses Enter, as the person does.
fn ask_in_chat(h: &mut Harness<'static, Shell>, text: &str) {
    let at = rect_of(h, "workspace.chat.composer").center();
    click(h, at);
    h.event(egui::Event::Text(text.into()));
    h.run();
    key(h, egui::Key::Enter, egui::Modifiers::NONE);
    // The composer's caret blinks while it has the keyboard: let it go, so the window settles.
    h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id("workspace.chat.composer")));
    h.run();
}

/// The host says `event` of the conversation shown.
fn chat_event(h: &mut Harness<'static, Shell>, rig: &crate::chat_fixture::Rig, event: crate::chat::ChatEvent) {
    let key = crate::chat_fixture::key(h.state());
    assert!(rig.agents.emit(&key, event), "the shell takes the conversation's events");
    h.run();
}

/// How many times `text` is painted whole.
fn painted_times(h: &Harness<'static, Shell>, text: &str) -> usize {
    painted(h).iter().filter(|t| t.as_str() == text).count()
}

/// The conversation shown, as the store keeps it.
fn kept_chat(h: &Harness<'static, Shell>, rig: &crate::chat_fixture::Rig) -> crate::chat_store::fake::Kept {
    let id = h.state().chat_panel.chat.as_ref().map(|c| c.id).unwrap();
    rig.store.kept(id).unwrap()
}

/// The fake host's conversation calls: starts, sends, stops, ends.
fn chat_calls(rig: &crate::chat_fixture::Rig) -> (usize, usize, usize, usize) {
    use std::sync::atomic::Ordering::Relaxed;
    let c = &rig.agents.calls;
    (c.start.load(Relaxed), c.send.load(Relaxed), c.stop.load(Relaxed), c.end.load(Relaxed))
}

/// Chat settles for a hold: the turn is stopped once, and it is settled only when its end is written.
#[test]
fn chat_settles_once_the_turn_has_ended_and_its_end_is_written() {
    use crate::hold::ChatSettling::{Settled, Waiting};
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "A long question");
    let key = crate::chat_fixture::key(h.state());
    let id = h.state().chat_panel.chat.as_ref().unwrap().id;
    h.state_mut().begin_hold().unwrap();
    assert_eq!((h.state_mut().settle_chat(), h.state_mut().settle_chat()), (Waiting, Waiting));
    assert_eq!(chat_calls(&rig).2, 1, "the turn was stopped once");
    h.state_mut().chat_panel.menu = true;
    assert_eq!(h.state_mut().settle_chat(), Waiting);
    assert!(h.state().chat_panel.menu, "only the first look leaves the conversation");
    rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(rig.agents.emit(&key, crate::chat::ChatEvent::TurnEnded(crate::chat::TurnEnd::Stopped)));
    h.run_steps(3);
    assert_eq!(h.state_mut().settle_chat(), Waiting, "its end has not been written");
    rig.store.release();
    h.run_steps(3);
    assert_eq!(h.state_mut().settle_chat(), Settled);
    assert_eq!(rig.store.kept(id).unwrap().turns[0].outcome, crate::chat_store::Outcome::Stopped);
}

/// Past the limit the operation is refused and the hold ends as failed, so a close that waited asks.
#[test]
fn chat_that_does_not_settle_in_time_ends_the_hold_as_failed() {
    use crate::hold::{
        CHAT_SETTLE_LIMIT,
        ChatSettling::{Refused, Waiting},
    };
    use std::sync::atomic::Ordering;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    assert_eq!(h.state_mut().settle_chat(), Refused, "there is no hold");
    rig.store.hold.store(true, Ordering::SeqCst);
    ask_in_chat(&mut h, "A question its project never answers");
    h.state_mut().begin_hold().unwrap();
    h.state_mut().close_tab(0);
    assert_eq!(h.state_mut().settle_chat(), Waiting);
    rig.now.fetch_add(CHAT_SETTLE_LIMIT - 1, Ordering::Relaxed);
    assert_eq!(h.state_mut().settle_chat(), Waiting, "within the limit");
    rig.now.fetch_add(1, Ordering::Relaxed);
    assert_eq!(h.state_mut().settle_chat(), Refused);
    assert!(!h.state().is_held());
    h.run_steps(3);
    assert!(h.state().ask.is_some(), "the close that waited asks again");
}

/// A question answered while held starts no turn, and the composer takes no new one until the end.
#[test]
fn while_held_a_question_starts_no_turn_and_the_composer_refuses() {
    use crate::chat_panel::TurnState;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
    ask_in_chat(&mut h, "Asked before the hold");
    let (started, sent) = (chat_calls(&rig).0, chat_calls(&rig).1);
    h.state_mut().begin_hold().unwrap();
    rig.store.release();
    h.run_steps(4);
    assert_eq!((chat_calls(&rig).0, chat_calls(&rig).1), (started, sent), "the question reached no program");
    assert_eq!(h.state().chat_panel.chat.as_ref().unwrap().turns.last().unwrap().state, TurnState::Stopped);
    let writes = rig.store.log.lock().unwrap().len();
    h.state_mut().chat_panel.draft = "Asked during the hold".into();
    crate::chat_panel::send(h.state_mut());
    h.run_steps(3);
    assert_eq!(rig.store.log.lock().unwrap().len(), writes, "the composer took nothing");
    assert_eq!(h.state().chat_panel.draft, "Asked during the hold", "and the text stays");
    h.state_mut().end_hold();
    crate::chat_panel::send(h.state_mut());
    h.run_steps(4);
    assert!(rig.store.log.lock().unwrap().len() > writes, "after the hold it goes");
    assert_eq!(chat_calls(&rig).1, sent + 1);
}

/// A deletion on its way keeps Chat from settling; one in another project does not.
#[test]
fn a_deletion_on_its_way_is_waited_for_only_in_its_own_project() {
    use crate::hold::ChatSettling::{Settled, Waiting};
    let start_deleting = || {
        let (mut h, rig, chats) = chat_history(ThemeKind::Light);
        rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
        crate::chat_delete::ask(h.state_mut(), chats[2], 2);
        crate::chat_delete::confirm(h.state_mut());
        h.run_steps(3);
        (h, rig)
    };
    let (mut h, rig) = start_deleting();
    h.state_mut().begin_hold().unwrap();
    assert_eq!(h.state_mut().settle_chat(), Waiting);
    rig.store.release();
    h.run_steps(3);
    assert_eq!(h.state_mut().settle_chat(), Settled);
    let (mut h, _rig) = start_deleting();
    h.state_mut().open_project(crate::Opened { name: "Second".into(), path: "/tmp/Second.archforge".into(), ..Default::default() });
    h.run_steps(4);
    h.state_mut().begin_hold().unwrap();
    assert_eq!(h.state_mut().settle_chat(), Settled, "the other project's deletion is not this one's");
}

/// A turn's end failed with `kind`.
fn failed_turn(kind: crate::chat::FailureKind) -> crate::chat::ChatEvent {
    crate::chat::ChatEvent::TurnEnded(crate::chat::TurnEnd::Failed(crate::chat::ChatFailure { kind, detail: "x".into(), reset: None }))
}

/// The choices of Settings ▸ AI Agents as an earlier build kept them, its switch for Chat `on` or
/// off, Claude the default with Sonnet (AR-237).
fn earlier_agent_choices(on: bool) -> crate::ai_agents::AgentPrefs {
    let file = format!(r#"{{"agents":{{"default_agent":"Claude","claude_model":"sonnet","chat_uses_model":{on}}}}}"#);
    serde_json::from_str::<UiState>(&file).unwrap().agents
}

/// With Claude set up as the default agent, a new conversation is ready to ask with no switch
/// involved (AR-237): the composer shows, and a question starts Claude's program and reaches it,
/// even with an earlier build's switch read as off. In both appearances.
#[test]
fn a_new_conversation_is_ready_to_ask_as_soon_as_the_default_agent_is_set_up() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        h.state_mut().state.agents = earlier_agent_choices(false);
        h.run();
        assert_eq!(crate::chat_panel::set_up(h.state(), crate::ai_agents::Provider::Claude), Some(true), "{theme:?}");
        assert!(!painted_has(&h, "No agent set up yet"), "{theme:?}: {:?}", painted(&h));
        let found = ids(&h);
        for id in ["workspace.chat.composer", "workspace.chat.send", "workspace.chat.badge"] {
            assert!(found.iter().any(|i| i == id), "{theme:?}: {id} in {found:?}");
        }
        ask_in_chat(&mut h, "Describe Customer.");
        assert_eq!((chat_calls(&rig).0, chat_calls(&rig).1), (1, 1), "{theme:?}: started and sent");
        let started = rig.agents.started.lock().unwrap().last().map(|s| (s.provider, s.allow_changes));
        assert_eq!(started, Some((crate::ai_agents::Provider::Claude, false)), "{theme:?}: Claude, read-only");
        assert!(sent_texts(&rig).last().is_some_and(|t| t.starts_with("Describe Customer.")), "{theme:?}");
    }
}

/// With no agent set up, Chat shows only the setup prompt with Configure agent (AR-237), even with
/// an earlier build's switch read as on: no composer, and nothing starts. In both appearances.
#[test]
fn without_an_agent_set_up_chat_shows_only_the_setup_prompt() {
    const PROMPT: &str = "Chat answers with Claude or Codex, run by the program installed on this computer. Set one up in Settings.";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, false);
        h.state_mut().state.agents = earlier_agent_choices(true);
        h.run();
        assert_eq!(crate::chat_panel::set_up(h.state(), crate::ai_agents::Provider::Claude), Some(false), "{theme:?}");
        for text in ["No agent set up yet", PROMPT, "Configure agent"] {
            assert!(painted_has(&h, text), "{theme:?}: {text} in {:?}", painted(&h));
        }
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.chat.configure"), "{theme:?}");
        for absent in ["workspace.chat.new", "workspace.chat.composer", "workspace.chat.send", "workspace.chat.badge"] {
            assert!(!found.iter().any(|i| i == absent), "{theme:?}: {absent} with no agent set up");
        }
        assert_eq!(chat_calls(&rig), (0, 0, 0, 0), "{theme:?}: nothing started");
    }
}

/// Chat's readiness is the default agent's alone (AR-237): Claude's key found refused by the test
/// message shows the setup prompt; Codex signed in with ChatGPT, its sign-in not read yet and its
/// program found, is ready. In both appearances.
#[test]
fn chat_is_ready_exactly_when_the_default_agent_is() {
    use crate::ai_agents::{CodexSignIn, Provider};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        // Claude is the first of the two agents.
        h.state_mut().agents.refused[0] = Some("401 invalid x-api-key".into());
        h.run();
        assert_eq!(crate::chat_panel::set_up(h.state(), Provider::Claude), Some(false), "{theme:?}: a refused key");
        assert!(painted_has(&h, "No agent set up yet"), "{theme:?}: {:?}", painted(&h));
        assert!(!ids(&h).iter().any(|i| i == "workspace.chat.composer"), "{theme:?}");
        h.state_mut().state.agents.default_agent = Provider::Codex;
        h.state_mut().state.agents.codex_sign_in = CodexSignIn::ChatGpt;
        h.run();
        assert!(h.state().agents.codex.is_none(), "{theme:?}: Codex's sign-in is not read yet");
        assert_eq!(crate::chat_panel::set_up(h.state(), Provider::Codex), Some(true), "{theme:?}: Codex is ready");
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.composer"), "{theme:?}");
        assert_eq!(chat_calls(&rig), (0, 0, 0, 0), "{theme:?}: nothing started");
    }
}

/// B1 without an agent (AR-63 rewritten): Configure agent opens Settings; B2 with one: intro, chips, composer, badge, switch.
#[test]
fn the_chat_icon_opens_configure_agent_without_an_agent_and_a_ready_conversation_with_one() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        // B1: no agent set up. Configure agent opens Settings on AI Agents; no composer.
        let (mut h, _rig) = chat_harness(theme, false);
        assert_eq!(h.state().state.panel, crate::state::Panel::Agent);
        assert!(painted_has(&h, "No agent set up yet") && painted_has(&h, "Configure agent"), "{theme:?}: {:?}", painted(&h));
        let found = ids(&h);
        assert!(found.iter().any(|i| i == "workspace.chat.history"), "{theme:?}");
        for absent in ["workspace.chat.new", "workspace.chat.composer", "workspace.chat.send", "workspace.chat.badge"] {
            assert!(!found.iter().any(|i| i == absent), "{theme:?}: {absent} in B1");
        }
        let at = rect_of(&h, "workspace.chat.configure").center();
        click(&mut h, at);
        assert_eq!(h.state().settings.as_ref().map(|d| d.section), Some(crate::settings::Section::Agents), "{theme:?}");

        // B2: ready. The intro, the three chips, the notice, the composer with Claude's badge and
        // Send, Allow changes off.
        let (mut h, _rig) = chat_harness(theme, true);
        for text in ["Ask about Commerce platform", "These and everything the agent reads are sent to the provider.", "Allow changes to the model"] {
            assert!(painted_has(&h, text), "{theme:?}: {text} in {:?}", painted(&h));
        }
        assert!(painted_has(&h, "Claude · Sonnet"), "{theme:?}: the badge names the model asked for");
        let found = ids(&h);
        for id in [
            "workspace.chat.history",
            "workspace.chat.new",
            "workspace.chat.composer",
            "workspace.chat.send",
            "workspace.chat.badge",
            "workspace.chat.allow-changes",
            "workspace.chat.chip.project.remove",
            "workspace.chat.chip.diagram.remove",
            "workspace.chat.chip.selection.remove",
        ] {
            assert!(found.iter().any(|i| i == id), "{theme:?}: {id} in {found:?}");
        }
        assert!(!h.state().chat_panel.chat.as_ref().unwrap().allow_changes, "a conversation starts read-only");
        // Nothing of Properties is drawn under it; back to Properties, the selection is as it was.
        assert!(!found.iter().any(|i| i.starts_with("workspace.properties.") || i.starts_with("workspace.skills.")), "{theme:?}");
        let at = rect_of(&h, "workspace.panels.properties").center();
        click(&mut h, at);
        assert!(painted(&h).iter().any(|t| t == "Order"));
        // Its icon again shows the same conversation; a second click on the open panel folds the dock.
        let chat = h.state().chat_panel.chat.as_ref().map(|c| c.id);
        let at = rect_of(&h, "workspace.panels.agent").center();
        click(&mut h, at);
        assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.id), chat);
        click(&mut h, at);
        assert!(h.state().state.panel_collapsed);
    }
}

/// Enter sends from the composer only with no modifier, read from the press itself: holding Shift
/// it starts a line, Alt with it too, and with Alt alone, a Mac's Control or the command key of
/// either shape it does nothing; a Shift+Enter whose Shift is let go by the end of its frame is
/// still a new line.
#[test]
fn enter_sends_only_with_no_modifier_held_on_the_press() {
    use egui::Modifiers as M;
    in_both_appearances(|theme| {
        let (mut h, rig) = chat_harness(theme, true);
        let at = rect_of(&h, "workspace.chat.composer").center();
        click(&mut h, at);
        h.event(egui::Event::Text("One".into()));
        h.run();
        for more in [M::ALT, M::SHIFT | M::ALT, M::CTRL, M::MAC_CMD | M::COMMAND, M::CTRL | M::COMMAND] {
            key(&mut h, egui::Key::Enter, more);
            assert_eq!(chat_calls(&rig).1, 0, "{theme:?}: {more:?} sends nothing");
        }
        for pressed in [true, false] {
            h.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: M::SHIFT });
        }
        h.run();
        assert_eq!(chat_calls(&rig).1, 0, "{theme:?}: the press held Shift");
        assert_eq!(h.state().chat_panel.draft, "One\n\n", "{theme:?}: Shift with Alt and Shift each started a line");
        key(&mut h, egui::Key::Enter, M::NONE);
        assert_eq!(chat_calls(&rig).1, 1, "{theme:?}: Enter alone sends");
    });
}

/// The "Sign-in expired" card of a Codex conversation reads Codex's sign-in again (AR-222), so
/// Settings shows it expired, with its email, without a restart. In both appearances.
#[test]
fn a_codex_sign_in_expired_card_reads_the_sign_in_again() {
    use crate::ai_agents::{CodexAccount, Provider};
    use crate::chat::{ChatEvent, ChatFailure, FailureKind, TurnEnd};
    use std::sync::atomic::Ordering::Relaxed;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        *rig.agents.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None });
        h.state_mut().state.agents.default_agent = Provider::Codex;
        h.run();
        ask_in_chat(&mut h, "Hello");
        assert_eq!(rig.agents.started.lock().unwrap().last().map(|s| s.provider), Some(Provider::Codex));
        let reads = rig.agents.calls.account.load(Relaxed);
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None })));
        *rig.agents.account.lock().unwrap() = Ok(CodexAccount::Expired { email: String::new() });
        let expired = ChatFailure { kind: FailureKind::SignInExpired, detail: "x".into(), reset: None };
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(TurnEnd::Failed(expired)));
        assert!(painted_has(&h, "Sign-in expired"));
        assert_eq!(rig.agents.calls.account.load(Relaxed), reads + 1, "{theme:?}: the card read the sign-in again");
        h.run();
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::Expired { email: "you@example.com".into() })), "expired, its email kept");
        // Should the codex program still read it as signed in (it may not try to refresh it), the
        // card's word stands until a sign-in or a sign-out.
        *rig.agents.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None });
        crate::ai_agents::reread_account(h.state_mut());
        h.run();
        assert_eq!(rig.agents.calls.account.load(Relaxed), reads + 2);
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::Expired { email: "you@example.com".into() })), "still expired");
    }
}

/// A parked tab's Codex conversation whose turn ends with an expired sign-in reads Codex's sign-in
/// again and shows it expired, with its email, as the conversation shown does (AR-222). In both
/// appearances.
#[test]
fn a_parked_codex_conversations_sign_in_expired_card_reads_the_sign_in_again() {
    use crate::ai_agents::{CodexAccount, Provider};
    use crate::chat::{ChatEvent, ChatFailure, FailureKind, TurnEnd};
    use std::sync::atomic::Ordering::Relaxed;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        *rig.agents.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None });
        h.state_mut().state.agents.default_agent = Provider::Codex;
        h.run();
        ask_in_chat(&mut h, "Hello");
        let key = crate::chat_fixture::key(h.state());
        let first = h.state().active.unwrap();
        h.state_mut().open_project(crate::Opened { name: "Second".into(), path: "/tmp/Second.archforge".into(), ..Default::default() });
        h.run();
        assert!(h.state().tabs[first].parked.as_ref().is_some_and(|p| p.chat_panel.chat.is_some()), "{theme:?}: parked");
        let reads = rig.agents.calls.account.load(Relaxed);
        *rig.agents.account.lock().unwrap() = Ok(CodexAccount::Expired { email: String::new() });
        let expired = ChatFailure { kind: FailureKind::SignInExpired, detail: "x".into(), reset: None };
        assert!(rig.agents.emit(&key, ChatEvent::TurnEnded(TurnEnd::Failed(expired))));
        h.run();
        h.run();
        assert_eq!(rig.agents.calls.account.load(Relaxed), reads + 1, "{theme:?}: the parked card read the sign-in again");
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::Expired { email: "you@example.com".into() })), "{theme:?}");
    }
}

/// While the agent's program retries a provider it cannot reach, the turn says so with the attempt
/// it reports, and Stop stays (AR-224); text from the provider takes the line away, and when the
/// retries give up for a lost network the usual Connection lost card shows. In both appearances.
#[test]
fn while_the_agent_retries_the_turn_says_so_with_the_attempt_and_stop_stays() {
    use crate::chat::{ChatEvent, FailureKind};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "Explain how Payment relates to Order.");
        assert!(!painted(&h).iter().any(|t| t.contains("Trying again")));
        chat_event(&mut h, &rig, ChatEvent::Retrying { attempt: 1, max: Some(10) });
        assert!(painted_has(&h, "No answer from Claude's provider yet. Trying again, attempt 1 of 10…"), "{theme:?}: {:?}", painted(&h));
        chat_event(&mut h, &rig, ChatEvent::Retrying { attempt: 2, max: Some(10) });
        assert!(painted_has(&h, "Trying again, attempt 2 of 10…") && !painted_has(&h, "attempt 1 of 10…"));
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.stop"), "{theme:?}: Stop stays");
        // Text from the provider: it got through, the line goes.
        chat_event(&mut h, &rig, ChatEvent::Text { delta: "Payment is".into() });
        assert!(!painted(&h).iter().any(|t| t.contains("Trying again")));
        // Retrying again, then giving up: the usual card, the line gone.
        chat_event(&mut h, &rig, ChatEvent::Retrying { attempt: 3, max: None });
        assert!(painted_has(&h, "No answer from Claude's provider yet. Trying again, attempt 3…"), "no most said: none shown");
        chat_event(&mut h, &rig, failed_turn(FailureKind::Network));
        assert!(painted_has(&h, "Connection lost"));
        assert!(!painted(&h).iter().any(|t| t.contains("Trying again")));
        // Stop while it retries stops the turn.
        ask_in_chat(&mut h, "And Invoice?");
        chat_event(&mut h, &rig, ChatEvent::Retrying { attempt: 0, max: Some(10) });
        assert!(painted_has(&h, "No answer from Claude's provider yet. Trying again…"), "no attempt reported: none said");
        let at = rect_of(&h, "workspace.chat.stop").center();
        click(&mut h, at);
        assert_eq!(chat_calls(&rig).2, 1, "Stop asks the program to stop");
        assert!(!painted(&h).iter().any(|t| t.contains("Trying again")), "once Stop is asked, it no longer says it tries again");
    }
}

/// Choosing in Conversations the conversation already shown brings it back read-only, as any
/// conversation opened again (AR-226): Allow changes goes off, its program, started with changes
/// allowed, ends, and the next question starts it again read-only. In both appearances.
#[test]
fn choosing_the_conversation_shown_from_conversations_brings_it_back_read_only() {
    use crate::chat::ChatEvent;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        assert!(h.state().chat_panel.chat.as_ref().unwrap().allow_changes, "{theme:?}");
        ask_in_chat(&mut h, "Describe Customer.");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        assert_eq!(rig.agents.started.lock().unwrap().last().map(|s| s.allow_changes), Some(true), "{theme:?}");
        let shown = h.state().chat_panel.chat.as_ref().unwrap().id;
        // Conversations, then the conversation shown, the only one listed.
        let at = rect_of(&h, "workspace.chat.history").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.chat.history.0").center();
        click(&mut h, at);
        h.run();
        let chat = h.state().chat_panel.chat.as_ref().unwrap();
        assert_eq!(chat.id, shown, "{theme:?}: the same conversation");
        assert!(!chat.allow_changes, "{theme:?}: back read-only");
        assert_eq!(h.state().chat_panel.view, crate::chat_panel::View::Conversation, "{theme:?}");
        assert_eq!(chat_calls(&rig).3, 1, "{theme:?}: its program, started with changes allowed, ended");
        // The next question starts the program again, read-only, where it was.
        ask_in_chat(&mut h, "And Product?");
        let started = rig.agents.started.lock().unwrap().clone();
        assert_eq!(started.len(), 2, "{theme:?}: started again");
        assert_eq!(started.last().map(|s| (s.allow_changes, s.resume)), Some((false, true)), "{theme:?}: read-only, resumed");
    }
}

/// The same with a program slow to end (AR-226): a question asked before its end waits for it,
/// then starts the program again read-only. In both appearances.
#[test]
fn a_question_asked_while_the_reopened_conversations_program_ends_starts_it_read_only() {
    use crate::chat::ChatEvent;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        ask_in_chat(&mut h, "Describe Customer.");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        let key = crate::chat_fixture::key(h.state());
        rig.agents.hold_ends.store(true, std::sync::atomic::Ordering::Relaxed);
        let at = rect_of(&h, "workspace.chat.history").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.chat.history.0").center();
        click(&mut h, at);
        h.run();
        assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.session), Some(crate::chat_panel::Session::Ending), "{theme:?}");
        ask_in_chat(&mut h, "And Product?");
        assert_eq!(chat_calls(&rig).0, 1, "{theme:?}: the question waits for the end");
        rig.agents.hold_ends.store(false, std::sync::atomic::Ordering::Relaxed);
        rig.agents.running.lock().unwrap().retain(|k| *k != key);
        chat_event(&mut h, &rig, ChatEvent::Ended { failure: None });
        let started = rig.agents.started.lock().unwrap().clone();
        assert_eq!(started.len(), 2, "{theme:?}: started again once ended");
        assert_eq!(started.last().map(|s| s.allow_changes), Some(false), "{theme:?}: read-only");
        assert!(sent_texts(&rig).last().is_some_and(|t| t.starts_with("And Product?")), "{theme:?}");
    }
}

/// Stop keeps the reply so far, marked cut off and kept; an unreachable program's turn ends here.
#[test]
fn stop_keeps_the_reply_so_far_marked_cut_off_and_keeps_it_in_the_project() {
    use crate::chat::ChatEvent;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "Explain how Payment relates to Order.");
        chat_event(&mut h, &rig, ChatEvent::Text { delta: "Payment is linked to Order by".into() });
        let at = rect_of(&h, "workspace.chat.stop").center();
        click(&mut h, at);
        assert_eq!(chat_calls(&rig).2, 1, "{theme:?}: the program is asked to stop the turn");
        click(&mut h, at);
        assert_eq!(chat_calls(&rig).2, 1, "{theme:?}: once");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Stopped));
        assert!(painted_has(&h, "Payment is linked to Order by") && !painted_has(&h, "▍"), "{theme:?}: the reply so far stays");
        assert!(painted_has(&h, "Stopped. Nothing was changed."), "{theme:?}");
        let chat = kept_chat(&h, &rig);
        assert_eq!(chat.turns[0].outcome, crate::chat_store::Outcome::Stopped);
        assert!(
            matches!(&chat.messages[1].body, crate::chat_store::StoredBody::Reply { text, complete: false, .. } if text == "Payment is linked to Order by"),
            "{theme:?}: kept, marked cut off: {:?}",
            chat.messages
        );
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.send"), "{theme:?}: Send is back");
        // A program that cannot be reached any more: the turn ends here as stopped.
        ask_in_chat(&mut h, "And Customer?");
        rig.agents.running.lock().unwrap().clear();
        let at = rect_of(&h, "workspace.chat.stop").center();
        click(&mut h, at);
        assert_eq!(kept_chat(&h, &rig).turns[1].outcome, crate::chat_store::Outcome::Stopped, "{theme:?}");
    }
}

/// Send again runs the failed turn in place: no new turn, the question shown and kept once.
#[test]
fn send_again_runs_the_failed_turn_in_place_and_its_question_shows_and_is_kept_once() {
    use crate::chat::{ChatEvent, FailureKind};
    use crate::chat_store::{Outcome, StoredBody};
    let question = "Which classes have no description yet?";
    let questions = |chat: &crate::chat_store::fake::Kept| -> Vec<String> {
        chat.messages.iter().filter_map(|m| if let StoredBody::Question { text, .. } = &m.body { Some(text.clone()) } else { None }).collect()
    };
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, question);
    chat_event(&mut h, &rig, failed_turn(FailureKind::Other));
    assert!(painted_has(&h, "No answer") && painted_has(&h, "Claude stopped before answering. Nothing was changed."), "{:?}", painted(&h));
    let at = rect_of(&h, "workspace.chat.turn.1.retry").center();
    click(&mut h, at);
    let chat = kept_chat(&h, &rig);
    assert_eq!((chat.turns.len(), chat.turns[0].outcome), (1, Outcome::Running), "the same turn runs again");
    assert_eq!(questions(&chat), [question], "the question is kept once");
    assert_eq!(painted_times(&h, question), 1, "and shown once");
    assert_eq!(chat_calls(&rig).1, 2, "the agent gets it again");
    assert!(rig.store.log.lock().unwrap().contains(&"rerun"));
    chat_event(&mut h, &rig, ChatEvent::Message { text: "Five of them.".into() });
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    assert_eq!(kept_chat(&h, &rig).turns.len(), 1);
    assert!(!ids(&h).iter().any(|i| i == "workspace.chat.turn.1.retry"), "a completed turn is not sent again");

    // A lost connection puts the question back in the empty composer, its bubble gone; edited and
    // sent, it replaces the kept one in place.
    let second = "List the classes without one.";
    ask_in_chat(&mut h, second);
    chat_event(&mut h, &rig, failed_turn(FailureKind::Network));
    assert_eq!(h.state().chat_panel.draft, second, "back in the composer");
    assert!(painted_has(&h, "Connection lost") && painted_has(&h, "Your message is back in the box below"), "{:?}", painted(&h));
    let at = rect_of(&h, "workspace.chat.composer").center();
    click(&mut h, at);
    key(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
    h.event(egui::Event::Text("List every class without one.".into()));
    h.run();
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    let chat = kept_chat(&h, &rig);
    assert_eq!(chat.turns.len(), 2, "no new turn");
    assert_eq!(questions(&chat), [question, "List every class without one."], "the edited question replaced the kept one");
    assert_eq!((painted_times(&h, second), painted_times(&h, "List every class without one.")), (0, 1));
    assert_eq!(h.state().chat_panel.draft, "");
}

/// The draft survives a project's refusal, a kept key in the question and a lost connection.
#[test]
fn the_draft_survives_a_lost_connection_and_a_project_that_refuses_the_question() {
    use crate::chat::FailureKind;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        // The project refuses: nothing is sent, the draft stays and the panel says why.
        *rig.store.refuse.lock().unwrap() = Some("the disk is full".into());
        ask_in_chat(&mut h, "Which classes have no description yet?");
        assert_eq!(h.state().chat_panel.draft, "Which classes have no description yet?", "{theme:?}");
        assert!(painted_has(&h, "This conversation could not be saved: the disk is full"), "{theme:?}: {:?}", painted(&h));
        assert_eq!(chat_calls(&rig), (0, 0, 0, 0), "{theme:?}: nothing reached the agent");
        // A question holding a kept key is refused before anything is written.
        *rig.store.refuse.lock().unwrap() = None;
        h.state_mut().chat_panel.draft.clear();
        ask_in_chat(&mut h, &format!("Why does {} fail?", crate::chat_fixture::KEY));
        assert!(painted_has(&h, "This question holds your saved Claude API key, so it was neither sent nor saved."), "{theme:?}");
        assert!(h.state().chat_panel.draft.contains(crate::chat_fixture::KEY), "{theme:?}: kept for the person to edit");
        assert!(rig.store.chats.lock().unwrap().iter().all(|c| !c.row.title.contains(crate::chat_fixture::KEY)));
        assert_eq!(chat_calls(&rig), (0, 0, 0, 0), "{theme:?}");
        // Sent once it can be: the draft empties. A lost connection with a new draft typed meanwhile
        // keeps that draft; the question stays in its bubble.
        h.state_mut().chat_panel.draft.clear();
        ask_in_chat(&mut h, "Which classes have no description yet?");
        assert_eq!(h.state().chat_panel.draft, "");
        let at = rect_of(&h, "workspace.chat.composer").center();
        click(&mut h, at);
        h.event(egui::Event::Text("And the next one".into()));
        h.run();
        // While the reply runs, typing works and Enter waits.
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(chat_calls(&rig).1, 1, "{theme:?}: Enter waits for the reply");
        assert!(painted_has(&h, "Stop the reply first"), "{theme:?}");
        chat_event(&mut h, &rig, failed_turn(FailureKind::Network));
        assert_eq!(h.state().chat_panel.draft, "And the next one", "{theme:?}: the draft typed meanwhile is kept");
        assert_eq!(painted_times(&h, "Which classes have no description yet?"), 1, "{theme:?}: the question stays in its bubble");
        assert!(painted_has(&h, "The network went away before Claude answered."), "{theme:?}");
    }
}

/// The badge names the model; Settings never switch a started conversation; the badge does, between turns.
#[test]
fn the_badge_names_the_model_and_a_changed_setting_never_switches_a_running_conversation() {
    use crate::ai_agents::Provider;
    use crate::chat::ChatEvent;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    // A conversation that has not started follows Settings.
    h.state_mut().state.agents.claude_model = "opus".into();
    h.run();
    assert!(painted_has(&h, "Claude · Opus"));
    h.state_mut().state.agents.claude_model = "sonnet".into();
    h.run();
    ask_in_chat(&mut h, "Hello");
    chat_event(&mut h, &rig, ChatEvent::Started { model: crate::chat_fixture::NAMED.into(), access: true });
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    // Settings change: the conversation keeps its agent and model, and its program.
    h.state_mut().state.agents.default_agent = Provider::Codex;
    h.state_mut().state.agents.claude_model = "opus".into();
    h.run();
    assert!(painted_has(&h, "Claude · Sonnet 4.5"), "{:?}", painted(&h));
    ask_in_chat(&mut h, "And again");
    assert_eq!(chat_calls(&rig), (1, 2, 0, 0), "the same program, no new start");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    // While a reply runs, the badge, the switch, New conversation and the history wait.
    ask_in_chat(&mut h, "Third");
    for id in ["workspace.chat.badge", "workspace.chat.allow-changes", "workspace.chat.new", "workspace.chat.history"] {
        let at = rect_of(&h, id).center();
        click(&mut h, at);
    }
    let shown = h.state().chat_panel.chat.as_ref().unwrap();
    assert!(!h.state().chat_panel.menu && !shown.allow_changes && shown.turns.len() == 3, "nothing changed while it ran");
    assert_eq!(h.state().chat_panel.view, crate::chat_panel::View::Conversation);
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    // The badge's menu: once started, the conversation's own agent only; Opus picked ends the idle
    // program, and the next question starts it again where it was, on Opus.
    let at = rect_of(&h, "workspace.chat.badge").center();
    click(&mut h, at);
    assert!(h.state().chat_panel.menu);
    let items: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.chat.badge.")).collect();
    assert_eq!(items.len(), Provider::Claude.models().len(), "{items:?}");
    let opus = Provider::Claude.models().iter().position(|(m, _)| *m == "opus").unwrap();
    let at = rect_of(&h, &format!("workspace.chat.badge.{opus}")).center();
    click(&mut h, at);
    assert_eq!(chat_calls(&rig).3, 1, "the idle program ended");
    assert!(painted_has(&h, "Claude · Opus"));
    assert!(rig.store.log.lock().unwrap().contains(&"model"));
    assert_eq!(h.state().chat_panel.chat.as_ref().unwrap().session, crate::chat_panel::Session::Off, "its end came");
    ask_in_chat(&mut h, "Fourth");
    let started = rig.agents.started.lock().unwrap().clone();
    assert_eq!(started.len(), 2);
    assert_eq!((started[1].provider, started[1].model.as_str(), started[1].resume), (Provider::Claude, "opus", true));
}

/// Only a web link in a reply is a control, and it opens only after its confirmation.
#[test]
fn only_a_web_link_in_a_reply_opens_and_only_after_its_confirmation() {
    use crate::chat::ChatEvent;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "Where are the docs?");
    let reply =
        "See [the docs](https://example.org/docs), [this](javascript:alert(1)), [mail](mailto:a@example.org) and [that](https://user@example.org/).";
    chat_event(&mut h, &rig, ChatEvent::Message { text: reply.into() });
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    let links: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.chat.link.")).collect();
    assert_eq!(links, ["workspace.chat.link.0"], "only the web address is a link");
    let press = |h: &mut Harness<'static, Shell>, id: &str| {
        let at = rect_of(h, id).center();
        h.event(egui::Event::PointerMoved(at));
        h.run_steps(8);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.step();
    };
    press(&mut h, "workspace.chat.link.0");
    assert!(!h.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::OpenUrl(_))), "nothing opens yet");
    h.run_steps(8);
    assert_eq!(h.state().chat_panel.link.as_deref(), Some("https://example.org/docs"));
    press(&mut h, "workspace.chat.link.open");
    let opened: Vec<String> = h
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| if let egui::OutputCommand::OpenUrl(u) = c { Some(u.url.clone()) } else { None })
        .collect();
    assert_eq!(opened, ["https://example.org/docs"]);
    h.run_steps(8);
    assert_eq!(h.state().chat_panel.link, None);
}

/// The Chat transcript and the keyboard (AR-477): the page keys scroll it while a link has the
/// keyboard, Tab reaches every link and it follows, and a reply coming in leaves the place.
mod chat_keys {
    use egui::{Key, Modifiers};
    use egui_kittest::kittest::{NodeT, Queryable};

    use super::*;
    use crate::chat::{ChatEvent, TurnEnd};

    /// Six answered questions, three links in each reply, at the design's window height: the
    /// transcript shown at its end, its first links far above the fold.
    fn long_chat(theme: ThemeKind) -> (Harness<'static, Shell>, crate::chat_fixture::Rig) {
        long_chat_in(theme, false)
    }

    /// The same, in wider fonts given before the first question when `wider`.
    fn long_chat_in(theme: ThemeKind, wider: bool) -> (Harness<'static, Shell>, crate::chat_fixture::Rig) {
        let (mut h, rig) = chat_harness(theme, true);
        if wider {
            widen(&mut h);
        }
        let filler = "The order keeps its lines, and each line names a product and how many. ".repeat(6);
        for n in 0..6 {
            ask_in_chat(&mut h, &format!("Question {n}"));
            let text = format!(
                "See [a](https://example.org/{n}/a) first.\n\n{filler}\n\nThen [b](https://example.org/{n}/b) and [c](https://example.org/{n}/c)."
            );
            chat_event(&mut h, &rig, ChatEvent::Message { text });
            chat_event(&mut h, &rig, ChatEvent::TurnEnded(TurnEnd::Completed));
        }
        assert!(h.state().chat_panel.at_end && scroll(&h) > 0.0 && !has_id(&h, &link(0)), "{theme:?}: at the end, the first link out of view");
        (h, rig)
    }

    /// The id of the `n`th link of the transcript.
    fn link(n: usize) -> String {
        format!("workspace.chat.link.{n}")
    }

    /// How far the transcript is scrolled.
    fn scroll(h: &Harness<'static, Shell>) -> f32 {
        h.state().chat_panel.scroll
    }

    /// Whether the control `id` is listed and drawn whole in the transcript.
    fn whole(h: &Harness<'static, Shell>, id: &str) -> bool {
        has_id(h, id) && h.state().chat_panel.area.unwrap().contains_rect(rect_of(h, id))
    }

    /// Presses `key_` and asserts that the transcript went to `to`, the keyboard still on `id`.
    fn press_to(h: &mut Harness<'static, Shell>, key_: Key, to: f32, id: &str) {
        key(h, key_, Modifiers::NONE);
        assert_eq!(scroll(h), to, "{key_:?}");
        assert!(focused_on(h, id), "{key_:?} left the keyboard on {id}");
    }

    /// While a link has the keyboard, the page keys scroll the transcript, in a frame the key asks
    /// for; not with the keyboard nowhere, with the control key, with a link's confirmation or the
    /// menu bar open, also in the frame a click of the pointer or a screen reader opens it in, nor
    /// from the composer, which keeps its keys.
    #[test]
    fn the_page_keys_scroll_the_transcript_while_a_link_has_the_keyboard() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = long_chat(theme);
            let room = scroll(&h);
            let page = h.state().chat_panel.area.unwrap().height() - crate::ux::types::UI.line;
            key(&mut h, Key::PageUp, Modifiers::NONE);
            assert_eq!(scroll(&h), room, "{theme:?}: with the keyboard nowhere");
            let last = link(17);
            focus_on(&mut h, &last);
            assert!(whole(&h, &last) && scroll(&h) == room, "{theme:?}: the last link shows, nothing moved");
            h.key_down(Key::PageUp);
            h.step();
            // The frame after the key's was drawn because the key's frame asked for it.
            h.step();
            let asked = h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("chat_view.rs"));
            assert!(asked, "{theme:?}: Page Up asks for the frame that draws it: {:?}", h.ctx.repaint_causes());
            h.key_up(Key::PageUp);
            h.run();
            assert_eq!(scroll(&h), room - page, "{theme:?}: Page Up");
            press_to(&mut h, Key::PageUp, (room - 2.0 * page).max(0.0), &last);
            press_to(&mut h, Key::Home, 0.0, &last);
            press_to(&mut h, Key::PageDown, page, &last);
            press_to(&mut h, Key::End, room, &last);
            key(&mut h, Key::Home, Modifiers::CTRL.plus(Modifiers::COMMAND));
            assert_eq!(scroll(&h), room, "{theme:?}: not with the control key as Windows and Linux send it");
            key(&mut h, Key::Enter, Modifiers::NONE);
            assert!(h.state().chat_panel.link.is_some() && focused_on(&h, &last), "{theme:?}: Enter asks before the link opens");
            key(&mut h, Key::Home, Modifiers::NONE);
            assert_eq!(scroll(&h), room, "{theme:?}: not while the link's confirmation is open");
            key(&mut h, Key::Escape, Modifiers::NONE);
            assert!(h.state().chat_panel.link.is_none(), "{theme:?}: Escape closes it");
            focus_on(&mut h, &last);
            press_to(&mut h, Key::Home, 0.0, &last);
            // The in-window menu bar opened by F10 leaves the keyboard on the link; its keys are its own.
            h.state_mut().in_window_menu = true;
            key(&mut h, Key::F10, Modifiers::NONE);
            assert!(h.state().menu_bar.open.is_some() && focused_on(&h, &last), "{theme:?}: F10 opens the bar's first menu");
            for key_ in [Key::PageDown, Key::End] {
                key(&mut h, key_, Modifiers::NONE);
                assert_eq!(scroll(&h), 0.0, "{theme:?}: {key_:?} with the menu bar open");
            }
            key(&mut h, Key::Escape, Modifiers::NONE);
            // Nor in the frame of the click that opens the bar, by the pointer or a screen reader (AR-522).
            // The pointer's click also takes the keyboard from the link as the link is drawn, before
            // the transcript reads the keys; the screen reader's leaves it there.
            for pointer in [true, false] {
                // The keyboard given back to the link brings it into view: Home leaves room to page down.
                press_to(&mut h, Key::Home, 0.0, &last);
                open_file_with(&mut h, Key::PageDown, Modifiers::NONE, pointer);
                assert_eq!(scroll(&h), 0.0, "{theme:?}: with the click that opens the menu bar, by the pointer: {pointer}");
                key(&mut h, Key::Escape, Modifiers::NONE);
                assert!(h.state().menu_bar.open.is_none(), "{theme:?}: Escape closed the menu bar, by the pointer: {pointer}");
                assert!(focused_on(&h, &last), "{theme:?}: back on the link");
            }
            h.state_mut().in_window_menu = false;
            focus_on(&mut h, &last);
            key(&mut h, Key::End, Modifiers::NONE);
            assert_eq!(scroll(&h), room, "{theme:?}: End with the menu bar closed");
            // The composer keeps Home and End: what is typed after them lands at its ends.
            let at = rect_of(&h, "workspace.chat.composer").center();
            click(&mut h, at);
            h.event(egui::Event::Text("abc".into()));
            for (key_, typed, reads) in [(Key::Home, "x", "xabc"), (Key::End, "y", "xabcy"), (Key::PageUp, "", "xabcy")] {
                key(&mut h, key_, Modifiers::NONE);
                h.event(egui::Event::Text(typed.into()));
                h.run();
                assert_eq!((h.state().chat_panel.draft.as_str(), scroll(&h)), (reads, room), "{theme:?}: {key_:?} in the composer");
            }
        });
    }

    /// The shell gives egui wider fonts in the next frame, where a face that loads is given.
    fn widen(h: &mut Harness<'static, Shell>) {
        let mut fonts = h.ctx.fonts(|f| f.definitions().clone());
        fonts.families.entry(egui::FontFamily::Name("ux-regular".into())).or_default().insert(0, "Hack".into());
        h.state_mut().fonts.give_next(fonts);
        h.step();
        let first = |h: &Harness<'static, Shell>| h.ctx.fonts(|f| f.definitions().families[&egui::FontFamily::Name("ux-regular".into())][0].clone());
        assert_ne!(first(h), "Hack", "the pass they are given in draws with the fonts before");
        h.run();
        assert_eq!(first(h), "Hack");
    }

    /// Replies laid out before the fonts change are laid out again in the new ones, shown or not:
    /// from the end, the first reply's second link, which a screen reader moves to, comes into view
    /// where it sits, as in a conversation laid out in the new fonts from the start.
    #[test]
    fn replies_are_laid_out_again_when_the_fonts_change() {
        in_both_appearances(|theme| {
            let reached = |wider_first: bool| {
                let (mut h, _rig) = long_chat_in(theme, wider_first);
                if !wider_first {
                    widen(&mut h);
                }
                focus_on(&mut h, &link(17));
                key(&mut h, Key::End, Modifiers::NONE);
                focus_on(&mut h, &link(1));
                assert!(whole(&h, &link(1)), "{theme:?}: the link is in view");
                let links = ids(&h).into_iter().filter(|i| i.starts_with("workspace.chat.link.")).map(|i| (rect_of(&h, &i), i));
                (scroll(&h), links.collect::<Vec<_>>())
            };
            assert_eq!(reached(false), reached(true), "{theme:?}: as laid out in the new fonts from the start");
        });
    }

    /// In a window so short, under a composer of six lines, that the transcript shows less than two
    /// lines, Page Up still moves it a line.
    #[test]
    fn a_page_key_moves_at_least_a_line_in_a_short_transcript() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = long_chat(theme);
            h.state_mut().chat_panel.draft = "1\n2\n3\n4\n5\n6".into();
            let line = crate::ux::types::UI.line;
            let short = |h: &Harness<'static, Shell>| h.state().chat_panel.area.unwrap().height() < 2.0 * line;
            for height in (400..1000).rev().step_by(8) {
                h.set_size(egui::vec2(1600.0, height as f32));
                h.run_steps(2);
                if short(&h) {
                    break;
                }
            }
            assert!(short(&h), "{theme:?}: a short transcript, {:?}", h.state().chat_panel.area);
            focus_on(&mut h, &link(17));
            let at = scroll(&h);
            assert!(at > line, "{theme:?}: room to move up ({at})");
            key(&mut h, Key::PageUp, Modifiers::NONE);
            assert_eq!(at - scroll(&h), line, "{theme:?}: Page Up moved a line");
        });
    }

    /// From the first link, out of view, Tab lands on every link in turn and Shift+Tab from the last
    /// on each back, none skipped, and the transcript brings each whole into view.
    #[test]
    fn tab_and_shift_tab_reach_every_link_and_the_transcript_follows() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = long_chat(theme);
            let order = (0..18).map(link).collect::<Vec<_>>();
            for back in [false, true] {
                let order: Vec<&String> = if back { order.iter().rev().collect() } else { order.iter().collect() };
                focus_on(&mut h, order[0]);
                assert!(whole(&h, order[0]), "{theme:?}: {} followed", order[0]);
                let mut landed = vec![order[0].clone()];
                while landed.len() < order.len() {
                    key(&mut h, Key::Tab, if back { Modifiers::SHIFT } else { Modifiers::NONE });
                    let on = order.iter().find(|id| focused_on(&h, id));
                    landed.push(on.map_or_else(|| format!("{:?}", h.ctx.memory(|m| m.focused())), |id| (*id).clone()));
                    if let Some(on) = on {
                        assert!(whole(&h, on), "{theme:?}: {on} in view, after {:?}", &landed[..landed.len() - 1]);
                    }
                }
                assert_eq!(landed.iter().collect::<Vec<_>>(), order, "{theme:?}: every link in turn");
            }
        });
    }

    /// Tab and End in one frame: the transcript goes to its end and brings the link Tab moved to into
    /// view from there, never back to its start.
    #[test]
    fn tab_and_end_in_one_frame_bring_the_link_into_view_from_the_end() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = long_chat(theme);
            let room = scroll(&h);
            focus_on(&mut h, &link(16));
            press_to(&mut h, Key::Home, 0.0, &link(16));
            // The harness gives each queued event a frame of its own: both go into the next frame's input.
            for pressed in [true, false] {
                for key_ in [Key::Tab, Key::End] {
                    let event = egui::Event::Key { key: key_, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
                    h.input_mut().events.push(event);
                }
                h.step();
            }
            h.run();
            assert!(focused_on(&h, &link(17)) && whole(&h, &link(17)), "{theme:?}: the link Tab moved to is in view");
            assert!(scroll(&h) > room / 2.0, "{theme:?}: near the end, not the start ({} of {room})", scroll(&h));
        });
    }

    /// A reply coming in while a link has the keyboard and the transcript is paged up leaves the
    /// place; back at the end by End, the end stays in view as the reply grows. A long piece that
    /// comes in the frame Tab is pressed takes the transcript to its new end, and the link the
    /// keyboard goes to is brought into view from there.
    #[test]
    fn a_reply_coming_in_leaves_the_place_the_keys_paged_to() {
        in_both_appearances(|theme| {
            let (mut h, rig) = long_chat(theme);
            ask_in_chat(&mut h, "Question 6");
            chat_event(&mut h, &rig, ChatEvent::Text { delta: "Reading the model.".into() });
            let last = link(17);
            focus_on(&mut h, &last);
            let more = "\n\nThe order keeps its lines, and each line names a product and how many.";
            // A piece comes in the frame right after Page Up's.
            h.key_down(Key::PageUp);
            h.step();
            let paged = scroll(&h);
            assert!(rig.agents.emit(&crate::chat_fixture::key(h.state()), ChatEvent::Text { delta: more.into() }));
            h.key_up(Key::PageUp);
            h.run();
            assert!(!h.state().chat_panel.at_end, "{theme:?}: paged up off the end");
            assert!(scroll(&h) == paged && focused_on(&h, &last), "{theme:?}: the place stays as the reply grows");
            key(&mut h, Key::End, Modifiers::NONE);
            let end = scroll(&h);
            assert!(end > paged && h.state().chat_panel.at_end && focused_on(&h, &last), "{theme:?}: End went to the end");
            chat_event(&mut h, &rig, ChatEvent::Text { delta: more.into() });
            assert!(scroll(&h) > end && h.state().chat_panel.at_end, "{theme:?}: at the end, the end stays in view");
            focus_on(&mut h, &link(16));
            key(&mut h, Key::End, Modifiers::NONE);
            assert!(h.state().chat_panel.at_end && focused_on(&h, &link(16)), "{theme:?}: on link 16 at the end");
            // Tab moves the keyboard in the frame it is pressed, the frame the long piece comes in.
            assert!(rig.agents.emit(&crate::chat_fixture::key(h.state()), ChatEvent::Text { delta: more.repeat(30) }));
            h.key_down(Key::Tab);
            h.step();
            h.key_up(Key::Tab);
            h.run();
            assert!(focused_on(&h, &link(17)) && whole(&h, &link(17)), "{theme:?}: the link is in view after the long piece");
        });
    }

    /// The transcript's own Tab stop.
    const STOP: &str = "workspace.chat.transcript";

    /// The canvas's controls of the diagram's boxes in the frame.
    fn boxes(h: &Harness<'static, Shell>) -> usize {
        ids(h).into_iter().filter(|i| i.starts_with("workspace.canvas.node.")).count()
    }

    /// Whether the transcript's focus ring is painted, in the focus colour just inside its edge.
    fn ringed(h: &Harness<'static, Shell>) -> bool {
        let (focus, area) = (crate::ux::palette(h.state().state.theme.is_dark()).focus, h.state().chat_panel.area.unwrap());
        let ring = |s: &egui::Shape| matches!(s, egui::Shape::Rect(r) if r.stroke.color == egui::Color32::from(focus) && r.rect == area.shrink(2.0));
        h.output().shapes.iter().any(|c| ring(&c.shape))
    }

    /// Six answered questions whose replies hold no link, at the design's window height, Order
    /// selected on the canvas: the transcript at its end, with nothing in it to take the keyboard
    /// but itself.
    fn plain_chat(theme: ThemeKind) -> (Harness<'static, Shell>, crate::chat_fixture::Rig) {
        let (mut h, rig) = chat_harness(theme, true);
        let text = "The order keeps its lines, and each line names a product and how many. ".repeat(6);
        for n in 0..6 {
            ask_in_chat(&mut h, &format!("Question {n}"));
            chat_event(&mut h, &rig, ChatEvent::Message { text: text.clone() });
            chat_event(&mut h, &rig, ChatEvent::TurnEnded(TurnEnd::Completed));
        }
        let no_link = !ids(&h).iter().any(|i| i.starts_with("workspace.chat.link."));
        assert!(h.state().chat_panel.at_end && scroll(&h) > 0.0 && no_link, "{theme:?}: at the end, no link in it");
        (h, rig)
    }

    /// With no link or button in it, Tab reaches the transcript itself, which a screen reader hears
    /// as the conversation, and nothing moves; while it has the keyboard, the page keys scroll it in
    /// a frame the key asks for, and Delete stays away from the canvas; not with the menu bar open,
    /// also in the frame a click of the pointer or a screen reader opens it in, nor from the composer.
    #[test]
    fn the_transcript_takes_the_keyboard_for_its_page_keys_with_no_link_in_it() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = plain_chat(theme);
            let (room, boxes_before) = (scroll(&h), boxes(&h));
            let page = h.state().chat_panel.area.unwrap().height() - crate::ux::types::UI.line;
            let node = h.get_by_label("Conversation").accesskit_node();
            assert_eq!(node.role(), egui::accesskit::Role::Log, "{theme:?}: a screen reader hears what it is");
            // The pointer leaves, so nothing shows for it; Tab from the agent's badge reaches the transcript.
            h.event(egui::Event::PointerMoved(egui::pos2(10.0, 600.0)));
            focus_on(&mut h, "workspace.chat.badge");
            assert!(!ringed(&h), "{theme:?}: no ring while the badge has the keyboard");
            for _ in 0..8 {
                if focused_on(&h, STOP) {
                    break;
                }
                key(&mut h, Key::Tab, Modifiers::NONE);
            }
            assert!(focused_on(&h, STOP) && ringed(&h), "{theme:?}: Tab reached the transcript, ringed, not {:?}", focused(&h));
            assert_eq!(scroll(&h), room, "{theme:?}: the keyboard coming to it moved nothing");
            h.key_down(Key::PageUp);
            h.step();
            h.step();
            let asked = h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("chat_view.rs"));
            assert!(asked, "{theme:?}: Page Up asks for the frame that draws it: {:?}", h.ctx.repaint_causes());
            h.key_up(Key::PageUp);
            h.run();
            assert_eq!(scroll(&h), room - page, "{theme:?}: Page Up");
            press_to(&mut h, Key::Home, 0.0, STOP);
            press_to(&mut h, Key::PageDown, page, STOP);
            press_to(&mut h, Key::End, room, STOP);
            // Delete, which takes Order off the diagram while the canvas has the keys, stays away from it.
            h.state_mut().canvas.focused = true;
            key(&mut h, Key::Delete, Modifiers::NONE);
            assert!(focused_on(&h, STOP) && boxes(&h) == boxes_before, "{theme:?}: Delete left the canvas alone");
            // The in-window menu bar holds the keys, also in the frame a click opens it in.
            h.state_mut().in_window_menu = true;
            key(&mut h, Key::F10, Modifiers::NONE);
            assert!(h.state().menu_bar.open.is_some() && focused_on(&h, STOP), "{theme:?}: F10 opens the bar's first menu");
            key(&mut h, Key::Home, Modifiers::NONE);
            assert_eq!(scroll(&h), room, "{theme:?}: Home with the menu bar open");
            key(&mut h, Key::Escape, Modifiers::NONE);
            for pointer in [true, false] {
                focus_on(&mut h, STOP);
                open_file_with(&mut h, Key::Home, Modifiers::NONE, pointer);
                assert_eq!(scroll(&h), room, "{theme:?}: with the click that opens the menu bar, by the pointer: {pointer}");
                key(&mut h, Key::Escape, Modifiers::NONE);
                assert!(h.state().menu_bar.open.is_none(), "{theme:?}: Escape closed the menu bar, by the pointer: {pointer}");
            }
            h.state_mut().in_window_menu = false;
            // The composer keeps Home: what is typed after it lands at its start.
            focus_on(&mut h, "workspace.chat.composer");
            h.event(egui::Event::Text("abc".into()));
            key(&mut h, Key::Home, Modifiers::NONE);
            h.event(egui::Event::Text("x".into()));
            h.run();
            assert_eq!((h.state().chat_panel.draft.as_str(), scroll(&h)), ("xabc", room), "{theme:?}: Home in the composer");
            // With nothing holding the keyboard, the canvas takes Delete.
            h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id("workspace.chat.composer")));
            h.state_mut().canvas.focused = true;
            key(&mut h, Key::Delete, Modifiers::NONE);
            assert!(boxes(&h) < boxes_before, "{theme:?}: Delete took Order off the diagram");
        });
    }

    /// The history of B7 with 19 older conversations under its five, 24 rows at the design's window
    /// height, the open one first: its later rows under the fold.
    fn long_history(theme: ThemeKind) -> (Harness<'static, Shell>, crate::chat_fixture::Rig) {
        use crate::chat_store::{Row, fake::Kept};
        let (mut h, rig) = chat_harness(theme, true);
        for n in 0..19_u64 {
            let id = (h.state().services.ids.clone().unwrap())();
            let (origin, title) = (format!("Chat {}", n + 6), format!("An older question {n}"));
            let row = Row {
                id,
                origin,
                title,
                provider: crate::ai_agents::Provider::Claude,
                model: "sonnet".into(),
                updated_at: 1_793_000_000 - n * 3600,
                messages: 2,
                removed: Default::default(),
            };
            rig.store.hold(Kept { row, turns: Vec::new(), messages: Vec::new(), calls: Vec::new(), ends: Vec::new() });
        }
        crate::chat_fixture::history(h.state_mut(), &rig).unwrap();
        h.run();
        assert!(has_id(&h, &row(0)) && !has_id(&h, &row(23)), "{theme:?}: the last row under the fold");
        (h, rig)
    }

    /// The id of the history's row `i`.
    fn row(i: usize) -> String {
        format!("workspace.chat.history.{i}")
    }

    /// Whether the history's row `i` is listed and shows whole: inside the list, from where its
    /// first row starts unscrolled to the panel's foot, where its resize handle ends.
    fn row_whole(h: &Harness<'static, Shell>, i: usize) -> bool {
        let (r, foot) = (rect_of(h, &row(i)), rect_of(h, "workspace.panel.resize").bottom());
        let first = r.top() - i as f32 * 50.0 + h.state().chat_panel.history_scroll;
        r.top() >= first && r.bottom() <= foot
    }

    /// Presses `key_` and asserts that the keyboard went to the row `to`, which shows whole.
    fn press_to_row(h: &mut Harness<'static, Shell>, key_: Key, to: usize) {
        key(h, key_, Modifiers::NONE);
        assert!(focused_on(h, &row(to)) && row_whole(h, to), "{key_:?}: on {:?}, not row {to}", focused(h));
    }

    /// While a row of the history has the keyboard, Page Down and Page Up move it by what shows less
    /// a row, End and Home to the last row and the first, each brought whole into view, Home's own
    /// frame drawing the list from its top; not with the keyboard nowhere, with the control key as
    /// Windows and Linux send it, under the row's menu or the confirmation (a screen reader can put
    /// the keyboard on a row there), or with the in-window menu bar open, also in the frame a click
    /// opens it in.
    #[test]
    fn the_page_keys_move_the_keyboard_through_the_history() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = long_history(theme);
            let scroll = |h: &Harness<'static, Shell>| h.state().chat_panel.history_scroll;
            key(&mut h, Key::End, Modifiers::NONE);
            assert!(scroll(&h) == 0.0 && focused(&h).is_none(), "{theme:?}: with the keyboard nowhere");
            let top = rect_of(&h, &row(0)).top();
            focus_on(&mut h, &row(0));
            // 780 points show: a page is 730, fourteen rows on.
            for (key_, to) in [(Key::PageDown, 14), (Key::PageDown, 23), (Key::Home, 0), (Key::End, 23), (Key::PageUp, 9), (Key::PageUp, 0)] {
                press_to_row(&mut h, key_, to);
            }
            assert_eq!(scroll(&h), 0.0, "{theme:?}: back at the top");
            // The frame Home is pressed in draws the first row at the list's top: none comes after.
            press_to_row(&mut h, Key::End, 23);
            h.key_down(Key::Home);
            h.step();
            assert_eq!(rect_of(&h, &row(0)).top(), top, "{theme:?}: Home's own frame draws the list from its top");
            h.key_up(Key::Home);
            h.run();
            key(&mut h, Key::End, Modifiers::CTRL.plus(Modifiers::COMMAND));
            assert!(focused_on(&h, &row(0)) && scroll(&h) == 0.0, "{theme:?}: not with the control key");
            // ⇧F10 opens the row's menu and ⌫ the confirmation, each taking the keyboard; a screen
            // reader can still put it on a row under them.
            for (key_, modifiers, menu) in [(Key::F10, Modifiers::SHIFT, true), (Key::Backspace, Modifiers::NONE, false)] {
                key(&mut h, key_, modifiers);
                focus_on(&mut h, &row(0));
                let open = if menu { h.state().chat_panel.row_menu.is_some() } else { h.state().chat_panel.ask.is_some() };
                assert!(open, "{theme:?}: {key_:?} opens over the row, the keyboard on {:?}", focused(&h));
                key(&mut h, Key::End, Modifiers::NONE);
                assert!(scroll(&h) == 0.0 && !has_id(&h, &row(23)), "{theme:?}: not with what {key_:?} opened");
                key(&mut h, Key::Escape, Modifiers::NONE);
                h.run();
                let closed = h.state().chat_panel.row_menu.is_none() && h.state().chat_panel.ask.is_none();
                assert!(closed && focused_on(&h, &row(0)), "{theme:?}: Escape gives the row the keyboard back");
            }
            // F10's menu bar leaves the keyboard on the row, and holds the keys.
            h.state_mut().in_window_menu = true;
            key(&mut h, Key::F10, Modifiers::NONE);
            assert!(h.state().menu_bar.open.is_some() && focused_on(&h, &row(0)), "{theme:?}: F10 opens the bar's first menu");
            key(&mut h, Key::End, Modifiers::NONE);
            assert_eq!(scroll(&h), 0.0, "{theme:?}: not with the menu bar open");
            key(&mut h, Key::Escape, Modifiers::NONE);
            focus_on(&mut h, &row(0));
            for pointer in [true, false] {
                open_file_with(&mut h, Key::End, Modifiers::NONE, pointer);
                assert_eq!(scroll(&h), 0.0, "{theme:?}: not in the frame of the click that opens the bar, by the pointer: {pointer}");
                key(&mut h, Key::Escape, Modifiers::NONE);
                focus_on(&mut h, &row(0));
            }
            press_to_row(&mut h, Key::End, 23);
        });
    }

    /// From the first row, Tab lands on each row in turn, and Shift+Tab from the last on each back,
    /// none skipped, each brought whole into view; the frame Tab moves the keyboard in asks for the
    /// next, which brings it there. The wheel moves the list from the row with the keyboard; Home
    /// and End bring the first and the last row back whole, the keyboard on them already; and a row
    /// the wheel left just past either edge is reached all the same.
    #[test]
    fn tab_and_shift_tab_reach_every_row_of_the_history_and_it_follows() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = long_history(theme);
            focus_on(&mut h, &row(0));
            h.key_down(Key::Tab);
            h.step();
            h.step();
            let asked = h.ctx.repaint_causes().iter().any(|c| c.file.ends_with("chat_view.rs"));
            assert!(asked, "{theme:?}: Tab asks for the frame that follows it: {:?}", h.ctx.repaint_causes());
            h.key_up(Key::Tab);
            h.run();
            let forth = (0..24).map(row).collect::<Vec<_>>();
            let back = (0..24).rev().map(row).collect::<Vec<_>>();
            for (order, shift) in [(&forth, false), (&back, true)] {
                focus_on(&mut h, &order[0]);
                let mut landed = vec![order[0].clone()];
                while landed.len() < order.len() {
                    key(&mut h, Key::Tab, if shift { Modifiers::SHIFT } else { Modifiers::NONE });
                    let on = focused(&h).unwrap_or_default();
                    let at = (0..24).find(|&i| on == row(i));
                    assert!(at.is_some_and(|i| row_whole(&h, i)), "{theme:?}: {on} in view, after {landed:?}");
                    landed.push(on);
                }
                assert_eq!(&landed, order, "{theme:?}: every row in turn, Shift: {shift}");
            }
            // The wheel leaves the first row or the last cut, the keyboard still on it, and the list
            // stays there; Home or End brings it whole into view, though the keyboard stays.
            for (to, dy, key_) in [(0, -20.0, Key::Home), (23, 20.0, Key::End)] {
                press_to_row(&mut h, key_, to);
                let at = rect_of(&h, &row(to)).center();
                h.event(egui::Event::PointerMoved(at));
                let (unit, phase, source) = (egui::MouseWheelUnit::Point, egui::TouchPhase::Move, egui::MouseWheelSource::Wheel);
                h.event(egui::Event::MouseWheel { unit, delta: egui::vec2(0.0, dy), modifiers: Modifiers::NONE, phase, source });
                // The wheel's scroll eases in over a few frames.
                h.run_steps(12);
                h.run();
                assert!(focused_on(&h, &row(to)) && !row_whole(&h, to), "{theme:?}: the wheel cut row {to}, the keyboard on it");
                press_to_row(&mut h, key_, to);
            }
            // The wheel can leave the next row just past either edge: Tab and Shift+Tab reach it. The
            // pointer leaves, so no row shows its bin for it.
            h.event(egui::Event::PointerMoved(egui::pos2(10.0, 600.0)));
            for (from, scroll, shift, to) in [(15, 19.0, false, 16), (5, 249.0, true, 4)] {
                focus_on(&mut h, &row(from));
                h.state_mut().chat_panel.history_scroll = scroll;
                h.run();
                assert!(row_whole(&h, from) && !has_id(&h, &row(to)), "{theme:?}: row {to} just out of view");
                key(&mut h, Key::Tab, if shift { Modifiers::SHIFT } else { Modifiers::NONE });
                assert!(focused_on(&h, &row(to)) && row_whole(&h, to), "{theme:?}: Tab reached row {to}, Shift: {shift}");
            }
        });
    }

    /// In the smallest window the history shows three rows whole and cuts the fourth: Tab from the
    /// third reaches the fourth and the fifth, each brought whole into view, and End and Home its
    /// ends. A page key moves at least to the next row that can be read, past rows that cannot.
    #[test]
    fn in_the_smallest_window_tab_and_the_page_keys_reach_every_row_of_the_history() {
        in_both_appearances(|theme| {
            let (mut h, _rig, _) = chat_history(theme);
            h.set_size(egui::vec2(1600.0, 400.0));
            h.run();
            assert!(row_whole(&h, 2) && !row_whole(&h, 3) && !has_id(&h, &row(4)), "{theme:?}: the fourth row cut, the fifth out of view");
            focus_on(&mut h, &row(2));
            press_to_row(&mut h, Key::Tab, 3);
            press_to_row(&mut h, Key::Tab, 4);
            press_to_row(&mut h, Key::Home, 0);
            press_to_row(&mut h, Key::End, 4);
            press_to_row(&mut h, Key::Home, 0);
            // A page is 130 points: Page Down from the first row goes past the two under it, which
            // cannot be read, to the fourth; Page Up from there back to the first.
            if let Some(Ok(rows)) = h.state_mut().chat_panel.history.as_mut() {
                rows[1].entry = Err("its provider is not one Archforge knows".into());
                rows[2].entry = Err("its provider is not one Archforge knows".into());
            }
            h.run();
            press_to_row(&mut h, Key::PageDown, 3);
            press_to_row(&mut h, Key::PageUp, 0);
        });
    }

    /// In the smallest window, with the four rows under the third unreadable, Tab from the third row
    /// lands on the eighth, the next that can be read, far past the list's foot, and Shift+Tab from
    /// there back on the third, far past its top; each brought whole into view.
    #[test]
    fn tab_and_shift_tab_reach_the_next_readable_row_past_unreadable_ones() {
        in_both_appearances(|theme| {
            let (mut h, _rig) = long_history(theme);
            h.set_size(egui::vec2(1600.0, 400.0));
            h.event(egui::Event::PointerMoved(egui::pos2(10.0, 600.0)));
            if let Some(Ok(rows)) = h.state_mut().chat_panel.history.as_mut() {
                for listed in &mut rows[3..7] {
                    listed.entry = Err("its provider is not one Archforge knows".into());
                }
            }
            h.run();
            focus_on(&mut h, &row(2));
            assert!(row_whole(&h, 2) && !has_id(&h, &row(7)), "{theme:?}: the eighth row far under the list's foot");
            press_to_row(&mut h, Key::Tab, 7);
            assert!(!has_id(&h, &row(2)), "{theme:?}: the third row far above the list's top");
            key(&mut h, Key::Tab, Modifiers::SHIFT);
            assert!(focused_on(&h, &row(2)) && row_whole(&h, 2), "{theme:?}: Shift+Tab is back on the third row, not {:?}", focused(&h));
        });
    }
}

/// The history lists the project's conversations and opens one whole, read-only, resuming its program.
#[test]
fn the_history_lists_the_projects_conversations_and_opens_one_whole_and_read_only() {
    use crate::chat::ChatEvent;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    let at = rect_of(&h, "workspace.chat.allow-changes").center();
    click(&mut h, at);
    ask_in_chat(&mut h, "First question");
    chat_event(&mut h, &rig, ChatEvent::Message { text: "First answer".into() });
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    let first = h.state().chat_panel.chat.as_ref().unwrap().id;
    let at = rect_of(&h, "workspace.chat.new").center();
    click(&mut h, at);
    assert!(painted_has(&h, "Ask about Commerce platform"), "a new conversation");
    ask_in_chat(&mut h, "Second question");
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    let at = rect_of(&h, "workspace.chat.history").center();
    click(&mut h, at);
    assert!(painted_has(&h, "SAVED WITH COMMERCE PLATFORM"), "{:?}", painted(&h));
    assert!(painted_has(&h, "Conversations are kept in the project, so they travel with it and never show in another project."));
    let rows: Vec<String> = ids(&h).into_iter().filter(|i| i.starts_with("workspace.chat.history.")).collect();
    assert_eq!(rows.len(), 2);
    assert!(painted_has(&h, "Today 14:26 · Claude · 2 messages") && painted_has(&h, "Today 14:26 · Claude · 1 message"), "{:?}", painted(&h));
    // The older one, opened again: whole, read-only, its program not started until asked.
    let at = rect_of(&h, "workspace.chat.history.1").center();
    click(&mut h, at);
    let shown = h.state().chat_panel.chat.as_ref().unwrap();
    assert_eq!((shown.id, shown.turns.len(), shown.allow_changes), (first, 1, false));
    assert!(painted_has(&h, "First question") && painted_has(&h, "First answer"), "{:?}", painted(&h));
    let starts = chat_calls(&rig).0;
    ask_in_chat(&mut h, "Go on");
    let started = rig.agents.started.lock().unwrap().clone();
    assert_eq!(started.len(), starts + 1);
    let last = started.last().unwrap();
    assert_eq!((last.origin.as_str(), last.resume, last.allow_changes), ("Chat 1", true, false));
    assert_eq!(kept_chat(&h, &rig).turns.len(), 2, "its turns go on from where they were");
}

/// A chip removed before the first question stays removed when the conversation is opened again
/// from Conversations (AR-227): it is kept in the project with the conversation, not drawn, and
/// the next question's context line leaves it out. In both appearances.
#[test]
fn a_removed_chip_stays_removed_when_the_conversation_is_opened_again() {
    use crate::chat::ChatEvent;
    use crate::chat_panel::Chip;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        let at = rect_of(&h, "workspace.chat.chip.diagram.remove").center();
        click(&mut h, at);
        ask_in_chat(&mut h, "Which classes have no description yet?");
        assert_eq!(*rig.store.log.lock().unwrap(), ["create", "chips", "ask"], "{theme:?}: kept once the conversation is made");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        let first = h.state().chat_panel.chat.as_ref().unwrap().id;
        assert_eq!(rig.store.kept(first).unwrap().row.removed, std::collections::BTreeSet::from([Chip::Diagram]), "{theme:?}");
        // Another conversation, then the first again from Conversations.
        let at = rect_of(&h, "workspace.chat.new").center();
        click(&mut h, at);
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.chip.diagram.remove"), "{theme:?}: a new conversation has every chip");
        ask_in_chat(&mut h, "Second question");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        let at = rect_of(&h, "workspace.chat.history").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.chat.history.1").center();
        click(&mut h, at);
        h.run();
        assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.id), Some(first), "{theme:?}");
        let shown = ids(&h);
        assert!(!shown.iter().any(|i| i == "workspace.chat.chip.diagram.remove"), "{theme:?}: still removed");
        assert!(shown.iter().any(|i| i == "workspace.chat.chip.project.remove"), "{theme:?}: the others stay");
        ask_in_chat(&mut h, "Go on");
        let sent = sent_texts(&rig).last().cloned().unwrap_or_default();
        assert!(sent.starts_with("Go on") && sent.contains("project \"Commerce platform\""), "{theme:?}: {sent}");
        assert!(!sent.contains("open diagram"), "{theme:?}: the removed chip is not sent: {sent}");
    }
}

/// A chip removed in a conversation already in the project is kept at once, and putting it back is
/// kept too (AR-227): opened again, the conversation shows the chip and sends it. In both
/// appearances.
#[test]
fn putting_a_removed_chip_back_is_kept_with_the_conversation() {
    use crate::chat::ChatEvent;
    use crate::chat_panel::Chip;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "Describe Order.");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        let first = h.state().chat_panel.chat.as_ref().unwrap().id;
        let at = rect_of(&h, "workspace.chat.chip.selection.remove").center();
        click(&mut h, at);
        assert_eq!(rig.store.log.lock().unwrap().last().copied(), Some("chips"), "{theme:?}: kept at once");
        assert_eq!(rig.store.kept(first).unwrap().row.removed, std::collections::BTreeSet::from([Chip::Selection]), "{theme:?}");
        crate::chat_panel::set_chip(h.state_mut(), Chip::Selection, false);
        h.run();
        assert!(rig.store.kept(first).unwrap().row.removed.is_empty(), "{theme:?}: put back, and kept so");
        let writes = rig.store.log.lock().unwrap().len();
        crate::chat_panel::set_chip(h.state_mut(), Chip::Selection, false);
        assert_eq!(rig.store.log.lock().unwrap().len(), writes, "{theme:?}: nothing changed, nothing written");
        // Opened again after another conversation: the chip is there and goes with the question.
        let at = rect_of(&h, "workspace.chat.new").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.chat.history").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.chat.history.0").center();
        click(&mut h, at);
        h.run();
        assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.id), Some(first), "{theme:?}");
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.chip.selection.remove"), "{theme:?}: back");
        ask_in_chat(&mut h, "And Payment?");
        let sent = sent_texts(&rig).last().cloned().unwrap_or_default();
        assert!(sent.contains("selected \"Order\""), "{theme:?}: {sent}");
    }
}

/// A chip removal the project refused is not lost (AR-227): the chip stays removed, the panel says
/// the conversation could not be saved, and the next question keeps the whole set first, so the
/// latest set is the one stored. In both appearances.
#[test]
fn a_refused_chip_removal_is_kept_again_with_the_next_question() {
    use crate::chat::ChatEvent;
    use crate::chat_panel::Chip;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "Describe Order.");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        let first = h.state().chat_panel.chat.as_ref().unwrap().id;
        *rig.store.refuse.lock().unwrap() = Some("the disk is full".into());
        let at = rect_of(&h, "workspace.chat.chip.diagram.remove").center();
        click(&mut h, at);
        h.run();
        assert!(!ids(&h).iter().any(|i| i == "workspace.chat.chip.diagram.remove"), "{theme:?}: still removed here");
        assert!(painted_has(&h, "This conversation could not be saved: the disk is full"), "{theme:?}: {:?}", painted(&h));
        assert!(rig.store.kept(first).unwrap().row.removed.is_empty(), "{theme:?}: not kept yet");
        // The project takes writes again: the next question keeps the removal before it goes.
        *rig.store.refuse.lock().unwrap() = None;
        ask_in_chat(&mut h, "And Payment?");
        let log = rig.store.log.lock().unwrap().clone();
        assert_eq!(log[log.len() - 2..], ["chips", "ask"], "{theme:?}: {log:?}");
        assert_eq!(rig.store.kept(first).unwrap().row.removed, std::collections::BTreeSet::from([Chip::Diagram]), "{theme:?}");
        let sent = sent_texts(&rig).last().cloned().unwrap_or_default();
        assert!(sent.starts_with("And Payment?") && !sent.contains("open diagram"), "{theme:?}: {sent}");
    }
}

/// Answers every write the fake store holds back with a refusal, `why`.
fn refuse_held(rig: &crate::chat_fixture::Rig, why: &str) {
    for (_, tx) in rig.store.held.lock().unwrap().drain(..) {
        let _ = tx.send(Err(crate::chat_store::Refusal::Failed(why.into())));
    }
}

/// A question asked while a chip removal is still unanswered waits for the set to be kept
/// (AR-227): it is not written or sent first, and when the project refuses the set the question
/// stays in the box with the reason; asked again once the project takes writes, the set is kept
/// first. In both appearances.
#[test]
fn a_question_waits_for_an_unanswered_chip_removal_and_stays_when_it_is_refused() {
    use crate::chat::ChatEvent;
    use crate::chat_panel::Chip;
    use std::sync::atomic::Ordering::SeqCst;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "Describe Order.");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        let first = h.state().chat_panel.chat.as_ref().unwrap().id;
        let sent = sent_texts(&rig).len();
        let asks = |rig: &crate::chat_fixture::Rig| rig.store.log.lock().unwrap().iter().filter(|k| **k == "ask").count();
        let asked = asks(&rig);
        rig.store.hold.store(true, SeqCst);
        let at = rect_of(&h, "workspace.chat.chip.diagram.remove").center();
        click(&mut h, at);
        ask_in_chat(&mut h, "And Payment?");
        let log = rig.store.log.lock().unwrap().clone();
        assert_eq!(log[log.len() - 2..], ["chips", "chips"], "{theme:?}: the set is kept again first, the question waits: {log:?}");
        assert_eq!(sent_texts(&rig).len(), sent, "{theme:?}: nothing sent yet");
        // The project refuses: the question stays in the box with the reason, nothing asked.
        refuse_held(&rig, "the disk is full");
        rig.store.hold.store(false, SeqCst);
        h.run();
        assert!(painted_has(&h, "This conversation could not be saved: the disk is full"), "{theme:?}: {:?}", painted(&h));
        assert_eq!(h.state().chat_panel.draft, "And Payment?", "{theme:?}");
        assert_eq!(asks(&rig), asked, "{theme:?}: the question was not written");
        assert_eq!(sent_texts(&rig).len(), sent, "{theme:?}: not sent");
        assert!(rig.store.kept(first).unwrap().row.removed.is_empty(), "{theme:?}");
        // Sent again: the set is kept, then the question goes without the diagram. Each answer is
        // taken on a frame of its own (the window looks again every 100 ms while it waits).
        crate::chat_panel::send(h.state_mut());
        for _ in 0..3 {
            h.run();
        }
        let log = rig.store.log.lock().unwrap().clone();
        assert_eq!(log[log.len() - 2..], ["chips", "ask"], "{theme:?}: {log:?}");
        assert_eq!(rig.store.kept(first).unwrap().row.removed, std::collections::BTreeSet::from([Chip::Diagram]), "{theme:?}");
        let last = sent_texts(&rig).last().cloned().unwrap_or_default();
        assert!(last.starts_with("And Payment?") && !last.contains("open diagram"), "{theme:?}: {last}");
    }
}

/// A chip removal the project refuses after the removal was drawn is said without other input
/// (AR-227): the panel keeps looking while the write is unanswered. In both appearances.
#[test]
fn a_late_refused_chip_removal_is_said_without_other_input() {
    use crate::chat::ChatEvent;
    use std::sync::atomic::Ordering::SeqCst;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "Describe Order.");
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        rig.store.hold.store(true, SeqCst);
        let at = rect_of(&h, "workspace.chat.chip.diagram.remove").center();
        click(&mut h, at);
        assert!(crate::chat_panel::waiting(h.state()), "{theme:?}: the panel keeps looking while the removal is unanswered");
        refuse_held(&rig, "the disk is full");
        rig.store.hold.store(false, SeqCst);
        h.run();
        assert!(painted_has(&h, "This conversation could not be saved: the disk is full"), "{theme:?}: {:?}", painted(&h));
        assert!(!crate::chat_panel::waiting(h.state()), "{theme:?}: answered");
    }
}

/// The Chat panel's history of page 07 (B7): five conversations kept, the first open, the history
/// shown; their ids in the list's order.
fn chat_history(theme: ThemeKind) -> (Harness<'static, Shell>, crate::chat_fixture::Rig, Vec<archforge_mof::Uuid>) {
    let (mut h, rig) = chat_harness(theme, true);
    let chats = crate::chat_fixture::history(h.state_mut(), &rig).unwrap();
    h.run();
    (h, rig, chats)
}

/// The pointer moved to `at`, nothing pressed.
fn point_at(h: &mut Harness<'static, Shell>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    h.run();
}

/// The widget the keyboard is on, by automation id.
fn focused(h: &Harness<'static, Shell>) -> Option<String> {
    let id = h.ctx.memory(|m| m.focused())?;
    h.state().widgets().iter().find(|w| crate::widgets::egui_id(&w.id) == id).map(|w| w.id.clone())
}

/// The keyboard put on the control `id`, as Tab would bring it there.
fn focus_on(h: &mut Harness<'static, Shell>, id: &str) {
    h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(id)));
    h.run();
}

/// Moves the fixed clock on by `secs` and draws a frame.
fn later(h: &mut Harness<'static, Shell>, rig: &crate::chat_fixture::Rig, secs: u64) {
    rig.now.fetch_add(secs, std::sync::atomic::Ordering::Relaxed);
    h.run();
}

/// The folders the fake host removed.
fn forgotten(rig: &crate::chat_fixture::Rig) -> Vec<String> {
    rig.agents.forgotten.lock().unwrap().clone()
}

/// B7a to B7c through the bin (AR-217): the bin shows on the row the pointer is on, the
/// confirmation names the conversation and what deleting does, starts on Cancel and Escape gives
/// the row its focus back; Delete takes the row away, writes one deletion, leaves the model and its
/// undo history alone, and the toast offers Undo, which brings the conversation back whole without
/// its folder ever being removed, and the row returns in its place with the keyboard on it.
#[test]
fn the_bin_asks_first_and_undo_on_the_toast_brings_the_conversation_back_whole() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig, chats) = chat_history(theme);
        let review = chats[2];
        let whole = rig.store.kept(review).unwrap();
        let undo_label = h.state().session.as_ref().and_then(|s| s.undo_label().map(str::to_string));
        assert!(!ids(&h).iter().any(|i| i == "workspace.chat.history.2.delete"), "no bin before the pointer comes");
        let row = rect_of(&h, "workspace.chat.history.2");
        point_at(&mut h, row.center());
        let bin = rect_of(&h, "workspace.chat.history.2.delete");
        assert!(row.contains(bin.center()) && bin.center().x > row.center().x, "{theme:?}: the bin at the row's right");
        click(&mut h, bin.center());
        for line in [
            "Delete this conversation?",
            "Review the checkout package · 12 messages with Codex, yesterday",
            "It is removed from this project's conversation list.",
            "Cancel",
            "Delete",
        ] {
            assert!(painted_has(&h, line), "{theme:?}: {line:?} in {:?}", painted(&h));
        }
        assert!(painted(&h).iter().any(|t| t.starts_with("It stays in the project's saved history: anyone with a copy")), "{theme:?}");
        assert!(!painted_has(&h, "Its reply in progress stops first."), "no reply runs");
        assert_eq!(focused(&h).as_deref(), Some("workspace.chat.delete.cancel"), "{theme:?}: the focus starts on Cancel");
        for more in NOT_ALONE {
            key(&mut h, egui::Key::Escape, more);
            assert!(h.state().chat_panel.ask.is_some(), "{theme:?}: Escape with {more:?} cancels nothing");
        }
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.run();
        assert!(h.state().chat_panel.ask.is_none() && rig.store.kept(review).is_some(), "Escape cancels");
        assert_eq!(focused(&h).as_deref(), Some("workspace.chat.history.2"), "{theme:?}: the focus is back on the row");
        point_at(&mut h, row.center());
        let bin = rect_of(&h, "workspace.chat.history.2.delete");
        click(&mut h, bin.center());
        let at = rect_of(&h, "workspace.chat.delete.confirm").center();
        click(&mut h, at);
        h.run();
        assert!(rig.store.kept(review).is_none(), "{theme:?}: deleted");
        assert_eq!(rig.store.log.lock().unwrap().iter().filter(|k| **k == "delete").count(), 1);
        assert!(!painted(&h).iter().any(|t| t == "Review the checkout package"), "{theme:?}: gone from the list");
        assert!(painted_has(&h, "Review the checkout package deleted · still in the project's saved history"), "{theme:?}: {:?}", painted(&h));
        assert!(painted_has(&h, "Undo"));
        assert_eq!(focused(&h).as_deref(), Some("workspace.chat.history.2"), "{theme:?}: the next row takes the keyboard");
        assert_eq!(h.state().session.as_ref().and_then(|s| s.undo_label().map(str::to_string)), undo_label, "the model's undo is untouched");
        assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.id), Some(chats[0]), "the open conversation stays open");
        assert!(forgotten(&rig).is_empty(), "the folder waits while Undo is offered");
        let at = rect_of(&h, "workspace.chat.toast.undo").center();
        click(&mut h, at);
        h.run();
        let back = rig.store.kept(review).unwrap();
        assert_eq!((back.row, back.turns, back.messages), (whole.row.clone(), whole.turns.clone(), whole.messages.clone()), "{theme:?}: whole");
        assert!(!ids(&h).iter().any(|i| i == "workspace.chat.toast"), "the toast is gone");
        h.run();
        assert!(painted(&h).iter().any(|t| t == "Review the checkout package"), "{theme:?}: back in the list");
        assert_eq!(focused(&h).as_deref(), Some("workspace.chat.history.2"), "{theme:?}: in its place, with the keyboard on it");
        later(&mut h, &rig, 60);
        assert!(forgotten(&rig).is_empty(), "{theme:?}: a conversation brought back keeps its folder");
        // The pointer was used last: no ring shows on the row the keyboard is on.
        assert!(!crate::settings::keyboard_last(&h.ctx));
    }
}

/// The row's menu (right-click, the menu key, ⇧F10) and ⌫ or Delete on the focused row open the
/// same confirmation; the toast stays while the pointer is on it and closes eight seconds after,
/// only then letting the folder go; ⌘Z meanwhile is the model's undo and never brings the
/// conversation back; and while the toast is up, a new conversation skips the deleted one's label.
#[test]
fn the_menu_and_the_keys_ask_the_same_and_the_folder_goes_only_when_the_toast_closes() {
    let (mut h, rig, chats) = chat_history(ThemeKind::Light);
    let row = rect_of(&h, "workspace.chat.history.0");
    // With the canvas holding the keys, the menu's keys never reach it.
    h.state_mut().canvas.focused = true;
    let selected = h.state().session.as_ref().unwrap().view().selection.clone();
    let revision = h.state().session.as_ref().unwrap().revision();
    right_click(&mut h, row.center());
    for k in [egui::Key::ArrowDown, egui::Key::ArrowUp, egui::Key::Backspace] {
        key(&mut h, k, egui::Modifiers::NONE);
    }
    let session = h.state().session.as_ref().unwrap();
    assert_eq!((session.revision(), &session.view().selection), (revision, &selected), "the canvas took none of the menu's keys");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    right_click(&mut h, row.center());
    assert!(ids(&h).iter().any(|i| i == "workspace.chat.history.menu.open") && painted_has(&h, "Delete conversation"), "{:?}", painted(&h));
    let at = rect_of(&h, "workspace.chat.history.menu.delete").center();
    click(&mut h, at);
    assert!(h.state().chat_panel.ask.as_ref().is_some_and(|a| a.chat == chats[0]), "the menu's item asks");
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // The keys on the focused row act alone: with a modifier ⌫, Delete and the menu key ask nothing.
    focus_on(&mut h, "workspace.chat.history.0");
    for (k, more) in [egui::Key::Backspace, egui::Key::Delete, egui::Key::Menu].into_iter().flat_map(|k| NOT_ALONE.map(|m| (k, m))) {
        key(&mut h, k, more);
        let quiet = h.state().chat_panel.ask.is_none() && h.state().chat_panel.row_menu.is_none();
        assert!(quiet && focused(&h).as_deref() == Some("workspace.chat.history.0"), "{k:?} with {more:?}");
    }
    // The keys on the focused row: ⌫, Delete, the menu key and ⇧F10.
    for (k, m, menu) in [
        (egui::Key::Backspace, egui::Modifiers::NONE, false),
        (egui::Key::Delete, egui::Modifiers::NONE, false),
        (egui::Key::Menu, egui::Modifiers::NONE, true),
        (egui::Key::F10, egui::Modifiers::SHIFT, true),
    ] {
        focus_on(&mut h, "workspace.chat.history.0");
        point_at(&mut h, egui::pos2(10.0, 600.0));
        key(&mut h, egui::Key::Tab, egui::Modifiers::SHIFT);
        key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(focused(&h).as_deref(), Some("workspace.chat.history.0"));
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.history.0.delete"), "the bin shows on the row the keyboard is on");
        key(&mut h, k, m);
        h.run();
        if menu {
            assert!(
                h.state().chat_panel.row_menu.as_ref().is_some_and(|r| r.chat == chats[0] && r.reached == Some(0)),
                "{k:?} opens the menu, Open reached"
            );
            // Its keys act alone: with a modifier they move, pick and close nothing.
            for (k, more) in [egui::Key::ArrowDown, egui::Key::Enter, egui::Key::Escape].into_iter().flat_map(|k| NOT_ALONE.map(|m| (k, m))) {
                key(&mut h, k, more);
                let open = h.state().chat_panel.row_menu.as_ref().is_some_and(|r| r.reached == Some(0));
                assert!(open && h.state().chat_panel.ask.is_none(), "{k:?} with {more:?}");
            }
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
            key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        }
        assert!(h.state().chat_panel.ask.as_ref().is_some_and(|a| a.chat == chats[0]), "{k:?} asks");
        assert!(crate::settings::keyboard_last(&h.ctx), "the keyboard was used last: focus rings show");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(h.state().chat_panel.ask.is_none());
    }
    // Deleted with Enter on the focused Delete button.
    focus_on(&mut h, "workspace.chat.history.0");
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    assert_eq!(focused(&h).as_deref(), Some("workspace.chat.delete.confirm"), "Tab reaches Delete");
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    let key_of = chats[0].to_string();
    assert!(rig.store.kept(chats[0]).is_none());
    // ⌘Z with nothing focused is the model's: the conversation stays deleted.
    h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id("workspace.chat.history.0")));
    key(&mut h, egui::Key::Z, egui::Modifiers::COMMAND);
    h.run();
    assert!(rig.store.kept(chats[0]).is_none() && !rig.store.log.lock().unwrap().contains(&"restore"), "⌘Z never restores");
    // A new conversation while Undo is offered: past Chat 5, the deleted one's label, the highest.
    let at = rect_of(&h, "workspace.chat.new").center();
    click(&mut h, at);
    ask_in_chat(&mut h, "A new question");
    assert_eq!(h.state().chat_panel.chat.as_ref().and_then(|c| c.origin.clone()).as_deref(), Some("Chat 6"));
    let at = rect_of(&h, "workspace.chat.history").center();
    click(&mut h, at);
    // The pointer on the toast holds it past its time.
    let toast = rect_of(&h, "workspace.chat.toast");
    point_at(&mut h, toast.center());
    for _ in 0..6 {
        later(&mut h, &rig, 5);
    }
    assert!(ids(&h).iter().any(|i| i == "workspace.chat.toast"), "held while the pointer is on it");
    point_at(&mut h, egui::pos2(10.0, 600.0));
    // So does the keyboard on Undo.
    focus_on(&mut h, "workspace.chat.toast.undo");
    for _ in 0..6 {
        later(&mut h, &rig, 5);
    }
    assert!(ids(&h).iter().any(|i| i == "workspace.chat.toast"), "held while the keyboard is on Undo");
    h.ctx.memory_mut(|m| m.surrender_focus(crate::widgets::egui_id("workspace.chat.toast.undo")));
    later(&mut h, &rig, 7);
    assert!(ids(&h).iter().any(|i| i == "workspace.chat.toast") && forgotten(&rig).is_empty(), "still within its eight seconds");
    later(&mut h, &rig, 2);
    h.run();
    assert!(!ids(&h).iter().any(|i| i == "workspace.chat.toast"), "closed");
    assert_eq!(forgotten(&rig), [key_of], "its folder goes once the toast closed, and only its");
}

/// ⇧F10 on the focused row opens its menu only as itself: with Alt more, a Mac's Control or the
/// command key of either shape it opens nothing, and the keyboard stays on the row.
#[test]
fn shift_f10_opens_a_conversations_menu_only_with_shift_alone() {
    use egui::Modifiers as M;
    in_both_appearances(|theme| {
        let (mut h, _rig, chats) = chat_history(theme);
        focus_on(&mut h, "workspace.chat.history.0");
        for more in [M::ALT, M::CTRL, M::MAC_CMD | M::COMMAND, M::CTRL | M::COMMAND] {
            key(&mut h, egui::Key::F10, M::SHIFT | more);
            let row = focused_on(&h, "workspace.chat.history.0");
            assert!(h.state().chat_panel.row_menu.is_none() && row, "{theme:?}: {more:?} more opens nothing");
        }
        key(&mut h, egui::Key::F10, M::SHIFT);
        assert!(h.state().chat_panel.row_menu.as_ref().is_some_and(|r| r.chat == chats[0]), "{theme:?}: ⇧F10 opens the row's menu");
    });
}

/// Deleting the open conversation while its reply runs (AR-217): the confirmation says the reply
/// stops first; its reply so far is kept and its turn ends stopped, both written before the
/// deletion; the next row opens and takes the keyboard, then the one above; the last one deleted
/// leaves a new empty conversation (B2) and the keyboard on New conversation.
#[test]
fn the_open_conversation_stops_first_and_gives_way_to_the_next_one() {
    use crate::chat::ChatEvent;
    let (mut h, rig, chats) = chat_history(ThemeKind::Light);
    // The first conversation, open, asked again and answering. The panel itself keeps the history
    // shut while a reply runs; it is shown here as when one starts while it is open.
    let at = rect_of(&h, "workspace.chat.history").center();
    click(&mut h, at);
    ask_in_chat(&mut h, "And the rest?");
    chat_event(&mut h, &rig, ChatEvent::Text { delta: "So far".into() });
    h.state_mut().chat_panel.view = crate::chat_panel::View::History;
    h.run();
    let row = rect_of(&h, "workspace.chat.history.0");
    point_at(&mut h, row.center());
    let bin = rect_of(&h, "workspace.chat.history.0.delete");
    click(&mut h, bin.center());
    assert!(painted_has(&h, "Its reply in progress stops first."), "{:?}", painted(&h));
    let at = rect_of(&h, "workspace.chat.delete.confirm").center();
    click(&mut h, at);
    h.run();
    let log = rig.store.log.lock().unwrap().clone();
    let at = log.iter().position(|k| *k == "delete").unwrap();
    assert_eq!(&log[at - 2..=at], ["reply", "end", "delete"], "stopped and written first: {log:?}");
    let deleted = rig.store.deleted.lock().unwrap().last().unwrap().1.clone();
    assert_eq!(deleted.turns.last().unwrap().outcome, crate::chat_store::Outcome::Stopped);
    assert!(
        deleted.messages.iter().any(|m| matches!(&m.body, crate::chat_store::StoredBody::Reply { text, complete: false, .. } if text == "So far"))
    );
    assert!(rig.agents.running.lock().unwrap().is_empty(), "its program was let go of");
    h.run();
    assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.id), Some(chats[1]), "the next one opens");
    assert_eq!(h.state().chat_panel.view, crate::chat_panel::View::History, "the history stays");
    assert_eq!(focused(&h).as_deref(), Some("workspace.chat.history.0"));
    // The last row of the list, open: the one above opens.
    let last = rect_of(&h, "workspace.chat.history.3").center();
    click(&mut h, last);
    let at = rect_of(&h, "workspace.chat.history").center();
    click(&mut h, at);
    focus_on(&mut h, "workspace.chat.history.3");
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
    key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    h.run();
    assert_eq!(h.state().chat_panel.chat.as_ref().map(|c| c.id), Some(chats[3]), "the one above opens");
    assert_eq!(focused(&h).as_deref(), Some("workspace.chat.history.2"), "the row above takes the keyboard");
    // Then every other one: the last leaves a new, empty conversation and New conversation.
    for _ in 0..3 {
        focus_on(&mut h, "workspace.chat.history.0");
        key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run();
    }
    assert!(rig.store.chats.lock().unwrap().is_empty());
    let shown = h.state().chat_panel.chat.as_ref().unwrap();
    assert!(shown.turns.is_empty() && shown.origin.is_none(), "a new, empty conversation");
    assert_eq!(focused(&h).as_deref(), Some("workspace.chat.new"));
}

/// A deletion confirmed while its conversation's question is still on its way to the project waits
/// for it: the question lands first, its turn ends stopped, then the deletion; Undo then brings
/// back a conversation with no turn left running (AR-217).
#[test]
fn a_deletion_waits_for_a_question_on_its_way() {
    let (mut h, rig, chats) = chat_history(ThemeKind::Light);
    let at = rect_of(&h, "workspace.chat.history").center();
    click(&mut h, at);
    rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
    ask_in_chat(&mut h, "A question on its way");
    assert!(h.state().chat_panel.chat.as_ref().unwrap().step.busy(), "the question waits for the writer");
    h.state_mut().chat_panel.view = crate::chat_panel::View::History;
    h.run();
    let row = rect_of(&h, "workspace.chat.history.0");
    point_at(&mut h, row.center());
    let bin = rect_of(&h, "workspace.chat.history.0.delete");
    click(&mut h, bin.center());
    let at = rect_of(&h, "workspace.chat.delete.confirm").center();
    click(&mut h, at);
    assert!(!rig.store.log.lock().unwrap().contains(&"delete"), "the deletion waits for the question");
    rig.store.release();
    h.run();
    h.run();
    let log = rig.store.log.lock().unwrap().clone();
    let at = log.iter().position(|k| *k == "delete").expect("deleted once the question landed");
    assert_eq!(&log[at - 2..=at], ["ask", "end", "delete"], "{log:?}");
    let at = rect_of(&h, "workspace.chat.toast.undo").center();
    click(&mut h, at);
    h.run();
    let back = rig.store.kept(chats[0]).unwrap();
    assert!(back.turns.iter().all(|t| t.outcome != crate::chat_store::Outcome::Running), "no turn left running");
}

/// Undo pressed before the deletion finished runs once it has, and keeps the folder; a deletion
/// whose toast is still up when its project closes, or when Archforge quits, lets its folder go
/// then; one still waiting for the project then hands its answer to the host, which lets the
/// folder go only once deleted, and an Undo still waiting keeps it once the conversation is back
/// (AR-217).
#[test]
fn undo_before_the_deletion_finished_runs_after_it_and_closing_or_quitting_settles_the_folders() {
    let (mut h, rig, chats) = chat_history(ThemeKind::Light);
    rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
    let row = rect_of(&h, "workspace.chat.history.2");
    point_at(&mut h, row.center());
    let bin = rect_of(&h, "workspace.chat.history.2.delete");
    click(&mut h, bin.center());
    let at = rect_of(&h, "workspace.chat.delete.confirm").center();
    click(&mut h, at);
    assert!(rig.store.kept(chats[2]).is_some(), "the writer has not taken the deletion yet");
    let at = rect_of(&h, "workspace.chat.toast.undo").center();
    click(&mut h, at);
    assert!(rig.store.kept(chats[2]).is_some(), "the writer has taken neither yet");
    rig.store.release();
    h.run();
    h.run();
    let log = rig.store.log.lock().unwrap().clone();
    assert_eq!(log.iter().rev().take(2).rev().copied().collect::<Vec<_>>(), ["delete", "restore"], "{log:?}");
    assert!(rig.store.kept(chats[2]).is_some(), "brought back");
    assert!(forgotten(&rig).is_empty());
    // A toast still up when the project closes: its folder goes.
    let row = rect_of(&h, "workspace.chat.history.3");
    point_at(&mut h, row.center());
    let bin = rect_of(&h, "workspace.chat.history.3.delete");
    click(&mut h, bin.center());
    let at = rect_of(&h, "workspace.chat.delete.confirm").center();
    click(&mut h, at);
    assert!(forgotten(&rig).is_empty());
    h.state_mut().close_project();
    h.run();
    assert_eq!(forgotten(&rig), [chats[3].to_string()], "closing the project settles it");
    // And at quit: a toast still up in the project shown lets its folder go.
    let (mut h, rig, chats) = chat_history(ThemeKind::Light);
    let row = rect_of(&h, "workspace.chat.history.4");
    point_at(&mut h, row.center());
    let bin = rect_of(&h, "workspace.chat.history.4.delete");
    click(&mut h, bin.center());
    let at = rect_of(&h, "workspace.chat.delete.confirm").center();
    click(&mut h, at);
    h.state_mut().before_exit();
    assert_eq!(forgotten(&rig), [chats[4].to_string()], "quitting settles it");
    // Quitting while a deletion, and another one's Undo, wait for the writer: the host takes
    // their answers; the deleted one's folder goes, the one brought back keeps its own.
    let (mut h, rig, chats) = chat_history(ThemeKind::Light);
    rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
    for (row, undo) in [(4, false), (3, true)] {
        let r = rect_of(&h, &format!("workspace.chat.history.{row}"));
        point_at(&mut h, r.center());
        let bin = rect_of(&h, &format!("workspace.chat.history.{row}.delete"));
        click(&mut h, bin.center());
        let at = rect_of(&h, "workspace.chat.delete.confirm").center();
        click(&mut h, at);
        if undo {
            let at = rect_of(&h, "workspace.chat.toast.undo").center();
            click(&mut h, at);
        }
    }
    h.state_mut().before_exit();
    assert!(forgotten(&rig).is_empty(), "nothing is removed before the project answers");
    rig.store.release();
    rig.agents.settle_after();
    assert_eq!(forgotten(&rig), [chats[4].to_string()], "the deleted one's folder alone");
    assert!(rig.store.kept(chats[3]).is_some() && rig.store.kept(chats[4]).is_none());
}

/// Undo pressed before the deletion answered, then the project closed (AR-218): the host is handed
/// both answers. When the deletion fails and the restoration after it fails too (the project could
/// not read its list), the conversation is still listed and its folder stays; only a deletion that
/// was made and a restoration that did not bring it back let the folder go.
#[test]
fn an_undo_before_a_failed_deletion_keeps_the_folder_of_a_conversation_still_listed_at_close() {
    use crate::chat_store::{Answer, Refusal};
    for (deletion, goes) in [(Err(Refusal::Failed("the project is busy".into())), false), (Ok(Answer::Deleted { before: "0".repeat(32) }), true)] {
        let (mut h, rig, chats) = chat_history(ThemeKind::Light);
        rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
        let row = rect_of(&h, "workspace.chat.history.2");
        point_at(&mut h, row.center());
        let bin = rect_of(&h, "workspace.chat.history.2.delete");
        click(&mut h, bin.center());
        let at = rect_of(&h, "workspace.chat.delete.confirm").center();
        click(&mut h, at);
        let at = rect_of(&h, "workspace.chat.toast.undo").center();
        click(&mut h, at);
        h.state_mut().close_project();
        h.run();
        // The writer answers the deletion as the case says, then refuses the restoration.
        let held: Vec<_> = std::mem::take(&mut *rig.store.held.lock().unwrap());
        let kinds: Vec<&str> = held.iter().map(|(w, _)| w.kind()).collect();
        assert_eq!(kinds, ["delete", "restore"], "both wait for the writer");
        let mut answers = [deletion.clone(), Err(Refusal::Failed("no deletion of it was asked in this project".into()))].into_iter();
        for (_, tx) in held {
            let _ = tx.send(answers.next().unwrap());
        }
        rig.agents.settle_after();
        let key = chats[2].to_string();
        if goes {
            assert_eq!(forgotten(&rig), [key], "deleted and not brought back: its folder goes");
        } else {
            assert!(forgotten(&rig).is_empty(), "not deleted: the folder of a conversation still listed stays");
            assert!(rig.store.kept(chats[2]).is_some(), "still listed");
        }
    }
}

/// Only some refusals of a restoration mean the conversation stays deleted: a changed branch may
/// still list it, so its folder stays, while the refusals that always let it go still do.
#[test]
fn a_restore_refused_for_a_changed_branch_keeps_the_folder_and_only_that_reason_decides() {
    use crate::chat_store::{Answer, Refusal};
    let cases = [
        (Refusal::BranchChanged, false),
        (Refusal::Listed, false),
        (Refusal::Failed("the project is busy".into()), true),
        (Refusal::HeldKey { provider: None }, true),
    ];
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for (refusal, goes) in cases.clone() {
            let (mut h, rig, chats) = chat_history(theme);
            rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
            let row = rect_of(&h, "workspace.chat.history.2");
            point_at(&mut h, row.center());
            let bin = rect_of(&h, "workspace.chat.history.2.delete");
            click(&mut h, bin.center());
            let at = rect_of(&h, "workspace.chat.delete.confirm").center();
            click(&mut h, at);
            let at = rect_of(&h, "workspace.chat.toast.undo").center();
            click(&mut h, at);
            // The writes are answered and never run: the new branch still holds the conversation.
            let held: Vec<_> = std::mem::take(&mut *rig.store.held.lock().unwrap());
            let mut answers = [Ok(Answer::Deleted { before: "0".repeat(32) }), Err(refusal.clone())].into_iter();
            for (_, tx) in held {
                let _ = tx.send(answers.next().unwrap());
            }
            h.run();
            h.run();
            let expected: Vec<String> = if goes { vec![chats[2].to_string()] } else { Vec::new() };
            assert_eq!(forgotten(&rig), expected, "{theme:?}: {refusal:?}");
            assert!(rig.store.kept(chats[2]).is_some(), "still listed");
        }
    }
}

/// A deleted branch lets go of the folders of the conversations only on it, a running program
/// ended first, once it is seen gone and only then: no refusal lets go of any, nor does a project
/// that stopped answering. An answer still comes after the project closed, and at quit one that
/// has come is taken; a conversation a panel shows keeps its folder. In both appearances.
#[test]
fn a_deleted_branchs_conversation_folders_go_only_once_it_is_seen_gone() {
    use crate::BranchNotDeleted::{Failed, InUse, Moved, Unclear};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        let host = std::sync::Arc::new(SavingHost::default());
        h.state_mut().services.projects = Some(host.clone());
        let path = std::path::PathBuf::from("/tmp/Commerce platform.archforge");
        let ask = |h: &mut Harness<'static, Shell>| {
            crate::chat_delete::delete_branch(h.state_mut(), &path, "topic");
            host.branch_deletes.lock().unwrap().pop().unwrap()
        };
        let id = |n: u8| archforge_mof::Uuid::v7(1_762_400_000_000, [n; 10]);
        for refusal in [Unclear("why".into()), Moved, InUse, Failed("why".into())] {
            let answer = ask(&mut h);
            h.run();
            answer.send(Err(refusal.clone())).unwrap();
            h.run();
            assert!(forgotten(&rig).is_empty(), "{refusal:?}");
            assert!(h.state().chats.branch_deletes.is_empty(), "{refusal:?}: answered");
        }
        drop(ask(&mut h));
        h.run();
        assert!(forgotten(&rig).is_empty() && h.state().chats.branch_deletes.is_empty(), "no answer will come");
        let running = id(1).to_string();
        rig.agents.running.lock().unwrap().push(running.clone());
        let shown = h.state().chat_panel.chat.as_ref().unwrap().id;
        let answer = ask(&mut h);
        assert!(crate::chat_delete::waiting(h.state()), "the window looks again for the answer");
        crate::chat_delete::settle(h.state_mut(), Some(&path));
        h.run();
        assert!(forgotten(&rig).is_empty() && !h.state().chats.branch_deletes.is_empty(), "the closing waits for no answer and drops none");
        answer.send(Ok(vec![id(1), shown])).unwrap();
        h.run();
        assert_eq!(forgotten(&rig), std::slice::from_ref(&running), "{theme:?}: only the one no panel shows");
        assert!(!rig.agents.running.lock().unwrap().contains(&running), "its program ended");
        let answered = ask(&mut h);
        answered.send(Ok(vec![id(2)])).unwrap();
        let _unanswered = ask(&mut h);
        crate::chat_delete::settle(h.state_mut(), None);
        assert_eq!(forgotten(&rig), [running, id(2).to_string()], "quitting takes what has come");
        assert_eq!(h.state().chats.branch_deletes.len(), 1, "and keeps the folders of what has not");
    }
}

/// A conversation started while the host removes its private folder (AR-218), as a copy of the
/// project can: the host refuses the start, the turn says it is not started yet as the conversation
/// is being removed (AR-225), with Send again, and nothing hangs; the refused start keeps the
/// folder although the deletion went through, and once the removal is done, Send again starts it.
/// In both appearances.
#[test]
fn a_conversation_started_while_its_folder_is_being_removed_says_so_and_sends_again_once_done() {
    use crate::ai_agents::FolderGoes;
    use crate::chat_store::Answer;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        ask_in_chat(&mut h, "First question");
        chat_event(&mut h, &rig, crate::chat::ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        let key = crate::chat_fixture::key(h.state());
        let _ = crate::chat::end(h.state(), &key);
        h.run();
        assert!(rig.agents.running.lock().unwrap().is_empty(), "its program ended");
        // A copy of the project deleted it, and its folder's removal waits for that project's answer.
        let (tx, rx) = std::sync::mpsc::channel();
        let removal = crate::ai_agents::AgentHost::forget_after(&*rig.agents, &key, vec![(rx, FolderGoes::IfDeleted)]);
        let starts = chat_calls(&rig).0;
        ask_in_chat(&mut h, "Second question");
        assert_eq!(chat_calls(&rig).0, starts + 1, "a start was asked");
        assert!(rig.agents.running.lock().unwrap().is_empty(), "and refused");
        // Its own card says why and what to do (AR-225), not the generic No answer.
        assert!(
            painted_has(&h, "Not started yet") && painted_has(&h, "This conversation is being removed. Try again in a moment."),
            "{theme:?}: {:?}",
            painted(&h)
        );
        assert!(!painted_has(&h, "No answer") && !painted_has(&h, "Claude stopped before answering."));
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.turn.2.retry"), "{theme:?}: Send again is offered");
        assert!(ids(&h).iter().any(|i| i == "workspace.chat.send"), "the composer is free: nothing hangs");
        // The copy's deletion went through, but a panel here asked for the conversation: its folder stays.
        tx.send(Ok(Answer::Deleted { before: "0".repeat(32) })).unwrap();
        rig.agents.settle_after();
        assert_eq!(removal.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), Ok(false));
        let at = rect_of(&h, "workspace.chat.turn.2.retry").center();
        click(&mut h, at);
        assert_eq!(*rig.agents.running.lock().unwrap(), std::slice::from_ref(&key), "it starts now");
        assert!(rig.agents.sent.lock().unwrap().iter().any(|(c, t)| *c == key && t.starts_with("Second question")), "and gets the question");
        assert!(forgotten(&rig).is_empty());
    }
}

/// When a re-read of the list fails, the rows read before stay and their changes are off, saying
/// why: the bin and the menu's item are disabled and ⌫ asks nothing (AR-217).
#[test]
fn with_chat_writing_off_the_bin_and_the_item_are_disabled_and_say_why() {
    let (mut h, rig, chats) = chat_history(ThemeKind::Dark);
    let at = rect_of(&h, "workspace.chat.history").center();
    click(&mut h, at);
    *rig.store.unreadable.lock().unwrap() = Some("the table chat_turn does not have the shape Archforge writes".into());
    click(&mut h, at);
    h.run();
    assert!(painted_has(&h, "Review the checkout package"), "the rows read before stay");
    let why = "The conversations in this project cannot be changed: the table chat_turn does not have the shape Archforge writes";
    let refused = crate::chat_delete::refusal(&h.state().chat_panel, &Ok(rig.store.kept(chats[2]).unwrap().row));
    assert_eq!(refused.as_deref(), Some(why));
    let row = rect_of(&h, "workspace.chat.history.2");
    point_at(&mut h, row.center());
    let bin = rect_of(&h, "workspace.chat.history.2.delete");
    click(&mut h, bin.center());
    assert!(h.state().chat_panel.ask.is_none(), "the disabled bin asks nothing");
    right_click(&mut h, row.center());
    let item = rect_of(&h, "workspace.chat.history.menu.delete").center();
    click(&mut h, item);
    assert!(h.state().chat_panel.ask.is_none(), "nor the item");
    focus_on(&mut h, "workspace.chat.history.2");
    key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
    assert!(h.state().chat_panel.ask.is_none() && rig.store.kept(chats[2]).is_some(), "nor ⌫");
    // A row that cannot be read gets a disabled bin of its own.
    let unreadable = crate::chat_store::Listed { id: chats[0], entry: Err("its provider is not one Archforge knows".into()) };
    let panel = crate::chat_panel::Panel::default();
    assert_eq!(crate::chat_delete::refusal(&panel, &unreadable.entry).as_deref(), Some("This conversation cannot be read"));
}

/// A title holding the confirmation's own placeholders is shown as it was typed (AR-217).
#[test]
fn a_title_holding_placeholders_is_named_as_typed_in_the_confirmation() {
    let (mut h, rig, chats) = chat_history(ThemeKind::Light);
    if let Some(k) = rig.store.chats.lock().unwrap().iter_mut().find(|k| k.row.id == chats[2]) {
        k.row.title = "Why {agent} said {when} and {messages}".into();
    }
    let at = rect_of(&h, "workspace.chat.history").center();
    click(&mut h, at);
    click(&mut h, at);
    crate::chat_delete::ask(h.state_mut(), chats[2], 2);
    let summary = h.state().chat_panel.ask.as_ref().map(|a| a.summary.clone());
    assert_eq!(summary.as_deref(), Some("Why {agent} said {when} and {messages} · 12 messages with Codex, yesterday"));
}

/// A conversation's age in its confirmation: today, yesterday, a day and month this year, with the
/// year before.
#[test]
fn a_conversations_age_says_today_yesterday_or_its_date() {
    crate::i18n::set_language("en");
    let clock = crate::chat_words::Clock { now: crate::chat_fixture::NOW, offset: Some(3_600) };
    let day = 86_400;
    assert_eq!(crate::chat_delete::age(clock.now - 60, clock), "today");
    assert_eq!(crate::chat_delete::age(clock.now - day, clock), "yesterday");
    assert_eq!(crate::chat_delete::age(clock.now - 4 * day, clock), "3 Nov");
    assert_eq!(crate::chat_delete::age(clock.now - 400 * day, clock), "3 Oct 2025");
}

/// A tool row comes from the channel's answer; an origin acts only while its turn runs.
#[test]
fn a_tool_row_is_worded_from_the_channels_answer_and_only_a_running_turn_of_the_project_shown_may_act() {
    use crate::chat::ChatEvent;
    use crate::chat_fixture as fx;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "What is on the diagram?");
    let diagram = h.state().canvas.active.unwrap();
    fx::call(h.state_mut(), "engine.execute", serde_json::json!({ "command": "diagram.get", "params": { "diagram": diagram.to_string() } })).unwrap();
    h.run();
    assert!(painted_has(&h, "Read diagram Commerce"), "{:?}", painted(&h));
    let kept = kept_chat(&h, &rig);
    let facts = kept.messages.iter().find_map(|m| if let crate::chat_store::StoredBody::Tool(f) = &m.body { Some(f.clone()) } else { None }).unwrap();
    assert_eq!(
        (facts.tool.as_str(), facts.command.as_deref(), facts.status),
        ("engine.execute", Some("diagram.get"), crate::chat_store::ToolStatus::Done)
    );
    assert!(facts.anchors.iter().any(|a| a.role == crate::chat_store::AnchorRole::Read && a.metaclass == "sketch_class"), "{facts:?}");
    // The window's own controls and the command list make no row.
    fx::call(h.state_mut(), "engine.commands", serde_json::json!({})).unwrap();
    assert_eq!(kept_chat(&h, &rig).messages.len(), kept.messages.len());
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    // Its turn over, the conversation's origin may not act.
    let refused =
        fx::call(h.state_mut(), "engine.execute", serde_json::json!({ "command": "diagram.get", "params": { "diagram": diagram.to_string() } }));
    assert!(refused.unwrap_err().contains("has no turn running"));
}

/// The messages the fake agent was sent, in order.
fn sent_texts(rig: &crate::chat_fixture::Rig) -> Vec<String> {
    rig.agents.sent.lock().unwrap().iter().map(|(_, t)| t.clone()).collect()
}

/// A Stop the program never answers ends the program and the turn after the grace, so the panel
/// never sticks; closing a project lets its conversation go until its program has ended.
#[test]
fn a_stop_the_program_never_answers_ends_the_turn_after_the_grace() {
    use std::sync::atomic::Ordering::Relaxed;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "Long");
    let at = rect_of(&h, "workspace.chat.stop").center();
    click(&mut h, at);
    assert!(h.state().chat_panel.chat.as_ref().unwrap().busy(), "waits for the program");
    rig.now.fetch_add(crate::chat_panel::STOP_GRACE, Relaxed);
    h.run();
    let chat = h.state().chat_panel.chat.as_ref().unwrap();
    assert!(!chat.busy(), "ended here");
    assert_eq!(chat_calls(&rig).3, 1, "its program was ended");
    assert_eq!(kept_chat(&h, &rig).turns[0].outcome, crate::chat_store::Outcome::Stopped);
    assert!(ids(&h).iter().any(|i| i == "workspace.chat.send"));
}

/// A reply cut to its limit is kept marked shortened, and says so read back; a conversation opened
/// again while its program still ends waits for that end before its next question starts it again.
#[test]
fn a_cut_reply_is_kept_shortened_and_a_conversation_reopened_while_ending_waits_for_its_end() {
    use crate::chat::ChatEvent;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "Write a lot");
    chat_event(&mut h, &rig, ChatEvent::Message { text: "x".repeat(crate::chat_panel::REPLY_BYTES + 10) });
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    let kept = kept_chat(&h, &rig);
    assert!(
        matches!(&kept.messages[1].body, crate::chat_store::StoredBody::Reply { text, shortened: true, .. } if text.len() == crate::chat_panel::REPLY_BYTES),
        "kept cut and marked"
    );
    assert!(painted_has(&h, "The reply was shortened to fit."));
    // Let go of, then opened again while its program still ends (held as a slow program's end).
    let id = h.state().chat_panel.chat.as_ref().unwrap().id;
    let key = crate::chat_fixture::key(h.state());
    rig.agents.hold_ends.store(true, std::sync::atomic::Ordering::Relaxed);
    let at = rect_of(&h, "workspace.chat.new").center();
    click(&mut h, at);
    assert!(h.state().chats.letting_go.iter().any(|(k, _)| *k == key), "let go of, still ending");
    crate::chat_panel::open(h.state_mut(), id);
    h.run();
    let shown = h.state().chat_panel.chat.as_ref().unwrap();
    assert_eq!((shown.id, shown.session), (id, crate::chat_panel::Session::Ending), "taken back, its program still ending");
    assert!(h.state().chats.letting_go.is_empty());
    let starts = chat_calls(&rig).0;
    ask_in_chat(&mut h, "Go on");
    assert_eq!(chat_calls(&rig).0, starts, "the question waits for the end");
    rig.agents.hold_ends.store(false, std::sync::atomic::Ordering::Relaxed);
    rig.agents.running.lock().unwrap().retain(|k| *k != key);
    chat_event(&mut h, &rig, ChatEvent::Ended { failure: None });
    assert_eq!(chat_calls(&rig).0, starts + 1, "then starts the program again");
    assert!(sent_texts(&rig).last().is_some_and(|t| t.starts_with("Go on")));
}

/// A card says why its turn cannot be undone: a version in view is only for now, and a Restore
/// ends that history for good, however the new session's revisions happen to fall. Never by the
/// numbers of another session: a card does not move to undone because a revision matches.
#[test]
fn a_cards_undo_goes_by_the_history_its_turn_was_made_in() {
    use crate::chat::ChatEvent;
    use crate::chat_fixture as fx;
    use crate::chat_panel::{UndoRefusal, undo_refusal};
    let refusal = |h: &Harness<'static, Shell>| {
        let chat = h.state().chat_panel.chat.as_ref().unwrap();
        undo_refusal(h.state(), chat.origin.as_deref(), chat.turns.last().unwrap())
    };
    let state = |h: &Harness<'static, Shell>| h.state().chat_panel.chat.as_ref().unwrap().turns.last().unwrap().change.clone().unwrap();
    let another = |h: &mut Harness<'static, Shell>| {
        let (tables, junctions) = NOTATIONS.storage();
        let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
        let mut n = 0u64;
        let mint = move || {
            n += 1;
            let mut r = [0u8; 10];
            r[2..].copy_from_slice(&n.to_be_bytes());
            archforge_mof::Uuid::v7(1_761_670_900_000, r)
        };
        NOTATIONS.session_from_rows_repaired(&rows, Box::new(mint), |_| {}).unwrap()
    };
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, rig) = chat_harness(theme, true);
        let undo = "workspace.chat.turn.1.undo";
        let at = rect_of(&h, "workspace.chat.allow-changes").center();
        click(&mut h, at);
        ask_in_chat(&mut h, "Describe Customer.");
        let id = fx::element(h.state(), archforge_mof::toy::CLASS, "Customer").unwrap();
        let params = serde_json::json!({ "command": "element.describe", "params": { "element": id.to_string(), "text": "A buyer" } });
        fx::call(h.state_mut(), "engine.execute", params).unwrap();
        h.run();
        chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        assert_eq!(refusal(&h), None, "{theme:?}");
        assert!(ids(&h).iter().any(|i| i == undo));
        // A version in view: the history is only set aside, and comes back with current work.
        let preview = another(&mut h);
        h.state_mut().show_version(preview, "c1".into());
        h.run();
        assert_eq!(refusal(&h), Some(UndoRefusal::Viewing), "{theme:?}");
        assert!(painted_has(&h, &UndoRefusal::Viewing.words()), "{theme:?}: {:?}", painted(&h));
        assert!(!ids(&h).iter().any(|i| i == undo));
        assert!(h.state_mut().back_to_current_work());
        h.run();
        assert_eq!(refusal(&h), None, "{theme:?}: as it was");
        assert!(ids(&h).iter().any(|i| i == undo));
        // A Restore: a new session and a new history, whichever revision it is at.
        let restored = another(&mut h);
        h.state_mut().install(restored, false);
        h.run();
        assert_eq!(refusal(&h), Some(UndoRefusal::HistoryGone), "{theme:?}");
        assert!(painted_has(&h, &UndoRefusal::HistoryGone.words()), "{theme:?}: {:?}", painted(&h));
        assert_ne!(UndoRefusal::HistoryGone.words(), UndoRefusal::ChangedAfter.words(), "the card names the real reason");
        assert!(!ids(&h).iter().any(|i| i == undo));
        let span = state(&h).span.unwrap();
        while h.state().session.as_ref().unwrap().revision() < span.before {
            let name = format!("later{}", h.state().session.as_ref().unwrap().revision());
            h.state_mut().session.as_mut().unwrap().execute("sketch.package.add", &serde_json::json!({ "name": name })).unwrap();
        }
        assert_eq!(h.state().session.as_ref().unwrap().revision(), span.before, "the same number, in another history");
        h.run();
        assert_eq!(state(&h).state, crate::chat_store::ChangeState::Applied, "{theme:?}: not read as undone");
        assert_eq!(refusal(&h), Some(UndoRefusal::HistoryGone), "{theme:?}");
    }
}

/// The branch a project was left on goes with its opening, stays through being opened again, and
/// reads as none from a file from before branches or when it is planted and too long.
#[test]
fn the_branch_a_project_was_left_on_is_kept_with_it() {
    let (mut h, host) = saving_harness(None, None);
    let path = std::path::PathBuf::from("/tmp/Commerce platform.archforge");
    let state = &mut h.state_mut().state;
    state.remember("Commerce platform", &path, 1);
    state.keep_branch(&path, Some("elsewhere".into()));
    state.remember("Commerce platform", &path, 2);
    assert_eq!(state.branch_of(&path), Some("elsewhere"), "opened again, it keeps it");
    h.state_mut().open_path(&path);
    assert_eq!(host.opened_on.lock().unwrap().as_slice(), [Some("elsewhere".to_string())]);
    let old: crate::state::RecentProject = serde_json::from_str(r#"{ "name": "Old", "path": "/tmp/Old.archforge", "opened": 1 }"#).unwrap();
    assert_eq!(old.branch, None, "from before branches");
    let mut planted = crate::UiState::default();
    planted.remember("Planted", std::path::Path::new("/tmp/Planted.archforge"), 1);
    planted.keep_branch(std::path::Path::new("/tmp/Planted.archforge"), Some("x".repeat(2048)));
    planted.sanitize();
    assert_eq!(planted.branch_of(std::path::Path::new("/tmp/Planted.archforge")), None);
}

/// A project that could not open on its branch says so in the status bar, in both appearances, and
/// opens on `main` from then on.
#[test]
fn the_status_bar_says_why_a_project_is_not_on_its_branch() {
    use crate::BranchNotice::{CannotOpen, Gone};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for notice in [Gone { name: "topic".into() }, CannotOpen { name: "topic".into(), why: "it holds triggers".into() }] {
            let (mut h, _host) = saving_harness(Some(commerce()), None);
            h.state_mut().state.theme = theme;
            let path = commerce().path;
            h.state_mut().state.keep_branch(&path, Some("topic".into()));
            h.state_mut().tabs.clear();
            h.state_mut().active = None;
            h.state_mut().open_project(crate::Opened { notice: Some(notice.clone()), ..commerce() });
            h.run_steps(3);
            assert!(painted_has(&h, &notice.words()), "{theme:?}: {:?}", painted(&h));
            assert_eq!(h.state().state.branch_of(&path), None, "{theme:?}: it is on main now");
        }
    }
}

/// A failed attempt's cut reply does not mark the next attempt's: Send again starts the turn's
/// per-attempt state afresh.
#[test]
fn send_again_starts_the_attempts_state_afresh() {
    use crate::chat::{ChatEvent, FailureKind};
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "Write a lot");
    chat_event(&mut h, &rig, ChatEvent::Text { delta: "y".repeat(crate::chat_panel::REPLY_BYTES + 10) });
    chat_event(&mut h, &rig, failed_turn(FailureKind::Other));
    // (The long reply has scrolled its card's Send again out of view: sent again as the card does.)
    crate::chat_panel::retry(h.state_mut(), None);
    h.run();
    let turn = h.state().chat_panel.chat.as_ref().unwrap().turns.last().unwrap();
    assert!(!turn.cut && !turn.stopping && turn.stopping_since.is_none() && turn.streaming.is_none() && !turn.returned);
    chat_event(&mut h, &rig, ChatEvent::Text { delta: "Short.".into() });
    chat_event(&mut h, &rig, ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
    let kept = kept_chat(&h, &rig);
    let replies: Vec<bool> = kept
        .messages
        .iter()
        .filter_map(|m| if let crate::chat_store::StoredBody::Reply { shortened, .. } = &m.body { Some(*shortened) } else { None })
        .collect();
    assert_eq!(replies, [true, false], "the first attempt's reply was cut, the second's not");
}

/// Closing a project while a reply runs ends its turn as stopped there and then, the reply so far
/// kept, written to that project: for the project shown and for a parked one.
#[test]
fn closing_a_project_while_a_reply_runs_keeps_the_reply_and_the_stopped_turn() {
    use crate::chat::ChatEvent;
    use crate::chat_store::{Outcome, StoredBody};
    let replies = |k: &crate::chat_store::fake::Kept| -> Vec<(String, bool)> {
        k.messages
            .iter()
            .filter_map(|m| if let StoredBody::Reply { text, complete, .. } = &m.body { Some((text.clone(), *complete)) } else { None })
            .collect()
    };
    // Shown.
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "Explain Order");
    chat_event(&mut h, &rig, ChatEvent::Text { delta: "Order holds".into() });
    let id = h.state().chat_panel.chat.as_ref().unwrap().id;
    let tab = h.state().active.unwrap();
    h.state_mut().close_tab(tab);
    h.run();
    let kept = rig.store.kept(id).unwrap();
    assert_eq!(kept.turns[0].outcome, Outcome::Stopped);
    assert_eq!(replies(&kept), [("Order holds".to_string(), false)]);
    // Parked: its stop asked when its tab was left, not answered before it closes.
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "Explain Payment");
    chat_event(&mut h, &rig, ChatEvent::Text { delta: "Payment is".into() });
    let id = h.state().chat_panel.chat.as_ref().unwrap().id;
    let first = h.state().active.unwrap();
    h.state_mut().open_project(crate::Opened { name: "Second".into(), path: "/tmp/Second.archforge".into(), ..Default::default() });
    h.run();
    assert_eq!(rig.store.kept(id).unwrap().turns[0].outcome, Outcome::Running, "its stop is not answered yet");
    rig.store.projects.lock().unwrap().clear();
    h.state_mut().close_tab(first);
    h.run();
    let kept = rig.store.kept(id).unwrap();
    assert_eq!(kept.turns[0].outcome, Outcome::Stopped);
    assert_eq!(replies(&kept), [("Payment is".to_string(), false)]);
    assert!(rig.store.projects.lock().unwrap().iter().all(|p| p.ends_with("Commerce platform.archforge")), "written to its own project");
}

/// Leaving the tab stops the running turn, written to its own project; closing ends the conversation.
#[test]
fn leaving_the_projects_tab_stops_its_running_turn_and_closing_it_ends_its_conversation() {
    use crate::chat::ChatEvent;
    let (mut h, rig) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h, "A long question");
    let key = crate::chat_fixture::key(h.state());
    let first = h.state().active.unwrap();
    h.state_mut().open_project(crate::Opened { name: "Second".into(), path: "/tmp/Second.archforge".into(), ..Default::default() });
    h.run();
    assert_eq!(chat_calls(&rig).2, 1, "leaving the tab stopped the turn");
    // Its end arrives while it is parked, and is written to its own project.
    assert!(rig.agents.emit(&key, ChatEvent::TurnEnded(crate::chat::TurnEnd::Stopped)));
    h.run();
    let parked = h.state().tabs[first].parked.as_ref().unwrap();
    let id = parked.chat_panel.chat.as_ref().unwrap().id;
    assert_eq!(rig.store.kept(id).unwrap().turns[0].outcome, crate::chat_store::Outcome::Stopped);
    assert!(rig.store.projects.lock().unwrap().iter().all(|p| p.ends_with("Commerce platform.archforge")));
    // A parked turn whose stop is never answered ends after the grace, against its own project.
    let (mut h2, rig2) = chat_harness(ThemeKind::Light, true);
    ask_in_chat(&mut h2, "Another long question");
    let first2 = h2.state().active.unwrap();
    h2.state_mut().open_project(crate::Opened { name: "Third".into(), path: "/tmp/Third.archforge".into(), ..Default::default() });
    h2.run();
    rig2.now.fetch_add(crate::chat_panel::STOP_GRACE, std::sync::atomic::Ordering::Relaxed);
    h2.run();
    let parked = h2.state().tabs[first2].parked.as_ref().unwrap();
    let id2 = parked.chat_panel.chat.as_ref().unwrap().id;
    assert_eq!(rig2.store.kept(id2).unwrap().turns[0].outcome, crate::chat_store::Outcome::Stopped);
    assert_eq!(chat_calls(&rig2).3, 1, "its program was ended");
    // Closing it ends its program.
    h.state_mut().close_tab(first);
    h.run();
    assert_eq!(chat_calls(&rig).3, 1, "closing the project ended its conversation");
}

/// Chat's workspace with a schema selected and File ▸ Generate… open for it, and the queue of
/// control requests attached as the app attaches it on every start. The sender keeps the queue open.
fn generate_with_the_queue(
    theme: ThemeKind,
) -> (Harness<'static, Shell>, std::sync::Arc<crate::ai_agents::fake::Fake>, std::sync::mpsc::Sender<crate::control::ControlRequest>) {
    let (mut h, f) = chat_workspace(theme);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let schema = s.execute("sketch.package.add", &serde_json::json!({ "name": "Shop" })).unwrap().value["id"].as_str().unwrap().to_string();
        s.execute("view.select", &serde_json::json!({ "ids": [schema] })).unwrap();
    }
    let (queue, requests) = std::sync::mpsc::channel();
    h.state_mut().attach_control(requests, Default::default());
    h.state_mut().open_generate();
    h.run();
    (h, f, queue)
}

/// AR-244: the queue is attached on every start, so it must not read as the person's agent access.
#[test]
fn generate_says_agent_access_is_off_with_the_queue_attached_and_no_control_port() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f, _queue) = generate_with_the_queue(theme);
        assert!(h.state().control.is_some(), "{theme:?}: the queue is attached");
        assert!(painted_has(&h, "Agent access is off: start Archforge with --control <port>"), "{theme:?}: {:?}", painted(&h));
        assert!(!painted_has(&h, "Agent access is on"), "{theme:?}: {:?}", painted(&h));
        // Chat's access is its own: a conversation holding it leaves the message alone.
        assert!(f.access(true), "{theme:?}");
        h.run();
        assert!(h.state().chats.holds_access(), "{theme:?}");
        assert!(painted_has(&h, "Agent access is off: start Archforge with --control <port>"), "{theme:?}: {:?}", painted(&h));
        assert!(!painted_has(&h, "Agent access is on"), "{theme:?}: {:?}", painted(&h));
    }
}

/// AR-244: only the person's own access says on, whether or not the queue or Chat's access is there.
#[test]
fn generate_says_agent_access_is_on_when_the_person_turned_it_on_for_this_start() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f, _queue) = generate_with_the_queue(theme);
        h.state_mut().set_own_agent_access(true);
        h.run();
        assert!(painted_has(&h, "Agent access is on: a connected agent reads this context through MCP."), "{theme:?}: {:?}", painted(&h));
        assert!(!painted_has(&h, "Agent access is off"), "{theme:?}: {:?}", painted(&h));
        // A conversation holding Chat's access and giving it back changes nothing.
        assert!(f.access(true), "{theme:?}");
        h.run();
        assert!(painted_has(&h, "Agent access is on: a connected agent"), "{theme:?}: {:?}", painted(&h));
        assert!(f.access(false), "{theme:?}");
        h.run();
        assert!(!h.state().chats.holds_access(), "{theme:?}");
        assert!(painted_has(&h, "Agent access is on: a connected agent"), "{theme:?}: {:?}", painted(&h));
        // The flag alone decides: with no queue at all it still says on.
        h.state_mut().control = None;
        h.run();
        assert!(painted_has(&h, "Agent access is on: a connected agent"), "{theme:?}: {:?}", painted(&h));
        assert!(!painted_has(&h, "Agent access is off"), "{theme:?}: {:?}", painted(&h));
    }
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

/// Opens the fake notation's sample model on a sketch diagram in the context's project, as the app's
/// notations open theirs: the package Commerce with Customer, Order, PricingService, Product,
/// OrderLine, Payment, Catalog and CardPayment laid out in three rows, their attributes and
/// operations, the associations places (Customer to Order), holds (Order to OrderLine), and the
/// unnamed ones between OrderLine and Product, Catalog and Product and Order and Payment, and
/// CardPayment a kind of Payment, every line drawn and the named ones labelled; saved, with nothing
/// to undo. Gives the diagram's id.
fn open_sample(cx: &mut crate::cx::Ctx) -> Result<archforge_mof::Uuid, String> {
    let session = cx.session.as_mut().ok_or("the sample needs a session")?;
    let mut run = |cmd: &str, p: serde_json::Value| -> Result<String, String> {
        let out = session.execute(cmd, &p).map_err(|e| format!("{cmd}: {e}"))?;
        Ok(out.value.get("id").and_then(|i| i.as_str()).unwrap_or_default().to_string())
    };
    let pkg = run("sketch.package.add", serde_json::json!({ "name": "Commerce" }))?;
    let classes: [(&str, &[&str], &[&str], i32, i32); 8] = [
        ("Customer", &["customerId", "email"], &[], 80, 80),
        ("Order", &["orderId", "placedOn"], &["total"], 560, 80),
        ("PricingService", &[], &["priceOf"], 1040, 80),
        ("Product", &["sku", "price"], &[], 80, 400),
        ("OrderLine", &["quantity"], &[], 560, 400),
        ("Payment", &["amount"], &[], 1040, 400),
        ("Catalog", &["title"], &[], 80, 700),
        ("CardPayment", &["cardNumber"], &[], 1040, 700),
    ];
    let mut ids = std::collections::BTreeMap::new();
    for (name, attributes, operations, ..) in &classes {
        let c = run("sketch.class.add", serde_json::json!({ "package": pkg, "name": name }))?;
        for a in *attributes {
            run("sketch.attribute.add", serde_json::json!({ "class": c, "name": a }))?;
        }
        for o in *operations {
            run("sketch.operation.add", serde_json::json!({ "class": c, "name": o }))?;
        }
        ids.insert(*name, c);
    }
    let mut named = Vec::new();
    for (from, to, name) in [
        ("Customer", "Order", "places"),
        ("Order", "OrderLine", "holds"),
        ("OrderLine", "Product", ""),
        ("Catalog", "Product", ""),
        ("Order", "Payment", ""),
    ] {
        let a = run("sketch.association.add", serde_json::json!({ "from": ids[from], "to": ids[to], "name": name }))?;
        if !name.is_empty() {
            named.push(a);
        }
    }
    run("sketch.inherit.add", serde_json::json!({ "specific": ids["CardPayment"], "general": ids["Payment"] }))?;
    let d = run("diagram.add", serde_json::json!({ "kind": SKETCH, "element": pkg, "name": "Commerce" }))?;
    for (name, attributes, operations, x, y) in &classes {
        let height = 72 + 24 * (attributes.len() + operations.len()).max(1);
        run("diagram.place", serde_json::json!({ "diagram": d, "element": ids[name], "x": x, "y": y, "width": 290, "height": height }))?;
    }
    run("diagram.connect-all", serde_json::json!({ "diagram": d }))?;
    let mut edges = Vec::new();
    for line in named {
        let store = session.doc();
        let edge = store
            .by_uuid(archforge_mof::Uuid::parse(&line).ok_or("a line's id")?)
            .and_then(|l| archforge_diagram::views_of(store, l).1.first().copied());
        edges.push(edge.and_then(|e| store.uuid(e).ok()).ok_or("a named line's edge")?);
    }
    let mut run = |cmd: &str, p: serde_json::Value| session.execute(cmd, &p).map(|_| ()).map_err(|e| format!("{cmd}: {e}"));
    for edge in edges {
        run("diagram.label", serde_json::json!({ "edge": edge.to_string(), "kind": "name", "at": "middle" }))?;
    }
    run("view.select", serde_json::json!({ "ids": [] }))?;
    // Read back from its rows, as an opening reads a project: saved, with nothing to undo.
    let (tables, junctions) = NOTATIONS.storage();
    let rows = archforge_mof::rows::snapshot(session.doc(), &tables, &junctions).map_err(|e| e.to_string())?;
    let n = std::sync::atomic::AtomicU64::new(1 << 32);
    let ids = Box::new(move || {
        let i = n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&i.to_be_bytes());
        archforge_mof::Uuid::v7(1_761_670_800_000, r)
    });
    *cx.session = Some(NOTATIONS.session_from_rows(&rows, ids)?);
    let d = archforge_mof::Uuid::parse(&d).ok_or("the diagram's id")?;
    cx.requests.push(archforge_shell_api::cx::Request::ExpandRows(archforge_mof::Uuid::parse(&pkg).into_iter().collect()));
    archforge_shell_api::canvas::open_cx(cx, d);
    Ok(d)
}

/// [`related_sample`] in `theme` in a window of `size`.
fn related_sample_in(theme: ThemeKind, size: egui::Vec2) -> (Harness<'static, Shell>, std::collections::BTreeMap<String, String>) {
    let mut h = workspace_harness(theme, size);
    h.state_mut().cx(open_sample).unwrap();
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

/// The boxes of the icons the last frame painted in `color`: each a mesh tinted that colour.
fn icon_boxes(h: &Harness<'static, Shell>, color: crate::ux::Colour) -> Vec<egui::Rect> {
    /// Collects the box of every mesh under `s` tinted `color` throughout.
    fn walk(s: &egui::Shape, color: crate::ux::Colour, out: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Mesh(m) if !m.vertices.is_empty() && m.vertices.iter().all(|v| v.color == egui::Color32::from(color)) => {
                out.push(egui::Rect::from_points(&m.vertices.iter().map(|v| v.pos).collect::<Vec<_>>()));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, color, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    h.output().shapes.iter().for_each(|c| walk(&c.shape, color, &mut out));
    out
}

/// The id of the live element of `class` named `name`.
fn named(h: &Harness<'static, Shell>, class: archforge_mof::ClassId, name: &str) -> archforge_mof::Uuid {
    let st = h.state().session.as_ref().unwrap().doc();
    let o = st.alive().find(|o| st.class_of(*o).ok() == Some(class) && archforge_core::name_of(st, *o) == name).expect(name);
    st.uuid(o).unwrap()
}

/// The node that shows `element` on the open diagram.
fn node_of(h: &Harness<'static, Shell>, element: archforge_mof::Uuid) -> String {
    let st = h.state().session.as_ref().unwrap().doc();
    let n = archforge_diagram::views_of(st, st.by_uuid(element).unwrap()).2[0];
    st.uuid(n).unwrap().to_string()
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

/// An `app.save` as the control channel brings it.
fn channel_save(h: &mut Harness<'static, Shell>, origin: Option<&str>, params: serde_json::Value) -> Result<serde_json::Value, String> {
    let (reply, _answer) = std::sync::mpsc::channel();
    let request = crate::control::ControlRequest {
        method: "app.save".into(),
        params,
        origin: origin.map(str::to_string),
        conversation: None,
        reply,
        state: Default::default(),
        read_only: false,
        live: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        generation: h.state().session_generation(),
    };
    crate::control::answer_request(h.state_mut(), &request)
}

/// An agent's save carries its description to the host; only text is taken, and text that a save
/// under way could not use is refused rather than lost. The revision is not checked, as nobody reviewed.
#[test]
fn an_agents_save_carries_its_description_and_never_loses_it() {
    use crate::SavedBy::Agent;
    use serde_json::json;
    let agent = || Agent(Some("Chat 3".to_string()));
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        add_schema(&mut h, "shop");

        let refused = channel_save(&mut h, Some("Chat 3"), json!({ "description": 3 })).unwrap_err();
        assert_eq!(refused, "A description has to be text, so nothing was saved.");
        assert!(host.saves.lock().unwrap().is_empty(), "the host is not asked");

        let begun = channel_save(&mut h, Some("Chat 3"), json!({ "description": "Added the shop" })).unwrap();
        assert_eq!(*host.descriptions.lock().unwrap(), [Some("Added the shop".to_string())]);
        assert_eq!(*host.saved_by.lock().unwrap(), [agent()]);

        let busy = channel_save(&mut h, Some("Chat 3"), json!({ "description": "A second one" })).unwrap_err();
        assert_eq!(busy, "A save is under way, so this description was not used. Try again when it has finished.");
        assert_eq!(channel_save(&mut h, Some("Chat 3"), json!({})).unwrap()["ticket"], begun["ticket"], "without text it follows the save");
        assert_eq!(host.descriptions.lock().unwrap().len(), 1, "the host was asked once");
    }
}

/// The close question's Save goes through the same save as the key: no description, the person's name.
#[test]
fn the_close_questions_save_is_the_persons_save() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        h.state_mut().state.user_name = "Ada".into();
        add_schema(&mut h, "shop");
        h.state_mut().close_tab(0);
        h.run_steps(2);
        let at = rect_of(&h, "close-ask.save").center();
        click(&mut h, at);
        h.run_steps(2);
        assert_eq!(*host.descriptions.lock().unwrap(), [None]);
        assert_eq!(*host.saved_by.lock().unwrap(), [crate::SavedBy::Person("Ada".into())]);
    }
}

/// A save at the window records the name from Settings; one over the control channel records an
/// agent, by its label or without one, which the path decides, whatever the name says.
#[test]
fn a_save_records_whether_the_person_or_an_agent_asked() {
    use crate::SavedBy::{Agent, Person};
    use serde_json::json;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        h.state_mut().state.user_name = "Agent".into();
        add_schema(&mut h, "shop");
        let saved = |h: &mut Harness<'static, Shell>, host: &SavingHost| {
            host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
            h.run_steps(2);
            host.saved_by.lock().unwrap().pop().unwrap()
        };
        h.state_mut().save();
        assert_eq!(saved(&mut h, &host), Person("Agent".into()));

        add_schema(&mut h, "own");
        crate::control::answer(h.state_mut(), "app.save", &json!({})).unwrap();
        assert_eq!(saved(&mut h, &host), Person("Agent".into()), "the window's own side is the person");

        for (at, origin) in [("crm", Some("Chat 3")), ("erp", None)] {
            add_schema(&mut h, at);
            channel_save(&mut h, origin, json!({})).unwrap();
            assert_eq!(saved(&mut h, &host), Agent(origin.map(str::to_string)));
        }
    }
}

/// A chat write refused for a changed branch says so in the panel's words.
#[test]
fn a_chat_write_refused_for_a_changed_branch_says_so() {
    crate::i18n::set_language("en");
    let words = crate::chat_panel::refusal_text(&crate::chat_store::Refusal::BranchChanged);
    assert_eq!(words, "This conversation could not be saved: the branch changed while it was on its way");
}

/// A failed save says whether a version exists, in each of the ways it can stand, and so does
/// one whose answer was lost; none of them marks the work saved.
#[test]
fn a_failed_save_says_whether_a_version_exists() {
    use crate::VersionAfter::{Made, None as NoVersion, Unknown};
    let commit = |version| Some(crate::SaveError::Commit { why: "the disk went away".into(), version });
    let told = [
        (Some(crate::SaveError::BranchChanged), "The branch changed before this save could run, so nothing was saved."),
        (commit(NoVersion), "No version was made: the disk went away"),
        (commit(Made("abc".into())), "A version was made, even though saving reported an error: the disk went away"),
        (commit(Unknown), "Archforge can't tell whether a version was made: the disk went away. It is safe to save again."),
        (None, "The save stopped without an answer, so Archforge can't tell whether a version was made. It is safe to save again."),
    ];
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for (error, words) in told.clone() {
            let (mut h, host) = saving_harness(Some(commerce()), None);
            h.state_mut().state.theme = theme;
            add_schema(&mut h, "shop");
            h.state_mut().save();
            let answer = host.saves.lock().unwrap().pop().unwrap();
            // With no answer at all the sender goes away, as when the writer stops.
            if let Some(e) = error {
                answer.send(Err(e)).unwrap();
            } else {
                drop(answer);
            }
            h.run_steps(2);
            assert_eq!(h.state().save_error.as_deref(), Some(words));
            assert!(h.state().session.as_ref().unwrap().is_dirty(), "the shell marks nothing saved");
        }
    }
}

/// Save version saves the state reviewed: the host gets its description, a model that moved
/// never reaches the host, and the host's own "changed" reads the same; plain Save has none.
#[test]
fn a_reviewed_save_reaches_the_host_only_while_the_model_stands_where_it_was_reviewed() {
    let changed = "The project changed after you reviewed it, so nothing was saved. Review the changes and save again.";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        add_schema(&mut h, "shop");
        let reviewed = h.state().session.as_ref().unwrap().revision();
        h.state_mut().save_reviewed("Added the shop".into(), reviewed);
        assert_eq!(*host.descriptions.lock().unwrap(), [Some("Added the shop".to_string())]);
        host.saves.lock().unwrap().pop().unwrap().send(Ok(true)).unwrap();
        h.run_steps(2);

        add_schema(&mut h, "crm");
        h.state_mut().save_reviewed("Reviewed before crm".into(), reviewed);
        h.run_steps(2);
        assert_eq!(host.descriptions.lock().unwrap().len(), 1, "the host is not asked");
        assert_eq!(h.state().save_error.as_deref(), Some(changed));

        let moved_on = h.state().session.as_ref().unwrap().revision();
        h.state_mut().save_reviewed("Host saw more".into(), moved_on);
        host.saves.lock().unwrap().pop().unwrap().send(Err(crate::SaveError::Changed)).unwrap();
        h.run_steps(2);
        assert_eq!(h.state().save_error.as_deref(), Some(changed));
        assert!(h.state().session.as_ref().unwrap().is_dirty(), "a save answered changed marks nothing saved");

        h.state_mut().save();
        assert_eq!(host.descriptions.lock().unwrap().last(), Some(&None), "Save stays one key, described by its time");
    }
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
    let titles = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let asked = titles.clone();
    let services = Services {
        projects: Some(host),
        ids: Some(test_ids()),
        pick_project: Some(std::sync::Arc::new(|_| Picked::at_once(None))),
        pick_folder: Some(std::sync::Arc::new(move |title: &str| {
            asked.lock().unwrap().push(title.to_string());
            Picked::at_once(None)
        })),
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
        Shell::new(services, NOTATIONS, &SCREENS),
    );
    h.run();
    // A cancel, taken on the next frame: nothing to say.
    h.state_mut().open_from_picker();
    h.run();
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
    h.run();
    assert_eq!(h.state().picker_error, None, "a cancel with a picker there clears it");
    unavailable.store(true, std::sync::atomic::Ordering::SeqCst);
    h.state_mut().pin_from_picker();
    h.run();
    assert_eq!(h.state().picker_error.as_deref(), Some("this system has no desktop portal"));
    // Home's picker says it pins a folder.
    assert!(titles.lock().unwrap().iter().all(|t| t == "Pin a folder") && !titles.lock().unwrap().is_empty(), "{:?}", titles.lock().unwrap());
}

/// Home's controls that start an opening or a pick, off while a folder pick waits (AR-749).
const HOME_OPENINGS: [&str; 4] = ["home.recent.0", "home.start.open-project", "strip.open-project", "home.pin-folder"];

/// Whether `id` is live, as the pointer over it shows: the pointing hand.
fn points(h: &mut Harness<'static, Shell>, id: &str) -> bool {
    h.event(egui::Event::PointerMoved(rect_of(h, id).center()));
    h.run();
    h.output().platform_output.cursor_icon == egui::CursorIcon::PointingHand
}

/// The fill and stroke colours of the rectangles the last frame painted exactly over `id`.
fn painted_over(h: &Harness<'static, Shell>, id: &str) -> Vec<(egui::Color32, egui::Color32)> {
    /// Collects every rectangle over `r` under `s`.
    fn walk(s: &egui::Shape, r: egui::Rect, out: &mut Vec<(egui::Color32, egui::Color32)>) {
        match s {
            egui::Shape::Rect(x) if x.rect == r => out.push((x.fill, x.stroke.color)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, r, out)),
            _ => {}
        }
    }
    let (r, mut out) = (rect_of(h, id), Vec::new());
    h.output().shapes.iter().for_each(|c| walk(&c.shape, r, &mut out));
    out
}

/// A folder pick is answered on a later frame (AR-737): until the answer is in, New project, Open
/// and Pin a folder are off, a recent project opens nothing and Home's ways to open one look off, a second pick is refused without
/// asking the host again, and the
/// window looks again for the answer. A picker that says it could not appear is said on Home until
/// a pick chooses a folder, one that answers nothing is asked why, and a host gone without
/// answering counts as a cancel, without that question.
#[test]
fn a_folder_pick_is_answered_on_a_later_frame_and_one_at_a_time() {
    type Waiting = std::sync::Arc<std::sync::Mutex<Vec<std::sync::mpsc::Sender<Picked>>>>;
    let pinned = || vec![std::path::PathBuf::from("/Users/robin/Work/Models")];
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        crate::i18n::set_language("en");
        let waiting = Waiting::default();
        let asked = waiting.clone();
        let mut shell = Shell::new(
            Services {
                projects: Some(std::sync::Arc::new(SavingHost::default())),
                pick_project: Some(std::sync::Arc::new(|_| Picked::at_once(Some("/tmp/Shop.archforge".into())))),
                pick_folder: Some(std::sync::Arc::new(move |_: &str| {
                    let (tx, rx) = std::sync::mpsc::channel();
                    asked.lock().unwrap().push(tx);
                    rx
                })),
                picker_unavailable: Some(std::sync::Arc::new(|| Some("this system has no desktop portal".into()))),
                ..Services::default()
            },
            NOTATIONS,
            &SCREENS,
        );
        shell.state.theme = theme;
        shell.state.remember("Library", std::path::Path::new("/tmp/Library.archforge"), 0);
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
            |ui, shell: &mut Shell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            shell,
        );
        h.run();
        let answer = |h: &mut Harness<'static, Shell>, picked: Option<Picked>| {
            let tx = waiting.lock().unwrap().pop().expect("a pick was asked for");
            if let Some(picked) = picked {
                tx.send(picked).unwrap();
            }
            drop(tx);
            h.run();
        };
        let at = rect_of(&h, "home.pin-folder").center();
        click(&mut h, at);
        assert!(!crate::actions::available(h.state(), "home.pin-folder") && !h.state().can_open_project(), "{theme:?}: both pickers off");
        // New project's handler waits too, however it is reached: its dialog would stand over the pick.
        crate::new_project::open(h.state_mut());
        assert!(h.state().new_project.is_none(), "{theme:?}: no dialog over the pick");
        // New project too, from its menu item and from Home's card: an Open answer would find the dialog.
        let new = rect_of(&h, "home.start.new-project").center();
        click(&mut h, new);
        assert!(!crate::actions::available(h.state(), "file.new") && h.state().new_project.is_none(), "{theme:?}: New project off");
        // No other opening starts meanwhile, so none is running when an Open answer comes.
        let row = rect_of(&h, "home.recent.0").center();
        click(&mut h, row);
        assert!(!h.state().is_opening(), "{theme:?}: a recent project opens nothing while the pick waits");
        // Home's ways to open a project look off meanwhile, a recent card in the gallery too (AR-749).
        assert!(HOME_OPENINGS.iter().all(|id| !points(&mut h, id)), "{theme:?}: Home's openings off while the pick waits");
        let p = crate::ux::palette(theme.is_dark());
        assert!(
            !points(&mut h, "home.recent.0") && !painted_over(&h, "home.recent.0").iter().any(|(fill, _)| *fill == egui::Color32::from(p.surface)),
            "{theme:?}: no hover look"
        );
        h.state_mut().state.recent_view = crate::state::RecentView::Gallery;
        h.run();
        assert!(!points(&mut h, "home.recent.0"), "{theme:?}: and a recent card");
        assert!(
            !painted_over(&h, "home.recent.0").iter().any(|(_, stroke)| *stroke == egui::Color32::from(p.border_hover)),
            "{theme:?}: no hover border"
        );
        h.state_mut().state.recent_view = crate::state::RecentView::List;
        h.run();
        h.state_mut().pin_from_picker();
        h.state_mut().open_from_picker();
        h.run();
        assert_eq!(waiting.lock().unwrap().len(), 1, "{theme:?}: a second pick is refused");
        assert!(!h.state().is_opening(), "{theme:?}: and Open asks no picker");
        h.step();
        assert!(h.ctx.has_requested_repaint(), "{theme:?}: the window looks again for the answer");
        answer(&mut h, Some(Picked::Unavailable("no folder picker here".into())));
        assert_eq!(h.state().picker_error.as_deref(), Some("no folder picker here"));
        assert!(painted(&h).iter().any(|t| t == "no folder picker here"), "{theme:?}: {:?}", painted(&h));
        assert!(crate::actions::available(h.state(), "home.pin-folder") && h.state().can_open_project(), "{theme:?}: both pickers back");
        assert!(HOME_OPENINGS.iter().all(|id| points(&mut h, id)), "{theme:?}: and Home's openings");
        assert!(
            points(&mut h, "home.recent.0") && painted_over(&h, "home.recent.0").iter().any(|(fill, _)| *fill == egui::Color32::from(p.surface)),
            "{theme:?}: hover look back"
        );
        h.state_mut().pin_from_picker();
        answer(&mut h, Some(Picked::Folder(pinned()[0].clone())));
        assert_eq!((h.state().state.pinned.clone(), h.state().picker_error.clone()), (pinned(), None), "{theme:?}: the folder chosen is pinned");
        h.state_mut().pin_from_picker();
        answer(&mut h, Some(Picked::Nothing));
        let why = Some("this system has no desktop portal".to_string());
        assert_eq!((h.state().state.pinned.clone(), h.state().picker_error.clone()), (pinned(), why), "{theme:?}: nothing chosen asks why");
        h.state_mut().pin_from_picker();
        answer(&mut h, None);
        assert_eq!((h.state().state.pinned.clone(), h.state().picker_error.clone()), (pinned(), None), "{theme:?}: a host gone is a cancel");
        assert!(crate::actions::available(h.state(), "home.pin-folder"), "{theme:?}: and Pin a folder is back");
        assert!(crate::actions::available(h.state(), "file.new"), "{theme:?}: and New project");
    }
}

/// The folder picks a [`pick_waits_harness`] was asked for, each answered when the test says.
type PickWaits = std::sync::Arc<std::sync::Mutex<Vec<std::sync::mpsc::Sender<Picked>>>>;

/// Home in `theme` with Library among the recent projects and Models pinned: a folder pick waits
/// until the test answers it, and the project picker counts its calls and answers a cancel.
fn pick_waits_harness(
    theme: ThemeKind,
) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>, PickWaits, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(SavingHost::default());
    let asked = PickWaits::default();
    let asked_for = asked.clone();
    let opens = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = opens.clone();
    let mut shell = Shell::new(
        Services {
            projects: Some(host.clone()),
            pick_project: Some(std::sync::Arc::new(move |_| {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Picked::at_once(None)
            })),
            pick_folder: Some(std::sync::Arc::new(move |_: &str| {
                let (tx, rx) = std::sync::mpsc::channel();
                asked_for.lock().unwrap().push(tx);
                rx
            })),
            ..Services::default()
        },
        NOTATIONS,
        &SCREENS,
    );
    shell.state.theme = theme;
    shell.state.remember("Library", std::path::Path::new("/tmp/Library.archforge"), 0);
    shell.state.pin(std::path::Path::new("/Users/robin/Work/Models"));
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1120.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    (h, host, asked, opens)
}

/// The More menus of a recent project and of a pinned folder keep the row the arrow keys reached
/// when a folder pick ending brings their opening back above it: Enter removes or unpins, as the
/// row reached says, and opens nothing. An opening reached that a pick starting takes away runs
/// nothing in its place, an arrow with Enter in the frame the opening comes back runs nothing
/// unseen, a click stays on the row pressed when the opening takes its place before the release or
/// on the press's frame, and a screen reader's click runs the row it names and nothing once that
/// row is gone (AR-749).
#[test]
fn a_more_menus_reached_row_stays_when_a_pick_ending_brings_its_opening_back() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for (menu, id) in [(crate::home::RECENT.at(0), "home.recent.0.menu"), (crate::home::PINNED.at(0), "home.pinned.0.menu")] {
            let (mut h, host, waits, opens) = pick_waits_harness(theme);
            h.state_mut().pin_from_picker();
            h.run();
            h.state_mut().panels.menu = Some(menu);
            h.run();
            let rows = |h: &Harness<'static, Shell>| ids(h).into_iter().filter(|i| i.starts_with(&format!("{id}."))).count();
            assert_eq!(rows(&h), 1, "{theme:?}: {id} offers no opening while the pick waits");
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
            // The pick ends with a cancel, taken on the frame that reads Enter.
            drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
            press_now(&mut h, egui::Key::Enter);
            let s = h.state();
            let opened = (s.is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(opened, (false, 0, 0), "{theme:?}: {id}: Enter opened nothing");
            let left = (s.state.recent.len(), s.state.pinned.len());
            assert_eq!(left, if menu == crate::home::RECENT.at(0) { (0, 1) } else { (1, 0) }, "{theme:?}: {id}: the row reached ran");
            // The row reached, then the pick ends and Up with Enter come in that frame: Up reaches
            // the opening, not yet seen, so Enter waits for it to be drawn.
            let (mut h, host, waits, opens) = pick_waits_harness(theme);
            h.state_mut().pin_from_picker();
            h.run();
            h.state_mut().panels.menu = Some(menu);
            h.run();
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
            drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
            let press = |key, pressed| egui::Event::Key { key, pressed, repeat: false, modifiers: egui::Modifiers::NONE, physical_key: None };
            h.input_mut().events.extend([press(egui::Key::ArrowUp, true), press(egui::Key::ArrowUp, false), press(egui::Key::Enter, true)]);
            h.step();
            h.run();
            let s = h.state();
            let opened = (s.is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!((opened, s.state.recent.len(), s.state.pinned.len()), ((false, 0, 0), 1, 1), "{theme:?}: {id}: Enter ran no row unseen");
            assert_eq!(rows(&h), 2, "{theme:?}: {id}: the menu stays, its opening drawn");
            // Nothing reached, then the pick ends and Down with Enter come in that frame: Enter
            // picks nothing, as no row was reached before the arrow.
            let (mut h, host, waits, opens) = pick_waits_harness(theme);
            h.state_mut().pin_from_picker();
            h.run();
            h.state_mut().panels.menu = Some(menu);
            h.run();
            drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
            h.input_mut().events.extend([press(egui::Key::ArrowDown, true), press(egui::Key::ArrowDown, false), press(egui::Key::Enter, true)]);
            h.step();
            h.run();
            let s = h.state();
            let opened = (s.is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(
                (opened, s.state.recent.len(), s.state.pinned.len()),
                ((false, 0, 0), 1, 1),
                "{theme:?}: {id}: Enter with nothing reached ran nothing"
            );
            // The opening reached, then a pick starts: Enter runs nothing in its place.
            let (mut h, host, _waits, opens) = pick_waits_harness(theme);
            h.state_mut().panels.menu = Some(menu);
            h.run();
            assert_eq!(rows(&h), 2, "{theme:?}: {id} offers its opening");
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
            h.state_mut().pin_from_picker();
            press_now(&mut h, egui::Key::Enter);
            let s = h.state();
            let opened = (s.is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!((opened, s.state.recent.len(), s.state.pinned.len()), ((false, 0, 0), 1, 1), "{theme:?}: {id}: nothing ran");
            // A press on the row, then the pick ends and the opening takes the row's place before
            // the release, or on the press's own frame: the click stays the row's, never handed to
            // the opening.
            for same_frame in [false, true] {
                let (mut h, host, waits, opens) = pick_waits_harness(theme);
                h.state_mut().pin_from_picker();
                h.run();
                h.state_mut().panels.menu = Some(menu);
                h.run();
                let at = rect_of(&h, &format!("{id}.0")).center();
                h.event(egui::Event::PointerMoved(at));
                h.run();
                let button =
                    |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
                if same_frame {
                    drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
                }
                h.input_mut().events.push(button(true));
                h.step();
                if !same_frame {
                    drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
                }
                h.input_mut().events.push(button(false));
                h.step();
                h.run();
                let s = h.state();
                let opened = (s.is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
                assert_eq!(opened, (false, 0, 0), "{theme:?}: {id}, same frame {same_frame}: the release opened nothing");
                let left = (s.state.recent.len(), s.state.pinned.len());
                assert_eq!(
                    left,
                    if menu == crate::home::RECENT.at(0) { (0, 1) } else { (1, 0) },
                    "{theme:?}: {id}, same frame {same_frame}: the row pressed ran"
                );
            }
            // A screen reader's click has no press to match, and runs its row.
            let (mut h, ..) = pick_waits_harness(theme);
            h.state_mut().panels.menu = Some(menu);
            h.run();
            let (opening, other) =
                if menu == crate::home::RECENT.at(0) { ("Open", "Remove from recent projects") } else { ("Open a project here", "Unpin") };
            reader_click(&mut h, archforge_shell_api::panels::menu_row_id(id, (other, ""), 0));
            let left = (h.state().state.recent.len(), h.state().state.pinned.len());
            assert_eq!(left, if menu == crate::home::RECENT.at(0) { (0, 1) } else { (1, 0) }, "{theme:?}: {id}: a screen reader's click ran its row");
            // A screen reader's click on the opening as a pick starts and takes it away runs
            // nothing in its place.
            let (mut h, host, _waits, opens) = pick_waits_harness(theme);
            h.state_mut().panels.menu = Some(menu);
            h.run();
            h.state_mut().pin_from_picker();
            reader_click(&mut h, archforge_shell_api::panels::menu_row_id(id, (opening, ""), 0));
            let s = h.state();
            let opened = (s.is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(
                (opened, s.state.recent.len(), s.state.pinned.len()),
                ((false, 0, 0), 1, 1),
                "{theme:?}: {id}: the click on a row gone ran nothing"
            );
        }
    }
}

/// A menu's rows of one text (types of one name, told apart by their notes) are rows of their own
/// to the toolkit, even two of one note: a click on the first picks the first. The row the keys
/// reached stays the row of its note when rows change places (AR-749).
#[test]
fn a_menus_rows_of_one_text_are_told_apart() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        crate::i18n::set_language("en");
        let picks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let swapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (picked, swap) = (picks.clone(), swapped.clone());
        let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
        shell.state.theme = theme;
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_ui_state(
            move |ui, shell: &mut Shell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
                let p = crate::ux::palette(shell.state.theme.is_dark());
                let items = ["Order", "Order", "Order"].map(String::from);
                let mut notes = ["Sales", "Billing", "Sales"].map(String::from);
                if swap.load(std::sync::atomic::Ordering::SeqCst) {
                    notes.swap(0, 1);
                }
                let anchor = egui::Rect::from_min_size(egui::pos2(700.0, 600.0), egui::vec2(220.0, 28.0));
                let k = shell.cx(|cx| archforge_shell_api::panels::menu_noted_cx(cx, ui, p, anchor, "test.menu", &items, &notes, &[]));
                picked.lock().unwrap().extend(k.map(|k| (k, notes[k].clone())));
            },
            shell,
        );
        h.run();
        let at = rect_of(&h, "test.menu.0").center();
        click(&mut h, at);
        assert_eq!(picks.lock().unwrap().as_slice(), [(0, "Sales".to_string())], "{theme:?}: the first row, alone, not its twin");
        // Billing reached, then the two change places: Enter picks Billing where it went.
        h.state_mut().panels.menu = Some(crate::panels::MenuId::shell("rows"));
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        swapped.store(true, std::sync::atomic::Ordering::SeqCst);
        press_now(&mut h, egui::Key::Enter);
        assert_eq!(picks.lock().unwrap().last(), Some(&(0, "Billing".to_string())), "{theme:?}: the row of its note");
    }
}

/// Twin rows of one text and note (template parameters `T` of two templates) keep their own ids
/// however far the menu is scrolled: with the first out of sight, a screen reader's click meant
/// for it runs nothing, and the second answers only to its own; a twin added is numbered with the
/// rest; and a click on the frame a twin goes or a moment after, or a press held while it goes,
/// runs nothing (AR-749).
#[test]
fn a_menus_twin_rows_keep_their_ids_when_it_scrolls() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        crate::i18n::set_language("en");
        let picks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        // Where the twins are: two apart, a third added above, the first gone, two side by side,
        // and the first of those gone.
        const TWINS: [&[usize]; 5] = [&[0, 3], &[0, 1, 4], &[2], &[0, 1], &[0]];
        let mode = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (picked, now) = (picks.clone(), mode.clone());
        let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
        shell.state.theme = theme;
        let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).with_step_dt(0.1).build_ui_state(
            move |ui, shell: &mut Shell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
                let p = crate::ux::palette(shell.state.theme.is_dark());
                let mut items: Vec<String> = (0..44).map(|n| format!("Row {n}")).collect();
                let mut notes = vec![String::new(); 44];
                for &twin in TWINS[now.load(std::sync::atomic::Ordering::SeqCst)] {
                    (items[twin], notes[twin]) = ("T".to_string(), "template parameter".to_string());
                }
                let anchor = egui::Rect::from_min_size(egui::pos2(700.0, 600.0), egui::vec2(220.0, 28.0));
                picked
                    .lock()
                    .unwrap()
                    .extend(shell.cx(|cx| archforge_shell_api::panels::menu_noted_cx(cx, ui, p, anchor, "test.long", &items, &notes, &[])));
            },
            shell,
        );
        h.run();
        h.state_mut().panels.menu_scroll = Some(("test.long".to_string(), 70.0));
        h.run();
        let ids = ids(&h);
        assert!(!ids.iter().any(|i| i == "test.long.0") && ids.iter().any(|i| i == "test.long.3"), "{theme:?}: the first twin out of sight: {ids:?}");
        let twin = |nth| archforge_shell_api::panels::menu_row_id("test.long", ("T", "template parameter"), nth);
        reader_click(&mut h, twin(0));
        assert!(picks.lock().unwrap().is_empty(), "{theme:?}: the click meant for the first twin ran nothing");
        reader_click(&mut h, twin(1));
        assert_eq!(picks.lock().unwrap().as_slice(), [3], "{theme:?}: the second twin answers to its own");
        // A third twin comes in above: the rows are numbered again, the last now the third.
        mode.store(1, std::sync::atomic::Ordering::SeqCst);
        h.state_mut().panels.menu_scroll = Some(("test.long".to_string(), 70.0));
        h.run_steps(8);
        reader_click(&mut h, twin(2));
        assert_eq!(picks.lock().unwrap().as_slice(), [3, 4], "{theme:?}: the third twin answers to its own");
        // Back to two twins, then the first goes on the frame a click meant for it comes: the
        // survivor takes its number, and the click runs nothing. On the next frame, rows
        // unchanged, the survivor answers to it.
        mode.store(0, std::sync::atomic::Ordering::SeqCst);
        h.state_mut().panels.menu_scroll = Some(("test.long".to_string(), 0.0));
        h.run();
        mode.store(2, std::sync::atomic::Ordering::SeqCst);
        reader_click(&mut h, twin(0));
        assert_eq!(picks.lock().unwrap().as_slice(), [3, 4], "{theme:?}: the click meant for the twin gone ran nothing");
        // And on the frame after, from a screen reader still reading the rows before.
        mode.store(0, std::sync::atomic::Ordering::SeqCst);
        h.run_steps(8);
        mode.store(2, std::sync::atomic::Ordering::SeqCst);
        h.step();
        reader_click(&mut h, twin(0));
        assert_eq!(picks.lock().unwrap().as_slice(), [3, 4], "{theme:?}: nor a moment later");
        // Once the moment has passed, rows unchanged, the survivor answers to it.
        h.run_steps(8);
        reader_click(&mut h, twin(0));
        assert_eq!(picks.lock().unwrap().as_slice(), [3, 4, 2], "{theme:?}: the survivor, on rows unchanged");
        // Twins side by side: a press on the first, which goes while the press is held, so the
        // second moves under the pointer: the release picks nothing.
        mode.store(3, std::sync::atomic::Ordering::SeqCst);
        h.run();
        let at = rect_of(&h, "test.long.0").center();
        let button =
            |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        h.event(egui::Event::PointerMoved(at));
        h.run();
        h.input_mut().events.push(button(true));
        h.step();
        mode.store(4, std::sync::atomic::Ordering::SeqCst);
        // Held past the moment a click is held for after a change, though still a click.
        h.run_steps(6);
        h.input_mut().events.push(button(false));
        h.step();
        h.run();
        assert_eq!(picks.lock().unwrap().as_slice(), [3, 4, 2], "{theme:?}: the release on the twin that moved in ran nothing");
        h.run_steps(8);
        click(&mut h, at);
        assert_eq!(picks.lock().unwrap().as_slice(), [3, 4, 2, 0], "{theme:?}: a click on what now shows runs it");
    }
}

/// The texts and notes a [`rows_menu`] shows, as the test sets them.
type MenuRowsSet = std::sync::Arc<std::sync::Mutex<(Vec<String>, Vec<String>)>>;

/// Home in `theme` with the menu `test.rows` drawn over it, its rows as `rows` holds them on each
/// frame, every row it picks pushed onto `picks`; its keys are the menu's, as for an open menu.
fn rows_menu(theme: ThemeKind, rows: &MenuRowsSet, picks: &std::sync::Arc<std::sync::Mutex<Vec<usize>>>) -> Harness<'static, Shell> {
    crate::i18n::set_language("en");
    let (shown, picked) = (rows.clone(), picks.clone());
    let mut shell = Shell::new(Services::default(), NOTATIONS, &SCREENS);
    shell.state.theme = theme;
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).with_step_dt(0.1).build_ui_state(
        move |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
            let p = crate::ux::palette(shell.state.theme.is_dark());
            let (items, notes) = shown.lock().unwrap().clone();
            let anchor = egui::Rect::from_min_size(egui::pos2(700.0, 600.0), egui::vec2(220.0, 28.0));
            picked
                .lock()
                .unwrap()
                .extend(shell.cx(|cx| archforge_shell_api::panels::menu_noted_cx(cx, ui, p, anchor, "test.rows", &items, &notes, &[])));
        },
        shell,
    );
    h.state_mut().panels.menu = Some(crate::panels::MenuId::shell("rows"));
    h.run();
    h
}

/// Rows of their texts and notes, for a [`rows_menu`].
fn menu_rows(rows: &[(&str, &str)]) -> (Vec<String>, Vec<String>) {
    rows.iter().map(|(text, note)| (text.to_string(), note.to_string())).unzip()
}

/// When the rows change and a twin of the row the keys reached comes in where it stood, or leaves
/// another in its place, the reach is dropped, so Enter runs no twin; on rows unchanged the keys
/// still reach a twin and Enter runs it (AR-749).
#[test]
fn a_menus_reach_on_a_twin_is_dropped_when_the_rows_change() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let t = ("T", "template parameter");
        let rows: MenuRowsSet = std::sync::Arc::new(std::sync::Mutex::new(menu_rows(&[("A", ""), t, ("B", "")])));
        let picks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = rows_menu(theme, &rows, &picks);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        *rows.lock().unwrap() = menu_rows(&[("A", ""), t, t, ("B", "")]);
        h.run_steps(8);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(picks.lock().unwrap().is_empty(), "{theme:?}: Enter ran neither twin");
        // The rows as they are now, unchanged: the keys reach the second twin and Enter runs it.
        for _ in 0..3 {
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        }
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(picks.lock().unwrap().as_slice(), [2], "{theme:?}: the twin reached ran");
        // The first of two twins reached, then it goes and the other moves up into its place:
        // dropped too, as the rows before had the twin.
        let mut h = rows_menu(theme, &rows, &picks);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        *rows.lock().unwrap() = menu_rows(&[("A", ""), t, ("B", "")]);
        h.run_steps(8);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(picks.lock().unwrap().as_slice(), [2], "{theme:?}: Enter ran not the twin left");
    }
}

/// Rows coming in above the row the keys reached move it out of sight: the menu scrolls to keep it
/// in sight, and Enter runs it; a row it still shows after the change leaves the scroll as it was
/// (AR-749).
#[test]
fn a_menus_reach_moved_by_new_rows_is_kept_in_sight() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let numbered = |from: usize, to: usize| (from..to).map(|n| format!("Row {n}")).collect::<Vec<_>>();
        let set = |texts: Vec<String>| (texts.clone(), vec![String::new(); texts.len()]);
        let rows: MenuRowsSet = std::sync::Arc::new(std::sync::Mutex::new(set(numbered(0, 44))));
        let picks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut h = rows_menu(theme, &rows, &picks);
        for _ in 0..3 {
            key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        }
        let scroll = |h: &Harness<'static, Shell>| h.state().panels.menu_scroll.as_ref().map(|(_, offset)| *offset);
        // One row above: the row reached still shows, and the menu does not scroll.
        let before = scroll(&h);
        *rows.lock().unwrap() = set([vec!["New 0".to_string()], numbered(0, 44)].concat());
        h.run();
        assert_eq!(scroll(&h), before, "{theme:?}: no scroll for a row still in sight");
        // Forty rows above: the row reached is kept in sight, and Enter runs it.
        *rows.lock().unwrap() = set([numbered(100, 140), vec!["New 0".to_string()], numbered(0, 44)].concat());
        h.run();
        assert!(ids(&h).iter().any(|i| i == "test.rows.43"), "{theme:?}: Row 2, now the 44th, in sight: {:?}", ids(&h));
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(picks.lock().unwrap().as_slice(), [43], "{theme:?}: the row reached ran");
    }
}

/// Clicks the control the toolkit knows as `id` as a screen reader does, with no pointer press, on
/// the next frame.
fn reader_click(h: &mut Harness<'static, Shell>, id: egui::Id) {
    let (action, target_tree) = (egui::accesskit::Action::Click, egui::accesskit::TreeId::ROOT);
    let target_node = id.accesskit_id();
    h.input_mut().events.push(egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest { action, target_tree, target_node, data: None }));
    h.step();
    h.run();
}

/// Presses `key`, alone, on the very next frame: [`key`] gives the modifiers a frame of their own
/// first.
fn press_now(h: &mut Harness<'static, Shell>, key: egui::Key) {
    let modifiers = egui::Modifiers::NONE;
    h.input_mut().events.push(egui::Event::Key { key, pressed: true, modifiers, repeat: false, physical_key: None });
    h.step();
    h.run();
}

/// Enter in Home's search runs the row last shown highlighted: a folder pick ending on that frame
/// lists Library above it, a row a pick has taken off the list meanwhile runs nothing in its
/// place, a query typed is listed on its own frame, the arrows move on from the row in sight (with
/// Enter in their frame too), and a click stays on the row pressed when Library takes its place
/// before the release (AR-749).
#[test]
fn enter_in_homes_search_runs_the_row_last_shown_while_it_is_still_listed() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host, waits, _) = pick_waits_harness(theme);
        // The list's first heading and its first row, as drawn.
        let first = |h: &Harness<'static, Shell>| {
            let texts = painted(h);
            let at = texts.iter().position(|t| ["PROJECTS", "PINNED FOLDERS", "ACTIONS"].contains(&t.as_str()))?;
            texts.get(at..at + 2).map(<[String]>::to_vec)
        };
        // While a pick waits, the list starts with the appearance switch.
        h.state_mut().pin_from_picker();
        h.run();
        crate::home_search::open_with(h.state_mut(), "");
        h.run();
        let switch = if theme.is_dark() { "Switch to the light appearance" } else { "Switch to the dark appearance" };
        assert_eq!(first(&h), Some(vec!["ACTIONS".to_string(), switch.to_string()]), "{theme:?}");
        drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
        press_now(&mut h, egui::Key::Enter);
        assert_ne!(h.state().state.theme, theme, "{theme:?}: the appearance switch shown ran");
        assert!(!h.state().is_opening() && host.opens.lock().unwrap().is_empty(), "{theme:?}: Library did not open");
        // Library shown first, then a pick starts: Enter runs nothing in its place.
        crate::home_search::open_with(h.state_mut(), "");
        h.run();
        assert_eq!(first(&h), Some(vec!["PROJECTS".to_string(), "Library".to_string()]), "{theme:?}");
        let shown = h.state().state.theme;
        h.state_mut().pin_from_picker();
        press_now(&mut h, egui::Key::Enter);
        assert_eq!(h.state().state.theme, shown, "{theme:?}: the row now first did not run");
        assert!(!h.state().is_opening() && h.state().home_search.open, "{theme:?}: nothing opened, and the list stays");
        assert_eq!(waits.lock().unwrap().len(), 1, "{theme:?}: the pick still waits");
        // What is typed is listed on its own frame, so Enter on the next runs its first row.
        drop(waits.lock().unwrap().pop());
        crate::home_search::close(h.state_mut());
        h.run();
        let at = rect_of(&h, "home.header.search").center();
        click(&mut h, at);
        h.event(egui::Event::Text("appearance".into()));
        h.step();
        press_now(&mut h, egui::Key::Enter);
        assert_ne!(h.state().state.theme, shown, "{theme:?}: the appearance switch typed for ran");
        // The arrow on the frame a pick ends moves on from the row in sight, not from its place.
        let (mut h, host, waits, opens) = pick_waits_harness(theme);
        h.state_mut().pin_from_picker();
        h.run();
        crate::home_search::open_with(h.state_mut(), "");
        h.run();
        drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
        press_now(&mut h, egui::Key::ArrowDown);
        press_now(&mut h, egui::Key::Enter);
        assert_eq!(h.state().state.recent_view, crate::state::RecentView::Gallery, "{theme:?}: the action after the switch ran");
        let opened = (h.state().is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(opened, (false, 0, 0), "{theme:?}: no project or pinned folder ran");
        // Down and Enter in one frame run the row after the one in sight.
        let (mut h, host, _waits, opens) = pick_waits_harness(theme);
        crate::home_search::open_with(h.state_mut(), "");
        h.run();
        let key = |key, pressed| egui::Event::Key { key, pressed, repeat: false, modifiers: egui::Modifiers::NONE, physical_key: None };
        h.input_mut().events.extend([key(egui::Key::ArrowDown, true), key(egui::Key::ArrowDown, false), key(egui::Key::Enter, true)]);
        h.step();
        h.run();
        let opened = (host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(opened, (0, 1), "{theme:?}: the pinned folder after Library ran, not Library");
        // Down and Enter together on the frame a pick ends, from the last row: Down wraps over the
        // rows drawn, not onto Library, which takes a frame to be seen before Enter runs a row.
        let (mut h, host, waits, opens) = pick_waits_harness(theme);
        h.state_mut().pin_from_picker();
        h.run();
        crate::home_search::open_with(h.state_mut(), "");
        h.run();
        press_now(&mut h, egui::Key::ArrowUp);
        drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
        let before = (h.state().state.theme, h.state().state.recent_view, h.state().state.recent_sort);
        h.input_mut().events.extend([key(egui::Key::ArrowDown, true), key(egui::Key::ArrowDown, false), key(egui::Key::Enter, true)]);
        h.step();
        h.run();
        let opened = (h.state().is_opening(), host.opens.lock().unwrap().len(), opens.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(opened, (false, 0, 0), "{theme:?}: Library, not yet seen, did not open");
        let after = (h.state().state.theme, h.state().state.recent_view, h.state().state.recent_sort);
        assert!(after == before && h.state().home_search.open, "{theme:?}: nothing ran, and the list stays");
        // A press on the switch, then the pick ends and Library takes its row before the release:
        // the click stays the switch's. A screen reader's click runs the row it names.
        let (mut h, host, waits, _) = pick_waits_harness(theme);
        h.state_mut().pin_from_picker();
        h.run();
        crate::home_search::open_with(h.state_mut(), "");
        h.run();
        let at = rect_of(&h, "home.header.search.result.0").center();
        h.event(egui::Event::PointerMoved(at));
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
        h.run();
        drop(waits.lock().unwrap().pop().expect("a pick was asked for"));
        let release = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE };
        h.input_mut().events.push(release);
        h.step();
        h.run();
        assert!(!h.state().is_opening() && host.opens.lock().unwrap().is_empty(), "{theme:?}: Library did not open");
        assert_ne!(h.state().state.theme, theme, "{theme:?}: the switch pressed ran");
        crate::home_search::open_with(h.state_mut(), "");
        h.run();
        reader_click(&mut h, crate::home_search::project_row_id("home.header.search", std::path::Path::new("/tmp/Library.archforge")));
        assert_eq!(host.opens.lock().unwrap().len(), 1, "{theme:?}: a screen reader's click opened Library");
    }
}

/// The colour the last frame painted `text` in, the first time it painted it whole.
fn text_colour(h: &Harness<'static, Shell>, text: &str) -> Option<egui::Color32> {
    /// The colour of `text` under `s`, if `s` paints it.
    fn walk(s: &egui::Shape, text: &str) -> Option<egui::Color32> {
        match s {
            egui::Shape::Text(t) if t.galley.job.text == text => {
                let colour = t.galley.job.sections.first().map_or(t.fallback_color, |s| s.format.color);
                let laid_out = if colour == egui::Color32::from(crate::ux::PLACEHOLDER_INK) { t.fallback_color } else { colour };
                Some(t.override_text_color.unwrap_or(laid_out))
            }
            egui::Shape::Vec(v) => v.iter().find_map(|s| walk(s, text)),
            _ => None,
        }
    }
    h.output().shapes.iter().find_map(|c| walk(&c.shape, text))
}

/// The tints of the meshes the last frame painted inside `id`: its icons, drawn as textures.
fn icons_in(h: &Harness<'static, Shell>, id: &str) -> Vec<egui::Color32> {
    /// Collects the tint of every mesh inside `r` under `s`.
    fn walk(s: &egui::Shape, r: egui::Rect, out: &mut Vec<egui::Color32>) {
        match s {
            egui::Shape::Mesh(m) if !m.vertices.is_empty() && m.vertices.iter().all(|v| r.contains(v.pos)) => {
                out.extend(m.vertices.first().map(|v| v.color))
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, r, out)),
            _ => {}
        }
    }
    let (r, mut out) = (rect_of(h, id), Vec::new());
    h.output().shapes.iter().for_each(|c| walk(&c.shape, r, &mut out));
    out
}

/// An opening waiting for the hold turns the title strip's Open project off and draws it muted, as
/// it turns File ▸ Open off: a click would be turned away. Once the opening has run, it is back
/// (AR-749).
#[test]
fn an_opening_waiting_for_the_hold_turns_the_strips_open_project_off() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        h.state_mut().state.theme = theme;
        h.run();
        let p = crate::ux::palette(theme.is_dark());
        assert!(points(&mut h, "strip.open-project"), "{theme:?}: Open project live in the project");
        assert_eq!(text_colour(&h, "Open project…"), Some(egui::Color32::from(p.text)), "{theme:?}");
        assert_eq!(icons_in(&h, "strip.open-project"), [egui::Color32::from(p.text_secondary)], "{theme:?}: its plus");
        h.state_mut().begin_hold().unwrap();
        h.state_mut().open_path(std::path::Path::new("/tmp/Later.archforge"));
        assert!(!points(&mut h, "strip.open-project"), "{theme:?}: off while the opening waits for the hold");
        assert_eq!(text_colour(&h, "Open project…"), Some(egui::Color32::from(p.text_muted)), "{theme:?}: and drawn muted");
        assert_eq!(icons_in(&h, "strip.open-project"), [egui::Color32::from(p.text_muted)], "{theme:?}: its plus too");
        end_hold(&mut h, Shell::end_hold);
        let (path, answer) = host.opens.lock().unwrap().pop().expect("the opening runs after the hold");
        answer.send(Ok(crate::Opened { name: "Later".into(), path, ..Default::default() })).unwrap();
        h.run();
        assert!(points(&mut h, "strip.open-project"), "{theme:?}: and back once it has opened");
        assert_eq!(text_colour(&h, "Open project…"), Some(egui::Color32::from(p.text)), "{theme:?}: drawn as live");
    }
}

/// A diagram takes a skill from the Skills panel, and shows its package's skill as inherited.
#[test]
fn a_diagram_takes_a_skill_from_the_skills_panel_and_shows_its_schemas_as_inherited() {
    let (mut h, _, diagram) = diagram_harness();
    let schema_skill = {
        let s = h.state_mut().session.as_mut().unwrap();
        let st = s.doc();
        let schema = st.uuid(st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::PACKAGE)).unwrap()).unwrap().to_string();
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

/// The links the last frame asked the window to open.
fn opened_links(h: &Harness<'static, Shell>) -> Vec<String> {
    h.output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::OpenUrl(u) => Some(u.url.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn help_opens_about_archforge_whose_support_destinations_say_they_are_not_set_up_yet() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let mut h = harness(theme);
        h.run();
        let at = rect_of(&h, "home.header.help").center();
        click(&mut h, at);
        assert!(h.state().about.is_some(), "{theme:?}");
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "About Archforge") && texts.iter().any(|t| t == "Support Archforge"), "{theme:?}: {texts:?}");
        assert!(texts.iter().any(|t| t == "GitHub Sponsors") && texts.iter().any(|t| t == "Ko-fi"), "{texts:?}");
        assert_eq!(texts.iter().filter(|t| *t == "Not set up yet").count(), 2, "{texts:?}");
        // A destination that is not set up opens nothing.
        let row = rect_of(&h, "about.support.ko-fi").center();
        h.event(egui::Event::PointerMoved(row));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: row, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.step();
        assert!(opened_links(&h).is_empty(), "{:?}", opened_links(&h));
        h.run();
        assert!(h.state().about.is_some());
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        assert!(h.state().about.is_none(), "Escape closes it");
    }
}

#[test]
fn about_scrolls_its_body_in_a_short_window_and_keeps_done_clear() {
    let mut h = harness(ThemeKind::Light);
    h.state_mut().services.support = crate::support::Destinations { github_sponsors: None, ko_fi: Some("https://ko-fi.com/archforge".into()) };
    // The smallest window the app allows.
    h.set_size(egui::vec2(800.0, 400.0));
    h.state_mut().open_about();
    h.run();
    let foot = rect_of(&h, "about.done").top() - 22.0;
    let ko_fi = |h: &Harness<'static, Shell>| ids(h).iter().any(|i| i == "about.support.ko-fi").then(|| rect_of(h, "about.support.ko-fi"));
    assert!(ko_fi(&h).is_none_or(|r| r.bottom() > foot), "Ko-fi does not fit above the footer before scrolling");
    let over = rect_of(&h, "about.support.github-sponsors").center();
    h.event(egui::Event::PointerMoved(over));
    for _ in 0..6 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -1200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        h.run_steps(30);
    }
    let r = ko_fi(&h).expect("Ko-fi in view");
    assert!(r.bottom() <= foot, "{r:?} ends above the footer at {foot}");
    // A click on Done is Done's, not the row's.
    let at = rect_of(&h, "about.done").center();
    click(&mut h, at);
    assert!(h.state().about.is_none());
}

/// New project from the native menu, with a project open: the dialog draws over the workspace,
/// and with nothing in it holding the keyboard no key reaches what is behind it.
#[test]
fn new_project_over_the_workspace_keeps_the_keys_from_it() {
    in_both_appearances(|theme| {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        crate::explorer::add(h.state_mut(), "package");
        h.run();
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
        let at = rect_of(&h, &format!("workspace.explorer.row.{}", rows[0].id)).center();
        click(&mut h, at);
        h.state_mut().explorer.focused = true;
        let revision = h.state().session.as_ref().unwrap().revision();
        crate::actions::run(h.state_mut(), crate::actions::NEW);
        h.run();
        assert!(has_id(&h, "new-project.name"), "{theme:?}: the dialog shows");
        // The keyboard off the dialog's field, so only the hold keeps the keys from what is behind.
        h.ctx.memory_mut(|m| m.focused().into_iter().for_each(|id| m.surrender_focus(id)));
        for (k, m) in
            [(egui::Key::Delete, egui::Modifiers::NONE), (egui::Key::Backspace, egui::Modifiers::NONE), (egui::Key::Z, egui::Modifiers::COMMAND)]
        {
            key(&mut h, k, m);
            assert!(h.state().new_project.is_some() && h.state().explorer.delete.is_none(), "{theme:?}: {k:?} goes nowhere");
            assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "{theme:?}: {k:?} did nothing");
        }
    });
}

/// While About is open, no key reaches the Models explorer or the workspace behind it.
#[test]
fn while_about_is_open_no_key_reaches_the_models_explorer_or_the_workspace_behind_it() {
    let mut h = workspace_harness(ThemeKind::Light, egui::vec2(1600.0, 1120.0));
    crate::explorer::add(h.state_mut(), "package");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    crate::explorer::add(h.state_mut(), "class");
    h.run();
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    // The table, childless, selected in Models with the explorer holding the keys.
    let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
    let at = rect_of(&h, &format!("workspace.explorer.row.{}", rows[1].id)).center();
    click(&mut h, at);
    h.state_mut().explorer.focused = true;
    let selection = h.state().session.as_ref().unwrap().view().selection.clone();
    let revision = h.state().session.as_ref().unwrap().revision();
    let tables = |h: &Harness<'static, Shell>| {
        let st = h.state().session.as_ref().unwrap().doc();
        st.alive().filter(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::CLASS)).count()
    };
    assert_eq!(tables(&h), 1);
    // About opens as the native menu opens it: nothing in the dialog has the keyboard yet.
    h.state_mut().open_about();
    h.run();
    for (k, m) in [
        (egui::Key::ArrowUp, egui::Modifiers::NONE),
        (egui::Key::ArrowDown, egui::Modifiers::NONE),
        (egui::Key::Delete, egui::Modifiers::NONE),
        (egui::Key::Backspace, egui::Modifiers::NONE),
        (egui::Key::Enter, egui::Modifiers::NONE),
        (egui::Key::F2, egui::Modifiers::NONE),
        (egui::Key::Z, egui::Modifiers::COMMAND),
        (egui::Key::G, egui::Modifiers::COMMAND | egui::Modifiers::SHIFT),
    ] {
        h.event(egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m });
        h.step();
        h.event(egui::Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: m });
        h.run();
        assert!(h.state().about.is_some(), "{k:?}: About is still open");
    }
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, selection, "the selection is as it was");
    assert_eq!(tables(&h), 1, "the table is still there");
    assert_eq!(h.state().session.as_ref().unwrap().revision(), revision, "nothing was done or undone");
    assert!(h.state().explorer.delete.is_none() && h.state().explorer.rename.is_none());
    assert!(h.state().generate.is_none(), "Generate does not open over About");
    // Escape closes About and goes no further; then the explorer's keys work again.
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().about.is_none());
    assert_eq!(h.state().session.as_ref().unwrap().view().selection, selection);
    h.state_mut().explorer.focused = true;
    key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert_ne!(h.state().session.as_ref().unwrap().view().selection, selection, "the arrows walk the tree again");
}

/// ⇧F10 opens the menu of the row selected in Models only as itself: with Alt more, a Mac's
/// Control or the command key of either shape it opens nothing.
#[test]
fn shift_f10_opens_the_models_row_menu_only_with_shift_alone() {
    use egui::Modifiers as M;
    in_both_appearances(|theme| {
        let mut h = workspace_harness(theme, egui::vec2(1600.0, 1120.0));
        crate::explorer::add(h.state_mut(), "package");
        h.run();
        key(&mut h, egui::Key::Escape, M::NONE);
        // The new schema stays selected; a click on it would name it in place.
        let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
        h.state_mut().explorer.focused = true;
        h.run();
        assert!(h.ctx.memory(|m| m.focused().is_none()), "{theme:?}: no widget has the keys, so the tree has them");
        for more in [M::ALT, M::CTRL, M::MAC_CMD | M::COMMAND, M::CTRL | M::COMMAND] {
            key(&mut h, egui::Key::F10, M::SHIFT | more);
            assert!(h.state().explorer.context.is_none(), "{theme:?}: {more:?} more opens nothing");
        }
        key(&mut h, egui::Key::F10, M::SHIFT);
        assert_eq!(h.state().explorer.context.as_ref().map(|c| c.target), Some(rows[0].id), "{theme:?}: ⇧F10 opens the row's menu");
    });
}

#[test]
fn a_created_project_opens_its_model_without_waiting_for_the_pointer() {
    let (mut h, host) = creating_harness();
    let at = centre_of(&h, "new-project.create");
    press(&mut h, at);
    let tx = host.pending.lock().unwrap().pop().expect("a creation was asked for");
    tx.send(Ok(crate::Opened {
        name: "Commerce platform".into(),
        path: "/Users/robin/Documents/Archforge/Commerce platform.archforge".into(),
        ..Default::default()
    }))
    .unwrap();
    // The frame that takes the answer opens the project's tab; its model opens on the next frame,
    // which the window asks for at once rather than waiting for the pointer to move.
    h.step();
    assert!(h.state().project.is_some(), "the tab is open");
    assert!(h.ctx.has_requested_repaint(), "the window asks for the frame that opens the model");
    h.step();
    assert_eq!(h.state().adopted.as_deref(), Some(std::path::Path::new("/Users/robin/Documents/Archforge/Commerce platform.archforge")));
}

#[test]
fn the_new_project_dialog_calls_a_project_the_folder_it_is_kept_in() {
    let (h, _host) = creating_harness();
    let texts = painted(&h);
    assert!(texts.iter().any(|t| t == "Creates the folder Commerce platform.archforge here: the project and every version of it."), "{texts:?}");
    assert!(!texts.iter().any(|t| t.contains("versioned file") || t.contains("project file")), "a project is a folder, never a file: {texts:?}");
}

/// A host whose Dolt is missing until the test installs it; it counts the looks for it.
#[derive(Default)]
struct DoltHost {
    installed: std::sync::atomic::AtomicBool,
    looks: std::sync::atomic::AtomicUsize,
}

impl crate::ProjectHost for DoltHost {
    fn create(&self, _folder: &std::path::Path, _name: &str) -> std::sync::mpsc::Receiver<Result<crate::Opened, crate::CreateError>> {
        std::sync::mpsc::channel().1
    }

    /// Counts the look, and answers at once whether dolt is installed, as the host is told.
    fn find_dolt(&self) -> std::sync::mpsc::Receiver<Result<(), String>> {
        self.looks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = std::sync::mpsc::channel();
        let installed = self.installed.load(std::sync::atomic::Ordering::SeqCst);
        tx.send(if installed { Ok(()) } else { Err("no dolt on the PATH".into()) }).unwrap();
        rx
    }
}

fn dolt_harness(theme: ThemeKind, installed: bool) -> (Harness<'static, Shell>, std::sync::Arc<DoltHost>) {
    crate::i18n::set_language("en");
    let host = std::sync::Arc::new(DoltHost::default());
    host.installed.store(installed, std::sync::atomic::Ordering::SeqCst);
    let mut shell = Shell::new(Services { projects: Some(host.clone()), ..Services::default() }, NOTATIONS, &SCREENS);
    shell.state.theme = theme;
    let mut h = harness_over(shell);
    h.run_steps(4);
    (h, host)
}

/// A press and release on `id` in one frame, after the pointer has come to it, so the frame's
/// output holds what the click asked for.
fn click_in_one_frame(h: &mut Harness<'static, Shell>, id: &str) {
    let at = centre_of(h, id);
    h.event(egui::Event::PointerMoved(at));
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
}

#[test]
fn home_says_when_dolt_is_missing_with_its_install_page_and_looks_again() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, host) = dolt_harness(theme, false);
        assert_eq!(h.state().dolt_missing.as_deref(), Some("no dolt on the PATH"), "{theme:?}: looked for at the start");
        let ids = ids(&h);
        for id in ["home.dolt.install", "home.dolt.look-again"] {
            assert!(ids.iter().any(|i| i == id), "{theme:?}: {id} in {ids:?}");
        }
        assert!(!ids.iter().any(|i| i == "home.banner.close"), "{theme:?}: the notice stands in the banner's place");
        let texts = painted(&h);
        assert!(texts.iter().any(|t| t == "Install Dolt to keep your projects"), "{theme:?}: {texts:?}");
        click_in_one_frame(&mut h, "home.dolt.install");
        assert_eq!(opened_links(&h), [crate::home::DOLT_INSTALL], "{theme:?}: Dolt's installation page, in the browser");
        // Installed meanwhile: Look again finds it, and Home is as it always was.
        host.installed.store(true, std::sync::atomic::Ordering::SeqCst);
        let at = centre_of(&h, "home.dolt.look-again");
        click(&mut h, at);
        h.run_steps(4);
        assert!(h.state().dolt_missing.is_none(), "{theme:?}");
        assert_eq!(host.looks.load(std::sync::atomic::Ordering::SeqCst), 2, "{theme:?}: once at the start, once for Look again");
        let ids = self::ids(&h);
        assert!(!ids.iter().any(|i| i.starts_with("home.dolt.")), "{theme:?}: {ids:?}");
        assert!(ids.iter().any(|i| i == "home.banner.close"), "{theme:?}: the banner is back");
    }
}

#[test]
fn a_project_made_or_opened_ends_the_notice_without_looking_again() {
    let (mut h, host) = dolt_harness(ThemeKind::Light, false);
    assert!(h.state().dolt_missing.is_some());
    // Dolt was installed and a project opened, Look again never pressed.
    h.state_mut().open_project(crate::Opened { name: "Commerce".into(), path: "/Users/robin/Commerce.archforge".into(), ..Default::default() });
    assert!(h.state().dolt_missing.is_none());
    assert_eq!(host.looks.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn with_dolt_there_home_shows_no_notice_and_looks_once() {
    let (mut h, host) = dolt_harness(ThemeKind::Light, true);
    h.run_steps(8);
    assert!(h.state().dolt_missing.is_none());
    assert!(!ids(&h).iter().any(|i| i.starts_with("home.dolt.")));
    assert_eq!(host.looks.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn the_new_project_dialog_links_to_dolts_install_page_when_dolt_is_missing() {
    let (mut h, host) = creating_harness();
    assert!(!ids(&h).iter().any(|i| i == "new-project.install-dolt"), "no link before a failure");
    let at = centre_of(&h, "new-project.create");
    press(&mut h, at);
    answer(&mut h, &host, Err(crate::CreateError::DoltMissing { detail: "no dolt on the PATH".into() }));
    click_in_one_frame(&mut h, "new-project.install-dolt");
    assert_eq!(opened_links(&h), [crate::home::DOLT_INSTALL]);
    // Any other failure has no such link.
    let at = centre_of(&h, "new-project.create");
    press(&mut h, at);
    answer(&mut h, &host, Err(crate::CreateError::Failed { detail: "the server did not start".into() }));
    assert!(!ids(&h).iter().any(|i| i == "new-project.install-dolt"));
}

#[test]
fn the_activity_rail_puts_diagrams_above_models_in_its_look_and_its_keyboard_order() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
        let (diagrams, models, source) = (
            rect_of(&h, "workspace.explorers.diagrams"),
            rect_of(&h, "workspace.explorers.models"),
            rect_of(&h, "workspace.explorers.source-control"),
        );
        assert!(diagrams.bottom() <= models.top() && models.bottom() <= source.top(), "{theme:?}: Diagrams, then Models, then Source Control");
        // Tab follows the order the controls are made in, which is the order drawn.
        let ids = ids(&h);
        let at = |id: &str| ids.iter().position(|i| i == id).unwrap();
        assert!(at("workspace.explorers.diagrams") < at("workspace.explorers.models"), "{theme:?}: {ids:?}");
    }
}

/// Settings → AI Agents over Home, from the fake agents host set up by `setup`; the window is tall
/// enough to show the whole page. The look at start has run once, and its answers are in.
fn agents_harness(
    theme: ThemeKind,
    setup: impl FnOnce(&mut crate::ai_agents::fake::Fake),
) -> (Harness<'static, Shell>, std::sync::Arc<crate::ai_agents::fake::Fake>) {
    use crate::ai_agents::{Look, Provider, Store, fake};
    crate::i18n::set_language("en");
    let mut f = fake::Fake::default();
    *f.look.lock().unwrap() = Look {
        store: Store::MacKeychain,
        claude: fake::found(Provider::Claude),
        codex: fake::found(Provider::Codex),
        claude_key: crate::ai_agents::Key::Absent,
        codex_key: crate::ai_agents::Key::Absent,
    };
    setup(&mut f);
    let f = std::sync::Arc::new(f);
    let mut shell = Shell::new(Services { agents: Some(f.clone()), ..Services::default() }, NOTATIONS, &SCREENS);
    shell.state.theme = theme;
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1400.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    (h, f)
}

/// Opens Settings → AI Agents the way a person does: the avatar, then the section.
fn open_agents(h: &mut Harness<'static, Shell>) {
    let at = rect_of(h, "home.header.account").center();
    click(h, at);
    let at = rect_of(h, "settings.nav.agents").center();
    click(h, at);
    h.run();
}

/// The fake's calls: look, keys, save, remove, test, account, login, cancel, logout.
fn calls(f: &crate::ai_agents::fake::Fake) -> [usize; 9] {
    use std::sync::atomic::Ordering::Relaxed;
    let c = &f.calls;
    [
        c.look.load(Relaxed),
        c.keys.load(Relaxed),
        c.save.load(Relaxed),
        c.remove.load(Relaxed),
        c.test.load(Relaxed),
        c.account.load(Relaxed),
        c.login.load(Relaxed),
        c.cancel.load(Relaxed),
        c.logout.load(Relaxed),
    ]
}

fn painted_has(h: &Harness<'static, Shell>, text: &str) -> bool {
    painted(h).iter().any(|t| t.contains(text))
}

#[test]
fn settings_has_a_profile_and_an_ai_agents_section_with_done_in_the_footer() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = agents_harness(theme, |_| {});
        let at = rect_of(&h, "home.header.account").center();
        click(&mut h, at);
        let found = ids(&h);
        for id in ["settings.nav.profile", "settings.nav.agents", "settings.name", "settings.close", "settings.done"] {
            assert!(found.iter().any(|i| i == id), "{theme:?}: {id} in {found:?}");
        }
        let dialog = rect_of(&h, "settings.close");
        let done = rect_of(&h, "settings.done");
        assert!(done.top() > rect_of(&h, "settings.name").bottom(), "Done sits in the footer");
        assert!(dialog.right() - rect_of(&h, "settings.nav.profile").left() > 800.0, "the dialog is 880 wide");
        let at = rect_of(&h, "settings.nav.agents").center();
        click(&mut h, at);
        let found = ids(&h);
        for id in [
            "settings.agents.default",
            "settings.agents.claude.key",
            "settings.agents.claude.save",
            "settings.agents.claude.model",
            "settings.agents.claude.status",
            "settings.agents.codex.way.chatgpt",
            "settings.agents.codex.way.api-key",
            "settings.agents.codex.sign-in",
            "settings.agents.codex.device-code",
            "settings.agents.codex.model",
            "settings.agents.check",
            "settings.agents.test",
        ] {
            assert!(found.iter().any(|i| i == id), "{theme:?}: {id} in {found:?}");
        }
        assert!(!found.iter().any(|i| i == "settings.name"), "Profile's field is not on AI Agents");
        // Escape closes it.
        h.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
        h.run();
        assert!(h.state().settings.is_none());
    }
}

/// Launching looks for the programs alone; opening AI Agents reads the keys once and Codex's
/// account on each opening (AR-222), and sends nothing. In both appearances.
#[test]
fn opening_ai_agents_reads_the_keys_once_and_sends_nothing() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f) = agents_harness(theme, |_| {});
        // The look at start: the programs once; no key read, no account read (it starts the codex
        // program) and nothing sent.
        for _ in 0..5 {
            h.run();
        }
        let at_start = calls(&f);
        assert_eq!(at_start, [1, 0, 0, 0, 0, 0, 0, 0, 0], "the programs' look at start, and nothing else");
        open_agents(&mut h);
        for _ in 0..5 {
            h.run();
        }
        let mut keys_read = at_start;
        keys_read[1] = 1;
        keys_read[5] = 1;
        assert_eq!(calls(&f), keys_read, "opening the page read the keys and Codex's account once, and asked for nothing else");
        assert!(painted_has(&h, "Saved in the macOS Keychain"), "the store the read found");
        // Kept: closed and opened again, the page reads nothing more.
        let at = rect_of(&h, "settings.done").center();
        click(&mut h, at);
        open_agents(&mut h);
        keys_read[5] = 2;
        assert_eq!(calls(&f), keys_read, "the keys are kept until Look again, a save or a removal; Codex's account is read again (AR-222)");
        assert_eq!(f.calls.outward(), 0, "no test message and no sign-in");
        // Opening Settings on a shell without the services draws the page and asks nothing either.
        crate::i18n::set_language("en");
        let mut bare = harness(ThemeKind::Dark);
        let at = rect_of(&bare, "home.header.account").center();
        click(&mut bare, at);
        let at = rect_of(&bare, "settings.nav.agents").center();
        click(&mut bare, at);
        assert!(painted_has(&bare, "This build of Archforge has no agent programs to talk to."));
    }
}

#[test]
fn every_designed_state_of_ai_agents_draws_from_the_fake_in_both_appearances() {
    use crate::ai_agents::{AccountError, CodexAccount, Key, LoginEvent, Program, Store, System};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        // A2: found, nothing set up: the model selects wait, Send test message waits.
        let (mut h, _) = agents_harness(theme, |_| {});
        open_agents(&mut h);
        assert!(painted_has(&h, "Found · claude 2.1.317") && painted_has(&h, "Found · codex 0.166.0"));
        assert!(painted_has(&h, "Not signed in"));
        let at = rect_of(&h, "settings.agents.claude.model").center();
        click(&mut h, at);
        assert!(h.state().settings.as_ref().unwrap().agents.menu.is_none(), "{theme:?}: the model waits for a key");
        // A3: Claude missing with its install steps; Codex on a saved API key.
        let (mut h, _) = agents_harness(theme, |f| {
            let mut look = f.look.lock().unwrap();
            look.claude = Program::Missing;
            look.codex_key = Key::Saved { last4: "9c1E".into() };
        });
        h.state_mut().state.agents.codex_sign_in = crate::ai_agents::CodexSignIn::ApiKey;
        open_agents(&mut h);
        assert!(painted_has(&h, "Not found") && painted_has(&h, "npm install -g @anthropic-ai/claude-code"));
        assert!(painted_has(&h, "API key saved · ends in 9c1E"));
        for id in [
            "settings.agents.claude.look-again",
            "settings.agents.claude.guide",
            "settings.agents.claude.copy-install",
            "settings.agents.codex.replace",
        ] {
            assert!(ids(&h).iter().any(|i| i == id), "{theme:?}: {id}");
        }
        let at = rect_of(&h, "settings.agents.check").center();
        click(&mut h, at);
        h.run();
        assert!(painted_has(&h, "Claude not found · Codex ready"), "{theme:?}: the badge sums it up");
        // A newer version than tested is accepted with a warning.
        let (mut h, _) = agents_harness(theme, |f| f.look.lock().unwrap().claude = Program::Found { version: "2.9.0".into(), tested: false });
        open_agents(&mut h);
        assert!(painted_has(&h, "Not yet tested with this version · claude 2.9.0"));
        // A4: ready: the key masked, Codex signed in, Check connection green.
        let (mut h, _) = agents_harness(theme, |f| {
            f.look.lock().unwrap().claude_key = Key::Saved { last4: "4F2a".into() };
            *f.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: Some("ChatGPT Plus".into()) });
        });
        open_agents(&mut h);
        assert!(painted_has(&h, "•••••••••••••••••••• 4F2a"));
        assert!(painted_has(&h, "Signed in as you@example.com · ChatGPT Plus"));
        let at = rect_of(&h, "settings.agents.check").center();
        click(&mut h, at);
        h.run();
        assert!(painted_has(&h, "Checked just now · Claude ready · Codex ready"));
        // A5: the key refused by a test message, Codex's sign-in expired.
        let (mut h, f) = agents_harness(theme, |f| {
            f.look.lock().unwrap().claude_key = Key::Saved { last4: "4F2a".into() };
            *f.account.lock().unwrap() = Ok(CodexAccount::Expired { email: "you@example.com".into() });
            *f.test.lock().unwrap() = Err(AccountError::KeyRefused { detail: "401, invalid key".into() });
        });
        open_agents(&mut h);
        let at = rect_of(&h, "settings.agents.test").center();
        click(&mut h, at);
        h.run();
        assert_eq!(f.calls.test.load(std::sync::atomic::Ordering::Relaxed), 1, "Send test message goes to the default agent");
        assert!(painted_has(&h, "The provider refused this key when the test message was sent (401, invalid key)."));
        assert!(painted_has(&h, "Sign-in expired") && painted_has(&h, "has expired."));
        let at = rect_of(&h, "settings.agents.check").center();
        click(&mut h, at);
        h.run();
        assert!(painted_has(&h, "Claude key refused · Codex sign-in expired"));
        // A6: the device code, Copy, Open sign-in page and Cancel.
        let (mut h, f) = agents_harness(theme, |f| {
            *f.login.lock().unwrap() =
                vec![LoginEvent::DeviceCode { code: "WXYZ-4821".into(), url: "https://auth.openai.com/codex/device".into(), minutes: 15 }];
        });
        open_agents(&mut h);
        let at = rect_of(&h, "settings.agents.codex.device-code").center();
        click(&mut h, at);
        h.run();
        assert!(painted_has(&h, "WXYZ-4821"));
        for id in ["settings.agents.codex.copy-code", "settings.agents.codex.open-page", "settings.agents.codex.cancel"] {
            assert!(ids(&h).iter().any(|i| i == id), "{theme:?}: {id}");
        }
        let at = rect_of(&h, "settings.agents.codex.cancel").center();
        click(&mut h, at);
        assert_eq!(f.calls.cancel.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(h.state().agents.login.is_none());
        // A sign-in that finishes shows who is signed in.
        let (mut h, _) = agents_harness(theme, |f| {
            *f.login.lock().unwrap() = vec![LoginEvent::Done(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None })];
        });
        open_agents(&mut h);
        let at = rect_of(&h, "settings.agents.codex.sign-in").center();
        click(&mut h, at);
        h.run();
        assert!(painted_has(&h, "Signed in as you@example.com"));
        // The real accounts service before AR-202: sign-in says it is not available yet.
        let (mut h, _) = agents_harness(theme, |f| *f.account.lock().unwrap() = Err(AccountError::NotAvailableYet));
        open_agents(&mut h);
        assert!(painted_has(&h, "Signing in with ChatGPT is not available in this version yet."));
        // A8: a system agents do not run on yet.
        let (mut h, f) = agents_harness(theme, |f| f.system = System::NotYet);
        open_agents(&mut h);
        assert!(painted_has(&h, "Not available on this system yet"));
        assert_eq!(calls(&f), [0; 9], "nothing is looked for on such a system");
        let _ = Store::NotLooked;
    }
}

#[test]
fn a_key_is_saved_shown_masked_replaced_and_removed() {
    let (mut h, f) = agents_harness(ThemeKind::Light, |_| {});
    open_agents(&mut h);
    // Save key waits for a key.
    let before = calls(&f);
    let at = rect_of(&h, "settings.agents.claude.save").center();
    click(&mut h, at);
    assert_eq!(calls(&f), before, "nothing to save");
    let at = rect_of(&h, "settings.agents.claude.key").center();
    click(&mut h, at);
    h.event(egui::Event::Text("  sk-ant-api03-canary-0000-4F2a\n".into()));
    h.run();
    assert!(!painted_has(&h, "sk-ant-api03"), "the field hides what is typed");
    let at = rect_of(&h, "settings.agents.claude.save").center();
    click(&mut h, at);
    h.run();
    assert_eq!(f.saved.lock().unwrap().as_slice(), &[(crate::ai_agents::Provider::Claude, "4F2a".to_string())]);
    // Saved: masked to its last four, with Replace and Remove; the model may be chosen now.
    assert!(painted_has(&h, "•••••••••••••••••••• 4F2a"));
    assert!(!painted_has(&h, "canary"), "the key is never shown again");
    assert!(ids(&h).iter().any(|i| i == "settings.agents.claude.key.masked"));
    let at = rect_of(&h, "settings.agents.claude.model").center();
    click(&mut h, at);
    let at = rect_of(&h, "settings.agents.claude.model.2").center();
    click(&mut h, at);
    assert_eq!(h.state().state.agents.claude_model, "opus");
    // Replace opens the field again; a new key replaces the saved one.
    let at = rect_of(&h, "settings.agents.claude.replace").center();
    click(&mut h, at);
    h.event(egui::Event::Text("sk-ant-api03-second-key-9z9Z".into()));
    h.run();
    let at = rect_of(&h, "settings.agents.claude.save").center();
    click(&mut h, at);
    h.run();
    assert!(painted_has(&h, "•••••••••••••••••••• 9z9Z"), "the new key replaced the old one");
    // A key with a space in it is refused under the field.
    let at = rect_of(&h, "settings.agents.claude.replace").center();
    click(&mut h, at);
    h.event(egui::Event::Text("not a key".into()));
    h.run();
    let at = rect_of(&h, "settings.agents.claude.save").center();
    click(&mut h, at);
    h.run();
    assert!(painted_has(&h, "A key is one word, without spaces."));
    let at = rect_of(&h, "settings.agents.claude.cancel").center();
    click(&mut h, at);
    // Remove.
    let at = rect_of(&h, "settings.agents.claude.remove").center();
    click(&mut h, at);
    h.run();
    assert_eq!(h.state().agents.look.claude_key, crate::ai_agents::Key::Absent);
    assert!(ids(&h).iter().any(|i| i == "settings.agents.claude.key"), "the field is back");
}

#[test]
fn without_a_secure_store_keys_cannot_be_saved_but_chatgpt_sign_in_works() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f) = agents_harness(theme, |f| {
            f.look.lock().unwrap().store = crate::ai_agents::Store::Missing { why: "no Secret Service".into() };
        });
        open_agents(&mut h);
        assert!(painted_has(&h, "No secure store on this system"));
        assert!(painted_has(&h, "Can't be saved: there is no secure store to keep it in."));
        // The key field takes no typing and Save key does nothing.
        let at = rect_of(&h, "settings.agents.claude.key").center();
        click(&mut h, at);
        h.event(egui::Event::Text("sk-ant-api03-canary-0000-4F2a".into()));
        h.run();
        let at = rect_of(&h, "settings.agents.claude.save").center();
        click(&mut h, at);
        h.run();
        assert_eq!(f.calls.save.load(std::sync::atomic::Ordering::Relaxed), 0, "{theme:?}: no key reaches the host");
        // Codex's API key way is off; its ChatGPT sign-in is not.
        let at = rect_of(&h, "settings.agents.codex.way.api-key").center();
        click(&mut h, at);
        assert_eq!(h.state().state.agents.codex_sign_in, crate::ai_agents::CodexSignIn::ChatGpt);
        let at = rect_of(&h, "settings.agents.codex.sign-in").center();
        click(&mut h, at);
        assert_eq!(f.calls.login.load(std::sync::atomic::Ordering::Relaxed), 1, "{theme:?}: ChatGPT sign-in still starts");
        // Why, in the system's words, and that a restart is what finds a store started later.
        assert!(painted_has(&h, "no Secret Service"));
        assert!(painted_has(&h, "Start one and restart Archforge to save keys"));
        assert!(!ids(&h).iter().any(|i| i == "settings.agents.store.look-again"), "looking again cannot find a store started later");
    }
}

/// The note that the codex program keeps its ChatGPT sign-in in its own files, so it works without
/// the secure store, shows where no secure store opened, and only there (AR-236: the sign-in is in
/// those files on every system, so the note follows the store alone).
#[test]
fn the_own_files_note_shows_where_no_secure_store_opened() {
    use crate::ai_agents::{CodexAccount, Store};
    let note = "keeps its ChatGPT sign-in in its own files";
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        for (store, shown) in [
            (Store::Missing { why: "no Secret Service".into() }, true),
            (Store::MacKeychain, false),
            (Store::SecretService, false),
            (Store::NotLooked, false),
            (Store::Other, false),
        ] {
            let (mut h, _) = agents_harness(theme, |f| {
                f.look.lock().unwrap().store = store.clone();
                *f.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None });
            });
            open_agents(&mut h);
            h.run();
            assert_eq!(painted_has(&h, note), shown, "{theme:?}: {store:?}");
            assert!(painted_has(&h, "you@example.com"), "{theme:?}: {store:?}: the account was read");
        }
    }
}

/// Settings ▸ AI Agents has no switch for Chat (AR-237): the default agent is kept in the
/// preferences and read back, nothing of a switch is written, and the choices an earlier build kept
/// with its switch, on or off, read as the same choices. In both appearances.
#[test]
fn ai_agents_has_no_chat_switch_and_keeps_the_default_agent() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, _) = agents_harness(theme, |_| {});
        open_agents(&mut h);
        assert!(!ids(&h).iter().any(|i| i == "settings.agents.chat-uses-model"), "{theme:?}");
        assert!(!painted_has(&h, "Let Chat use the model"), "{theme:?}");
        let at = rect_of(&h, "settings.agents.default").center();
        click(&mut h, at);
        let at = rect_of(&h, "settings.agents.default.1").center();
        click(&mut h, at);
        assert_eq!(h.state().state.agents.default_agent, crate::ai_agents::Provider::Codex, "{theme:?}");
        let text = serde_json::to_string(&h.state().state).unwrap();
        assert!(!text.contains("chat_uses_model"), "{theme:?}: {text}");
        let mut back: UiState = serde_json::from_str(&text).unwrap();
        back.sanitize();
        assert_eq!(back.agents.default_agent, crate::ai_agents::Provider::Codex, "{theme:?}");
    }
    assert_eq!(earlier_agent_choices(true), earlier_agent_choices(false));
    assert_eq!(earlier_agent_choices(true).claude_model, "sonnet");
}

#[test]
fn a_key_typed_and_not_saved_is_dropped_with_the_fields_memory_of_it() {
    let (mut h, f) = agents_harness(ThemeKind::Light, |_| {});
    open_agents(&mut h);
    let field = egui::Id::unique(("automation", "settings.agents.claude.key"));
    for leave in ["escape", "profile", "done"] {
        if leave != "escape" || h.state().settings.is_none() {
            open_agents(&mut h);
        }
        let at = rect_of(&h, "settings.agents.claude.key").center();
        click(&mut h, at);
        for part in ["sk-ant-", "api03-", "canary-0000-4F2a"] {
            h.event(egui::Event::Text(part.into()));
            h.run();
        }
        assert!(crate::settings_agents::drafted(&h.state().settings.as_ref().unwrap().agents), "{leave}: typed");
        let undo = egui::text_edit::TextEditState::load(&h.ctx, field).map(|s| s.undoer().has_undo(&Default::default()));
        assert_ne!(undo, Some(true), "{leave}: the hidden field keeps no undo history of the key");
        match leave {
            "escape" => {
                h.event(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
                h.run();
                assert!(h.state().settings.is_none());
            }
            "profile" => {
                let at = rect_of(&h, "settings.nav.profile").center();
                click(&mut h, at);
                assert!(!crate::settings_agents::drafted(&h.state().settings.as_ref().unwrap().agents), "another section drops the draft");
                let at = rect_of(&h, "settings.done").center();
                click(&mut h, at);
            }
            _ => {
                let at = rect_of(&h, "settings.done").center();
                click(&mut h, at);
            }
        }
        let state = egui::text_edit::TextEditState::load(&h.ctx, field);
        assert!(state.is_none_or(|s| !s.undoer().has_undo(&Default::default())), "{leave}: the field's memory is cleared");
    }
    assert_eq!(f.calls.save.load(std::sync::atomic::Ordering::Relaxed), 0, "nothing was saved");
}

/// A Codex sign-in that expires while Archforge runs shows as expired, with its email, after Check
/// connection and after Settings opens again, without a restart (AR-222). The codex program reads
/// an expired sign-in without an email; the one known before is kept. In both appearances.
#[test]
fn a_codex_sign_in_that_expires_mid_run_shows_as_expired_after_check_connection_and_reopening() {
    use crate::ai_agents::CodexAccount;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        const EXPIRED: &str = "The ChatGPT sign-in for you@example.com has expired.";
        for trigger in ["check connection", "reopening Settings", "another section and back"] {
            let (mut h, f) = agents_harness(theme, |f| {
                *f.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None });
            });
            open_agents(&mut h);
            assert!(painted_has(&h, "Signed in as you@example.com"), "{trigger}");
            // The sign-in expires; the codex program now reads it so, without an email.
            *f.account.lock().unwrap() = Ok(CodexAccount::Expired { email: String::new() });
            if trigger == "check connection" {
                let at = rect_of(&h, "settings.agents.check").center();
                click(&mut h, at);
            } else if trigger == "reopening Settings" {
                let at = rect_of(&h, "settings.done").center();
                click(&mut h, at);
                open_agents(&mut h);
            } else {
                let at = rect_of(&h, "settings.nav.profile").center();
                click(&mut h, at);
                let at = rect_of(&h, "settings.nav.agents").center();
                click(&mut h, at);
            }
            h.run();
            assert_eq!(f.calls.account.load(std::sync::atomic::Ordering::Relaxed), 2, "{theme:?} {trigger}: read again");
            assert!(painted_has(&h, EXPIRED), "{theme:?} {trigger}: {:?}", painted(&h));
            assert!(!painted_has(&h, "Signed in as"), "{trigger}");
        }
    }
}

/// A Codex conversation found the sign-in expired before any read gave its email (the first read
/// failed): a later read that still says signed in shows it expired, now with that email
/// (AR-222, review). In both appearances.
#[test]
fn an_expiry_found_before_the_email_was_known_stays_once_a_read_brings_it() {
    use crate::ai_agents::{AccountError, CodexAccount};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f) = agents_harness(theme, |f| *f.account.lock().unwrap() = Err(AccountError::Failed { detail: "no answer".into() }));
        open_agents(&mut h);
        assert!(painted_has(&h, "The sign-in could not be read: no answer"), "{theme:?}");
        *f.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None });
        crate::ai_agents::sign_in_expired(h.state_mut());
        h.run();
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::Expired { email: "you@example.com".into() })), "{theme:?}");
        assert!(painted_has(&h, "The ChatGPT sign-in for you@example.com has expired."), "{theme:?}: {:?}", painted(&h));
    }
}

/// A read of Codex's account that began before a sign-out answers after it: its older answer is
/// dropped, so the page keeps saying Not signed in (AR-222, review). In both appearances.
#[test]
fn an_account_read_older_than_a_sign_out_does_not_undo_it() {
    use crate::ai_agents::CodexAccount;
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f) = agents_harness(theme, |f| {
            *f.account.lock().unwrap() = Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None });
        });
        open_agents(&mut h);
        assert!(painted_has(&h, "Signed in as you@example.com"), "{theme:?}");
        // A read again begins, slow to answer; meanwhile the person signs out.
        f.hold_account.store(true, std::sync::atomic::Ordering::Relaxed);
        crate::ai_agents::reread_account(h.state_mut());
        h.run();
        assert_eq!(f.held_account.lock().unwrap().len(), 1, "{theme:?}: the read waits");
        let at = rect_of(&h, "settings.agents.codex.sign-in").center();
        click(&mut h, at);
        h.run();
        assert!(painted_has(&h, "Not signed in"), "{theme:?}");
        // The older read answers "signed in" now: dropped.
        f.release_account();
        h.run();
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::NotSignedIn)), "{theme:?}");
        assert!(painted_has(&h, "Not signed in") && !painted_has(&h, "Signed in as"), "{theme:?}");
    }
}

/// A read of Codex's account that began before a sign-in answers after it: its older answer is
/// dropped, so the page keeps the new sign-in; a Check connection that waited only for that read
/// ends at once, saying Codex is ready (AR-222, review). In both appearances.
#[test]
fn an_account_read_older_than_a_sign_in_does_not_undo_it_nor_hold_check_connection() {
    use crate::ai_agents::{CodexAccount, LoginEvent};
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f) = agents_harness(theme, |f| {
            *f.login.lock().unwrap() = vec![LoginEvent::Done(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None })];
        });
        open_agents(&mut h);
        assert!(painted_has(&h, "Not signed in"), "{theme:?}");
        // Check connection reads the account again, slow to answer; meanwhile the person signs in.
        f.hold_account.store(true, std::sync::atomic::Ordering::Relaxed);
        let at = rect_of(&h, "settings.agents.check").center();
        click(&mut h, at);
        assert_eq!(f.held_account.lock().unwrap().len(), 1, "{theme:?}: the read waits");
        assert!(h.state().agents.checking(), "{theme:?}: the check waits for it");
        // Frame by frame: the click starts the sign-in, the next frame takes its end; the check
        // ends in that same frame, as nothing else asks for one more.
        let at = rect_of(&h, "settings.agents.codex.sign-in").center();
        h.event(egui::Event::PointerMoved(at));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.step();
        h.step();
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::SignedIn { email: "you@example.com".into(), plan: None })), "{theme:?}");
        assert!(!h.state().agents.checking(), "{theme:?}: the check ended with the sign-in");
        h.run();
        assert!(painted_has(&h, "Signed in as you@example.com"), "{theme:?}");
        assert!(painted_has(&h, "Codex ready"), "{theme:?}: {:?}", painted(&h));
        // The older read answers "not signed in" now: dropped.
        f.release_account();
        h.run();
        assert!(painted_has(&h, "Signed in as you@example.com") && !painted_has(&h, "Not signed in"), "{theme:?}");
    }
}

/// A sign-in found expired, then read as another account signed in, is that other account's: its
/// later reads show it signed in, not expired, as no conversation found it so (AR-222, review). In
/// both appearances.
#[test]
fn another_account_read_after_an_expiry_is_not_taken_as_expired() {
    use crate::ai_agents::CodexAccount;
    let signed_in = |email: &str| Ok(CodexAccount::SignedIn { email: email.into(), plan: None });
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f) = agents_harness(theme, |f| *f.account.lock().unwrap() = signed_in("a@example.com"));
        open_agents(&mut h);
        crate::ai_agents::sign_in_expired(h.state_mut());
        h.run();
        assert_eq!(h.state().agents.codex, Some(Ok(CodexAccount::Expired { email: "a@example.com".into() })), "{theme:?}");
        // Another account is signed in now, outside this window.
        *f.account.lock().unwrap() = signed_in("b@example.com");
        for read in ["first", "second"] {
            crate::ai_agents::reread_account(h.state_mut());
            h.run();
            assert_eq!(h.state().agents.codex, Some(signed_in("b@example.com")), "{theme:?}: {read} read");
        }
        assert!(painted_has(&h, "Signed in as b@example.com"), "{theme:?}: {:?}", painted(&h));
    }
}

/// A sign-in that runs out of time, or that the codex program cancels, says so under the sign-in
/// in the page's words, and the sign-in can start again.
#[test]
fn a_sign_in_that_times_out_or_is_cancelled_by_codex_says_so() {
    use crate::ai_agents::{AccountError, LoginEvent};
    for (error, said) in [
        (AccountError::SignInTimedOut, "The sign-in was not finished in time. Sign in again."),
        (AccountError::SignInCancelled, "The codex program cancelled the sign-in. Sign in again."),
    ] {
        let (mut h, f) = agents_harness(ThemeKind::Light, |f| {
            *f.login.lock().unwrap() = vec![
                LoginEvent::DeviceCode { code: "WXYZ-4821".into(), url: "https://auth.openai.com/codex/device".into(), minutes: 15 },
                LoginEvent::Failed(error.clone()),
            ];
        });
        open_agents(&mut h);
        let at = rect_of(&h, "settings.agents.codex.device-code").center();
        click(&mut h, at);
        h.run();
        assert!(h.state().agents.login.is_none(), "{error:?}: the sign-in is over");
        assert!(painted_has(&h, said), "{error:?}");
        let at = rect_of(&h, "settings.agents.codex.sign-in").center();
        click(&mut h, at);
        assert_eq!(f.calls.login.load(std::sync::atomic::Ordering::Relaxed), 2, "{error:?}: it can start again");
    }
}

/// The device-code card shows the sign-in page's address the codex program sent, beside the code,
/// as text a person can select, with an icon that copies it, so the code can be entered on another
/// device (AR-223). An address Archforge would not open is not shown, and nothing copies it.
#[test]
fn the_device_code_card_shows_the_sign_in_address_to_select_and_copy() {
    use crate::ai_agents::LoginEvent;
    let url = "https://auth.openai.com/codex/device";
    let (mut h, _) = agents_harness(ThemeKind::Light, |f| {
        *f.login.lock().unwrap() = vec![LoginEvent::DeviceCode { code: "WXYZ-4821".into(), url: url.into(), minutes: 15 }];
    });
    open_agents(&mut h);
    let at = rect_of(&h, "settings.agents.codex.device-code").center();
    click(&mut h, at);
    h.run();
    assert!(painted_has(&h, "WXYZ-4821"));
    assert!(painted_has(&h, url), "the card shows the address");
    for id in ["settings.agents.codex.address", "settings.agents.codex.copy-address", "settings.agents.codex.open-page"] {
        assert!(ids(&h).iter().any(|i| i == id), "{id}");
    }
    assert!(rect_of(&h, "settings.agents.codex.address").top() > rect_of(&h, "settings.agents.codex.code").bottom(), "under the code");
    // The address is text to select: clicking it gives it a text cursor, and it stays as sent.
    let at = rect_of(&h, "settings.agents.codex.address").center();
    click(&mut h, at);
    let field = crate::widgets::egui_id("settings.agents.codex.address");
    assert!(egui::text_edit::TextEditState::load(&h.ctx, field).and_then(|s| s.cursor.char_range()).is_some(), "the address is selectable");
    assert!(painted_has(&h, url), "selecting it changes nothing");
    // Copy puts exactly the address on the clipboard.
    let at = rect_of(&h, "settings.agents.codex.copy-address").center();
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    h.step();
    let copied: Vec<String> = h
        .output()
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, vec![url.to_string()]);
    // An address off the sign-in host is neither shown nor copied.
    let (mut h, _) = agents_harness(ThemeKind::Light, |f| {
        *f.login.lock().unwrap() =
            vec![LoginEvent::DeviceCode { code: "WXYZ-4821".into(), url: "https://example.com/codex/device".into(), minutes: 15 }];
    });
    open_agents(&mut h);
    let at = rect_of(&h, "settings.agents.codex.device-code").center();
    click(&mut h, at);
    h.run();
    assert!(painted_has(&h, "WXYZ-4821"));
    assert!(!painted_has(&h, "example.com"));
    assert!(painted_has(&h, "Not an address Archforge opens"));
    assert!(!ids(&h).iter().any(|i| i == "settings.agents.codex.copy-address"));
}

#[test]
fn a_device_code_that_is_not_letters_digits_and_hyphens_is_not_shown() {
    use crate::ai_agents::LoginEvent;
    let (mut h, _) = agents_harness(ThemeKind::Light, |f| {
        *f.login.lock().unwrap() =
            vec![LoginEvent::DeviceCode { code: "WXYZ\u{202E}4821".into(), url: "https://auth.openai.com/codex/device".into(), minutes: 15 }];
    });
    open_agents(&mut h);
    let at = rect_of(&h, "settings.agents.codex.device-code").center();
    click(&mut h, at);
    h.run();
    assert!(h.state().agents.login.is_none());
    assert!(painted_has(&h, "The sign-in gave a code that is not letters, digits and hyphens."));
    assert!(!painted_has(&h, "4821"));
}

#[test]
fn ai_agents_choices_a_later_version_wrote_fall_back_to_the_defaults_alone() {
    let mut state: UiState = serde_json::from_str(r#"{"user_name":"Ada","agents":{"default_agent":"Gemini","chat_uses_model":true}}"#).unwrap();
    state.sanitize();
    assert_eq!(state.user_name, "Ada", "the rest of the file is read");
    assert_eq!(state.agents, crate::ai_agents::AgentPrefs::default());
}

#[test]
fn starting_and_opening_other_pages_reads_no_key() {
    let (mut h, f) = agents_harness(ThemeKind::Light, |_| {});
    let keys = |f: &crate::ai_agents::fake::Fake| f.calls.keys.load(std::sync::atomic::Ordering::Relaxed);
    for _ in 0..5 {
        h.run();
    }
    assert_eq!(keys(&f), 0, "launching never touches the secure store");
    // Settings on Profile, About, Home's search: still no key read.
    let at = rect_of(&h, "home.header.account").center();
    click(&mut h, at);
    assert!(ids(&h).iter().any(|i| i == "settings.name"));
    let at = rect_of(&h, "settings.done").center();
    click(&mut h, at);
    h.state_mut().open_about();
    h.run();
    h.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.run();
    let at = rect_of(&h, "home.header.search").center();
    click(&mut h, at);
    assert_eq!(keys(&f), 0, "no page but AI Agents reads a key");
    // AI Agents reads them once; Look again reads them again.
    h.event(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE });
    h.run();
    open_agents(&mut h);
    assert_eq!(keys(&f), 1);
    crate::ai_agents::look(h.state_mut());
    h.run();
    assert_eq!(keys(&f), 2, "Look again reads the keys again");
}

#[test]
fn a_key_saved_while_the_keys_are_being_read_stays_when_the_read_answers() {
    let (mut h, f) = agents_harness(ThemeKind::Light, |f| {
        f.hold_keys = true;
        f.look.lock().unwrap().codex_key = crate::ai_agents::Key::Saved { last4: "9c1E".into() };
    });
    open_agents(&mut h);
    assert!(!h.state().agents.keys_known(), "the read is still running");
    // Saved while the read runs: the store answers the save at once.
    let at = rect_of(&h, "settings.agents.claude.key").center();
    click(&mut h, at);
    h.event(egui::Event::Text("sk-ant-api03-saved-meanwhile-4F2a".into()));
    h.run();
    let at = rect_of(&h, "settings.agents.claude.save").center();
    click(&mut h, at);
    h.run();
    let saved = crate::ai_agents::Key::Saved { last4: "4F2a".into() };
    assert_eq!(h.state().agents.look.claude_key, saved);
    // The read, begun before the save, answers with no Claude key: the saved one stays, and the
    // read's answer is taken for the other agent.
    f.release_keys();
    h.run();
    assert!(h.state().agents.keys_known());
    assert_eq!(h.state().agents.look.claude_key, saved, "the older read does not undo the save");
    assert_eq!(h.state().agents.look.codex_key, crate::ai_agents::Key::Saved { last4: "9c1E".into() });
    // A later read applies in full again.
    crate::ai_agents::look(h.state_mut());
    h.run();
    f.release_keys();
    h.run();
    assert_eq!(h.state().agents.look.claude_key, saved, "the store holds it now");
}

/// The workspace over a project with the fake agents host: its programs found, nothing else set up.
fn chat_workspace(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<crate::ai_agents::fake::Fake>) {
    use crate::ai_agents::{Look, Provider, fake};
    crate::i18n::set_language("en");
    let f = fake::Fake::default();
    *f.look.lock().unwrap() = Look { claude: fake::found(Provider::Claude), codex: fake::found(Provider::Codex), ..Look::default() };
    let f = std::sync::Arc::new(f);
    let mut shell = Shell::new(Services { ids: Some(test_ids()), agents: Some(f.clone()), ..Services::default() }, NOTATIONS, &SCREENS);
    shell.state.theme = theme;
    shell.open_project(crate::Opened { name: "Commerce platform".into(), path: "/tmp/Commerce platform.archforge".into(), ..Default::default() });
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_ui_state(
        |ui, shell: &mut Shell| {
            shell.logic(ui.ctx());
            shell.ui(ui);
        },
        shell,
    );
    h.run();
    (h, f)
}

/// The fill colours of the filled circles drawn in the last frame.
fn painted_dots(h: &Harness<'static, Shell>) -> Vec<egui::Color32> {
    /// Collects the fill of every filled circle under `s`.
    fn walk(s: &egui::Shape, out: &mut Vec<egui::Color32>) {
        match s {
            egui::Shape::Circle(c) => out.push(c.fill),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &h.output().shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

/// A conversation id: a lowercase UUID numbered `n`.
fn chat_id(n: u64) -> String {
    format!("00000000-0000-4000-8000-{n:012x}")
}

/// A Claude conversation `n` that may only read.
fn chat_start(n: u64, provider: crate::ai_agents::Provider) -> crate::chat::ChatStart {
    crate::chat::ChatStart {
        chat: chat_id(n),
        provider,
        model: "default".into(),
        origin: format!("Chat {n}"),
        allow_changes: false,
        resume: false,
        codex_sign_in: crate::ai_agents::CodexSignIn::ChatGpt,
    }
}

/// The status bar's Agent item: one state at a time, a real control that opens Settings on AI Agents by click or by Tab and Enter.
#[test]
fn the_status_bar_says_whether_chat_holds_agent_access_and_opens_ai_agents() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, f) = chat_workspace(theme);
        let p = crate::ux::palette(theme.is_dark());
        // Off: the purple dot and its words, one state at a time.
        assert!(painted_has(&h, "Agent access off") && !painted_has(&h, "Chat is using the model"), "{theme:?}");
        assert!(painted_dots(&h).contains(&egui::Color32::from(p.purple)), "{theme:?}: the purple dot");
        assert!(ids(&h).iter().any(|i| i == "workspace.status.agent"));
        // Held: the accent dot and "Chat is using the model".
        assert!(f.access(true));
        h.run();
        assert!(h.state().chats.holds_access());
        assert!(painted_has(&h, "Chat is using the model") && !painted_has(&h, "Agent access off"), "{theme:?}");
        assert!(
            painted_dots(&h).contains(&egui::Color32::from(p.accent)) && !painted_dots(&h).contains(&egui::Color32::from(p.purple)),
            "{theme:?}: the accent dot alone"
        );
        // A control: the pointer over it shows the hand and says where it leads.
        let at = rect_of(&h, "workspace.status.agent").center();
        h.event(egui::Event::PointerMoved(at));
        for _ in 0..90 {
            h.step();
        }
        assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::PointingHand, "{theme:?}");
        assert!(painted_has(&h, "AI Agents"), "{theme:?}: its tooltip");
        // A click opens Settings on AI Agents.
        click(&mut h, at);
        assert_eq!(h.state().settings.as_ref().map(|d| d.section), Some(crate::settings::Section::Agents), "{theme:?}");
        h.state_mut().settings = None;
        h.run();
        // Closed again: off. Reached with Tab, Enter opens Settings on AI Agents too.
        assert!(f.access(false));
        h.run();
        assert!(painted_has(&h, "Agent access off") && !painted_has(&h, "Chat is using the model"), "{theme:?}");
        let target = crate::widgets::egui_id("workspace.status.agent");
        let mut reached = false;
        for _ in 0..400 {
            key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
            if h.ctx.memory(|m| m.focused()) == Some(target) {
                reached = true;
                break;
            }
        }
        assert!(reached, "{theme:?}: Tab reaches the item");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(h.state().settings.as_ref().map(|d| d.section), Some(crate::settings::Section::Agents), "{theme:?}: Enter opens it");
    }
}

/// A conversation's id, duplicate and the 64 kept are checked in the shell; its events are kept in order within the cap, the end never dropped.
#[test]
fn a_conversation_is_checked_before_the_host_is_asked_and_its_events_are_kept_in_order() {
    use crate::ai_agents::Provider;
    use crate::chat::{ChatEvent, ChatRefusal, MAX_SESSIONS, MAX_WAITING};
    use std::sync::atomic::Ordering::Relaxed;
    let (mut h, f) = agents_harness(ThemeKind::Light, |_| {});
    assert_eq!(f.calls.connect.load(Relaxed), 1, "the window connected its events once");
    // A bad id is refused here, never asked of the host.
    for bad in ["", "Chat 1", "00000000-0000-4000-8000-00000000000G", "00000000-0000-4000-8000-0000000000001", "../../etc/passwd"] {
        let mut start = chat_start(1, Provider::Claude);
        start.chat = bad.into();
        assert_eq!(crate::chat::start(h.state_mut(), start), Err(ChatRefusal::BadId { bytes: bad.len() }), "{bad:?}");
        assert!(crate::chat::send(h.state(), bad, "hi".into()).is_err());
    }
    assert_eq!(f.calls.start.load(Relaxed), 0);
    // A Claude conversation: started once, a second start refused; Codex's account stays unread.
    crate::chat::start(h.state_mut(), chat_start(1, Provider::Claude)).unwrap();
    assert_eq!(f.calls.start.load(Relaxed), 1, "the host is asked");
    crate::chat::send(h.state(), &chat_id(1), "hi".into()).unwrap();
    assert_eq!(f.calls.send.load(Relaxed), 1, "and sent the message");
    assert_eq!(crate::chat::start(h.state_mut(), chat_start(1, Provider::Claude)), Err(ChatRefusal::AlreadyRunning));
    h.run();
    assert_eq!(f.calls.account.load(Relaxed), 0, "a Claude conversation does not read Codex's account");
    // A Codex conversation reads it, the first time only.
    crate::chat::start(h.state_mut(), chat_start(2, Provider::Codex)).unwrap();
    h.run();
    assert_eq!(f.calls.account.load(Relaxed), 1, "a Codex conversation reads Codex's account");
    // Events in order; past the cap the later ones are dropped and counted, but the end is kept.
    let one = chat_id(1);
    assert!(f.emit(&one, ChatEvent::Started { model: "sonnet".into(), access: false }));
    for n in 0..MAX_WAITING + 9 {
        f.emit(&one, ChatEvent::Text { delta: n.to_string() });
    }
    crate::chat::end(h.state(), &one).unwrap();
    h.run();
    assert!(!h.state().chats.running(&one));
    assert_eq!(h.state().chats.dropped(&one), 11, "ten past the cap, and one to make room for the end");
    let events = crate::chat::take_events(h.state_mut(), &one);
    assert_eq!(events.len(), MAX_WAITING);
    // The oldest gave its place to the end: the events from the first text on, in order.
    assert_eq!(events.first(), Some(&ChatEvent::Text { delta: "0".into() }));
    assert_eq!(events.get(MAX_WAITING - 2), Some(&ChatEvent::Text { delta: (MAX_WAITING - 2).to_string() }));
    assert_eq!(events.last(), Some(&ChatEvent::Ended { failure: None }), "the end is never dropped");
    assert!(crate::chat::take_events(h.state_mut(), &one).is_empty(), "an ended conversation, its events taken, is forgotten");
    // At most 64 kept: ended conversations whose events wait count, the 65th is refused.
    crate::chat::end(h.state(), &chat_id(2)).unwrap();
    h.run();
    for n in 3..=(MAX_SESSIONS as u64 + 1) {
        crate::chat::start(h.state_mut(), chat_start(n, Provider::Claude)).unwrap();
        crate::chat::end(h.state(), &chat_id(n)).unwrap();
        h.run();
    }
    assert_eq!(h.state().chats.kept(), MAX_SESSIONS);
    let refused = crate::chat::start(h.state_mut(), chat_start(100, Provider::Claude));
    assert_eq!(refused, Err(ChatRefusal::TooMany { most: MAX_SESSIONS }));
    // Taking one conversation's events makes room again.
    crate::chat::take_events(h.state_mut(), &chat_id(3));
    assert!(crate::chat::start(h.state_mut(), chat_start(100, Provider::Claude)).is_ok());
}

/// Waiting events are capped by their bytes in the shell and on the host's ticket.
#[test]
fn a_conversations_waiting_events_are_capped_by_their_bytes_too() {
    use crate::ai_agents::Provider;
    use crate::chat::{ChatEvent, MAX_WAITING_BYTES};
    let (mut h, f) = agents_harness(ThemeKind::Light, |_| {});
    crate::chat::start(h.state_mut(), chat_start(1, Provider::Claude)).unwrap();
    let one = chat_id(1);
    let big = || ChatEvent::Message { text: "x".repeat(MAX_WAITING_BYTES / 3 + 1) };
    for _ in 0..3 {
        f.emit(&one, big());
    }
    h.run();
    assert_eq!(h.state().chats.dropped(&one), 1, "the third would pass the bytes' cap");
    // The end still comes, making room by dropping the oldest: here a failure as long as a message.
    let failure = crate::chat::ChatFailure { kind: crate::chat::FailureKind::Other, detail: "y".repeat(MAX_WAITING_BYTES / 3), reset: None };
    let end = ChatEvent::Ended { failure: Some(failure) };
    f.emit(&one, end.clone());
    h.run();
    assert_eq!(h.state().chats.dropped(&one), 2, "the oldest message gave its place to the end");
    let events = crate::chat::take_events(h.state_mut(), &one);
    assert_eq!(events.len(), 2, "one message and the end");
    assert_eq!(events.last(), Some(&end));
    // The host's own count: places are taken by events and bytes, and given back as tickets drop.
    let waiting = std::sync::Arc::new(crate::chat::Waiting::default());
    let first = crate::chat::Ticket::take(&waiting, MAX_WAITING_BYTES / 2 + 1).unwrap();
    assert!(crate::chat::Ticket::take(&waiting, MAX_WAITING_BYTES / 2).is_none(), "past the bytes' cap");
    drop(first);
    assert!(crate::chat::Ticket::take(&waiting, MAX_WAITING_BYTES / 2).is_some(), "given back");
}

/// What the open project has selected.
fn selection(h: &Harness<'static, Shell>) -> Vec<archforge_mof::Obj> {
    h.state().session.as_ref().unwrap().view().selection.clone()
}

/// The kind of a diagram, the kind of the diagram shown and whether the open popover takes the
/// keys are asked of the kinds the shell registered, and the kind shown is the active tab's: the
/// sketch diagram is the sketch kind's, which kinds without it do not know; the second sketch's
/// popover, which the second sketch's kind walks with the keys, holds none in the shell, as that
/// kind is not the shell's. With no tab active, or no project open, no kind shows.
#[test]
fn the_kinds_asked_are_the_shells_and_the_kind_shown_is_the_active_tabs() {
    let (mut h, d, ..) = sketch_harness();
    let shell = h.state_mut();
    let id = archforge_mof::Uuid::parse(&d).unwrap();
    let store = shell.session.as_ref().unwrap().doc();
    let diagram = store.by_uuid(id).unwrap();
    assert_eq!(crate::kinds::kind_of(shell, store, diagram).map(|k| k.kind()), Some(SKETCH));
    let others: &[&'static dyn crate::kinds::DiagramKind] = &[&SECOND_SKETCH_KINDS[0], &SECOND_SKETCH_KINDS[1]];
    assert!(archforge_shell_api::kinds::kind_of(others, store, diagram).is_none(), "kinds without the sketch's know no sketch");
    assert_eq!(crate::kinds::active_kind(shell).map(|k| k.kind()), Some(SKETCH));
    assert!(archforge_shell_api::kinds::active_kind(others, &shell.canvas, shell.session.as_ref()).is_none());
    shell.canvas.popover = Some(SECOND_KINDS);
    assert!(!crate::kinds::popover_takes_keys(shell), "the second sketch is not the shell's");
    assert!(archforge_shell_api::kinds::popover_takes_keys(others, &shell.canvas), "the second sketch's kind walks them");
    shell.canvas.popover = None;
    assert_eq!(shell.canvas.tabs, vec![id]);
    shell.canvas.active = None;
    assert!(crate::kinds::active_kind(shell).is_none(), "the diagram is open in a tab, but no tab is active");
    shell.canvas.active = Some(id);
    shell.session = None;
    assert!(crate::kinds::active_kind(shell).is_none(), "no project open");
}

/// The zoom the user makes is the diagram's kind's to follow once (AR-788): zooms made before a
/// drawing add up, and once a diagram whose kind follows none has drawn, none is left over for the
/// next. In light and dark.
#[test]
fn a_zoom_made_is_the_kinds_to_follow_once() {
    let mut state = crate::canvas::State::default();
    crate::canvas::zoom_by(&mut state, 1.25, egui::Vec2::ZERO);
    crate::canvas::zoom_by(&mut state, 1.25, egui::Vec2::ZERO);
    assert!((state.zoom_intent - 1.5625).abs() < 1e-6, "two zooms in one frame: {}", state.zoom_intent);
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, ..) = sketch_harness();
        h.state_mut().state.theme = theme;
        h.run();
        let before = h.state().canvas.zoom;
        h.event(egui::Event::PointerMoved(rect_of(&h, "workspace.canvas").center()));
        h.event(egui::Event::Zoom(1.25));
        h.step();
        assert!(h.state().canvas.zoom > before, "{theme:?}: the canvas zoomed");
        assert_eq!(h.state().canvas.zoom_intent, 0.0, "{theme:?}: nothing left over for the next frame");
    }
}

/// A test-only notation over the fake notation's model that draws and places as the fake notation
/// does, but a click on Order's figure selects Order's package. The fake notation selects what was
/// clicked, so only a notation like this one shows that the canvas asks a node's notation what a click on it
/// selects, and draws a card selected with it (AR-784, ADR 0005).
struct PackageClicks;

impl archforge_diagram::Notation for PackageClicks {
    /// The fake notation's name, as the notation it stands in for.
    fn name(&self) -> &'static str {
        "Sketch"
    }
    /// The fake notation's metamodel.
    fn metamodel(&self) -> u16 {
        archforge_mof::toy::INDEX
    }
    /// The fake notation's diagram kinds.
    fn kinds(&self) -> &'static [&'static str] {
        archforge_notations::fake::Sketch.kinds()
    }
    /// As the fake notation answers.
    fn needs_element(&self, kind: &str) -> bool {
        archforge_notations::fake::Sketch.needs_element(kind)
    }
    /// As the fake notation answers.
    fn is_of(&self, kind: &str, class: archforge_mof::ClassId) -> bool {
        archforge_notations::fake::Sketch.is_of(kind, class)
    }
    /// As the fake notation answers, and a package too, so its own card can stand beside its
    /// classes'.
    fn shows(&self, kind: &str, class: archforge_mof::ClassId) -> bool {
        archforge_notations::fake::Sketch.shows(kind, class) || (kind == SKETCH && class == archforge_mof::toy::PACKAGE)
    }
    /// As the fake notation answers.
    fn may_show(&self, store: &archforge_mof::Store, diagram: archforge_mof::Obj, element: archforge_mof::Obj) -> bool {
        archforge_notations::fake::Sketch.may_show(store, diagram, element)
    }
    /// As the fake notation answers.
    fn connects(&self, kind: &str, class: archforge_mof::ClassId) -> bool {
        archforge_notations::fake::Sketch.connects(kind, class)
    }
    /// As the fake notation answers.
    fn ends(&self, store: &archforge_mof::Store, element: archforge_mof::Obj) -> Option<(archforge_mof::Obj, archforge_mof::Obj)> {
        archforge_notations::fake::Sketch.ends(store, element)
    }
    /// The fake notation's card, so the diagram draws as it does.
    fn card(
        &self,
        store: &archforge_mof::Store,
        element: archforge_mof::Obj,
        diagram: archforge_mof::Obj,
    ) -> Option<archforge_diagram::figure::Card> {
        archforge_notations::fake::Sketch.card(store, element, diagram)
    }
    /// The fake notation's line, so the diagram draws as it does.
    fn line(&self, store: &archforge_mof::Store, element: archforge_mof::Obj) -> Option<archforge_diagram::figure::Line> {
        archforge_notations::fake::Sketch.line(store, element)
    }
    /// A click on Order's figure selects Order's package; anything else selects itself.
    fn selects_as(&self, store: &archforge_mof::Store, element: archforge_mof::Obj) -> archforge_mof::Obj {
        if store.class_of(element).ok() == Some(archforge_mof::toy::CLASS) && archforge_core::name_of(store, element) == "Order" {
            store.container(element).ok().flatten().map_or(element, |(package, _)| package)
        } else {
            element
        }
    }
}

/// A project of [`PackageClicks`] alone.
static PACKAGE_CLICKS: &[&dyn archforge_diagram::Notation] = &[&PackageClicks];

/// The sample class diagram open in `theme`, its session over a view of [`PackageClicks`]: Order's
/// card's node, Order's package, and Customer with its card's node.
fn package_clicks_sample(theme: ThemeKind) -> (Harness<'static, Shell>, String, archforge_mof::Obj, archforge_mof::Obj, archforge_mof::Obj) {
    let mut h = workspace_harness(theme, egui::vec2(1600.0, 1000.0));
    let d = h.state_mut().cx(open_sample).unwrap();
    let (tables, junctions) = NOTATIONS.storage();
    let rows = archforge_mof::rows::snapshot(h.state().session.as_ref().unwrap().doc(), &tables, &junctions).unwrap();
    let next = std::sync::atomic::AtomicU64::new(1 << 40);
    let ids = Box::new(move || {
        let mut r = [0u8; 10];
        r[2..].copy_from_slice(&next.fetch_add(1, std::sync::atomic::Ordering::Relaxed).to_be_bytes());
        archforge_mof::Uuid::v7(1_761_670_800_000, r)
    });
    let mut st = NOTATIONS.store(ids).unwrap();
    archforge_mof::rows::restore(&mut st, &rows, &tables, &junctions).unwrap();
    let view = archforge_diagram::ProjectView::new(PACKAGE_CLICKS);
    let session = archforge_engine::Session::with_view(NOTATIONS.registry().unwrap(), st, 1, archforge_engine::Options::default(), view);
    h.state_mut().session = Some(session);
    crate::canvas::open(h.state_mut(), d);
    h.run();
    let st = h.state().session.as_ref().unwrap().doc();
    let class =
        |name: &str| st.alive().find(|o| st.class_of(*o).ok() == Some(archforge_mof::toy::CLASS) && archforge_core::name_of(st, *o) == name).unwrap();
    let (order, customer) = (class("Order"), class("Customer"));
    let node = st.uuid(archforge_diagram::views_of(st, order).2[0]).unwrap().to_string();
    let package = st.container(order).unwrap().unwrap().0;
    let customer_node = archforge_diagram::views_of(st, customer).2[0];
    (h, node, package, customer, customer_node)
}

/// The centre of the card `node` on the canvas.
fn card(h: &Harness<'static, Shell>, node: &str) -> egui::Pos2 {
    rect_of(h, &format!("workspace.canvas.node.{node}")).center()
}

/// Whether the last frame stroked a rectangle in the accent colour of `theme` round the centre of
/// `card`, as a selected card is drawn.
fn card_ringed(h: &Harness<'static, Shell>, theme: ThemeKind, card: egui::Rect) -> bool {
    let accent = crate::ux::palette(theme == ThemeKind::Dark).accent;
    rect_shapes(h).iter().any(|(r, _, stroke)| *stroke == egui::Color32::from(accent) && r.contains(card.center()) && r.width() >= card.width() - 4.0)
}

/// What a click on a node selects is its notation's answer, on every way a node is selected (a
/// click with Select, a drag that starts a move, Select all, a marquee, and a click with the
/// Note, Link, Class and Inherit tools); while that answer is selected the card draws
/// selected, shows its resize handles and moves with the rest of the selection, the first drag
/// that makes the selection included: through a notation whose click on Order selects its package.
/// In light and dark.
#[test]
fn every_way_of_selecting_a_node_asks_its_notation_what_it_selects() {
    for theme in [ThemeKind::Light, ThemeKind::Dark] {
        let (mut h, node, package, customer, customer_node) = package_clicks_sample(theme);
        let clear = |h: &mut Harness<'static, Shell>| {
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [] })).unwrap();
            h.state_mut().canvas.tool = crate::canvas::Tool::Select;
            h.run();
        };
        let at = card(&h, &node);
        assert!(!card_ringed(&h, theme, rect_of(&h, &format!("workspace.canvas.node.{node}"))), "{theme:?}: nothing selected yet");
        click(&mut h, at);
        assert_eq!(selection(&h), vec![package], "{theme:?}: a click with Select");
        h.run();
        assert!(card_ringed(&h, theme, rect_of(&h, &format!("workspace.canvas.node.{node}"))), "{theme:?}: the card draws selected with its package");
        // Its package selected, the card is the one selected node: its corner handle resizes it.
        let order_node = h.state().session.as_ref().unwrap().doc().by_uuid(archforge_mof::Uuid::parse(&node).unwrap()).unwrap();
        let bounds =
            |h: &Harness<'static, Shell>, n: archforge_mof::Obj| archforge_diagram::grid::bounds_of(h.state().session.as_ref().unwrap().doc(), n);
        let before = bounds(&h, order_node);
        let corner = rect_of(&h, &format!("workspace.canvas.node.{node}")).expand(4.0).right_bottom();
        drag(&mut h, corner, corner + egui::vec2(40.0, 30.0));
        assert!(bounds(&h, order_node).width > before.width, "{theme:?}: resized by its handle");
        // With Customer selected beside the package, a drag on Customer's card moves Order's too,
        // and keeps the selection.
        let customer_id = h.state().session.as_ref().unwrap().doc().uuid(customer).unwrap().to_string();
        let package_id = h.state().session.as_ref().unwrap().doc().uuid(package).unwrap().to_string();
        h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [package_id, customer_id] })).unwrap();
        h.run();
        let was = bounds(&h, order_node);
        let customer_card = h.state().session.as_ref().unwrap().doc().uuid(customer_node).unwrap().to_string();
        let at = card(&h, &customer_card);
        drag(&mut h, at, at + egui::vec2(30.0, 20.0));
        assert!(bounds(&h, order_node).x > was.x, "{theme:?}: Order moves with Customer");
        assert_eq!(selection(&h), vec![package, customer], "{theme:?}: the selection kept");
        clear(&mut h);
        let at = card(&h, &node);
        drag(&mut h, at, at + egui::vec2(20.0, 10.0));
        assert_eq!(selection(&h), vec![package], "{theme:?}: a drag that moves the card");
        clear(&mut h);
        let d = h.state().session.as_ref().unwrap().doc().by_uuid(h.state().canvas.active.unwrap()).unwrap();
        crate::editing::select_all(h.state_mut(), d);
        let order = h.state().session.as_ref().unwrap().doc().get(order_node, archforge_diagram::node::ELEMENT).unwrap().as_ref().unwrap();
        assert!(selection(&h).contains(&package), "{theme:?}: Select all takes the package");
        assert!(!selection(&h).contains(&order), "{theme:?}: and not Order");
        clear(&mut h);
        let r = rect_of(&h, &format!("workspace.canvas.node.{node}"));
        drag(&mut h, r.left_top() - egui::vec2(20.0, 20.0), r.center());
        assert_eq!(selection(&h), vec![package], "{theme:?}: a marquee over the card");
        for tool in
            [crate::canvas::Tool::Note, crate::canvas::Tool::Link, crate::canvas::Tool::Place("class"), crate::canvas::Tool::Connect("inherit")]
        {
            clear(&mut h);
            h.state_mut().canvas.tool = tool;
            let at = card(&h, &node);
            click(&mut h, at);
            assert_eq!(selection(&h), vec![package], "{theme:?}: a click with {tool:?}");
        }
        // With the package's own card on the diagram, the first drag on Order, unselected, moves
        // both cards, as the selection it makes takes both in, and so does the first drag on the
        // package's card; a drag on Customer moves Customer alone.
        clear(&mut h);
        let (d_id, package_id) = {
            let st = h.state().session.as_ref().unwrap().doc();
            (h.state().canvas.active.unwrap().to_string(), st.uuid(package).unwrap().to_string())
        };
        let placed = h
            .state_mut()
            .session
            .as_mut()
            .unwrap()
            .execute("diagram.place", &serde_json::json!({ "diagram": d_id, "element": package_id, "x": 1400, "y": 700 }))
            .unwrap();
        let package_node =
            h.state().session.as_ref().unwrap().doc().by_uuid(archforge_mof::Uuid::parse(placed.value["id"].as_str().unwrap()).unwrap()).unwrap();
        h.run();
        let (order_was, package_was) = (bounds(&h, order_node), bounds(&h, package_node));
        let at = card(&h, &node);
        drag(&mut h, at, at + egui::vec2(30.0, 20.0));
        assert_eq!(selection(&h), vec![package], "{theme:?}: the drag selects the package");
        assert!(bounds(&h, order_node).x > order_was.x, "{theme:?}: Order moves");
        assert!(bounds(&h, package_node).x > package_was.x, "{theme:?}: and the package's card with it, on the first drag");
        // The other way round: the first drag on the package's card, unselected, moves Order's too,
        // as selecting the package takes Order's card in.
        clear(&mut h);
        let (order_was, package_was) = (bounds(&h, order_node), bounds(&h, package_node));
        let package_card = h.state().session.as_ref().unwrap().doc().uuid(package_node).unwrap().to_string();
        // The package's card brought into view, its centre at the canvas's.
        let canvas = rect_of(&h, "workspace.canvas");
        let (zoom, pan) = (h.state().canvas.zoom, h.state().canvas.pan);
        let centre = egui::pos2((package_was.x + package_was.width / 2.0) as f32, (package_was.y + package_was.height / 2.0) as f32);
        h.state_mut().canvas.pan = canvas.center() - canvas.min - centre.to_vec2() * zoom;
        h.run();
        let at = card(&h, &package_card);
        drag(&mut h, at, at + egui::vec2(30.0, 20.0));
        assert_eq!(selection(&h), vec![package], "{theme:?}: the drag on the package's card selects it");
        assert!(bounds(&h, package_node).x > package_was.x, "{theme:?}: the package's card moves");
        assert!(bounds(&h, order_node).x > order_was.x, "{theme:?}: and Order's with it, on the first drag");
        h.state_mut().canvas.pan = pan;
        clear(&mut h);
        let package_was = bounds(&h, package_node);
        let customer_card = h.state().session.as_ref().unwrap().doc().uuid(customer_node).unwrap().to_string();
        let at = card(&h, &customer_card);
        drag(&mut h, at, at + egui::vec2(30.0, 20.0));
        assert_eq!(bounds(&h, package_node).x, package_was.x, "{theme:?}: Customer's drag leaves the package's card");
    }
}
