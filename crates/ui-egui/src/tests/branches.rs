//! The Branches section of the Versions panel (`crate::versions`).

use egui_kittest::kittest::Queryable;

use super::*;
use crate::close_ask::Ask;

const MAIN: &str = "workspace.versions.branch.row.main";
const OTHER: &str = "workspace.versions.branch.row.try-cancellations";
const MORE: &str = "workspace.versions.branch.more.try-cancellations";
const MENU: &str = "workspace.versions.branch.menu.try-cancellations.0";

/// `main`, in use and with a version, and when `other`, a branch with unsaved work and a version
/// of its own; `main` is marked unsaved, which only its session may decide.
fn listed(other: bool) -> Vec<crate::BranchEntry> {
    let main =
        crate::BranchEntry { name: "main".into(), shown: "main".into(), current: true, unsaved: true, latest: Some("2026-11-08T14:02:00Z".into()) };
    let try_it = crate::BranchEntry {
        name: "try-cancellations".into(),
        shown: "Try cancellations".into(),
        current: false,
        latest: Some("2026-11-09T15:30:00Z".into()),
        ..main.clone()
    };
    if other { vec![main, try_it] } else { vec![main] }
}

/// `listed(true)` with `names`, each another branch with nothing unsaved and no version, in order.
fn with(names: impl IntoIterator<Item = String>) -> Vec<crate::BranchEntry> {
    let mut all = listed(true);
    all.extend(names.into_iter().map(|n| crate::BranchEntry { shown: n.clone(), name: n, current: false, unsaved: false, latest: None }));
    all.sort_by(|a, b| a.name.cmp(&b.name));
    all
}

/// The panel on `main` with its versions read, then the branches answered with `answer`.
fn branches_read(theme: ThemeKind, answer: Result<Vec<crate::BranchEntry>, String>) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = versions_panel(theme);
    *host.branch_now.lock().unwrap() = Some("main".to_string());
    *host.branch_list.lock().unwrap() = Some(answer);
    assert_eq!(*host.branch_reads.lock().unwrap(), 0, "not before the versions");
    versions_answer(&mut h, &host, Ok(two_versions()));
    assert_eq!(*host.branch_reads.lock().unwrap(), 1, "the branches were asked for");
    (h, host)
}

/// When the version saved at `date` was, as a row says it.
fn when(h: &Harness<'static, Shell>, date: &str) -> String {
    crate::versions::when(date, crate::chat_panel::clock(h.state()))
}

/// With only `main` the section is folded, and opens by hand until another project shows; with two,
/// or when the read failed, it is open; a list cut short says so.
#[test]
fn branches_fold_with_only_main() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(false)));
        assert!(!has_id(&h, MAIN) && !painted_has(&h, "Branches can't be merged yet."), "{theme:?}: folded");
        click_id(&mut h, "workspace.versions.branches");
        h.run_steps(2);
        assert!(has_id(&h, MAIN) && painted_has(&h, "Branches can't be merged yet."), "{theme:?}: opened by hand");
        h.state_mut().project.as_mut().unwrap().path = "/tmp/Other.archforge".into();
        versions_answer(&mut h, &host, Ok(two_versions()));
        assert!(!has_id(&h, MAIN), "{theme:?}: folded again in another project");
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        assert!(has_id(&h, MAIN) && has_id(&h, OTHER) && painted_has(&h, "Branches can't be merged yet."), "{theme:?}");
        assert!(!painted_has(&h, "Only the first"), "{theme:?}");
        (*host.branch_cut.lock().unwrap(), h.state_mut().versions.stale) = (true, true);
        versions_answer(&mut h, &host, Ok(two_versions()));
        let cut = "Only the first 2 branches are listed.";
        assert!(painted_has(&h, cut) && h.query_by_label_contains(cut).is_some(), "{theme:?}: a list cut short says so");
        let (h, _host) = branches_read(theme, Err("the server stopped".into()));
        assert!(has_id(&h, "workspace.versions.branches.failed"), "{theme:?}: a failed first read opens it to say so");
    });
}

/// A row says its name, Current for the branch in use, unsaved work (the session's, for the branch
/// in use) and when its latest version was saved; a failed read keeps the rows and says so, Retry
/// reads again and a row that then holds another branch closes its menu; the mark moves with a
/// switch before the next read.
#[test]
fn a_row_says_what_its_branch_holds() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        let (main, other) = (when(&h, "2026-11-08T14:02:00Z"), when(&h, "2026-11-09T15:30:00Z"));
        assert!(h.query_by_label(&format!("main, Current, {main}")).is_some(), "{theme:?}: its session is saved");
        assert!(h.query_by_label(&format!("Try cancellations, Unsaved changes, {other}")).is_some(), "{theme:?}");
        assert!(painted_has(&h, "Try cancellations") && painted_has(&h, &other), "{:?}", painted(&h));
        add_schema(&mut h, "crm");
        assert!(h.query_by_label(&format!("main, Current, Unsaved changes, {main}")).is_some(), "{theme:?}: as the session changes");
        (*host.branch_list.lock().unwrap(), h.state_mut().versions.stale) = (Some(Err("the server stopped".into())), true);
        versions_answer(&mut h, &host, Ok(two_versions()));
        assert_eq!(*host.branch_reads.lock().unwrap(), 2, "read again after the versions");
        assert!(has_id(&h, OTHER) && has_id(&h, "workspace.versions.branches.failed"), "{theme:?}: the rows stay");
        assert!(rect_of(&h, MAIN).top() >= rect_of(&h, "workspace.versions.branches.failed").bottom(), "{theme:?}: under the banner");
        assert!(painted_has(&h, "The branches couldn't be read."), "{theme:?}");
        *host.branch_list.lock().unwrap() = Some(Ok(with(["other".to_string()])));
        click_id(&mut h, "workspace.versions.branches.retry");
        h.run_steps(2);
        assert!(has_id(&h, "workspace.versions.branch.row.other") && !has_id(&h, "workspace.versions.branches.failed"), "{theme:?}: Retry");
        click_id(&mut h, "workspace.versions.branch.more.other");
        (*host.branch_list.lock().unwrap(), h.state_mut().versions.stale) = (Some(Ok(with(["new".to_string()]))), true);
        versions_answer(&mut h, &host, Ok(two_versions()));
        assert!(!ids(&h).iter().any(|i| i.starts_with("workspace.versions.branch.menu.")), "{theme:?}: its row moved");
        *host.branch_now.lock().unwrap() = Some("TRY-cancellations".into());
        h.run_steps(1);
        assert!(h.query_by_label(&format!("main, Unsaved changes, {main}")).is_some(), "{theme:?}: the mark follows a switch");
        assert!(h.query_by_label(&format!("Try cancellations, Current, Unsaved changes, {other}")).is_some(), "{theme:?}");
    });
}

