//! Generalization sets (AR-129): a UML GeneralizationSet groups generalizations that specialize one
//! general classifier, saying whether its specific classifiers cover every instance of the general
//! one (`isCovering`, written `complete` or `incomplete`) and whether an instance may be of several
//! of them (`isDisjoint`, written `disjoint` or `overlapping`). No table or column of its own: the
//! metamodel's GeneralizationSet holds it, owned by the package of its general classifier
//! (`packagedElement`), its members named by `GeneralizationSet::generalization`, whose opposite
//! `Generalization::generalizationSet` the store keeps in step. A generalization may belong to
//! several sets; each membership is explicit.
//!
//! The rules, kept here before anything changes: every member is a generalization of the model,
//! named once, and all members specialize the same general classifier; a set's name is told apart
//! from its package's other sets' whatever the case. A generalization in a set keeps its ends: a
//! reconnection of it is refused, naming its sets ([`reconnect_problem`]). Removing a set keeps its
//! generalizations; removing generalizations takes them out of their sets, and a set left with no
//! member goes too, in the same step ([`emptied_by`], [`crate::commands::remove_elements`]), as a
//! set left empty by `uml.generalization-set.set` does.
//!
//! How a diagram shows a set is the diagram's own: the canvas draws a dashed line across the
//! member lines it shows, with `name {complete, disjoint}` beside it, worked out from the model.
//! Where the label sits, once moved, is a node of the kind [`LABEL`] showing the set, nested in
//! the node of its general classifier so it moves with it ([`place`]). No view record is new.

use std::collections::BTreeSet;

