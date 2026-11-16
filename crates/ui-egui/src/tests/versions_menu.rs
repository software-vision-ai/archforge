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
        let keys = ["Ctrl+Alt+←", "Ctrl+Alt+→"].map(|keys| painted_times(&h, keys));
        assert_eq!(keys, [1, 1], "{theme:?}: the steps' keys as the bar writes them, beside Older version and Newer version");
        key(&mut h, Key::Escape, Modifiers::NONE);
        assert_eq!(on(&h), [VERSIONS, COMPARE, OLDER], "{theme:?}: nothing to save, one branch, nothing newer");
        let earlier = rows_now(&h);
        add_schema(&mut h, "crm");
        compared(&mut h);
        assert_eq!(on(&h), [SAVE_VERSION, VERSIONS, COMPARE, OLDER], "{theme:?}: a change to save");
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
        assert!(!available(h.state(), NEWER), "{theme:?}: nothing newer than current work");
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
        h.state_mut().versions.stale = true;
        assert!(!available(h.state(), COMPARE), "{theme:?}: a save may have made a newer one than the list holds");
        assert_eq!(crate::menu_bar::why_off(h.state(), COMPARE), "Loading", "{theme:?}");
        h.state_mut().versions.stale = false;
        assert_eq!(crate::version_details::to_compare(h.state()).map(|e| e.hash).as_deref(), Some("c2"), "{theme:?}: the newest");
        run(h.state_mut(), COMPARE);
        h.run_steps(2);
        assert_eq!(h.state().state.explorer, crate::state::Explorer::SourceControl, "{theme:?}");
        assert!(has_id(&h, "workspace.version-compare.close"), "{theme:?}: the Compare page shows");
        assert!(!available(h.state(), COMPARE), "{theme:?}: already open");
        assert_eq!(crate::menu_bar::why_off(h.state(), COMPARE), "Already showing", "{theme:?}: and the row says so");
    });
}

/// The command key as a Mac and as Windows and Linux report it; ⌥ or Alt makes a step's chord.
const SHAPES: [Modifiers; 2] = [Modifiers::MAC_CMD.plus(Modifiers::COMMAND), Modifiers::CTRL.plus(Modifiers::COMMAND)];

/// A project with a host in `theme`, Customer's card on its open diagram and no read of versions
/// waiting; gives the schema's id and the card's control.
fn card_on_a_diagram(theme: ThemeKind) -> (Harness<'static, Shell>, std::sync::Arc<SavingHost>, archforge_mof::Uuid, String) {
    let (mut h, host) = saving_harness(Some(commerce()), None);
    h.state_mut().state.theme = theme;
    let s = h.state_mut().session.as_mut().unwrap();
    let mut made = |command: &str, with: serde_json::Value| s.execute(command, &with).unwrap().value["id"].as_str().unwrap().to_string();
    let schema = made("er.schema.add", serde_json::json!({ "name": "Shop" }));
    let customer = made("er.entity.add", serde_json::json!({ "schema": schema, "name": "Customer" }));
    let diagram = made("diagram.add", serde_json::json!({ "kind": "er", "element": schema, "name": "Shop schema" }));
    let placed = serde_json::json!({ "diagram": diagram, "element": customer, "x": 100, "y": 100, "width": 240, "height": 166 });
    let card = format!("workspace.canvas.node.{}", made("diagram.place", placed));
    crate::canvas::open(h.state_mut(), archforge_mof::Uuid::parse(&diagram).unwrap());
    h.run_steps(4);
    host.version_reads.lock().unwrap().clear();
    (h, host, archforge_mof::Uuid::parse(&schema).unwrap(), card)
}