/// A click, Enter or the menu's Switch on another branch asks to switch to it; `main` in use has
/// no menu, and a click on it does nothing; ↑ and ↓ move between rows, out of view too.
#[test]
fn a_row_switches_to_its_branch() {
    in_both_appearances(|theme| {
        let switched = |h: &mut Harness<'static, Shell>, host: &SavingHost| {
            h.run_steps(4);
            host.switches.lock().unwrap().iter().map(|s| s.branch.clone()).collect::<Vec<_>>()
        };
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        assert!(!has_id(&h, "workspace.versions.branch.more.main") && has_id(&h, MORE), "{theme:?}");
        click_id(&mut h, MAIN);
        assert!(switched(&mut h, &host).is_empty() && !h.state().is_held(), "{theme:?}");
        click_id(&mut h, OTHER);
        assert_eq!(switched(&mut h, &host), ["try-cancellations"], "{theme:?}: a click");

        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(OTHER)));
        h.run_steps(1);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(switched(&mut h, &host), ["try-cancellations"], "{theme:?}: Enter");

        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        click_id(&mut h, MORE);
        h.run_steps(1);
        click_id(&mut h, MENU);
        assert_eq!(switched(&mut h, &host), ["try-cancellations"], "{theme:?}: the menu");

        let (mut h, host) = branches_read(theme, Ok(with((0..20).map(|i| format!("b{i:02}")))));
        h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id("workspace.versions.branch.row.b00")));
        h.run_steps(1);
        (0..12).for_each(|_| key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE));
        key(&mut h, egui::Key::ArrowUp, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(switched(&mut h, &host), ["b11"], "{theme:?}: the arrow keys reach a row out of view");
    });
}

/// While a row's menu is open the arrows are the menu's, with the row focused too, by the pointer
/// and by the keys: ↓ and Enter pick its Switch, and the focus stays on the row.
#[test]
fn a_row_s_open_menu_keeps_the_arrows() {
    let (row, more) = ("workspace.versions.branch.row.b00", "workspace.versions.branch.more.b00");
    in_both_appearances(|theme| {
        let switched = |h: &mut Harness<'static, Shell>, host: &SavingHost| {
            h.run_steps(4);
            host.switches.lock().unwrap().iter().map(|s| s.branch.clone()).collect::<Vec<_>>()
        };
        let focus = |h: &mut Harness<'static, Shell>, id: &str| {
            h.ctx.memory_mut(|m| m.request_focus(crate::widgets::egui_id(id)));
            h.run_steps(1);
        };
        let (mut h, host) = branches_read(theme, Ok(with((0..20).map(|i| format!("b{i:02}")))));
        click_id(&mut h, more);
        focus(&mut h, row);
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        assert!(h.ctx.memory(|m| m.has_focus(crate::widgets::egui_id(row))), "{theme:?}: the row kept the focus");
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(switched(&mut h, &host), ["b00"], "{theme:?}: by the pointer, the menu's Switch");

        let (mut h, host) = branches_read(theme, Ok(with((0..20).map(|i| format!("b{i:02}")))));
        focus(&mut h, more);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(h.state().panels.menu.is_some(), "{theme:?}: Enter opened the menu");
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        assert_eq!(switched(&mut h, &host), ["b00"], "{theme:?}: by the keys, the menu's Switch");
    });
}

/// A branch whose name holds a dot has controls of its own, not another branch's.
#[test]
fn every_branch_control_has_an_id_of_its_own() {
    in_both_appearances(|theme| {
        let (h, _host) = branches_read(theme, Ok(with(["topic", "topic.more", "topic.more.topic"].map(String::from))));
        let all = ids(&h).into_iter().filter(|i| i.starts_with("workspace.versions.branch.")).collect::<Vec<_>>();
        let unique = all.iter().collect::<std::collections::BTreeSet<_>>();
        assert!(all.len() == 9 && unique.len() == all.len(), "{theme:?}: {all:?}");
    });
}

/// In the shortest window, many branches or many changes leave History's header and first row in
/// the column and a branch row in reach, by the keys too under a failed read's banner, which scrolls
/// with the rows; a taller one keeps two History rows, and shows the filter only with a change under
/// it.
#[test]
fn history_keeps_its_room_under_many_branches_or_changes() {
    let in_column = |h: &mut Harness<'static, Shell>, height: f32, row: &str| {
        h.set_size(egui::vec2(1600.0, height));
        h.run_steps(2);
        let id = format!("workspace.versions.row.{row}");
        has_id(h, &id) && rect_of(h, &id).bottom() <= height - 40.0
    };
    let fail = |h: &mut Harness<'static, Shell>, host: &SavingHost| {
        (*host.branch_list.lock().unwrap(), h.state_mut().versions.stale) = (Some(Err("the server stopped".into())), true);
        versions_answer(h, host, Ok(two_versions()));
    };
    in_both_appearances(|theme| {
        let many = || Ok(with((0..20).map(|i| format!("b{i:02}"))));
        let (mut h, host) = branches_read(theme, many());
        assert!(in_column(&mut h, 400.0, "aa"), "{theme:?}: under many branches");
        click_id(&mut h, "workspace.versions.branch.row.b00");
        h.run_steps(4);
        assert!(host.switches.lock().unwrap().iter().any(|s| s.branch == "b00"), "{theme:?}: a row in reach");
        let (mut h, host) = branches_read(theme, many());
        fail(&mut h, &host);
        assert!(in_column(&mut h, 400.0, "aa"), "{theme:?}: and a failed read");
        // By the keys alone: from nothing focused, Tab until Retry, then ↓.
        let retry = crate::widgets::egui_id("workspace.versions.branches.retry");
        h.ctx.memory_mut(|m| m.focused().map(|id| m.surrender_focus(id)));
        for _ in 0..80 {
            if h.ctx.memory(|m| m.has_focus(retry)) {
                break;
            }
            key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        }
        assert!(h.ctx.memory(|m| m.has_focus(retry)), "{theme:?}: Tab reaches Retry");
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(4);
        assert!(host.switches.lock().unwrap().iter().any(|s| s.branch == "b00"), "{theme:?}: ↓ from Retry reaches a row");
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        (0..30).for_each(|i| add_schema(&mut h, &format!("s{i}")));
        compared(&mut h);
        assert!(h.state().versions.working == 30 && in_column(&mut h, 400.0, "aa"), "{theme:?}: under many changes");
        assert!(in_column(&mut h, 640.0, "aa") && !has_id(&h, "workspace.versions.filter"), "{theme:?}: no filter over nothing");
        let listing = |h: &Harness<'static, Shell>| ids(h).iter().any(|i| i.starts_with("workspace.versions.change."));
        assert!(in_column(&mut h, 700.0, "bb") && listing(&h), "{theme:?}: two History rows under the changes");
        fail(&mut h, &host);
        assert!(in_column(&mut h, 400.0, "aa"), "{theme:?}: under a failed read and many changes");
    });
}

