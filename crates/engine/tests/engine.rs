//! The engine against a toy notation: a list of named items, and a view that selects one.

use std::sync::Arc;

use archforge_engine::testing::round_trip;
use archforge_engine::{
    CommandError, CommandSpec, Ctx, EngineError, Facts, JournalEntry, Options, Params, Registry, Session, Snapshot, Snapshotted, View, always, cmd,
};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Default)]
struct Items(Vec<String>);

/// The toy records by copying: the engine's own change-recording document is the MOF store.
type Doc = Snapshotted<Items>;

impl Snapshot for Items {
    /// Panics on an item named `boom-weight`, to prove a notation's panic changes nothing.
    #[allow(clippy::panic)]
    fn weight(&self) -> usize {
        if self.0.iter().any(|s| s == "boom-weight") {
            panic!("weight");
        }
        self.0.iter().map(|s| s.len() + 24).sum::<usize>() + 24
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Selection(Option<usize>);

impl View<Items> for Selection {
    /// Panics on an item named `boom-view`, likewise.
    #[allow(clippy::panic)]
    fn retain_valid(&mut self, doc: &Items) {
        if doc.0.iter().any(|s| s == "boom-view") {
            panic!("view");
        }
        if self.0.is_some_and(|i| i >= doc.0.len()) {
            self.0 = None;
        }
    }
}

type S = Session<Doc, Selection>;
const NOTHING_TO_REMOVE: &str = "There is nothing to remove.";

fn has_items(f: &Facts<'_, Doc, Selection>) -> Result<(), &'static str> {
    if f.doc.0.is_empty() { Err(NOTHING_TO_REMOVE) } else { Ok(()) }
}

fn index(ctx: &Ctx<'_, Doc, Selection>, p: &Params, key: &str) -> Result<usize, CommandError> {
    let i = p.integer(key).unwrap_or(0);
    usize::try_from(i).ok().filter(|i| *i < ctx.doc.0.len()).ok_or_else(|| CommandError::about(key, format!("there is no item {i}")))
}

fn add(ctx: &mut Ctx<'_, Doc, Selection>, p: &Params) -> Result<Value, CommandError> {
    let name = p.str("name").unwrap_or_default().to_string();
    ctx.doc.0.push(name);
    let at = ctx.doc.0.len() - 1;
    *ctx.view = Selection(Some(at));
    Ok(json!({ "index": at }))
}

fn rename(ctx: &mut Ctx<'_, Doc, Selection>, p: &Params) -> Result<Value, CommandError> {
    let i = index(ctx, p, "index")?;
    ctx.doc.0[i] = p.str("name").unwrap_or_default().to_string();
    Ok(json!({ "index": i }))
}

fn remove(ctx: &mut Ctx<'_, Doc, Selection>, p: &Params) -> Result<Value, CommandError> {
    let i = index(ctx, p, "index")?;
    ctx.doc.0.remove(i);
    Ok(json!({ "removed": i }))
}

fn move_item(ctx: &mut Ctx<'_, Doc, Selection>, p: &Params) -> Result<Value, CommandError> {
    let from = index(ctx, p, "from")?;
    let to = index(ctx, p, "to")?;
    let item = ctx.doc.0.remove(from);
    ctx.doc.0.insert(to, item);
    Ok(json!({ "to": to }))
}

fn select(ctx: &mut Ctx<'_, Doc, Selection>, p: &Params) -> Result<Value, CommandError> {
    *ctx.view = Selection(Some(index(ctx, p, "index")?));
    Ok(Value::Null)
}

fn count(ctx: &mut Ctx<'_, Doc, Selection>, _: &Params) -> Result<Value, CommandError> {
    Ok(json!(ctx.doc.0.len()))
}

fn fail(_: &mut Ctx<'_, Doc, Selection>, _: &Params) -> Result<Value, CommandError> {
    Err(CommandError::new("it always fails"))
}

#[allow(clippy::panic)]
fn explode(ctx: &mut Ctx<'_, Doc, Selection>, _: &Params) -> Result<Value, CommandError> {
    ctx.doc.0.push("half".into());
    panic!("boom")
}

fn sneak(ctx: &mut Ctx<'_, Doc, Selection>, _: &Params) -> Result<Value, CommandError> {
    ctx.doc.0.push("unseen".into());
    Ok(Value::Null)
}

/// Adds an item named after a fresh id from the counter, failing afterwards when asked to.
fn tag(ctx: &mut Ctx<'_, Doc, Selection>, p: &Params) -> Result<Value, CommandError> {
    let n = ctx.fresh().ok_or_else(|| CommandError::new("no ids left"))?;
    ctx.doc.0.push(format!("t{n}"));
    if p.bool("fail").unwrap_or(false) {
        return Err(CommandError::new("asked to fail"));
    }
    Ok(json!({ "id": format!("t{n}") }))
}

/// Takes an id and changes nothing: refused.
fn burn(ctx: &mut Ctx<'_, Doc, Selection>, _: &Params) -> Result<Value, CommandError> {
    ctx.fresh();
    Ok(Value::Null)
}

/// Renames an item to a fresh id's name when `text` is empty, else to `text`.
fn label(ctx: &mut Ctx<'_, Doc, Selection>, p: &Params) -> Result<Value, CommandError> {
    let i = index(ctx, p, "index")?;
    let text = p.str("text").unwrap_or_default();
    ctx.doc.0[i] = if text.is_empty() { format!("t{}", ctx.fresh().ok_or_else(|| CommandError::new("no ids left"))?) } else { text.to_string() };
    Ok(Value::Null)
}

fn specs() -> Vec<CommandSpec<Doc, Selection>> {
    let mut s = archforge_engine::cmd::edit::specs();
    s.extend([
        cmd!("item.add", "Add item", &["Item"], Some("Ctrl+N"), "name: string = \"Item\"; the new item's name", always, add),
        cmd!("item.rename", "Rename item", &["Item"], None, "index: integer, 0..1000; which item\nname: string; its new name", has_items, rename),
        cmd!("item.remove", "Remove item", &["Item"], None, "index: integer = 0, 0..1000; which item", has_items, remove),
        cmd!(
            "item.move",
            "Move item",
            &["Item"],
            None,
            "from: integer, 0..1000; which item\nto: integer, 0..1000; where it goes",
            has_items,
            move_item
        ),
        cmd!(noundo "view.select", "Select", &["View"], None, "index: integer, 0..1000; which item", has_items, select),
        cmd!(query "item.count", "Count items", &["Item"], None, "", always, count),
        cmd!("item.fail", "Fail", &["Item"], None, "", always, fail),
        cmd!("item.explode", "Explode", &["Item"], None, "", always, explode),
        cmd!(noundo "item.sneak", "Sneak", &["Item"], None, "", always, sneak),
        cmd!("item.tag", "Tag", &["Item"], None, "fail: bool = false; fail after taking an id", always, tag),
        cmd!("item.burn", "Burn", &["Item"], None, "", always, burn),
        cmd!(
            "item.label",
            "Label",
            &["Item"],
            None,
            "index: integer, 0..1000; which item\ntext: string; the name, a fresh id's when empty",
            has_items,
            label
        ),
    ]);
    s
}

fn registry() -> Arc<Registry<Doc, Selection>> {
    Registry::new(specs()).unwrap()
}

fn session() -> S {
    Session::new(registry(), Doc::default(), 1, Options::default())
}

fn names(s: &S) -> Vec<&str> {
    s.doc().0.iter().map(String::as_str).collect()
}

/// Everything a refused call must leave as it was.
fn state(s: &S) -> (Doc, Selection, bool, bool, bool, usize) {
    (s.doc().clone(), s.view().clone(), s.can_undo(), s.can_redo(), s.is_dirty(), s.journal().count())
}

#[test]
fn the_registry_refuses_duplicate_and_malformed_ids_and_undocumented_parameters() {
    let reg = registry();
    let infos = reg.infos(&Facts { doc: &Doc::default(), view: &Selection::default(), can_undo: false, can_redo: false });
    assert!(infos.iter().all(|i| i.params.iter().all(|p| !p.meaning.is_empty())));
    let undo = infos.iter().find(|i| i.id == "edit.undo").unwrap();
    assert_eq!((undo.enabled, undo.disabled_reason), (false, Some(archforge_engine::cmd::edit::NOTHING_TO_UNDO)));
    let mut twice = specs();
    twice.push(cmd!("item.add", "Again", &["Item"], None, "", always, count));
    assert!(Registry::new(twice).is_err());
    assert!(Registry::new(vec![cmd!("add", "Add", &["Item"], None, "", always, count)]).is_err());
    assert!(Registry::new(vec![cmd!("item.x", "X", &["Item"], None, "n: integer", always, count)]).is_err(), "no meaning");
}

/// What a query is has one answer: neither journaled nor undoable and run by its own handler, so
/// undo and redo, which are neither too, are not queries; an unknown id has no answer.
#[test]
fn a_query_is_neither_journaled_nor_undoable_and_runs_its_own_handler() {
    let reg = registry();
    let queries: Vec<&str> = reg.ids().filter(|id| reg.is_query(id) == Some(true)).collect();
    assert_eq!(queries, vec!["item.count"]);
    for id in ["edit.undo", "edit.redo", "view.select", "item.sneak", "item.add"] {
        assert_eq!(reg.is_query(id), Some(false), "{id}");
    }
    assert_eq!(reg.is_query("item.nope"), None);
    // The session runs a query as one: it may run while nothing else may, and changes nothing.
    let mut s = session();
    s.execute("item.add", &json!({ "name": "a" })).unwrap();
    let before = state(&s);
    assert_eq!(s.execute("item.count", &json!({})).unwrap().value, json!(1));
    assert_eq!(state(&s), before);
}

#[test]
fn every_command_applies_undoes_and_redoes_to_the_identical_document() {
    let mut s = session();
    round_trip(&mut s, "item.add", &json!({ "name": "A" })).unwrap();
    round_trip(&mut s, "item.add", &json!({ "name": "B" })).unwrap();
    round_trip(&mut s, "item.rename", &json!({ "index": 0, "name": "A2" })).unwrap();
    round_trip(&mut s, "item.move", &json!({ "from": 0, "to": 1 })).unwrap();
    round_trip(&mut s, "item.remove", &json!({ "index": 1 })).unwrap();
    assert_eq!(names(&s), ["B"]);
}

#[test]
fn every_refusal_leaves_everything_untouched() {
    let mut s = session();
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    let before = state(&s);
    let cases: Vec<(&str, Value)> = vec![
        ("item.nope", json!({})),
        ("item.add", json!([1])),
        ("item.add", json!({ "name": "x".repeat(2 * 1024 * 1024) })),
        ("item.add", json!({ "name": "A", "colour": 1 })),
        ("item.count", json!({ "coalesce": "k" })),
        ("item.rename", json!({ "name": "B" })),
        ("item.rename", json!({ "index": "0", "name": "B" })),
        ("item.rename", json!({ "index": 0.5, "name": "B" })),
        ("item.rename", json!({ "index": 5000, "name": "B" })),
        ("item.rename", json!({ "index": 0, "name": "x".repeat(70_000) })),
        ("item.rename", json!({ "index": 9, "name": "B" })),
        ("item.fail", json!({})),
        ("item.explode", json!({})),
        ("item.sneak", json!({})),
    ];
    for (id, params) in &cases {
        assert!(s.execute(id, params).is_err(), "{id} {params}");
        assert_eq!(state(&s), before, "{id} changed something");
    }
    let mut deep = json!(1);
    for _ in 0..40 {
        deep = json!([deep]);
    }
    assert!(matches!(s.execute("item.add", &json!({ "name": deep })), Err(EngineError::TooDeep { .. })));
    match s.execute("item.add", &json!({ "colour": 1 })) {
        Err(EngineError::UnknownKey { accepted, .. }) => assert_eq!(accepted, ["name", "coalesce"]),
        other => panic!("{other:?}"),
    }
    assert!(matches!(s.execute("item.explode", &json!({})), Err(EngineError::Internal { message, .. }) if message == "boom"));
    match s.execute("item.ad", &json!({})) {
        Err(EngineError::UnknownCommand { close, .. }) => assert!(close.contains(&"item.add".to_string())),
        other => panic!("{other:?}"),
    }
    let mut empty = session();
    assert!(matches!(empty.execute("item.remove", &json!({})), Err(EngineError::Disabled { reason: NOTHING_TO_REMOVE, .. })));
}

#[test]
fn the_view_is_never_undone_and_never_names_what_is_gone() {
    let mut s = session();
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    s.execute("item.add", &json!({ "name": "B" })).unwrap();
    s.mark_saved().unwrap();
    s.execute("view.select", &json!({ "index": 0 })).unwrap();
    assert!(!s.is_dirty(), "a view-only change keeps the project clean");
    assert_eq!(s.undo_label(), Some("Add item"), "selecting made no step");
    s.undo().unwrap();
    assert_eq!(*s.view(), Selection(Some(0)), "undo leaves the selection");
    // Adding selects the new item; undoing the add drops that selection.
    s.execute("item.add", &json!({ "name": "C" })).unwrap();
    assert_eq!(*s.view(), Selection(Some(1)));
    s.undo().unwrap();
    assert_eq!(*s.view(), Selection(None));
}

#[test]
fn unsaved_changes_follow_save_undo_and_redo() {
    let mut s = session();
    assert!(!s.is_dirty());
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    assert!(s.is_dirty());
    s.mark_saved().unwrap();
    assert!(!s.is_dirty());
    s.execute("item.add", &json!({ "name": "B" })).unwrap();
    assert!(s.is_dirty());
    s.undo().unwrap();
    assert!(!s.is_dirty(), "undo back to the saved state is clean");
    s.undo().unwrap();
    assert!(s.is_dirty(), "undo past it is not");
    s.redo().unwrap();
    assert!(!s.is_dirty(), "redo back to the saved state is clean");
    // The same document reached by new edits has a new state, so it is unsaved.
    s.execute("item.add", &json!({ "name": "B" })).unwrap();
    s.execute("item.remove", &json!({ "index": 1 })).unwrap();
    assert_eq!(names(&s), ["A"]);
    assert!(s.is_dirty());
}

#[test]
fn coalescing_merges_only_into_the_step_the_previous_call_made() {
    let mut s = session();
    for name in ["A", "B", "C"] {
        s.execute("item.add", &json!({ "name": name })).unwrap();
    }
    let steps_before = 3;
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "drag" })).unwrap();
    s.execute("item.move", &json!({ "from": 1, "to": 2, "coalesce": "drag" })).unwrap();
    let mut undos = 0;
    while s.can_undo() {
        s.undo().unwrap();
        undos += 1;
    }
    assert_eq!(undos, steps_before + 1, "two moves with one key are one step");
    while s.can_redo() {
        s.redo().unwrap();
    }
    // A different key, or an undo in between, breaks the chain.
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "a" })).unwrap();
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "b" })).unwrap();
    s.undo().unwrap();
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "a" })).unwrap();
    assert_eq!(s.undo_label(), Some("Move item"));
    s.undo().unwrap();
    s.undo().unwrap();
    assert_eq!(names(&s), ["B", "C", "A"], "the first 'a' move was not merged into");
    // A merge back to the saved start removes the step and is clean.
    s.mark_saved().unwrap();
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "back" })).unwrap();
    s.execute("item.move", &json!({ "from": 1, "to": 0, "coalesce": "back" })).unwrap();
    assert!(!s.is_dirty());
    assert_eq!(names(&s), ["B", "C", "A"]);
    // A save in between breaks the chain too.
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "s" })).unwrap();
    s.mark_saved().unwrap();
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "s" })).unwrap();
    s.undo().unwrap();
    assert!(!s.is_dirty(), "the step after the save stayed its own");
}

