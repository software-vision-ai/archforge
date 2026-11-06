//! N-ary associations (AR-128): an association with three ends or more, UML's Association with as
//! many memberEnds, each a Property the association owns, typed by the classifier at that end. It
//! is one element however many legs it has: on a class diagram it is a diamond, a node of the
//! association itself, with a leg from the diamond to the card of each end's classifier, an edge of
//! that end's Property. The diamond's place is the diagram's own, kept in the diagram's existing
//! node rows; the legs are worked out of the ends wherever the diagram shows the diamond and the
//! end's classifier (`diagram.connect`, `diagram.connect-all`), so nothing new is stored.
//!
//! A classifier may sit at several ends of one association, each end then with a role of its own:
//! the ends are properties of the association, told apart by name whatever the case
//! ([`crate::names`]). An n-ary association's ends are never aggregations (UML 2.5.1, 11.8.1.8:
//! only binary associations may be aggregations), so none takes one; each is navigable, ordered
//! and unique as an attribute is. Ends are added, removed and moved onto another classifier by
//! the commands here, each one undo step that keeps every diagram's legs in step. An n-ary
//! association keeps at least [`MIN_ENDS`] ends: removing one of three is refused unless the call
//! says to make it binary, which turns it into a plain association between the two classifiers
//! left, its diamond giving way to a line on every diagram that shows both, never silently.

use std::collections::BTreeSet;

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, cmd};
use archforge_mof::{Obj, Store, Value};
use serde_json::{Value as Json, json};

use crate::commands::{a_what, classifier_of, fail, id_of, name_of, refuse, set_name, short, text, uuid_of};
use crate::{ASSOCIATION, PROPERTY, association, package, property, typed_element};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The fewest ends an n-ary association has: two make a binary one.
pub const MIN_ENDS: usize = 3;
/// The most ends one association takes: its legs are drawn every frame, so they never grow with
/// what a call asks for.
pub const MAX_ENDS: usize = 32;
/// The diamond's width and height as it is placed, the design's N-ary diamond.
pub const DIAMOND_SIZE: (f64, f64) = (44.0, 28.0);
/// The multiplicity a new end starts at: none to many, until it is changed.
pub const NEW_MULTIPLICITY: &str = "*";

fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

/// Every end of association `a`, in its member order, however many it has.
pub fn ends(store: &Store, a: Obj) -> Vec<Obj> {
    list(store, a, association::MEMBER_END)
}

/// Whether `a` is an n-ary association: a plain association with [`MIN_ENDS`] ends or more.
pub fn is_nary(store: &Store, a: Obj) -> bool {
    store.class_of(a).ok() == Some(ASSOCIATION) && ends(store, a).len() >= MIN_ENDS
}

/// The n-ary association whose end `end` is, when it is one's.
pub fn association_of_end(store: &Store, end: Obj) -> Option<Obj> {
    if store.class_of(end).ok() != Some(PROPERTY) {
        return None;
    }
    one(store, end, property::ASSOCIATION).filter(|a| is_nary(store, *a))
}

/// The classifier at end `end`: its type.
pub fn end_type(store: &Store, end: Obj) -> Option<Obj> {
    one(store, end, typed_element::TYPE)
}

/// What a leg joins, as a diagram's edge of end `end` runs: from its association's diamond to the
/// classifier at the end. None for anything but an n-ary association's end with a type.
pub fn leg_ends(store: &Store, end: Obj) -> Option<(Obj, Obj)> {
    Some((association_of_end(store, end)?, end_type(store, end)?))
}

/// The role a new end at classifier `c` takes: the classifier's name with its first letter in
/// lower case (`Customer` gives `customer`), or `end` where that is no name, then a number after
/// it (`customer2`) until no other end's role, `taken`, is alike whatever the case.
pub fn default_role(store: &Store, c: Obj, taken: &[String]) -> String {
    let own = name_of(store, c);
    let mut chars = own.chars();
    let lowered: String = chars.next().map(|f| f.to_lowercase().chain(chars).collect()).unwrap_or_default();
    // A role leaves room for the number after it.
    let stem = if lowered.is_empty() || archforge_core::skills::name_problem(&lowered, crate::commands::MAX_NAME.saturating_sub(4)).is_some() {
        "end".to_string()
    } else {
        lowered
    };
    let taken: BTreeSet<String> = taken.iter().map(|t| t.to_lowercase()).collect();
    if !taken.contains(&stem.to_lowercase()) {
        return stem;
    }
    // At most MAX_ENDS roles are taken, so a free number is among the next few.
    (2..=MAX_ENDS.saturating_add(2)).map(|n| format!("{stem}{n}")).find(|r| !taken.contains(&r.to_lowercase())).unwrap_or(stem)
}