/// A read of the branches on its way is not asked for again, and the one wanted meanwhile follows.
#[test]
fn a_branch_read_waits_for_the_one_on_its_way() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        *host.branch_held.lock().unwrap() = Some(Vec::new());
        for _ in 0..2 {
            h.state_mut().versions.stale = true;
            versions_answer(&mut h, &host, Ok(two_versions()));
        }
        assert_eq!(*host.branch_reads.lock().unwrap(), 2, "{theme:?}: one read on its way");
        let held = host.branch_held.lock().unwrap().take().unwrap();
        held[0].send(Ok((listed(true), false))).unwrap();
        h.run_steps(3);
        assert_eq!(*host.branch_reads.lock().unwrap(), 3, "{theme:?}: then the one wanted meanwhile");
    });
}

/// In the shortest window, with the branches and the versions both failing to read, folding
/// Branches and Working changes brings History's error and Retry whole into the column, clear of
/// everything else, and Tab reaches Retry, which reads the versions again.
#[test]
fn history_s_failure_is_in_reach_once_the_sections_above_fold() {
    let (failed, retry) = ("workspace.versions.failed", "workspace.versions.retry");
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(with((0..20).map(|i| format!("b{i:02}")))));
        h.set_size(egui::vec2(1600.0, 400.0));
        (*host.branch_list.lock().unwrap(), h.state_mut().versions.stale) = (Some(Err("down".into())), true);
        versions_answer(&mut h, &host, Ok(two_versions()));
        h.state_mut().versions.stale = true;
        versions_answer(&mut h, &host, Err("the server stopped".into()));
        click_id(&mut h, "workspace.versions.branches");
        click_id(&mut h, "workspace.versions.working");
        h.run_steps(2);
        let banner = rect_of(&h, failed);
        let column = rect_of(&h, "workspace.versions.history").left_top().y..=400.0 - 40.0;
        assert!(column.contains(&banner.top()) && column.contains(&banner.bottom()), "{theme:?}: {banner:?} in the column");
        let others = h.state().widgets().iter().filter(|w| ![failed, retry].contains(&w.id.as_str()) && w.id.starts_with("workspace.versions."));
        let clear = others.map(|w| egui::Rect::from_min_max(egui::pos2(w.rect[0], w.rect[1]), egui::pos2(w.rect[2], w.rect[3])));
        assert!(clear.clone().all(|o| !o.intersects(banner.shrink(1.0))), "{theme:?}: {:?}", clear.collect::<Vec<_>>());
        h.ctx.memory_mut(|m| m.focused().map(|id| m.surrender_focus(id)));
        let id = crate::widgets::egui_id(retry);
        for _ in 0..80 {
            if h.ctx.memory(|m| m.has_focus(id)) {
                break;
            }
            key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        }
        let asked = host.version_asks.lock().unwrap().len();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(2);
        assert_eq!(host.version_asks.lock().unwrap().len(), asked + 1, "{theme:?}: Tab and Enter on Retry read again");
    });
}

/// In the shortest window after a good read, with the branch in use first and the next row out of
/// view, Tab alone reaches the list on the current row, where Enter switches to nothing, and ↓ and
/// Enter switch to the next branch.
#[test]
fn the_keys_reach_the_list_through_the_current_row() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(with((0..20).map(|i| format!("n{i:02}")))));
        h.set_size(egui::vec2(1600.0, 400.0));
        h.run_steps(2);
        assert!(!has_id(&h, "workspace.versions.branch.row.n00"), "{theme:?}: only the current row in view");
        let main = crate::widgets::egui_id(MAIN);
        h.ctx.memory_mut(|m| m.focused().map(|id| m.surrender_focus(id)));
        for _ in 0..80 {
            if h.ctx.memory(|m| m.has_focus(main)) {
                break;
            }
            key(&mut h, egui::Key::Tab, egui::Modifiers::NONE);
        }
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(4);
        assert!(h.ctx.memory(|m| m.has_focus(main)) && host.switches.lock().unwrap().is_empty(), "{theme:?}: Tab reaches it");
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(4);
        assert!(host.switches.lock().unwrap().iter().any(|s| s.branch == "n00"), "{theme:?}: ↓ and Enter switch to the next");
    });
}

/// The width of `text` where it is drawn starting inside `area`.
fn drawn_in(h: &Harness<'static, Shell>, area: egui::Rect, text: &str) -> Option<f32> {
    text_boxes(h).into_iter().find(|(t, b)| t == text && area.contains(b.min)).map(|(_, b)| b.width())
}

/// Opens the chip's list with ⇧⌘H, or ⇧⌘B for the branches, and answers its read: more to come.
fn chip_list(h: &mut Harness<'static, Shell>, host: &SavingHost, branches: bool) {
    key(h, if branches { egui::Key::B } else { egui::Key::H }, egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT));
    let page = crate::VersionsPage { more: true, ..two_versions() };
    host.version_reads.lock().unwrap().drain(..).for_each(|read| drop(read.send(Ok(page.clone()))));
    h.run_steps(3);
}

/// With one branch the chip, its list and the status bar never say the word; with two the chip has
/// the branch's icon and name, its list a branch row first, and the status bar "On {branch}", all
/// following a switch at once and only of the project shown.
#[test]
fn the_chip_and_the_status_bar_name_the_branch_once_there_are_two() {
    let on_branch = |h: &Harness<'static, Shell>| painted(h).iter().any(|t| t.starts_with("On "));
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(false)));
        assert!(h.query_by_label("Current work").is_some() && !on_branch(&h), "{theme:?}: one branch");
        assert!(!crate::actions::available(h.state(), crate::actions::BRANCHES), "{theme:?}: no branch list");
        chip_list(&mut h, &host, false);
        assert!(h.query_by_label_contains("Current work, ").is_some_and(|n| n.rect() == rect_of(&h, "workspace.header.version.row.0")));
        let (mut h, host) = branches_read(theme, Ok(listed(false)));
        *host.branch_held.lock().unwrap() = Some(Vec::new());
        h.state_mut().versions.stale = true;
        versions_answer(&mut h, &host, Ok(two_versions()));
        host.branch_held.lock().unwrap().take().unwrap()[0].send(Ok((listed(true), false))).unwrap();
        h.step();
        let delay = h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(delay.is_zero() && h.query_by_label("Current work").is_some(), "{theme:?}: drawn before, it asks for a frame");
        h.step();
        click_id(&mut h, MORE);
        *host.branch_held.lock().unwrap() = Some(Vec::new());
        h.state_mut().versions.stale = true;
        versions_answer(&mut h, &host, Ok(two_versions()));
        host.branch_held.lock().unwrap().take().unwrap()[0].send(Ok((listed(true), true))).unwrap();
        h.step();
        let delay = h.output().viewport_output[&egui::ViewportId::ROOT].repaint_delay;
        assert!(delay.is_zero() && has_id(&h, MENU), "{theme:?}: only the limit changed: a frame, and the menu stays");
        let chip = rect_of(&h, "workspace.header.version");
        assert!(h.query_by_label("main · Current work").is_some() && drawn_in(&h, chip, "main").is_some(), "{theme:?}");
        assert!(painted(&h).iter().any(|t| t.starts_with("On main · ")), "{theme:?}: {:?}", painted(&h));
        *host.branch_now.lock().unwrap() = Some("TRY-cancellations".into());
        h.run_steps(2);
        assert!(h.query_by_label("Try cancellations · Current work").is_some() && painted_has(&h, "On Try cancellations · Saved"), "{theme:?}");
        chip_list(&mut h, &host, false);
        assert!(h.query_by_label("Try cancellations, Branches").is_some(), "{theme:?}: the branch row first");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        h.state_mut().view_version("aa").unwrap();
        h.run_steps(3);
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        let older = rect_of(&h, "workspace.header.version.older");
        let before = egui::Rect::from_min_max(older.left_top() - egui::vec2(200.0, 8.0), older.left_bottom());
        let label = h.query_by_label("Try cancellations · Viewing a version").is_some();
        assert!(label && drawn_in(&h, before, "Try cancellations").is_some(), "{theme:?}: viewing, before the steps");
        h.state_mut().versions.about.as_mut().unwrap().0 = "/tmp/Other.archforge".into();
        assert_eq!(crate::version_chip::branch_name(h.state()), None, "{theme:?}: a list read of another project");
    });
}