use archforge_diagram::{ProjectView, diagram, node};
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, cmd};
use archforge_mof::{Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::commands::{fail, name_of, refuse, set_name, short, text, uuid_of};
use crate::{GENERALIZATION, GENERALIZATION_SET, generalization, generalization_set, package};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The kind of a node that holds where a diagram shows a set's label.
pub const LABEL: &str = "uml.generalization-set.label";
/// The most generalizations one set groups.
pub const MAX_MEMBERS: usize = 256;
/// A label's height, and how wide one character of it is taken to be, for its node.
const LABEL_HEIGHT: f64 = 20.0;
const LABEL_CHAR: f64 = 6.5;
const LABEL_MAX_WIDTH: f64 = 480.0;

/// Why a command is not available.
pub const NO_GENERALIZATIONS: &str = "There is no generalization yet.";
const NO_SETS: &str = "There is no generalization set yet.";

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn refs(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

/// Whether `o` is a generalization set.
pub fn is_set(store: &Store, o: Obj) -> bool {
    store.is_alive(o) && store.class_of(o).ok() == Some(GENERALIZATION_SET)
}

/// Whether `o` is a generalization.
pub fn is_generalization(store: &Store, o: Obj) -> bool {
    store.is_alive(o) && store.class_of(o).ok() == Some(GENERALIZATION)
}

/// The generalizations set `s` groups, in its order.
pub fn members(store: &Store, s: Obj) -> Vec<Obj> {
    refs(store, s, generalization_set::GENERALIZATION)
}

/// The sets generalization `g` belongs to, in the order it joined them.
pub fn sets_of(store: &Store, g: Obj) -> Vec<Obj> {
    refs(store, g, generalization::GENERALIZATION_SET).into_iter().filter(|s| is_set(store, *s)).collect()
}

/// The classifier a generalization specializes.
pub fn general(store: &Store, g: Obj) -> Option<Obj> {
    one(store, g, generalization::GENERAL)
}

/// The classifier that specializes in generalization `g`: the one that owns it.
pub fn specific(store: &Store, g: Obj) -> Option<Obj> {
    store.container(g).ok().flatten().map(|(c, _)| c).filter(|c| store.is_alive(*c))
}

/// The general classifier set `s`'s members specialize: its first member's.
pub fn general_of(store: &Store, s: Obj) -> Option<Obj> {
    members(store, s).first().and_then(|g| general(store, *g))
}

/// The live sets of the model whose members specialize `general`, in a stable order: found
/// through the generalizations to it, from the store's index of referrers, not a scan of the store.
pub fn sets_for(store: &Store, general: Obj) -> Vec<Obj> {
    let mut out: Vec<Obj> = store
        .referrers(general)
        .into_iter()
        .filter(|(g, prop)| *prop == generalization::GENERAL && is_generalization(store, *g))
        .flat_map(|(g, _)| sets_of(store, g))
        .filter(|s| general_of(store, *s) == Some(general))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Whether set `s` is complete (UML's `isCovering`) and disjoint (`isDisjoint`).
pub fn flags(store: &Store, s: Obj) -> (bool, bool) {
    let flag = |p| store.get(s, p).ok().and_then(|v| v.as_bool()).unwrap_or(false);
    (flag(generalization_set::IS_COVERING), flag(generalization_set::IS_DISJOINT))
}

/// The set's constraint as the canvas writes it: `{complete, disjoint}`, `{incomplete, overlapping}`.
pub fn braces(store: &Store, s: Obj) -> String {
    let (complete, disjoint) = flags(store, s);
    format!("{{{}, {}}}", if complete { "complete" } else { "incomplete" }, if disjoint { "disjoint" } else { "overlapping" })
}

/// The set's label as a diagram shows it: its name, then its braces.
pub fn label(store: &Store, s: Obj) -> String {
    let name = name_of(store, s);
    if name.is_empty() { braces(store, s) } else { format!("{name} {}", braces(store, s)) }
}

/// Why a selection of elements may not form one set, when it may not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grouping {
    /// Nothing was given.
    Empty,
    /// The element is no generalization.
    NotAGeneralization(Obj),
    /// The generalizations specialize different classifiers: one generalization for each general
    /// classifier met, in the order they were given.
    ParentsDiffer(Vec<Obj>),
}

/// The general classifier `gens` all specialize, which a set of them groups; or why they may not
/// form one set.
pub fn shared_general(store: &Store, gens: &[Obj]) -> Result<Obj, Grouping> {
    let mut seen: Vec<(Obj, Obj)> = Vec::new();
    for g in gens {
        if !is_generalization(store, *g) {
            return Err(Grouping::NotAGeneralization(*g));
        }
        let Some(general) = general(store, *g) else { return Err(Grouping::NotAGeneralization(*g)) };
        if !seen.iter().any(|(x, _)| *x == general) {
            seen.push((general, *g));
        }
    }
    match seen.as_slice() {
        [] => Err(Grouping::Empty),
        [(general, _)] => Ok(*general),
        _ => Err(Grouping::ParentsDiffer(seen.into_iter().map(|(_, g)| g).collect())),
    }
}

/// A generalization as a refusal names it: `CardPayment → Payment`.
pub fn generalization_name(store: &Store, g: Obj) -> String {
    let n = |o: Option<Obj>| o.map(|o| name_of(store, o)).unwrap_or_default();
    format!("{} → {}", n(specific(store, g)), n(general(store, g)))
}

/// Why generalization `g` may not be reconnected, when it may not: it is in a set, which groups
/// the specializations of its general classifier; the refusal names its sets.
pub fn reconnect_problem(store: &Store, g: Obj) -> Option<String> {
    let sets = sets_of(store, g);
    if sets.is_empty() {
        return None;
    }
    let names: Vec<String> = sets.iter().map(|s| format!("{} ({})", name_of(store, *s), uuid_of(store, *s).unwrap_or_default())).collect();
    Some(format!(
        "{} ({}) is in the generalization set{} {}, which group{} the specializations of its general classifier; take it out of {} (uml.generalization-set.set with remove) before reconnecting it",
        generalization_name(store, g),
        uuid_of(store, g).unwrap_or_default(),
        if sets.len() == 1 { "" } else { "s" },
        names.join(", "),
        if sets.len() == 1 { "s" } else { "" },
        if sets.len() == 1 { "it" } else { "them" },
    ))
}

/// The sets removing `going` (generalizations, and whatever holds them) leaves with no member:
/// they go with them. `going` is every element that goes, the generalizations among them.
pub fn emptied_by(store: &Store, going: &[Obj]) -> Vec<Obj> {
    let gone: BTreeSet<Obj> = going.iter().copied().filter(|o| is_generalization(store, *o)).collect();
    let mut out: Vec<Obj> = Vec::new();
    for g in &gone {
        for s in sets_of(store, *g) {
            if !out.contains(&s) && !going.contains(&s) && members(store, s).iter().all(|m| gone.contains(m)) {
                out.push(s);
            }
        }
    }
    out.sort();
    out
}

/// The label's node width for set `s`'s label.
fn label_width(store: &Store, s: Obj) -> f64 {
    let chars = label(store, s).chars().count().min(1000) as f64;
    (chars * LABEL_CHAR + 12.0).clamp(archforge_diagram::rules::MIN_SIZE, LABEL_MAX_WIDTH)
}

/// The node of diagram `d` showing classifier `c`, a card, not a label.
fn card_node(store: &Store, d: Obj, c: Obj) -> Option<Obj> {
    let (_, _, nodes) = archforge_diagram::views_of(store, c);
    nodes.into_iter().find(|n| archforge_diagram::diagram_of(store, *n) == Some(d))
}

/// The node of diagram `d` that holds where set `s`'s label sits, when it was moved there.
pub fn label_node(store: &Store, d: Obj, s: Obj) -> Option<Obj> {
    let (_, _, nodes) = archforge_diagram::views_of(store, s);
    nodes.into_iter().find(|n| archforge_diagram::diagram_of(store, *n) == Some(d))
}

/// Whether node `n` holds a set's label.
pub fn is_label(store: &Store, n: Obj) -> bool {
    one(store, n, node::ELEMENT).is_some_and(|e| is_set(store, e))
}

/// Puts set `s`'s label on class diagram `d` with its top left at `(x, y)`: its node moved there,
/// or made, nested in the node of the set's general classifier (or else of a member's specific
/// one) so it moves with it. Refused when the diagram shows none of those classifiers.
pub fn place(store: &mut Store, d: Obj, s: Obj, x: f64, y: f64) -> Result<Obj, CommandError> {
    for (key, v) in [("x", x), ("y", y)] {
        if let Some(why) = archforge_diagram::rules::coord_problem(v) {
            return Err(refuse(key, format!("the {key} {why}")));
        }
    }
    let width = label_width(store, s);
    if let Some(n) = label_node(store, d, s) {
        return store
            .transaction(|st| {
                st.set(n, node::X, Value::Real(x))?;
                st.set(n, node::Y, Value::Real(y))?;
                st.set(n, node::WIDTH, Value::Real(width))?;
                Ok(n)
            })
            .map_err(fail);
    }
    let anchors = general_of(store, s).into_iter().chain(members(store, s).into_iter().filter_map(|g| specific(store, g)));
    let anchor = anchors.into_iter().find_map(|c| card_node(store, d, c)).ok_or_else(|| {
        refuse("diagram", format!("this diagram shows none of {}'s classifiers, so its label has nothing to sit by", name_of(store, s)))
    })?;
    store
        .transaction(|st| {
            let n = st.create(archforge_diagram::NODE)?;
            st.set(n, node::ELEMENT, Value::Ref(s))?;
            st.set(n, node::KIND, Value::str(LABEL))?;
            st.set(n, node::X, Value::Real(x))?;
            st.set(n, node::Y, Value::Real(y))?;
            st.set(n, node::WIDTH, Value::Real(width))?;
            st.set(n, node::HEIGHT, Value::Real(LABEL_HEIGHT))?;
            st.push(anchor, node::CHILD, Value::Ref(n))?;
            Ok(n)
        })
        .map_err(fail)
}

/// Fits every node of set `s`'s label to its text, on every diagram.
fn fit_labels(st: &mut Store, s: Obj) -> Result<(), archforge_mof::Invalid> {
    let width = label_width(st, s);
    let (_, _, nodes) = archforge_diagram::views_of(st, s);
    for n in nodes {
        st.set(n, node::WIDTH, Value::Real(width))?;
    }
    Ok(())
}

// Commands.

fn has_generalizations(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| is_generalization(f.doc, o)) { Ok(()) } else { Err(NO_GENERALIZATIONS) }
}

fn has_sets(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| is_set(f.doc, o)) { Ok(()) } else { Err(NO_SETS) }
}

fn object(store: &Store, key: &str, id: &str) -> Result<Obj, CommandError> {
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{:?} is not an id", short(id)))) };
    store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {} in this model", short(id))))
}

/// The set under `key`.
fn set_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let o = object(store, key, text(p, key))?;
    if !is_set(store, o) {
        return Err(refuse(key, format!("{} is not a generalization set", short(text(p, key)))));
    }
    Ok(o)
}