/// The diamonds of association `a`: each node that shows it, with its diagram.
pub fn diamonds(store: &Store, a: Obj) -> Vec<(Obj, Obj)> {
    let (_, _, nodes) = archforge_diagram::views_of(store, a);
    nodes.into_iter().filter_map(|n| archforge_diagram::diagram_of(store, n).map(|d| (d, n))).collect()
}

/// The first node of diagram `d`, at any depth, that shows `element`.
pub fn node_in(store: &Store, d: Obj, element: Obj) -> Option<Obj> {
    let (_, _, nodes) = archforge_diagram::views_of(store, element);
    let mut on: Vec<Obj> = nodes.into_iter().filter(|n| archforge_diagram::diagram_of(store, *n) == Some(d)).collect();
    on.sort();
    on.first().copied()
}

/// A new edge of `element` on diagram `d`, from `source` to `target`.
fn new_edge(store: &mut Store, d: Obj, element: Obj, source: Obj, target: Obj) -> Result<Obj, CommandError> {
    use archforge_diagram::{EDGE, diagram, edge};
    let e = store.create(EDGE).map_err(fail)?;
    store.set(e, edge::ELEMENT, Value::Ref(element)).map_err(fail)?;
    store.set(e, edge::SOURCE, Value::Ref(source)).map_err(fail)?;
    store.set(e, edge::TARGET, Value::Ref(target)).map_err(fail)?;
    store.push(d, diagram::EDGE, Value::Ref(e)).map_err(fail)?;
    Ok(e)
}

/// Draws end `end`'s leg on every diagram that shows its association's diamond and its classifier
/// and has no leg of it yet.
fn draw_legs(store: &mut Store, a: Obj, end: Obj) -> Result<(), CommandError> {
    let Some(ty) = end_type(store, end) else { return Ok(()) };
    let (_, shown, _) = archforge_diagram::views_of(store, end);
    for (d, diamond) in diamonds(store, a) {
        if shown.iter().any(|e| store.container(*e).ok().flatten().map(|(c, _)| c) == Some(d)) {
            continue;
        }
        if let Some(card) = node_in(store, d, ty) {
            new_edge(store, d, end, diamond, card)?;
        }
    }
    Ok(())
}

/// Moves the legs of association `a`'s ends onto the cards of the classifiers now at those ends,
/// on each diagram; a leg whose classifier the diagram does not show goes, and a moved leg drops
/// the bends it had for its old card.
pub fn follow_legs(store: &mut Store, a: Obj) -> Result<(), CommandError> {
    use archforge_diagram::edge;
    for end in ends(store, a) {
        let ty = end_type(store, end);
        let (_, legs, _) = archforge_diagram::views_of(store, end);
        for leg in legs {
            let Some((d, _)) = store.container(leg).map_err(fail)? else { continue };
            let target = one(store, leg, edge::TARGET);
            let showing = |n: Obj| one(store, n, archforge_diagram::node::ELEMENT);
            if target.is_some_and(|t| showing(t) == ty && ty.is_some()) {
                continue;
            }
            let Some(card) = ty.and_then(|t| node_in(store, d, t)) else {
                archforge_diagram::destroy_edge_with_links(store, leg).map_err(fail)?;
                continue;
            };
            store.set(leg, edge::TARGET, Value::Ref(card)).map_err(fail)?;
            for w in list(store, leg, edge::WAYPOINT) {
                store.destroy(w).map_err(fail)?;
            }
        }
    }
    Ok(())
}

/// Makes a new end of association `a` at classifier `c` with `role`, starting at
/// [`NEW_MULTIPLICITY`]: owned by the association, navigable from none, no aggregation.
fn new_end(store: &mut Store, a: Obj, c: Obj, role: &str) -> Result<Obj, CommandError> {
    let end = store.create(PROPERTY).map_err(fail)?;
    store.set(end, typed_element::TYPE, Value::Ref(c)).map_err(fail)?;
    // A member end first, which tells the end its association, then owned by it.
    store.push(a, association::MEMBER_END, Value::Ref(end)).map_err(fail)?;
    if !store.list(a, association::OWNED_END).map_err(fail)?.contains(&Value::Ref(end)) {
        store.push(a, association::OWNED_END, Value::Ref(end)).map_err(fail)?;
    }
    set_name(store, end, role)?;
    crate::members::set_multiplicity(store, end, NEW_MULTIPLICITY, "multiplicity")?;
    Ok(end)
}