/// The branch row, or ⇧⌘B, lists the branches with when each was last saved; Enter on another
/// switches to it as a panel row does and closes the list, on the one in use only closes, and a
/// refused switch keeps the list and says why; typing narrows the branches.
#[test]
fn the_chip_s_list_switches_branch() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        chip_list(&mut h, &host, false);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(2);
        let other = |h: &Harness<'static, Shell>| format!("Try cancellations, {}", when(h, "2026-11-09T15:30:00Z"));
        assert!(h.query_by_label("main, Current").is_some() && h.query_by_label(&other(&h)).is_some(), "{theme:?}: the branches");
        assert!(painted_has(&h, "Filter branches…") && !has_id(&h, "workspace.header.version.row.2"), "{theme:?}: and no version");
        h.state_mut().session.as_mut().unwrap().begin_interaction("er.schema.add").unwrap();
        key(&mut h, egui::Key::ArrowDown, egui::Modifiers::NONE);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(4);
        assert!(host.switches.lock().unwrap().is_empty() && has_id(&h, "workspace.header.version.row.1"), "{theme:?}: refused, it stays");
        let editing = crate::version_chip::refused_words(h.state(), crate::hold::HoldRefused::Editing);
        assert_eq!(h.state().canvas.notice, editing, "{theme:?}: said why");
        h.state_mut().session.as_mut().unwrap().cancel().unwrap();
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(4);
        let switched: Vec<_> = host.switches.lock().unwrap().iter().map(|s| s.branch.clone()).collect();
        assert!(switched == ["try-cancellations"] && !has_id(&h, "workspace.header.version.row.0"), "{theme:?}: {switched:?}");

        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        chip_list(&mut h, &host, true);
        let reads = host.version_reads.lock().unwrap().len();
        assert!(h.query_by_label("main, Current").is_some() && reads == 0, "{theme:?}: ⇧⌘B, reading no more versions");
        h.state_mut().versions.branches_cut = true;
        h.run_steps(1);
        let list = rect_of(&h, "workspace.header.version.row.0").expand2(egui::vec2(10.0, 120.0));
        let said = h.query_by_label("Only the first 2 branches are listed.").is_some()
            && list.contains(rect_of(&h, "workspace.header.version.cut").center());
        assert!(said && drawn_in(&h, list, "Only the first 2 branches are listed.").is_some(), "{theme:?}: a list cut short says so");
        h.event(egui::Event::Text("try".into()));
        h.run_steps(2);
        assert!(h.query_by_label("main, Current").is_none() && h.query_by_label(&other(&h)).is_some(), "{theme:?}: narrowed");
        (0..3).for_each(|_| key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE));
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        h.run_steps(4);
        assert!(host.switches.lock().unwrap().is_empty() && !has_id(&h, "workspace.header.version.row.0"), "{theme:?}: the one in use");
    });
}

/// As the window narrows the branch's name truncates first, then the label goes, then the chip is
/// one icon, then search hides; each in turn.
#[test]
fn the_branch_chip_narrows_in_the_design_s_order() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        *host.branch_now.lock().unwrap() = Some("try-cancellations".into());
        h.state_mut().state.explorer_collapsed = true;
        let mut seen = Vec::new();
        h.run_steps(2);
        let whole = drawn_in(&h, rect_of(&h, "workspace.header.version"), "Try cancellations").unwrap();
        for width in (640..=1600).rev().step_by(8) {
            h.set_size(egui::vec2(width as f32, 860.0));
            h.run_steps(2);
            let chip = rect_of(&h, "workspace.header.version");
            let name = drawn_in(&h, chip, "Try cancellations");
            let full = name.is_some_and(|w| w >= whole - 0.5);
            let stage = match (name, drawn_in(&h, chip, "Current work").is_some()) {
                _ if !has_id(&h, "workspace.header.search") => 0,
                (None, _) => 1,
                (Some(_), false) => 2,
                (Some(_), true) if !full => 3,
                (Some(_), true) => 4,
            };
            assert!(seen.last().is_none_or(|&last| last >= stage), "{theme:?}: at {width} {stage} after {seen:?}");
            if seen.last() != Some(&stage) {
                seen.push(stage);
            }
        }
        assert_eq!(seen, [4, 3, 2, 1, 0], "{theme:?}");
    });
}