#[test]
fn a_step_made_for_an_origin_carries_it_through_undo_and_redo_and_never_merges_across_origins() {
    let mut s = session();
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    assert_eq!((s.undo_label(), s.undo_origin()), (Some("Add item"), None), "the user's own step has no origin");
    s.execute_from("item.add", &json!({ "name": "B" }), Some("Chat 1")).unwrap();
    assert_eq!((s.undo_label(), s.undo_origin()), (Some("Chat 1: Add item"), Some("Chat 1")));
    s.batch_from("Add two", Some("Chat 1"), |b| {
        b.execute("item.add", &json!({ "name": "C" }))?;
        b.execute("item.add", &json!({ "name": "D" }))?;
        Ok(())
    })
    .unwrap();
    assert_eq!((s.undo_label(), s.undo_origin()), (Some("Chat 1: Add two"), Some("Chat 1")));
    // Undo and redo keep the origin with its step.
    s.undo().unwrap();
    assert_eq!((s.redo_label(), s.redo_origin()), (Some("Chat 1: Add two"), Some("Chat 1")));
    assert_eq!(s.undo_origin(), Some("Chat 1"));
    s.redo().unwrap();
    assert_eq!(s.undo_label(), Some("Chat 1: Add two"));
    // A gesture's coalescing key never joins steps of two origins: the user's drag after the chat's
    // is a step of its own, and so is another chat's.
    s.execute_from("item.move", &json!({ "from": 0, "to": 1, "coalesce": "drag" }), Some("Chat 1")).unwrap();
    s.execute("item.move", &json!({ "from": 1, "to": 2, "coalesce": "drag" })).unwrap();
    assert_eq!((s.undo_label(), s.undo_origin()), (Some("Move item"), None));
    s.execute_from("item.move", &json!({ "from": 2, "to": 3, "coalesce": "drag" }), Some("Chat 2")).unwrap();
    assert_eq!(s.undo_origin(), Some("Chat 2"));
    s.undo().unwrap();
    assert_eq!(s.undo_origin(), None, "the user's move stayed its own step");
    s.undo().unwrap();
    assert_eq!(s.undo_label(), Some("Chat 1: Move item"), "and the chat's too");
    // The same origin and key still merge, as the user's own do.
    s.execute_from("item.move", &json!({ "from": 0, "to": 1, "coalesce": "g" }), Some("Chat 1")).unwrap();
    s.execute_from("item.move", &json!({ "from": 1, "to": 2, "coalesce": "g" }), Some("Chat 1")).unwrap();
    s.undo().unwrap();
    assert_eq!(s.undo_label(), Some("Chat 1: Move item"), "two moves of one origin and key were one step");
    // The batch label's cap is the label's own, the origin aside.
    let long = "x".repeat(256);
    assert!(s.batch_from(&long, Some("Chat 1"), |b| b.execute("item.add", &json!({ "name": "E" })).map(|_| ())).is_ok());
    assert!(s.batch_from(&format!("{long}x"), Some("Chat 1"), |b| b.execute("item.add", &json!({ "name": "F" })).map(|_| ())).is_err());
}