/// Why `role` may not be the role of an end of association `a` (other than `except`): a name that
/// does not read, or one another end has, whatever the case.
pub fn role_problem(store: &Store, a: Obj, role: &str, except: Option<Obj>) -> Option<String> {
    if role.is_empty() {
        return None;
    }
    if let Some(p) = archforge_core::skills::name_problem(role, crate::commands::MAX_NAME) {
        return Some(format!("the role {p}"));
    }
    let other = crate::names::clash(store, a, PROPERTY, role, except)?;
    Some(format!(
        "another end of the association, {}, has the role {} already; each end takes a role of its own",
        uuid_of(store, other).unwrap_or_default(),
        name_of(store, other)
    ))
}

/// The classifiers the ids under `key` name, in order, repeats kept: an association may have
/// several ends at one classifier.
fn classifiers_of(store: &Store, p: &Params, key: &str) -> Result<Vec<Obj>, CommandError> {
    let items = p.get(key).and_then(Json::as_array).map(Vec::as_slice).unwrap_or_default();
    if items.len() < MIN_ENDS || items.len() > MAX_ENDS {
        return Err(refuse(
            key,
            format!(
                "an n-ary association joins {MIN_ENDS} to {MAX_ENDS} ends; {} given (two make a binary association: uml.association.add)",
                items.len()
            ),
        ));
    }
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        let id = v.as_str().ok_or_else(|| refuse(key, "every id must be a string"))?;
        let c = archforge_mof::Uuid::parse(id)
            .and_then(|u| store.by_uuid(u))
            .filter(|o| store.is_alive(*o))
            .ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))?;
        if !store.class_of(c).is_ok_and(crate::is_classifier) {
            return Err(refuse(key, format!("{} is {}; an association's ends are classes and interfaces", short(id), a_what(store, c))));
        }
        out.push(c);
    }
    Ok(out)
}

/// The n-ary association under `key`.
fn nary_of(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let a = id_of(store, p, key, ASSOCIATION, "an association")?;
    let n = ends(store, a).len();
    if n < MIN_ENDS {
        return Err(refuse(
            key,
            format!("{} has {n} ends; ends are added and removed only on an n-ary association, of {MIN_ENDS} ends or more", uuid_of(store, a)?),
        ));
    }
    Ok(a)
}

/// The end of `a` whose place among its ends is under `key`.
fn end_at(store: &Store, a: Obj, p: &Params, key: &str) -> Result<(usize, Obj), CommandError> {
    let all = ends(store, a);
    let last = all.len().saturating_sub(1);
    let i = p.integer(key).and_then(|i| usize::try_from(i).ok()).filter(|i| *i <= last);
    let i = i.ok_or_else(|| refuse(key, format!("the end is one of 0 to {last}")))?;
    all.get(i).map(|e| (i, *e)).ok_or_else(|| refuse(key, format!("the end is one of 0 to {last}")))
}

fn has_classes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(crate::is_classifier)) { Ok(()) } else { Err(crate::members::NO_CLASSES) }
}

/// Why the end commands are not available.
pub const NO_NARY: &str = "There is no n-ary association yet.";

fn has_nary(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| is_nary(f.doc, o)) { Ok(()) } else { Err(NO_NARY) }
}

/// `uml.association.add-nary`: one association with an end at each classifier given, three or
/// more, in the first one's package; a classifier given twice has two ends. Each end is named
/// after its classifier and starts at `*`, navigable from none, no aggregation.
fn add_nary(ctx: &mut C<'_>, p: &Params) -> R {
    let classes = classifiers_of(ctx.doc, p, "classes")?;
    for c in &classes {
        if let Some(why) = crate::relations::association_end_problem(ctx.doc, *c) {
            return Err(refuse("classes", why));
        }
    }
    let first = classes.first().copied().ok_or_else(|| refuse("classes", "no classifier is given"))?;
    let pkg = crate::nesting::home_package(ctx.doc, first)
        .ok_or_else(|| refuse("classes", format!("{} is in no package", uuid_of(ctx.doc, first).unwrap_or_default())))?;
    let name = text(p, "name");
    if !name.is_empty()
        && let Some(problem) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME)
    {
        return Err(refuse("name", format!("the name {problem}")));
    }
    let a = ctx.doc.create(ASSOCIATION).map_err(fail)?;
    set_name(ctx.doc, a, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(a)).map_err(fail)?;
    let mut roles: Vec<String> = Vec::new();
    let mut made = Vec::new();
    for c in classes {
        let role = default_role(ctx.doc, c, &roles);
        made.push(new_end(ctx.doc, a, c, &role)?);
        roles.push(role);
    }
    ctx.view.selection = vec![a];
    let ids = made.iter().map(|e| uuid_of(ctx.doc, *e)).collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)?, "ends": ids }))
}