/// Chat on a project of two branches (frame V5.5): the history says conversations stay on their
/// branch; a switch keeps the history open and reads it from the branch arrived on, the old rows
/// gone, and an empty conversation there says the branch has none until one is made.
#[test]
fn chat_shows_only_the_conversations_of_the_branch_shown() {
    const NOTE: &str = "Conversations stay on the branch where they started.";
    const NONE: &str = "No conversations on this branch yet.";
    in_both_appearances(|theme| {
        let (mut h, rig, _) = chat_history(theme);
        assert!(painted_has(&h, "Review the checkout package") && !painted_has(&h, NOTE), "{theme:?}: one branch never says it");
        let host = std::sync::Arc::new(SavingHost::default());
        *host.branch_now.lock().unwrap() = Some("main".to_string());
        *host.branch_list.lock().unwrap() = Some(Ok(listed(true)));
        h.state_mut().services.projects = Some(host.clone());
        versions_answer(&mut h, &host, Ok(two_versions()));
        assert!(painted_has(&h, NOTE) && painted_has(&h, "Review the checkout package"), "{theme:?}: {:?}", painted(&h));
        let rows = rows_now(&h);
        let ends = switch_asked(&mut h, &host);
        switch_checked(&mut h, &ends, rows);
        rig.store.chats.lock().unwrap().clear();
        switch_moved(&mut h, &ends);
        assert_eq!(h.state().chat_panel.view, crate::chat_panel::View::History, "{theme:?}: the history stays open");
        assert!(has_id(&h, "workspace.chat.history.empty") && h.query_by_label(NONE).is_some(), "{theme:?}: {:?}", painted(&h));
        assert!(!painted_has(&h, "Review the checkout package"), "{theme:?}: the branch left's rows are gone");
        click_id(&mut h, "workspace.chat.history");
        h.run_steps(2);
        let hint = "Ask about the model below to start one.";
        assert!(has_id(&h, "workspace.chat.branch-note") && painted_has(&h, NONE) && painted_has(&h, hint), "{theme:?}: {:?}", painted(&h));
        assert!(h.query_by_label(&format!("{NOTE} {NONE} {hint}")).is_some(), "{theme:?}: a status node reads it whole");
        // The first question. In Light the list read before says none; in Dark none is read yet
        // and a list read before the conversation was made answers in the frame its creation is
        // answered. Either list, saying none, is not kept.
        let at = rect_of(&h, "workspace.chat.composer").center();
        click(&mut h, at);
        h.event(egui::Event::Text("First".into()));
        h.run();
        rig.store.hold.store(true, std::sync::atomic::Ordering::SeqCst);
        key(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
        if theme == ThemeKind::Dark {
            let path = h.state().project.as_ref().unwrap().path.clone();
            h.state_mut().chat_panel.history = None;
            h.state_mut().chat_panel.listing = Some(crate::chat_store::ChatStore::list(&*rig.store, &path));
        }
        rig.store.release();
        h.run_steps(2);
        assert_eq!(rig.store.chats.lock().unwrap()[0].row.origin, "Chat 1", "{theme:?}: numbered on its branch");
        assert!(!painted_has(&h, NONE), "{theme:?}: the branch has one: {:?}", painted(&h));
        chat_event(&mut h, &rig, crate::chat::ChatEvent::TurnEnded(crate::chat::TurnEnd::Completed));
        click_id(&mut h, "workspace.chat.new");
        h.run_steps(3);
        assert!(!painted_has(&h, NONE) && painted_has(&h, "Ask about Commerce platform"), "{theme:?}: the branch has one now: {:?}", painted(&h));
        // Its one conversation deleted, the branch has none; brought back from the conversation
        // view, it has one again.
        click_id(&mut h, "workspace.chat.history");
        h.run_steps(2);
        let row = rect_of(&h, "workspace.chat.history.0").center();
        point_at(&mut h, row);
        click_id(&mut h, "workspace.chat.history.0.delete");
        click_id(&mut h, "workspace.chat.delete.confirm");
        h.run_steps(2);
        click_id(&mut h, "workspace.chat.history");
        h.run_steps(2);
        assert!(painted_has(&h, NONE) && has_id(&h, "workspace.chat.toast.undo"), "{theme:?}: {:?}", painted(&h));
        click_id(&mut h, "workspace.chat.toast.undo");
        h.run_steps(3);
        assert!(!painted_has(&h, NONE) && painted_has(&h, "Ask about Commerce platform"), "{theme:?}: back: {:?}", painted(&h));
    });
}

const NEW: &str = "workspace.versions.branches.new";
const NAME: &str = "branch-ask.name";

/// New branch… from the Branches header over `listed(true)`, with `typed` in its name field.
fn naming(theme: ThemeKind, typed: &str) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>) {
    let (mut h, host) = branches_read(theme, Ok(listed(true)));
    click_id(&mut h, NEW);
    h.run_steps(2);
    h.event(egui::Event::Text(typed.into()));
    h.run_steps(2);
    (h, host)
}

/// How many branches the host was asked to make once Create is clicked: one more only when it is on.
fn create(h: &mut Harness<'static, Shell>, host: &SavingHost) -> usize {
    click_id(h, "branch-ask.create");
    h.run_steps(2);
    host.branch_makes.lock().unwrap().len()
}

