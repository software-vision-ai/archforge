//! The Branches section of the Versions panel (`crate::versions`).

use egui_kittest::kittest::Queryable;

use super::*;

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

/// A click, Enter or the menu's Switch on another branch asks to switch to it; the branch in use
/// has no menu, and a click on it does nothing; ↑ and ↓ move between rows, out of view too.
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
