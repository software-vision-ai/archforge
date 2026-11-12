//! The Versions menu: its rows in the design's order, each on only while it can act, and what
//! viewing a version, a hold and a dialog turn on and off.

use egui::{Key, Modifiers};

use super::*;
use crate::actions::{BACK, BRANCHES, COMPARE, NEWER, OLDER, SAVE_VERSION, VERSIONS, VERSIONS_MENU, available, run};

/// The rows that can act now, in the menu's order.
fn on(h: &Harness<'static, Shell>) -> Vec<&'static str> {
    VERSIONS_MENU.concat().into_iter().filter(|id| available(h.state(), id)).collect()
}

/// The menu holds the design's rows; Save version… waits for changes, History gives way to Back to
/// current work while a version is in view, and ⇧⌘S and ⇧⌘H reach the row that is on.
#[test]
fn the_rows_follow_viewing_a_version() {
    in_both_appearances(|theme| {
        let (mut h, host) = changes_panel(theme);
        h.state_mut().in_window_menu = true;
        h.run();
        click_menu(&mut h, "strip.menu.versions");
        let drawn: Vec<String> = menu_ids(&h).iter().filter_map(|i| i.strip_prefix("strip.menu.row.")).map(String::from).collect();
        assert_eq!(drawn, VERSIONS_MENU.concat(), "{theme:?}: the design's order");
        key(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(on(&h), [VERSIONS, COMPARE, OLDER, NEWER], "{theme:?}: nothing to save, one branch");
        let earlier = rows_now(&h);
        add_schema(&mut h, "crm");
        compared(&mut h);
        assert_eq!(on(&h), [SAVE_VERSION, VERSIONS, COMPARE, OLDER, NEWER], "{theme:?}: a change to save");
        key(&mut h, Key::S, Modifiers::COMMAND | Modifiers::SHIFT);
        assert!(h.state().versions.dialog.is_some(), "{theme:?}: ⇧⌘S asks for the description");
        assert!(host.saves.lock().unwrap().is_empty(), "{theme:?}: and does not save");
        assert_eq!(on(&h), Vec::<&str>::new(), "{theme:?}: nothing behind the dialog");
        click_id(&mut h, "workspace.versions.dialog.cancel");
        start_viewing(&mut h, "c1");
        assert_eq!(on(&h), [BACK], "{theme:?}: while the version is read, only the way back");
        answer_read(&mut h, &host, Ok(earlier));
        assert_eq!(on(&h), [OLDER, NEWER, BACK], "{theme:?}: viewing, the list read again for what shows");
        crate::support::open(h.state_mut());
        assert_eq!(on(&h), Vec::<&str>::new(), "{theme:?}: not even the way back behind a dialog");
        h.state_mut().about = None;
        key(&mut h, Key::H, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(h.state().viewing(), None, "{theme:?}: ⇧⌘H goes back");
        assert!(available(h.state(), VERSIONS) && !available(h.state(), BRANCHES), "{theme:?}");
        h.state_mut().begin_hold().unwrap();
        assert_eq!(on(&h), Vec::<&str>::new(), "{theme:?}: a hold, as a switch of branch takes, turns every row off");
    });
}

/// Compare… sets the newest version against current work in the Versions explorer, and is off
/// while that page is open.
#[test]
fn compare_opens_the_newest_version_against_current_work() {
    in_both_appearances(|theme| {
        let date = "2026-11-08T15:30:00Z";
        let (mut h, _host) =
            versions_listed(theme, vec![version_row("c2", Some("Second"), (None, false), date), version_row("c1", None, (None, false), date)]);
        h.state_mut().state.show_explorer(crate::state::Explorer::Models);
        assert_eq!(crate::version_details::to_compare(h.state()).map(|e| e.hash).as_deref(), Some("c2"), "{theme:?}: the newest");
        run(h.state_mut(), COMPARE);
        h.run_steps(2);
        assert_eq!(h.state().state.explorer, crate::state::Explorer::SourceControl, "{theme:?}");
        assert!(has_id(&h, "workspace.version-compare.close"), "{theme:?}: the Compare page shows");
        assert!(!available(h.state(), COMPARE), "{theme:?}: already open");
    });
}