/// The header's + is offered while only `main` folds the section. The dialog says where the branch
/// starts, and that unsaved work comes along; a name taken in another case or emptied says why
/// under the field with Create off. Create asks the host for the name as stored; while it works
/// nothing closes the dialog; made, the dialog closes, the branches are read again and the switch
/// to the new branch starts.
#[test]
fn new_branch_names_makes_and_switches_to_the_branch() {
    in_both_appearances(|theme| {
        let (h, _host) = branches_read(theme, Ok(listed(false)));
        assert!(has_id(&h, NEW) && !has_id(&h, MAIN), "{theme:?}: offered while folded");
        let (mut h, host) = naming(theme, "TRY Cancellations");
        let starts = ["New branch", "A branch keeps its own versions, unsaved work and conversations.", "It starts from current work on main."];
        assert!(starts.iter().all(|t| painted_has(&h, t)) && !painted_has(&h, "come along"), "{theme:?}: {:?}", painted(&h));
        assert!(painted_has(&h, "That name is already in use.") && create(&mut h, &host) == 0, "{theme:?}: taken in another case");
        let at = rect_of(&h, NAME).center();
        click(&mut h, at);
        key(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
        key(&mut h, egui::Key::Backspace, egui::Modifiers::NONE);
        assert!(painted_has(&h, "Enter a name.") && create(&mut h, &host) == 0, "{theme:?}: emptied");
        type_into(&mut h, NAME, "Fresh idea");
        assert!(painted_has(&h, "At most 60 characters.") && create(&mut h, &host) == 1, "{theme:?}");
        assert_eq!(host.branch_makes.lock().unwrap().iter().map(|m| m.0.clone()).collect::<Vec<_>>(), ["fresh-idea"], "{theme:?}");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        click_id(&mut h, "branch-ask.cancel");
        assert!(matches!(h.state().ask, Some(Ask::Branch { .. })) && create(&mut h, &host) == 1, "{theme:?}: up while the host works");
        assert!(!has_id(&h, NEW), "{theme:?}: one at a time");
        let reads = *host.branch_reads.lock().unwrap();
        host.branch_makes.lock().unwrap().remove(0).1.send(Ok(())).unwrap();
        h.run_steps(4);
        assert!(h.state().ask.is_none() && *host.branch_reads.lock().unwrap() > reads, "{theme:?}: closed, read again");
        assert_eq!(host.switches.lock().unwrap().iter().map(|s| s.branch.clone()).collect::<Vec<_>>(), ["fresh-idea"], "{theme:?}");

        let (mut h, _host) = naming(theme, "");
        add_schema(&mut h, "crm");
        let unsaved = "It starts from current work on main. Your unsaved changes come along, and main keeps them too.";
        assert!(painted_has(&h, unsaved) && painted_has(&h, "At most 60 characters."), "{theme:?}: nothing said of a name not typed yet");
    });
}

/// Enter makes the branch too. Each refusal is said under the field in its own words and the
/// dialog stays, Create on again; changing the name ends it. A lost answer may have made the
/// branch. Nothing switches.
#[test]
fn a_refused_branch_says_why_and_the_dialog_stays() {
    let cases = [
        (Some(crate::BranchRefused::Taken), "That name is already in use."),
        (Some(crate::BranchRefused::NotCleanedUp), "The last new branch is not cleaned up yet. Opening the project again removes it."),
        (Some(crate::BranchRefused::Moved), "The project changed before the branch was made, so none was made."),
        (Some(crate::BranchRefused::Failed("disk full".into())), "Couldn't make the branch: disk full"),
        (None, "Archforge could not tell whether the branch was made. Look at Branches before trying again."),
    ];
    in_both_appearances(|theme| {
        for (i, (answer, words)) in cases.clone().into_iter().enumerate() {
            let (mut h, host) = naming(theme, "Fresh");
            // Enter keeps the field's keyboard; a click on Create takes it, and a refusal gives it back.
            if i == 0 {
                key(&mut h, egui::Key::Enter, egui::Modifiers::NONE)
            } else {
                _ = create(&mut h, &host)
            }
            let (_, reply) = host.branch_makes.lock().unwrap().remove(0);
            match answer {
                Some(refused) => reply.send(Err(refused)).unwrap(),
                None => drop(reply),
            }
            h.run_steps(3);
            assert!(matches!(h.state().ask, Some(Ask::Branch { .. })) && painted_has(&h, words), "{theme:?}: {words}");
            assert!(host.switches.lock().unwrap().is_empty(), "{theme:?}: {words}");
            h.event(egui::Event::Text("er".into()));
            h.run_steps(2);
            assert!(!painted_has(&h, words), "{theme:?}: {words}: a new name ends it");
            assert_eq!(create(&mut h, &host), 1, "{theme:?}: {words}: Create is on again");
        }
    });
}

/// With the keyboard off the field nothing behind the dialog takes a key; a branch made after the
/// dialog went with its session switches to nothing; a held project offers no new branch.
#[test]
fn new_branch_keeps_the_keys_and_its_answer_to_itself() {
    in_both_appearances(|theme| {
        let (mut h, host) = saving_harness(Some(commerce()), None);
        (h.state_mut().state.theme, *host.branch_now.lock().unwrap()) = (theme, Some("main".into()));
        crate::explorer::add(h.state_mut(), "schema");
        h.run();
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
        let at = rect_of(&h, &format!("workspace.explorer.row.{}", rows[0].id)).center();
        click(&mut h, at);
        h.state_mut().explorer.focused = true;
        crate::branch_ask::open(h.state_mut());
        h.run_steps(2);
        let revision = h.state().session.as_ref().unwrap().revision();
        h.ctx.memory_mut(|m| m.focused().into_iter().for_each(|id| m.surrender_focus(id)));
        key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
        let kept = h.state().session.as_ref().unwrap().revision() == revision;
        assert!(matches!(h.state().ask, Some(Ask::Branch { .. })) && kept, "{theme:?}: Delete didn't reach the selected row");
        let (mut h, host) = naming(theme, "Fresh");
        create(&mut h, &host);
        h.state_mut().session_replaced();
        h.run_steps(2);
        host.branch_makes.lock().unwrap().remove(0).1.send(Ok(())).unwrap();
        h.run_steps(4);
        assert!(h.state().ask.is_none() && h.state().versions.making.is_none() && host.switches.lock().unwrap().is_empty(), "{theme:?}");
        h.state_mut().begin_hold().unwrap();
        h.run_steps(1);
        assert!(!has_id(&h, NEW), "{theme:?}: not while the project is held");
    });
}

/// Create keeps the skill text still being typed first, as a switch does, so the branch holds it;
/// text the store refuses stops it, the reason under the field.
#[test]
fn new_branch_keeps_the_skill_text_being_typed_first() {
    in_both_appearances(|theme| {
        for (typed, kept) in [("Plain.".to_string(), true), ("x".repeat(16_001), false)] {
            let (mut h, host) = branches_read(theme, Ok(listed(true)));
            let made = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "name": "DDL", "body": "Old." })).unwrap();
            let skill = made["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap();
            crate::skill_tab::open(h.state_mut(), skill);
            // Typed just now, so only a forced keep takes it.
            let doc = h.state_mut().canvas.skills.iter_mut().find(|d| d.skill == skill).unwrap();
            (doc.text, doc.changed_at) = (typed.clone(), f64::MAX);
            click_id(&mut h, NEW);
            h.event(egui::Event::Text("Fresh".into()));
            h.run_steps(2);
            let asked = create(&mut h, &host);
            let saved = h.state().canvas.skills.iter().any(|d| d.skill == skill && d.saved == typed);
            assert_eq!((asked, saved), (usize::from(kept), kept), "{theme:?}");
            assert_eq!(painted_has(&h, &crate::skill_tab::unkept_reason()), !kept, "{theme:?}");
        }
    });
}

const RENAME: &str = "branch-ask.rename";

/// Rename… from `branch`'s menu, its `at`th entry, with the field's name replaced by `typed`.
fn renaming(h: &mut Harness<'static, Shell>, (branch, at): (&str, usize), typed: &str) {
    click_id(h, &format!("workspace.versions.branch.more.{branch}"));
    h.run_steps(1);
    click_id(h, &format!("workspace.versions.branch.menu.{branch}.{at}"));
    h.run_steps(2);
    retype(h, typed);
}

/// The dialog's name field holding `typed` in place of what it held.
fn retype(h: &mut Harness<'static, Shell>, typed: &str) {
    let at = rect_of(h, NAME).center();
    click(h, at);
    key(h, egui::Key::A, egui::Modifiers::COMMAND);
    h.event(egui::Event::Text(typed.into()));
    h.run_steps(2);
}

/// The renames the host was asked for once Rename is clicked, from and to as stored.
fn renamed(h: &mut Harness<'static, Shell>, host: &SavingHost) -> Vec<(String, String)> {
    click_id(h, RENAME);
    h.run_steps(2);
    host.branch_renames.lock().unwrap().iter().map(|r| (r.0.clone(), r.1.clone())).collect()
}