/// The generalizations under `key`, an array of ids, each a generalization, none twice, at most
/// [`MAX_MEMBERS`]; none when the key is left out.
fn generalizations_param(store: &Store, p: &Params, key: &str) -> Result<Vec<Obj>, CommandError> {
    let Some(items) = p.get(key) else { return Ok(Vec::new()) };
    let items = items.as_array().ok_or_else(|| refuse(key, "give the generalizations as an array of ids"))?;
    if items.len() > MAX_MEMBERS {
        return Err(refuse(key, format!("a set groups at most {MAX_MEMBERS} generalizations; {} were given", items.len())));
    }
    let mut out: Vec<Obj> = Vec::with_capacity(items.len());
    for item in items {
        let id = item.as_str().ok_or_else(|| refuse(key, "every generalization is an id, as a string"))?;
        let o = object(store, key, id)?;
        if !is_generalization(store, o) {
            return Err(refuse(key, format!("{} is not a generalization; a set groups generalizations", short(id))));
        }
        if out.contains(&o) {
            return Err(refuse(key, format!("{} is named twice; name each generalization once", short(id))));
        }
        out.push(o);
    }
    Ok(out)
}

/// Why `gens` may not be grouped, as a refusal under `key` says it.
fn grouping_refusal(store: &Store, key: &str, problem: Grouping) -> CommandError {
    match problem {
        Grouping::Empty => refuse(key, "a set groups at least one generalization"),
        Grouping::NotAGeneralization(o) => {
            refuse(key, format!("{} is not a generalization of two classifiers", uuid_of(store, o).unwrap_or_default()))
        }
        Grouping::ParentsDiffer(gens) => {
            let said: Vec<String> = gens.iter().map(|g| generalization_name(store, *g)).collect();
            refuse(key, format!("{} specialize different classifiers; a set groups generalizations of one general classifier", said.join(" and ")))
        }
    }
}