/// A step's chord is never a plain arrow, in either key shape. Newer version is off from current
/// work, so its press stays in the input: it nudges no selected card and unfolds no row of the
/// tree, as the arrow alone does. Older version is on: its press asks for the versions and nudges
/// nothing.
#[test]
fn a_version_chord_is_never_a_plain_arrow() {
    in_both_appearances(|theme| {
        for shape in SHAPES {
            let (mut h, host, _, card) = card_on_a_diagram(theme);
            // With the window's own bar and its Alt letters, as on Windows and Linux (AR-538).
            h.state_mut().in_window_menu = true;
            let place = |h: &Harness<'static, Shell>| rect_of(h, &card).min;
            let at = rect_of(&h, &card).center();
            click(&mut h, at);
            let was = place(&h);
            assert!(available(h.state(), OLDER) && !available(h.state(), NEWER), "{theme:?}: current work");
            key(&mut h, Key::ArrowRight, shape | Modifiers::ALT);
            assert_eq!(place(&h), was, "{theme:?} {shape:?}: Newer version's chord, off, nudges nothing");
            for (arrow, held) in [(Key::ArrowLeft, shape), (Key::ArrowUp, shape | Modifiers::ALT), (Key::ArrowDown, shape | Modifiers::ALT)] {
                key(&mut h, arrow, held);
                assert_eq!(place(&h), was, "{theme:?} {held:?}: nor does {arrow:?} in a chord that is nobody's");
            }
            key(&mut h, Key::ArrowRight, Modifiers::NONE);
            let nudged = place(&h);
            assert!(nudged.x > was.x, "{theme:?}: the arrow alone nudges the card");
            key(&mut h, Key::ArrowLeft, shape | Modifiers::ALT);
            assert_eq!((host.version_reads.lock().unwrap().len(), place(&h)), (1, nudged), "{theme:?} {shape:?}: Older version, and no nudge");

            let (mut h, _, schema, _) = card_on_a_diagram(theme);
            h.state_mut().state.show_explorer(crate::state::Explorer::Models);
            h.state_mut().session.as_mut().unwrap().execute("view.select", &serde_json::json!({ "ids": [schema.to_string()] })).unwrap();
            h.run();
            (h.state_mut().explorer.focused, h.state_mut().canvas.focused) = (true, false);
            h.state_mut().explorer.expanded.clear();
            key(&mut h, Key::ArrowRight, shape | Modifiers::ALT);
            assert!(h.state().explorer.expanded.is_empty(), "{theme:?} {shape:?}: the chord unfolds no row");
            key(&mut h, Key::ArrowRight, Modifiers::NONE);
            assert!(h.state().explorer.expanded.contains(&schema), "{theme:?}: the arrow alone unfolds it");
            let chosen = |h: &Harness<'static, Shell>| h.state().session.as_ref().unwrap().view().selection.clone();
            let on_schema = chosen(&h);
            for arrow in [Key::ArrowDown, Key::ArrowLeft] {
                key(&mut h, arrow, shape);
                assert_eq!(
                    (chosen(&h), h.state().explorer.expanded.len()),
                    (on_schema.clone(), 1),
                    "{theme:?} {shape:?}: {arrow:?} with the command key"
                );
            }
            key(&mut h, Key::ArrowDown, Modifiers::NONE);
            let below = chosen(&h);
            assert_ne!(below, on_schema, "{theme:?}: the arrow alone goes down a row");
            key(&mut h, Key::ArrowUp, shape);
            assert_eq!(chosen(&h), below, "{theme:?} {shape:?}: ArrowUp with the command key");
            key(&mut h, Key::ArrowUp, Modifiers::NONE);
            assert_eq!(chosen(&h), on_schema, "{theme:?}: and up again");
            key(&mut h, Key::ArrowLeft, Modifiers::NONE);
            assert!(h.state().explorer.expanded.is_empty(), "{theme:?}: and folds the row");
        }
    });
}

/// With the keyboard in a field a step's chord is the window's while its action is on: Older
/// version asks for the versions and the caret stays. Newer version's, off from current work, is
/// the field's, which moves its caret by a word and changes no text.
#[test]
fn a_version_chord_in_a_field_steps_or_moves_the_caret() {
    in_both_appearances(|theme| {
        for shape in SHAPES {
            let (mut h, host, _, card) = card_on_a_diagram(theme);
            let at = rect_of(&h, &card).center();
            click(&mut h, at);
            click_id(&mut h, "workspace.properties.columns.add");
            click_id(&mut h, "workspace.properties.column.name");
            let name = |h: &Harness<'static, Shell>| h.state().panels.column.as_ref().map(|c| c.name.clone()).unwrap_or_default();
            let typed = |h: &mut Harness<'static, Shell>, text: &str| {
                h.event(egui::Event::Text(text.into()));
                h.run();
                name(h)
            };
            key(&mut h, Key::Home, Modifiers::NONE);
            key(&mut h, Key::ArrowRight, shape | Modifiers::ALT);
            assert_eq!(typed(&mut h, "x"), "column_1x", "{theme:?} {shape:?}: Newer version is off, and the field moved a word on");
            key(&mut h, Key::ArrowLeft, shape | Modifiers::ALT);
            assert_eq!(host.version_reads.lock().unwrap().len(), 1, "{theme:?} {shape:?}: Older version asks for the versions");
            assert_eq!(typed(&mut h, "y"), "column_1xy", "{theme:?} {shape:?}: and the caret stayed at the end");
        }
    });
}