/// Another branch's menu has Switch, Rename… and Delete…, the branch in use only Rename…, and
/// `main` no Rename…. The dialog holds the name; unchanged, Rename is off; a name taken says so; Rename asks
/// the host for the names as stored, and done, the dialog closes and the branches are read again.
#[test]
fn a_branch_row_menu_renames_any_branch_but_main() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        assert!(!has_id(&h, "workspace.versions.branch.more.main"), "{theme:?}: main in use has no menu");
        renaming(&mut h, ("try-cancellations", 1), "Try cancellations");
        let words = ["Rename branch", "The conversations and versions on this branch are not affected."];
        assert!(words.iter().all(|w| painted_has(&h, w)) && !painted_has(&h, "starts from"), "{theme:?}: {:?}", painted(&h));
        assert!(renamed(&mut h, &host).is_empty() && painted_has(&h, "At most 60 characters."), "{theme:?}: unchanged");
        retype(&mut h, "x");
        retype(&mut h, "Try cancellations");
        assert!(!painted_has(&h, "already in use") && renamed(&mut h, &host).is_empty(), "{theme:?}: typed again, its own name");
        retype(&mut h, "Main");
        assert!(painted_has(&h, "That name is already in use.") && renamed(&mut h, &host).is_empty(), "{theme:?}: taken");
        retype(&mut h, "Later idea");
        assert_eq!(renamed(&mut h, &host), [("try-cancellations".into(), "later-idea".into())], "{theme:?}");
        assert!(host.branch_makes.lock().unwrap().is_empty(), "{theme:?}: nothing made");
        let reads = *host.branch_reads.lock().unwrap();
        host.branch_renames.lock().unwrap().remove(0).2.send(Ok(())).unwrap();
        h.run_steps(4);
        assert!(h.state().ask.is_none() && *host.branch_reads.lock().unwrap() > reads, "{theme:?}: closed, read again");
        assert!(host.switches.lock().unwrap().is_empty(), "{theme:?}: a rename switches to nothing");

        *host.branch_now.lock().unwrap() = Some("try-cancellations".into());
        h.run_steps(2);
        click_id(&mut h, "workspace.versions.branch.more.main");
        h.run_steps(1);
        let main = ["workspace.versions.branch.menu.main.0", "workspace.versions.branch.menu.main.1"];
        assert!(has_id(&h, main[0]) && !has_id(&h, main[1]), "{theme:?}: main only switches");
        click_id(&mut h, "workspace.versions.branch.more.main");
        let path = h.state().project.as_ref().unwrap().path.clone();
        h.state_mut().state.keep_branch(&path, Some("try-cancellations".into()));
        assert_eq!(h.state().state.branch_of(&path), Some("try-cancellations"), "{theme:?}: kept before");
        renaming(&mut h, ("try-cancellations", 0), "Later idea");
        assert_eq!(renamed(&mut h, &host), [("try-cancellations".into(), "later-idea".into())], "{theme:?}: the branch in use");
        host.branch_renames.lock().unwrap().remove(0).2.send(Ok(())).unwrap();
        h.run_steps(4);
        assert_eq!(h.state().state.branch_of(&path), Some("later-idea"), "{theme:?}: it reopens under the new name");
        click_id(&mut h, "workspace.versions.branch.more.try-cancellations");
        h.run_steps(1);
        assert!(has_id(&h, "workspace.versions.branch.menu.try-cancellations.0"), "{theme:?}: open");
        h.state_mut().begin_hold().unwrap();
        h.run_steps(2);
        h.state_mut().end_hold();
        h.run_steps(2);
        assert!(h.state().panels.menu.is_none(), "{theme:?}: a menu left empty by a hold doesn't come back after it");
    });
}

/// A refused rename says why in its own words and the dialog stays. A lost answer may have renamed
/// it, and only asking for the same rename again settles that, though the list now shows the new
/// name: another name is refused, naming the one to ask for.
#[test]
fn a_refused_rename_says_why_and_a_lost_one_can_be_asked_again() {
    let cases = [
        (Some(crate::BranchRefused::Moved), "The project changed before the branch was renamed, so its name stayed."),
        (Some(crate::BranchRefused::Failed("disk full".into())), "Couldn't rename the branch: disk full"),
        (None, "Archforge could not tell whether the branch was renamed to later. Rename it to later again to find out."),
    ];
    in_both_appearances(|theme| {
        for (answer, words) in cases.clone() {
            let (mut h, host) = branches_read(theme, Ok(listed(true)));
            renaming(&mut h, ("try-cancellations", 1), "Later");
            assert_eq!(renamed(&mut h, &host).len(), 1, "{theme:?}");
            let (.., reply) = host.branch_renames.lock().unwrap().pop().unwrap();
            assert!(renamed(&mut h, &host).is_empty(), "{theme:?}: one at a time");
            let lost = answer.is_none();
            match answer {
                Some(refused) => reply.send(Err(refused)).unwrap(),
                None => drop(reply),
            }
            *host.branch_list.lock().unwrap() = Some(Ok(with(["later".to_string()])));
            h.run_steps(4);
            assert!(matches!(h.state().ask, Some(Ask::Branch { .. })) && painted_has(&h, words), "{theme:?}: {words}");
            if lost {
                retype(&mut h, "Other");
                assert!(painted_has(&h, words) && renamed(&mut h, &host).is_empty(), "{theme:?}: only the unsettled name");
                retype(&mut h, "Later");
            }
            let again = renamed(&mut h, &host).len();
            assert_eq!(again, usize::from(lost), "{theme:?}: {words}: the listed name counts as taken but after a lost answer");
        }
    });
}

/// A rename of the branch in use keeps the skill text still being typed first, as a switch does,
/// and text the store refuses stops it, the reason under the field; another branch's rename leaves
/// the text where it is.
#[test]
fn renaming_the_branch_in_use_keeps_the_text_being_typed_first() {
    in_both_appearances(|theme| {
        let huge = "x".repeat(16_001);
        for (on, typed, asked, saved) in
            [("main", &*huge, 1, false), ("try-cancellations", "Plain.", 1, true), ("try-cancellations", &*huge, 0, false)]
        {
            let (mut h, host) = branches_read(theme, Ok(listed(true)));
            *host.branch_now.lock().unwrap() = Some(on.into());
            let made = crate::panels::run(h.state_mut(), "skill.add", serde_json::json!({ "name": "DDL", "body": "Old." })).unwrap();
            let skill = made["id"].as_str().and_then(archforge_mof::Uuid::parse).unwrap();
            crate::skill_tab::open(h.state_mut(), skill);
            // Typed just now, so only a forced keep takes it.
            let doc = h.state_mut().canvas.skills.iter_mut().find(|d| d.skill == skill).unwrap();
            (doc.text, doc.changed_at) = (typed.to_string(), f64::MAX);
            h.run_steps(2);
            renaming(&mut h, ("try-cancellations", usize::from(on == "main")), "Later");
            assert_eq!(renamed(&mut h, &host).len(), asked, "{theme:?}: on {on}");
            assert_eq!(h.state().canvas.skills.iter().any(|d| d.skill == skill && d.saved == typed), saved, "{theme:?}: on {on}");
            let refused = painted_has(&h, &crate::skill_tab::unkept_reason());
            assert_eq!(refused, asked == 0, "{theme:?}: on {on}: only the branch in use waits for the text");
        }
    });
}