/// The set's name, checked: not empty, a plain name, free among `package`'s other sets.
fn check_set_name(store: &Store, name: &str, package: Obj, except: Option<Obj>) -> Result<(), CommandError> {
    if name.trim().is_empty() {
        return Err(refuse("name", "a generalization set needs a name"));
    }
    let siblings: Vec<Obj> = refs(store, package, package::PACKAGED_ELEMENT).into_iter().filter(|o| is_set(store, *o)).collect();
    if let Some(why) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {why}")));
    }
    if siblings.iter().any(|s| Some(*s) != except && name_of(store, *s).to_lowercase() == name.to_lowercase()) {
        return Err(refuse("name", format!("there is a generalization set named {name} already")));
    }
    Ok(())
}

/// `uml.generalization-set.add`: a set of generalizations to one general classifier, in that
/// classifier's package.
fn set_add(ctx: &mut C<'_>, p: &Params) -> R {
    let gens = generalizations_param(ctx.doc, p, "generalizations")?;
    let general = shared_general(ctx.doc, &gens).map_err(|e| grouping_refusal(ctx.doc, "generalizations", e))?;
    let pkg = crate::nesting::home_package(ctx.doc, general)
        .ok_or_else(|| refuse("generalizations", format!("{} is in no package", name_of(ctx.doc, general))))?;
    let name = text(p, "name").trim();
    check_set_name(ctx.doc, name, pkg, None)?;
    let complete = p.bool("complete").unwrap_or(false);
    let disjoint = p.bool("disjoint").unwrap_or(false);
    let s = ctx
        .doc
        .transaction(|st| {
            let s = st.create(GENERALIZATION_SET)?;
            st.set(s, generalization_set::IS_COVERING, Value::Bool(complete))?;
            st.set(s, generalization_set::IS_DISJOINT, Value::Bool(disjoint))?;
            st.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(s))?;
            for g in &gens {
                st.push(s, generalization_set::GENERALIZATION, Value::Ref(*g))?;
            }
            Ok(s)
        })
        .map_err(fail)?;
    set_name(ctx.doc, s, name)?;
    ctx.view.selection = vec![s];
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

