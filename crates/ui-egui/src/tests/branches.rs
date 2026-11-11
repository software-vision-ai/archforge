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
/// or when the read failed, it is open.
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
        let (h, _host) = branches_read(theme, Ok(listed(true)));
        assert!(has_id(&h, MAIN) && has_id(&h, OTHER) && painted_has(&h, "Branches can't be merged yet."), "{theme:?}");
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
/// has no menu, and a click on it does nothing.
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

/// Many branches, or many changes, in the shortest window still leave History's first row in the
/// column.
#[test]
fn history_keeps_its_room_under_many_branches_or_changes() {
    let in_column = |h: &mut Harness<'static, Shell>| {
        h.set_size(egui::vec2(1600.0, 400.0));
        h.run_steps(2);
        has_id(h, "workspace.versions.row.aa") && rect_of(h, "workspace.versions.row.aa").bottom() <= 400.0
    };
    in_both_appearances(|theme| {
        let (mut h, _host) = branches_read(theme, Ok(with((0..20).map(|i| format!("b{i:02}")))));
        assert!(in_column(&mut h), "{theme:?}: under many branches");
        let (mut h, host) = branches_read(theme, Ok(listed(true)));
        let rows = rows_now(&h);
        answer_read(&mut h, &host, Ok(rows));
        (0..30).for_each(|i| add_schema(&mut h, &format!("s{i}")));
        compared(&mut h);
        assert!(h.state().versions.working == 30 && in_column(&mut h), "{theme:?}: under many changes");
    });
}