const DELETE: &str = "branch-ask.delete";

/// Delete… from `branch`'s menu, its `at`th entry; gives the count the host was asked for.
fn deleting(
    h: &mut Harness<'static, Shell>,
    host: &SavingHost,
    (branch, at): (&str, usize),
) -> std::sync::mpsc::Sender<Result<crate::BranchCounts, String>> {
    click_id(h, &format!("workspace.versions.branch.more.{branch}"));
    h.run_steps(1);
    click_id(h, &format!("workspace.versions.branch.menu.{branch}.{at}"));
    h.run_steps(2);
    let (asked, counts) = host.branch_counts.lock().unwrap().pop().unwrap();
    assert_eq!(asked, branch);
    counts
}

/// How many deletes the host was asked for once Delete branch is clicked.
fn deleted(h: &mut Harness<'static, Shell>, host: &SavingHost) -> usize {
    click_id(h, DELETE);
    h.run_steps(2);
    host.branch_deletes.lock().unwrap().len()
}

/// Delete… is on another branch's menu, never on `main` or the branch in use. The dialog names the
/// branch, what goes with it once counted, that it may remain in the project folder and that there
/// is no Undo; Delete branch waits for the count. While the host works nothing closes it; deleted,
/// it closes and the branches are read again.
#[test]
fn a_branch_row_menu_deletes_another_branch_after_saying_what_goes() {
    in_both_appearances(|theme| {
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        let counts = deleting(&mut h, &host, ("try-cancellations", 2));
        let says = ["Delete Try cancellations?", "Goes with the branch:", "Counting what goes with it…", "may remain in the project folder's"];
        assert!(says.iter().all(|t| painted_has(&h, t)), "{theme:?}: {:?}", painted(&h));
        let no_undo = "There is no Undo for deleting a branch.";
        assert!(painted_has(&h, no_undo) && h.query_by_label(no_undo).is_some(), "{theme:?}: a status node reads it");
        assert_eq!(deleted(&mut h, &host), 0, "{theme:?}: not before the count");
        counts.send(Ok(crate::BranchCounts { versions_only_here: 2, conversations: 1, unsaved: true })).unwrap();
        h.run_steps(2);
        let line = "2 versions only on this branch · 1 conversation · unsaved changes";
        assert!(painted_has(&h, line) && h.query_by_label(line).is_some(), "{theme:?}: {:?}", painted(&h));
        assert_eq!(deleted(&mut h, &host), 1, "{theme:?}");
        key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
        click_id(&mut h, "branch-ask.cancel");
        assert!(matches!(h.state().ask, Some(Ask::Branch { .. })) && deleted(&mut h, &host) == 1, "{theme:?}: up while the host works");
        let reads = *host.branch_reads.lock().unwrap();
        host.branch_deletes.lock().unwrap().remove(0).send(Ok(Vec::new())).unwrap();
        h.run_steps(4);
        assert!(h.state().ask.is_none() && *host.branch_reads.lock().unwrap() > reads, "{theme:?}: closed, read again");
        let others = [host.branch_makes.lock().unwrap().len(), host.branch_renames.lock().unwrap().len(), host.switches.lock().unwrap().len()];
        assert_eq!(others, [0; 3], "{theme:?}: nothing made, renamed or switched to");
        *host.branch_now.lock().unwrap() = Some("try-cancellations".into());
        h.run_steps(2);
        click_id(&mut h, MORE);
        h.run_steps(1);
        assert!(has_id(&h, MENU) && !has_id(&h, "workspace.versions.branch.menu.try-cancellations.1"), "{theme:?}: in use, only Rename…");
    });
}

/// A refused delete says why the branch stays in the dialog, which stays with Delete branch on;
/// a lost answer may have deleted it, and asking again finds out. A count that can't be read says
/// so and lets the delete go ahead.
#[test]
fn a_refused_delete_says_why_the_branch_stays() {
    let unclear = "Archforge could not tell whether the branch was deleted. Delete it again to find out.";
    let cases = [
        (Some(crate::BranchNotDeleted::Moved), "The project changed before the branch was deleted, so it stays."),
        (Some(crate::BranchNotDeleted::Failed("disk full".into())), "The branch stays: disk full"),
        (Some(crate::BranchNotDeleted::Unclear("lost".into())), unclear),
        (None, unclear),
    ];
    in_both_appearances(|theme| {
        for (answer, words) in cases.clone() {
            let (mut h, host) = branches_read(theme, Ok(listed(true)));
            deleting(&mut h, &host, ("try-cancellations", 2)).send(Err("unreadable".into())).unwrap();
            h.run_steps(2);
            assert!(painted_has(&h, "What goes with it couldn't be read: unreadable"), "{theme:?}");
            assert_eq!(deleted(&mut h, &host), 1, "{theme:?}: {words}");
            let reply = host.branch_deletes.lock().unwrap().remove(0);
            match answer {
                Some(not) => reply.send(Err(not)).unwrap(),
                None => drop(reply),
            }
            h.run_steps(4);
            assert!(matches!(h.state().ask, Some(Ask::Branch { .. })), "{theme:?}: {words}");
            assert!(painted_has(&h, "Couldn't delete Try cancellations") && h.query_by_label(words).is_some(), "{theme:?}: {words}");
            assert_eq!(deleted(&mut h, &host), 1, "{theme:?}: {words}: Delete branch is on again");
        }
    });
}

/// With the delete dialog up and the keyboard off it, Delete doesn't reach the explorer's selected
/// row; with no dialog the same key removes it.
#[test]
fn delete_branch_keeps_the_keys_to_itself() {
    in_both_appearances(|theme| {
        for up in [true, false] {
            let (mut h, host) = saving_harness(Some(commerce()), None);
            (h.state_mut().state.theme, *host.branch_now.lock().unwrap()) = (theme, Some("main".into()));
            crate::explorer::add(h.state_mut(), "schema");
            h.run();
            key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
            let rows = crate::explorer::rows(h.state().notation_uis, h.state().session.as_ref().unwrap().doc(), &h.state().explorer);
            let at = rect_of(&h, &format!("workspace.explorer.row.{}", rows[0].id)).center();
            click(&mut h, at);
            h.state_mut().explorer.focused = true;
            if up {
                crate::branch_ask::open_delete(h.state_mut(), &listed(true)[1]);
                h.run_steps(2);
                assert!(has_id(&h, DELETE), "{theme:?}: up");
            }
            let revision = h.state().session.as_ref().unwrap().revision();
            h.ctx.memory_mut(|m| m.focused().into_iter().for_each(|id| m.surrender_focus(id)));
            key(&mut h, egui::Key::Delete, egui::Modifiers::NONE);
            let changed = h.state().session.as_ref().unwrap().revision() != revision;
            assert_eq!(changed, !up, "{theme:?}: with the dialog up: {up}");
        }
    });
}