/// `uml.generalization-set.set`: a set renamed, made complete or disjoint, given members or losing
/// some; a field left out keeps what the set has. Every member keeps the one general classifier;
/// a set left with no member goes, in the same step.
fn set_set(ctx: &mut C<'_>, p: &Params) -> R {
    let s = set_param(ctx.doc, p, "set")?;
    let add = generalizations_param(ctx.doc, p, "add")?;
    let remove = generalizations_param(ctx.doc, p, "remove")?;
    let now = members(ctx.doc, s);
    if let Some(g) = remove.iter().find(|g| !now.contains(g)) {
        return Err(refuse("remove", format!("{} is not in the set", generalization_name(ctx.doc, *g))));
    }
    if let Some(g) = add.iter().find(|g| remove.contains(g)) {
        return Err(refuse("add", format!("{} is both added and removed; name it once", generalization_name(ctx.doc, *g))));
    }
    let adding: Vec<Obj> = add.into_iter().filter(|g| !now.contains(g)).collect();
    let after: Vec<Obj> = now.iter().copied().filter(|g| !remove.contains(g)).chain(adding.iter().copied()).collect();
    if after.len() > MAX_MEMBERS {
        return Err(refuse("add", format!("a set groups at most {MAX_MEMBERS} generalizations")));
    }
    if !after.is_empty() {
        let next = shared_general(ctx.doc, &after).map_err(|e| grouping_refusal(ctx.doc, "add", e))?;
        // A set keeps its general classifier, whose package holds it and tells its name apart.
        if let Some(now) = general_of(ctx.doc, s).filter(|now| *now != next) {
            return Err(refuse(
                "add",
                format!(
                    "the set groups the specializations of {}; {} is another classifier, so its generalizations go in a set of their own",
                    name_of(ctx.doc, now),
                    name_of(ctx.doc, next)
                ),
            ));
        }
    }
    let pkg = crate::commands::package_of(ctx.doc, s);
    let name = p.str("name").map(str::trim);
    if let (Some(name), Some(pkg)) = (name, pkg) {
        check_set_name(ctx.doc, name, pkg, Some(s))?;
    }
    if after.is_empty() {
        // Left with no member, the set goes with its last one.
        let id = uuid_of(ctx.doc, s)?;
        ctx.doc
            .transaction(|st| {
                archforge_diagram::remove_views_of(st, s)?;
                archforge_core::destroy_with_links(st, s)
            })
            .map_err(fail)?;
        ctx.view.selection.retain(|o| *o != s);
        return Ok(json!({ "id": id, "removed": true }));
    }
    let complete = p.bool("complete");
    let disjoint = p.bool("disjoint");
    ctx.doc
        .transaction(|st| {
            for g in &remove {
                st.remove_value(s, generalization_set::GENERALIZATION, &Value::Ref(*g))?;
            }
            for g in &adding {
                st.push(s, generalization_set::GENERALIZATION, Value::Ref(*g))?;
            }
            if let Some(v) = complete {
                st.set(s, generalization_set::IS_COVERING, Value::Bool(v))?;
            }
            if let Some(v) = disjoint {
                st.set(s, generalization_set::IS_DISJOINT, Value::Bool(v))?;
            }
            Ok(())
        })
        .map_err(fail)?;
    if let Some(name) = name {
        set_name(ctx.doc, s, name)?;
    }
    fit_labels(ctx.doc, s).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, s)?, "removed": false }))
}