/// `uml.association.add-end`: one more end of an n-ary association, at a classifier, its leg drawn
/// on every diagram that shows the diamond and the classifier.
fn add_end(ctx: &mut C<'_>, p: &Params) -> R {
    let a = nary_of(ctx.doc, p, "association")?;
    let c = classifier_of(ctx.doc, p, "class")?;
    if let Some(why) = crate::relations::association_end_problem(ctx.doc, c) {
        return Err(refuse("class", why));
    }
    let all = ends(ctx.doc, a);
    if all.len() >= MAX_ENDS {
        return Err(refuse("association", format!("{} has {MAX_ENDS} ends, the most an association takes", uuid_of(ctx.doc, a)?)));
    }
    let taken: Vec<String> = all.iter().map(|e| name_of(ctx.doc, *e)).collect();
    let role = match p.str("role") {
        Some(r) => r.to_string(),
        None => default_role(ctx.doc, c, &taken),
    };
    if let Some(why) = role_problem(ctx.doc, a, &role, None) {
        return Err(refuse("role", why));
    }
    let end = new_end(ctx.doc, a, c, &role)?;
    draw_legs(ctx.doc, a, end)?;
    ctx.view.selection = vec![a];
    Ok(json!({ "id": uuid_of(ctx.doc, end)?, "association": uuid_of(ctx.doc, a)? }))
}

/// Gives the name label at its middle to binary line `e`, as drawing one does.
fn name_label(store: &mut Store, e: Obj) -> Result<(), CommandError> {
    use archforge_diagram::{LABEL, LabelPlace, edge, label};
    let l = store.create(LABEL).map_err(fail)?;
    store.set(l, label::KIND, Value::str("name")).map_err(fail)?;
    store.set(l, label::AT, Value::Enum(LabelPlace::Middle.index())).map_err(fail)?;
    store.set(l, label::DX, Value::Real(0.0)).map_err(fail)?;
    store.set(l, label::DY, Value::Real(0.0)).map_err(fail)?;
    store.push(e, edge::LABEL, Value::Ref(l)).map_err(fail)?;
    Ok(())
}

/// `uml.association.remove-end`: one end of an n-ary association goes with its legs. Of three
/// ends, two would remain: refused unless `binary` says to make it a binary association, whose
/// diamond then gives way, on every diagram that shows both classifiers left, to a line with its
/// name. What the ends left subset or redefine that the rules no longer allow goes too, listed.
fn remove_end(ctx: &mut C<'_>, p: &Params) -> R {
    let a = nary_of(ctx.doc, p, "association")?;
    let (_, end) = end_at(ctx.doc, a, p, "end")?;
    let count = ends(ctx.doc, a).len();
    let binary = p.bool("binary").unwrap_or(false);
    let id = uuid_of(ctx.doc, a)?;
    if count == MIN_ENDS && !binary {
        return Err(refuse(
            "binary",
            format!(
                "{id} has {MIN_ENDS} ends, and without {} only two would remain: an n-ary association needs three ends or more. Say binary: true to make it a binary association between the two classifiers left, or keep its ends",
                short(&name_of(ctx.doc, end))
            ),
        ));
    }
    if count > MIN_ENDS && binary {
        return Err(refuse(
            "binary",
            format!("{id} keeps {} ends without this one, so it stays n-ary; binary is said only when two would remain", count - 1),
        ));
    }
    // A constraint left constraining nothing goes with the end, as with any element that goes.
    let orphans = crate::constraints::orphans(&crate::constraints::dropped_by(ctx.doc, &[end]));
    let diamonds = diamonds(ctx.doc, a);
    archforge_diagram::remove_views_of(ctx.doc, end).map_err(fail)?;
    ctx.doc.destroy(end).map_err(fail)?;
    for o in orphans {
        if ctx.doc.is_alive(o) {
            archforge_diagram::remove_views_of(ctx.doc, o).map_err(fail)?;
            ctx.doc.destroy(o).map_err(fail)?;
        }
    }
    let mut dropped: Vec<Json> = Vec::new();
    let left = ends(ctx.doc, a);
    for e in &left {
        for (list, t) in crate::modifiers::drop_stale(ctx.doc, *e)? {
            dropped.push(json!({ "end": uuid_of(ctx.doc, *e)?, "list": list, "id": uuid_of(ctx.doc, t)?, "name": name_of(ctx.doc, t) }));
        }
    }
    let mut lines = Vec::new();
    if binary {
        let types: Vec<Option<Obj>> = left.iter().map(|e| end_type(ctx.doc, *e)).collect();
        for (d, diamond) in diamonds {
            if ctx.doc.is_alive(diamond) {
                archforge_diagram::destroy_node_with_edges(ctx.doc, diamond).map_err(fail)?;
            }
            let cards: Vec<Option<Obj>> = types.iter().map(|t| t.and_then(|t| node_in(ctx.doc, d, t))).collect();
            if let [Some(from), Some(to)] = cards.as_slice() {
                let e = new_edge(ctx.doc, d, a, *from, *to)?;
                name_label(ctx.doc, e)?;
                lines.push(uuid_of(ctx.doc, e)?);
            }
        }
    }
    ctx.view.selection = vec![a];
    Ok(json!({ "id": id, "ends": left.len(), "binary": binary, "lines": lines, "dropped": dropped }))
}