/// An origin's group is one undo step of its changes, ended by anyone else's change, and capped.
#[test]
fn a_group_makes_one_step_of_its_origins_changes_and_never_takes_anyone_elses() {
    let mut s = session();
    s.execute("item.add", &json!({ "name": "Mine" })).unwrap();
    assert!(s.end_group("Chat 1").is_none(), "no group, nothing to end");
    s.begin_group("Chat 1", "Turn 1").unwrap();
    assert!(s.grouping("Chat 1") && !s.grouping("Chat 2"));
    let before = s.revision();
    s.execute_from("item.add", &json!({ "name": "A" }), Some("Chat 1")).unwrap();
    s.execute_from("item.rename", &json!({ "index": 1, "name": "A2" }), Some("Chat 1")).unwrap();
    s.batch_from("Add two", Some("Chat 1"), |b| {
        b.execute("item.add", &json!({ "name": "B" }))?;
        b.execute("item.add", &json!({ "name": "C" }))?;
        Ok(())
    })
    .unwrap();
    // A query of the origin changes nothing and joins nothing.
    s.execute_from("item.count", &json!({}), Some("Chat 1")).unwrap();
    assert_eq!((s.undo_label(), s.undo_origin()), (Some("Chat 1: Turn 1"), Some("Chat 1")));
    assert_eq!(s.undo_revision(), Some(before), "one step, from the state before the group's first change");
    let after = s.revision();
    let end = s.end_group("Chat 1").unwrap();
    assert_eq!((end.before, end.after, end.steps, end.changes), (before, after, 1, 4));
    assert_eq!(
        end.delta.map(|d| (d.before.0, d.after.0)),
        Some((vec!["Mine".to_string()], vec!["Mine".to_string(), "A2".into(), "B".into(), "C".into()])),
        "the whole of what it changed"
    );
    // One undo takes the group back, and only the group: the user's own item stays.
    s.undo().unwrap();
    assert_eq!(names(&s), ["Mine"]);
    s.redo().unwrap();
    assert_eq!(names(&s), ["Mine", "A2", "B", "C"]);
    // Once ended, the origin's changes are steps of their own again.
    s.execute_from("item.add", &json!({ "name": "D" }), Some("Chat 1")).unwrap();
    assert_eq!(s.undo_label(), Some("Chat 1: Add item"));

    // Anyone else's change in between ends the group's step there; the group's next change starts
    // another, so undoing it never takes the change in between.
    s.begin_group("Chat 1", "Turn 2").unwrap();
    let before = s.revision();
    s.execute_from("item.add", &json!({ "name": "E" }), Some("Chat 1")).unwrap();
    s.execute("item.add", &json!({ "name": "Yours" })).unwrap();
    s.execute_from("item.add", &json!({ "name": "F" }), Some("Chat 2")).unwrap();
    s.execute_from("item.add", &json!({ "name": "G" }), Some("Chat 1")).unwrap();
    let end = s.end_group("Chat 1").unwrap();
    assert_eq!((end.before, end.steps, end.changes), (before, 2, 2));
    assert_ne!(s.undo_revision(), Some(end.before), "its last step does not start where the group did");
    s.undo().unwrap();
    assert_eq!(s.undo_origin(), Some("Chat 2"), "Chat 2's step stayed its own");
    s.undo().unwrap();
    assert_eq!(s.undo_label(), Some("Add item"), "and the user's too");

    // Undone and changed again, the group starts a new step rather than reaching under the undo.
    s.begin_group("Chat 3", "Turn 1").unwrap();
    s.execute_from("item.add", &json!({ "name": "H" }), Some("Chat 3")).unwrap();
    s.undo().unwrap();
    s.execute_from("item.add", &json!({ "name": "I" }), Some("Chat 3")).unwrap();
    assert_eq!(s.end_group("Chat 3").map(|e| e.steps), Some(2));
    // A group that changed nothing ends with nothing; a bad group is refused.
    s.begin_group("Chat 4", "Turn 1").unwrap();
    assert!(s.end_group("Chat 4").is_none());
    // A step that cancels out mid-group no longer counts: what follows is still one step.
    s.begin_group("Chat 7", "Turn 1").unwrap();
    let before = s.revision();
    s.execute_from("item.add", &json!({ "name": "K" }), Some("Chat 7")).unwrap();
    let last = s.doc().0.len() - 1;
    s.execute_from("item.remove", &json!({ "index": last }), Some("Chat 7")).unwrap();
    s.execute_from("item.add", &json!({ "name": "L" }), Some("Chat 7")).unwrap();
    assert_eq!(s.undo_revision(), Some(before));
    let end = s.end_group("Chat 7").unwrap();
    assert_eq!((end.before, end.steps), (before, 1));
    s.undo().unwrap();
    // Changes that cancel out leave nothing to undo, and no group.
    s.begin_group("Chat 6", "Turn 1").unwrap();
    s.execute_from("item.add", &json!({ "name": "J" }), Some("Chat 6")).unwrap();
    let last = s.doc().0.len() - 1;
    s.execute_from("item.remove", &json!({ "index": last }), Some("Chat 6")).unwrap();
    assert!(s.end_group("Chat 6").is_none());
    assert!(s.begin_group("", "Turn 1").is_err());
    assert!(s.begin_group("Chat 5", &"x".repeat(257)).is_err());
    for n in 0..archforge_engine::MAX_GROUPS {
        s.begin_group(&format!("G{n}"), "Turn 1").unwrap();
    }
    assert!(s.begin_group("one more", "Turn 1").is_err(), "the open groups are capped");
    assert!(s.begin_group("G0", "Turn 2").is_ok(), "an origin's group opened again replaces its own");
}