/// `uml.generalization-set.place`: where a class diagram shows a set's label.
fn set_place(ctx: &mut C<'_>, p: &Params) -> R {
    let d = object(ctx.doc, "diagram", text(p, "diagram"))?;
    let kind = ctx.doc.get(d, diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    if ctx.doc.class_of(d).ok() != Some(archforge_diagram::DIAGRAM) || kind != crate::CLASS_DIAGRAM {
        return Err(refuse("diagram", format!("{} is not a class diagram", short(text(p, "diagram")))));
    }
    let s = set_param(ctx.doc, p, "set")?;
    let (Some(x), Some(y)) = (p.number("x"), p.number("y")) else { return Err(refuse("x", "give both x and y")) };
    let n = place(ctx.doc, d, s, x, y)?;
    Ok(json!({ "id": uuid_of(ctx.doc, n)? }))
}

/// A set as the reads give it.
pub fn set_json(store: &Store, s: Obj) -> Json {
    let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
    let (complete, disjoint) = flags(store, s);
    let gens: Vec<Json> = members(store, s)
        .into_iter()
        .map(|g| {
            json!({
                "id": id(g),
                "specific": specific(store, g).map_or(Json::Null, |c| json!(id(c))),
                "general": general(store, g).map_or(Json::Null, |c| json!(id(c))),
                "name": generalization_name(store, g),
            })
        })
        .collect();
    json!({
        "id": id(s),
        "name": name_of(store, s),
        "general": general_of(store, s).map_or(Json::Null, |c| json!(id(c))),
        "complete": complete,
        "disjoint": disjoint,
        "label": label(store, s),
        "generalizations": gens,
        "description": crate::descriptions::description_of(store, s),
    })
}

/// `uml.generalization-set.list`: the sets of the model, of one general classifier, or of one
/// generalization.
fn set_list(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let of = text(p, "of");
    let sets: Vec<Obj> = if of.is_empty() {
        let mut all: Vec<Obj> = store.alive().filter(|o| is_set(store, *o)).collect();
        all.sort();
        all
    } else {
        let o = object(store, "of", of)?;
        match store.class_of(o).ok() {
            Some(c) if c == GENERALIZATION => sets_of(store, o),
            Some(c) if crate::is_classifier(c) => sets_for(store, o),
            _ => return Err(refuse("of", format!("{} is neither a classifier nor a generalization", short(of)))),
        }
    };
    Ok(json!({ "sets": sets.into_iter().map(|s| set_json(store, s)).collect::<Vec<_>>() }))
}

/// The generalization set commands; a set is removed through `element.remove`, which keeps its
/// generalizations.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.generalization-set.add",
            "Create generalization set",
            &["Generalization set", "Generalization"],
            None,
            concat!(
                "generalizations: array; the ids of the generalizations it groups, each once, at least one and at most 256, all specializing the same general classifier; a generalization may be in several sets\n",
                "name: string; its name, free among the sets of the general classifier's package, where it goes\n",
                "complete: bool = false; every instance of the general classifier is an instance of one of the specific ones (UML's isCovering), written complete, else incomplete\n",
                "disjoint: bool = false; no instance is of two of the specific classifiers (UML's isDisjoint), written disjoint, else overlapping"
            ),
            has_generalizations,
            set_add
        ),
        cmd!(
            "uml.generalization-set.set",
            "Set generalization set",
            &["Generalization set"],
            None,
            concat!(
                "set: string; the generalization set's id; every field left out keeps its value\n",
                "name: string?; its new name, free among its package's sets; left out, kept\n",
                "complete: bool?; UML's isCovering, written complete or incomplete; left out, kept\n",
                "disjoint: bool?; UML's isDisjoint, written disjoint or overlapping; left out, kept\n",
                "add: array?; ids of generalizations to add, each specializing the set's general classifier\n",
                "remove: array?; ids of members to take out; a set left with no member is removed with them, said by removed in the result; the generalizations stay"
            ),
            has_sets,
            set_set
        ),
        cmd!(
            "uml.generalization-set.place",
            "Place generalization set label",
            &["Generalization set", "Diagram"],
            None,
            concat!(
                "diagram: string; the class diagram's id\n",
                "set: string; the generalization set's id\n",
                "x: number; the label's left edge; the dashed set line runs under it across the members' lines\n",
                "y: number; the label's top edge"
            ),
            has_sets,
            set_place
        ),
        cmd!(
            query "uml.generalization-set.list",
            "List generalization sets",
            &["Generalization set"],
            None,
            "of: string = \"\"; a classifier's id for the sets of its specializations, a generalization's for the sets it is in, nothing for every set",
            has_generalizations,
            set_list
        ),
    ]
}