/// `uml.association.reconnect-end`: one end of an n-ary association moved onto another
/// classifier, its legs following on every diagram; what the ends subset or redefine that the
/// rules no longer allow goes, listed. A classifier another end has already is fine: the ends'
/// roles tell them apart.
fn reconnect_end(ctx: &mut C<'_>, p: &Params) -> R {
    let a = nary_of(ctx.doc, p, "association")?;
    let (_, end) = end_at(ctx.doc, a, p, "end")?;
    let c = classifier_of(ctx.doc, p, "class")?;
    if let Some(why) = crate::relations::association_end_problem(ctx.doc, c) {
        return Err(refuse("class", why));
    }
    let mut dropped: Vec<Json> = Vec::new();
    if end_type(ctx.doc, end) != Some(c) {
        ctx.doc.set(end, typed_element::TYPE, Value::Ref(c)).map_err(fail)?;
        for e in ends(ctx.doc, a) {
            for (list, t) in crate::modifiers::drop_stale(ctx.doc, e)? {
                dropped.push(json!({ "end": uuid_of(ctx.doc, e)?, "list": list, "id": uuid_of(ctx.doc, t)?, "name": name_of(ctx.doc, t) }));
            }
        }
        follow_legs(ctx.doc, a)?;
        draw_legs(ctx.doc, a, end)?;
    }
    ctx.view.selection = vec![a];
    Ok(json!({ "id": uuid_of(ctx.doc, end)?, "class": uuid_of(ctx.doc, c)?, "kind": a_what(ctx.doc, c), "dropped": dropped }))
}

/// The n-ary associations' commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.association.add-nary",
            "Add n-ary association",
            &["Association"],
            None,
            concat!(
                "classes: array; the ids of the classes or interfaces at its ends, 3 to 32, in order; one given twice has two ends, told apart by their roles; never an enumeration, a data type or a primitive type; it goes in the first one's package\n",
                "name: string = \"\"; the association's name, or nothing; each end's role is its class's name with a lower-case first letter (customer, customer2 for a second end there) and its multiplicity *, navigable from none and no aggregation, until set with uml.association.set-end"
            ),
            has_classes,
            add_nary
        ),
        cmd!(
            "uml.association.add-end",
            "Add association end",
            &["Association"],
            None,
            concat!(
                "association: string; the id of an n-ary association, of three ends or more; a binary one takes no third end\n",
                "class: string; the id of the class or interface at the new end; one at another end already is fine; never an enumeration, a data type or a primitive type\n",
                "role: string?; the new end's role, free among its ends' roles whatever the case; left out, its class's name with a lower-case first letter, numbered when taken"
            ),
            has_nary,
            add_end
        ),
        cmd!(
            "uml.association.remove-end",
            "Remove association end",
            &["Association"],
            None,
            concat!(
                "association: string; the id of an n-ary association\n",
                "end: integer, 0..31; which end, by its place among the ends from 0\n",
                "binary: bool = false; with three ends, two would remain: refused unless true, which makes it a binary association between the two classifiers left, a line with its name in place of the diamond on each diagram that shows both; refused when more than two remain"
            ),
            has_nary,
            remove_end
        ),
        cmd!(
            "uml.association.reconnect-end",
            "Reconnect association end",
            &["Association"],
            None,
            concat!(
                "association: string; the id of an n-ary association\n",
                "end: integer, 0..31; which end, by its place among the ends from 0\n",
                "class: string; the id of the class or interface the end moves to; one at another end already is fine; never an enumeration, a data type or a primitive type; the end's legs follow it on every diagram, and what the ends subset or redefine that the rules no longer allow is dropped, listed under dropped"
            ),
            has_nary,
            reconnect_end
        ),
    ]
}