#[test]
fn a_batch_is_one_step_and_one_entry_or_nothing() {
    let mut s = session();
    s.batch("Add two", |b| {
        b.execute("item.add", &json!({ "name": "A" }))?;
        b.execute("item.rename", &json!({ "index": 0, "name": "A2", "coalesce": "x" }))?;
        b.execute("item.add", &json!({ "name": "B" }))?;
        b.execute("item.count", &json!({}))?;
        Ok(())
    })
    .unwrap();
    assert_eq!(names(&s), ["A2", "B"]);
    assert_eq!(s.undo_label(), Some("Add two"));
    let entry = s.journal().last().cloned().unwrap();
    match &entry {
        JournalEntry::Batch { commands, .. } => {
            assert_eq!(commands.len(), 3, "the query is not journaled");
            assert!(commands.iter().all(|(_, p)| p.get("coalesce").is_none()));
        }
        other => panic!("{other:?}"),
    }
    let before = state(&s);
    assert!(
        s.batch("Fails", |b| {
            b.execute("item.add", &json!({ "name": "X" }))?;
            b.execute("item.fail", &json!({}))?;
            Ok(())
        })
        .is_err()
    );
    assert!(
        s.batch("Panics", |b| {
            b.execute("item.add", &json!({ "name": "X" }))?;
            b.execute("item.explode", &json!({}))?;
            Ok(())
        })
        .is_err()
    );
    assert!(matches!(
        s.batch("Undo inside", |b| {
            b.execute("edit.undo", &json!({}))?;
            Ok(())
        }),
        Err(EngineError::NotInBatch { .. })
    ));
    assert!(
        s.batch("Sneaks", |b| {
            b.execute("item.sneak", &json!({}))?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(state(&s), before);
    // The entry replays to the same document on a fresh session.
    let mut fresh = session();
    fresh.replay(&entry).unwrap();
    assert_eq!(fresh.doc(), s.doc());
    s.undo().unwrap();
    assert!(names(&s).is_empty(), "one undo takes the whole batch back");
}

#[test]
fn a_gesture_previews_freely_and_commits_one_step() {
    let mut s = session();
    for name in ["A", "B", "C"] {
        s.execute("item.add", &json!({ "name": name })).unwrap();
    }
    let start = s.doc().clone();
    assert!(matches!(s.preview(&json!({})), Err(EngineError::NoInteraction)));
    s.begin_interaction("item.move").unwrap();
    assert!(matches!(s.begin_interaction("item.move"), Err(EngineError::InteractionRunning)));
    s.preview(&json!({ "from": 0, "to": 1 })).unwrap();
    s.preview(&json!({ "from": 0, "to": 2 })).unwrap();
    assert_eq!(names(&s), ["B", "C", "A"], "a preview runs on the state before the gesture");
    assert_eq!(s.execute("item.count", &json!({})).unwrap().value, json!(3), "a query runs meanwhile");
    for refused in [
        s.execute("item.add", &json!({})).err(),
        s.execute("view.select", &json!({ "index": 0 })).err(),
        s.undo().err(),
        s.redo().err(),
        s.mark_saved().err(),
    ] {
        assert!(matches!(refused, Some(EngineError::InteractionRunning)), "{refused:?}");
    }
    assert!(s.batch("x", |_| Ok(())).is_err());
    // A failing or panicking preview shows the state before the gesture.
    assert!(s.preview(&json!({ "from": 9, "to": 0 })).is_err());
    assert_eq!(*s.doc(), start);
    s.preview(&json!({ "from": 0, "to": 2 })).unwrap();
    s.commit().unwrap();
    assert_eq!(names(&s), ["B", "C", "A"]);
    s.undo().unwrap();
    assert_eq!(*s.doc(), start, "the whole gesture was one step");
    // An unchanged gesture records nothing; a cancelled one restores the start.
    let steps = s.can_redo();
    s.begin_interaction("item.move").unwrap();
    s.preview(&json!({ "from": 0, "to": 0 })).unwrap();
    s.commit().unwrap();
    assert_eq!(s.can_redo(), steps, "nothing was recorded, the redo stays");
    s.begin_interaction("item.move").unwrap();
    s.preview(&json!({ "from": 0, "to": 2 })).unwrap();
    s.cancel().unwrap();
    assert_eq!(*s.doc(), start);
    assert!(matches!(s.commit(), Err(EngineError::NoInteraction)));
}

#[test]
fn the_caps_drop_the_oldest_and_a_step_too_heavy_clears_the_history() {
    let options = Options { max_steps: 3, max_weight: 10_000, max_journal: 4, max_journal_bytes: 1_000_000 };
    let mut s = Session::new(registry(), Doc::default(), 1, options);
    for i in 0..6 {
        s.execute("item.add", &json!({ "name": format!("{i}") })).unwrap();
    }
    let mut undos = 0;
    while s.can_undo() {
        s.undo().unwrap();
        undos += 1;
    }
    assert_eq!(undos, 3);
    assert_eq!(s.journal().count(), 4);
    assert!(s.dropped().steps >= 3 && s.dropped().journal >= 2);
    while s.can_redo() {
        s.redo().unwrap();
    }
    let out = s.execute("item.add", &json!({ "name": "x".repeat(20_000) })).unwrap();
    assert!(out.history_cleared, "a step whose delta is over the whole cap clears the history");
    assert!(!s.can_undo() && !s.can_redo());
    let out = s.execute("item.add", &json!({ "name": "y" })).unwrap();
    assert!(out.history_cleared, "every later delta carries the heavy state too");
    // The journal's byte cap.
    let options = Options { max_journal_bytes: 300, ..Options::default() };
    let mut s = Session::new(registry(), Doc::default(), 1, options);
    for _ in 0..5 {
        s.execute("item.add", &json!({ "name": "z".repeat(100) })).unwrap();
    }
    assert!(s.journal().count() < 5 && s.dropped().journal > 0);
}

#[test]
fn a_panic_in_the_notations_own_code_changes_nothing_even_in_undo() {
    let mut s = session();
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    let before = state(&s);
    assert!(matches!(s.execute("item.add", &json!({ "name": "boom-view" })), Err(EngineError::Internal { message, .. }) if message == "view"));
    assert_eq!(state(&s), before);
    // A step whose delta cannot be weighed is rolled back whole.
    assert!(matches!(s.execute("item.add", &json!({ "name": "boom-weight" })), Err(EngineError::Internal { message, .. }) if message == "weight"));
    assert_eq!(state(&s), before);
    s.undo().unwrap();
    assert!(names(&s).is_empty(), "undo works on stored weights and never weighs again");
}

#[test]
fn the_sweep_reports_a_contained_panic() {
    let mut s = session();
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    let found = archforge_engine::testing::sweep(&mut s, &["0", "1", "t1"]).unwrap_err();
    assert!(found.starts_with("item.explode") && found.contains("panicked"), "{found}");
}

#[test]
fn the_id_counter_is_never_undone_and_a_replay_makes_the_same_ids() {
    let mut s = session();
    let first = s.execute("item.tag", &json!({})).unwrap().value;
    s.undo().unwrap();
    let second = s.execute("item.tag", &json!({})).unwrap().value;
    assert_ne!(first, second, "an id freed by undo is not given again");
    let counter = s.counter();
    assert!(s.execute("item.tag", &json!({ "fail": true })).is_err());
    assert_eq!(s.counter(), counter, "a failed call leaves the counter");
    s.batch("Tags", |b| {
        b.execute("item.tag", &json!({}))?;
        b.execute("item.tag", &json!({}))?;
        Ok(())
    })
    .unwrap();
    let entries: Vec<JournalEntry> = s.journal().cloned().collect();
    let mut fresh = session();
    for e in &entries {
        fresh.replay(e).unwrap();
    }
    assert_eq!(fresh.doc(), s.doc(), "the same ids, from the same start counter");
    assert_eq!(fresh.counter(), s.counter());
}

#[test]
fn the_counter_moves_only_with_a_change_that_is_kept() {
    let mut s = session();
    let c0 = s.counter();
    assert!(matches!(s.execute("item.burn", &json!({})), Err(EngineError::Failed { .. })), "an id taken for nothing is refused");
    assert_eq!(s.counter(), c0);
    // A batch keeps its counter only whole.
    assert!(
        s.batch("half", |b| {
            b.execute("item.tag", &json!({}))?;
            b.execute("item.fail", &json!({}))?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(s.counter(), c0);
    // Every preview starts from the gesture's counter; commit keeps the last one's, cancel restores.
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    let c1 = s.counter();
    s.begin_interaction("item.label").unwrap();
    s.preview(&json!({ "index": 0, "text": "" })).unwrap();
    s.preview(&json!({ "index": 0, "text": "" })).unwrap();
    assert_eq!((names(&s), s.counter()), (vec![format!("t{c1}").as_str()], c1 + 1));
    s.cancel().unwrap();
    assert_eq!((names(&s), s.counter()), (vec!["A"], c1));
    s.begin_interaction("item.label").unwrap();
    s.preview(&json!({ "index": 0, "text": "A" })).unwrap();
    s.commit().unwrap();
    assert_eq!(s.counter(), c1, "a gesture that changed nothing restores the counter");
    s.begin_interaction("item.label").unwrap();
    s.preview(&json!({ "index": 0, "text": "" })).unwrap();
    s.commit().unwrap();
    assert_eq!(s.counter(), c1 + 1);
    // A coalesced chain that merges back to its start keeps the advanced counter.
    s.mark_saved().unwrap();
    let before = s.counter();
    s.execute("item.label", &json!({ "index": 0, "text": "", "coalesce": "k" })).unwrap();
    s.execute("item.label", &json!({ "index": 0, "text": format!("t{c1}"), "coalesce": "k" })).unwrap();
    assert!(!s.is_dirty());
    assert_eq!(s.counter(), before + 1, "the id taken is not handed out again");
}

#[test]
fn a_session_opened_over_unsaved_changes_stays_unsaved_until_a_save() {
    let mut s = session();
    s.mark_unsaved();
    assert!(s.is_dirty());
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    s.undo().unwrap();
    assert!(s.is_dirty(), "undo back to the opened state is still unsaved");
    s.mark_saved().unwrap();
    assert!(!s.is_dirty());
}

#[test]
fn a_counter_only_call_is_refused_inside_a_batch_too() {
    let mut s = session();
    let c = s.counter();
    assert!(
        s.batch("burns", |b| {
            b.execute("item.tag", &json!({}))?;
            b.execute("item.burn", &json!({}))?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(s.counter(), c);
}

#[test]
fn a_trimmed_journal_says_where_its_counter_starts() {
    let options = Options { max_journal: 2, ..Options::default() };
    let mut s = Session::new(registry(), Doc::default(), 1, options);
    for _ in 0..4 {
        s.execute("item.tag", &json!({})).unwrap();
    }
    assert_eq!(s.journal().count(), 2);
    assert_eq!(s.journal_start_counter(), 3, "the two kept entries took ids 3 and 4");
}

#[test]
fn a_call_that_changes_nothing_keeps_a_gesture_one_step() {
    let mut s = session();
    for name in ["A", "B", "C"] {
        s.execute("item.add", &json!({ "name": name })).unwrap();
    }
    s.mark_saved().unwrap();
    s.execute("item.move", &json!({ "from": 0, "to": 1, "coalesce": "k" })).unwrap();
    s.execute("item.move", &json!({ "from": 1, "to": 1, "coalesce": "k" })).unwrap();
    s.execute("item.move", &json!({ "from": 1, "to": 2, "coalesce": "k" })).unwrap();
    s.undo().unwrap();
    assert!(!s.is_dirty(), "the three moves were one step");
}

#[test]
fn undo_and_redo_run_as_commands_and_say_why_when_there_is_nothing() {
    let mut s = session();
    assert!(matches!(s.execute("edit.undo", &json!({})), Err(EngineError::Disabled { reason: archforge_engine::cmd::edit::NOTHING_TO_UNDO, .. })));
    s.execute("item.add", &json!({ "name": "A" })).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(names(&s).is_empty());
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(names(&s), ["A"]);
    assert!(matches!(s.execute("edit.redo", &json!({})), Err(EngineError::Disabled { .. })));
    // Undo called directly, as a shortcut would, is journaled as the command is.
    let n = s.journal().count();
    s.undo().unwrap();
    assert_eq!(s.journal().count(), n + 1);
    s.redo().unwrap();
    // A journal that was never trimmed replays to the same document.
    let entries: Vec<JournalEntry> = s.journal().cloned().collect();
    let mut fresh = session();
    for e in &entries {
        fresh.replay(e).unwrap();
    }
    assert_eq!(fresh.doc(), s.doc());
}

/// A small fixed-seed generator, so the property test needs no dependency and always runs the
/// same sequences.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

#[test]
fn random_sequences_undo_to_the_start_and_redo_to_the_end_and_clean_means_saved() {
    for seed in 1..=40u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut s = session();
        let start = s.doc().clone();
        let mut saved = s.doc().clone();
        for _ in 0..60 {
            let len = s.doc().0.len() as u64;
            let r = match rng.below(9) {
                0 | 1 => s.execute("item.add", &json!({ "name": format!("n{}", rng.below(5)) })).map(|_| ()),
                2 if len > 0 => s.execute("item.rename", &json!({ "index": rng.below(len), "name": format!("r{}", rng.below(5)) })).map(|_| ()),
                3 if len > 0 => s.execute("item.remove", &json!({ "index": rng.below(len) })).map(|_| ()),
                4 if len > 0 => s
                    .execute("item.move", &json!({ "from": rng.below(len), "to": rng.below(len), "coalesce": format!("k{}", rng.below(2)) }))
                    .map(|_| ()),
                5 => s.undo().map(|_| ()),
                6 => s.redo().map(|_| ()),
                7 => s.mark_saved().map(|_| {
                    saved = s.doc().clone();
                }),
                _ if len > 0 => s.execute("view.select", &json!({ "index": rng.below(len) })).map(|_| ()),
                _ => Ok(()),
            };
            let _ = r;
            if !s.is_dirty() {
                assert_eq!(*s.doc(), saved, "seed {seed}: clean but not the saved document");
            }
            assert!(s.view().0.is_none_or(|i| i < s.doc().0.len()), "seed {seed}: the view names a missing item");
        }
        // A sequence may end after undos: the newest state is at the top of the redo stack.
        while s.can_redo() {
            s.redo().unwrap();
        }
        let end = s.doc().clone();
        let mut undone = 0;
        while s.can_undo() {
            s.undo().unwrap();
            undone += 1;
        }
        if undone > 0 && s.dropped().steps == 0 {
            assert_eq!(*s.doc(), start, "seed {seed}: undo all");
        }
        while s.can_redo() {
            s.redo().unwrap();
        }
        assert_eq!(*s.doc(), end, "seed {seed}: redo all");
    }
}
