//! Qualifiers of an association's ends (AR-127): UML's `Property::qualifier`, an ordered list of
//! properties an association end owns, each with a name, a type and a multiplicity. The end that
//! owns them is the qualified end, where the qualifier box is drawn beside the class the end is
//! typed by (owner decision: the Order end of Order to OrderLine); the other end's multiplicity
//! then counts the instances one qualified instance and one qualifier value pick.
//!
//! A qualifier is never an attribute of a classifier and never an end's role: it is a property of
//! its own, owned by the end through `qualifier` (composite, its opposite `associationEnd`), so
//! the attribute commands refuse it, a card never lists it and an end's role is never one. Only an
//! association's or an association class's end is qualified (UML 2.5.1, 9.9.17,
//! `qualified_is_association_end`). Its name is told apart from its end's other qualifiers'
//! whatever the case (owner decision), and an end holds at most [`MAX_QUALIFIERS`]. Its type is
//! named as a member's is: a type's id or a built-in type's name. Nothing about a qualifier depends
//! on the classes at the association's ends, so renaming or reconnecting the association keeps
//! them; a reconnection refused for its own reasons leaves them as they were. Removed through
//! `element.remove`; an association that goes takes them with its ends.

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, cmd};
use archforge_mof::{Obj, Store, Value};
use serde_json::{Value as Json, json};

use crate::commands::{fail, id_of, name_of, refuse, set_name, short, text, uuid_of};
use crate::{ASSOCIATION, ASSOCIATION_CLASS, PROPERTY, property, typed_element};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The most qualifiers one end holds: a box is drawn on every frame, so its rows never grow with
/// what the model holds.
pub const MAX_QUALIFIERS: usize = 32;
/// Why the qualifier commands are not available.
pub const NO_ASSOCIATIONS: &str = "There is no association or association class yet.";

fn has_associations(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(crate::is_association)) { Ok(()) } else { Err(NO_ASSOCIATIONS) }
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

/// The qualifiers of association end `end`, in its order.
pub fn qualifiers(store: &Store, end: Obj) -> Vec<Obj> {
    store.list(end, property::QUALIFIER).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|q| store.is_alive(*q)).collect()
}

/// The association end that owns qualifier `q`: its container through `qualifier`.
pub fn qualified_end(store: &Store, q: Obj) -> Option<Obj> {
    if store.class_of(q).ok() != Some(PROPERTY) {
        return None;
    }
    match store.container(q).ok()?? {
        (end, prop) if prop == property::QUALIFIER => Some(end),
        _ => None,
    }
}

/// Whether `o` is a qualifier of an association's end.
pub fn is_qualifier(store: &Store, o: Obj) -> bool {
    qualified_end(store, o).is_some()
}

/// The association or association class whose end owns qualifier `q`, and which of its two ends
/// that is.
pub fn association_of(store: &Store, q: Obj) -> Option<(Obj, usize)> {
    let end = qualified_end(store, q)?;
    let a = one(store, end, property::ASSOCIATION)?;
    let i = crate::figures::member_ends(store, a)?.iter().position(|e| *e == end)?;
    Some((a, i))
}

/// The qualifier of `end`, other than `except`, that a qualifier named `name` would clash with:
/// an end's qualifiers, the properties it holds, are told apart whatever the case by the shared
/// rule ([`crate::names`]). The commands and the shell ask this one rule.
pub fn name_clash(store: &Store, end: Obj, name: &str, except: Option<Obj>) -> Option<Obj> {
    crate::names::clash(store, end, PROPERTY, name, except)
}

/// A qualifier as its box and Properties list it: `lineNo : Integer`, the multiplicity in
/// brackets when it is not one, as an attribute's is.
pub fn qualifier_text(store: &Store, q: Obj) -> String {
    let mut out = name_of(store, q);
    if let Some(t) = one(store, q, typed_element::TYPE) {
        out.push_str(&format!(" : {}", name_of(store, t)));
    }
    let m = crate::figures::multiplicity(store, q);
    if !m.is_empty() && m != "1" {
        out.push_str(&format!(" [{m}]"));
    }
    out.chars().take(200).collect()
}

/// End `end`'s qualifiers as the reads give them: each with its id, name, type and multiplicity.
pub fn qualifiers_json(store: &Store, end: Obj) -> Json {
    Json::Array(
        qualifiers(store, end)
            .into_iter()
            .map(|q| {
                json!({
                    "id": uuid_of(store, q).unwrap_or_default(),
                    "name": name_of(store, q),
                    "type": one(store, q, typed_element::TYPE).map_or(Json::Null, |t| json!({ "id": uuid_of(store, t).unwrap_or_default(), "name": name_of(store, t) })),
                    "multiplicity": crate::figures::multiplicity(store, q),
                })
            })
            .collect(),
    )
}

/// A qualifier's name on `end`: a name, none of the end's other qualifiers' whatever the case. A
/// clash names the name typed and the end's role and class.
fn check_name(store: &Store, end: Obj, name: &str, except: Option<Obj>) -> Result<(), CommandError> {
    if let Some(p) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {p}")));
    }
    if let Some(q) = name_clash(store, end, name, except) {
        let class = one(store, end, typed_element::TYPE).map(|t| name_of(store, t)).unwrap_or_default();
        return Err(refuse(
            "name",
            format!(
                "the end at {class} has a qualifier named {} already, {}, and {:?} matches it whatever the case; choose a name none of the end's other qualifiers has",
                name_of(store, q),
                uuid_of(store, q)?,
                short(name)
            ),
        ));
    }
    Ok(())
}

/// The bounds typed under `multiplicity`, refused when they do not read.
fn check_multiplicity(p: &Params) -> Result<(), CommandError> {
    crate::members::parse_multiplicity(text(p, "multiplicity")).map(|_| ()).map_err(|why| refuse("multiplicity", format!("the multiplicity {why}")))
}

/// The qualifier under `key`, with the end that owns it.
fn qualifier_of(store: &Store, p: &Params, key: &str) -> Result<(Obj, Obj), CommandError> {
    let q = id_of(store, p, key, PROPERTY, "a qualifier")?;
    let end = qualified_end(store, q).ok_or_else(|| {
        refuse(
            key,
            format!("{} is no qualifier; an attribute is set with uml.attribute.set and an end with uml.association.set-end", short(text(p, key))),
        )
    })?;
    Ok((q, end))
}

/// `uml.qualifier.add`: a qualifier of one end of an association or an association class, its
/// name free among the end's qualifiers whatever the case, at the place `at` asks for (from 0 to
/// their count) or, at -1, at the end.
fn qualifier_add(ctx: &mut C<'_>, p: &Params) -> R {
    let a = id_of(ctx.doc, p, "association", ASSOCIATION, "")
        .or_else(|_| id_of(ctx.doc, p, "association", ASSOCIATION_CLASS, ""))
        .map_err(|_| refuse("association", format!("{} is no association or association class", short(text(p, "association")))))?;
    let ends = crate::figures::member_ends(ctx.doc, a).ok_or_else(|| refuse("association", "the association has not two ends"))?;
    let i = match p.integer("end") {
        Some(i @ 0..=1) => i as usize,
        _ => return Err(refuse("end", "the end is 0 or 1")),
    };
    let end = ends[i];
    let count = qualifiers(ctx.doc, end).len();
    if count >= MAX_QUALIFIERS {
        return Err(refuse(
            "end",
            format!("the end holds {count} qualifiers already, and an end holds at most {MAX_QUALIFIERS}; remove one before adding another"),
        ));
    }
    let name = text(p, "name");
    check_name(ctx.doc, end, name, None)?;
    check_multiplicity(p)?;
    let at = match p.integer("at").unwrap_or(-1) {
        -1 => count,
        at => usize::try_from(at)
            .ok()
            .filter(|k| *k <= count)
            .ok_or_else(|| refuse("at", format!("{at} is past the end of {count} qualifiers; a place from 0 to {count}, or -1 for the end")))?,
    };
    let at = u32::try_from(at).map_err(|_| refuse("at", format!("{at} is too far; a place from 0 to {count}, or -1 for the end")))?;
    let ty = crate::members::resolve_type(ctx.doc, "type", text(p, "type"))?;
    let q = ctx.doc.create(PROPERTY).map_err(fail)?;
    set_name(ctx.doc, q, name)?;
    ctx.doc.set(q, typed_element::TYPE, ty.map_or(Value::Null, Value::Ref)).map_err(fail)?;
    crate::members::set_multiplicity(ctx.doc, q, text(p, "multiplicity"), "multiplicity")?;
    ctx.doc.insert(end, property::QUALIFIER, at, Value::Ref(q)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, q)?, "end": uuid_of(ctx.doc, end)? }))
}

/// `uml.qualifier.set`: a qualifier's name, type and multiplicity changed, everything checked
/// before anything changes. A field the call leaves out keeps what the qualifier has; an empty type
/// or multiplicity clears it.
fn qualifier_set(ctx: &mut C<'_>, p: &Params) -> R {
    let (q, end) = qualifier_of(ctx.doc, p, "qualifier")?;
    let name = match p.str("name") {
        Some(n) => n.to_string(),
        None => name_of(ctx.doc, q),
    };
    check_name(ctx.doc, end, &name, Some(q))?;
    if p.str("multiplicity").is_some() {
        check_multiplicity(p)?;
    }
    let ty = match p.str("type") {
        Some(t) => crate::members::resolve_type(ctx.doc, "type", t)?,
        None => one(ctx.doc, q, typed_element::TYPE),
    };
    set_name(ctx.doc, q, &name)?;
    ctx.doc.set(q, typed_element::TYPE, ty.map_or(Value::Null, Value::Ref)).map_err(fail)?;
    if p.str("multiplicity").is_some() {
        crate::members::set_multiplicity(ctx.doc, q, text(p, "multiplicity"), "multiplicity")?;
    }
    Ok(json!({ "id": uuid_of(ctx.doc, q)? }))
}

/// `uml.qualifier.move`: a qualifier to the place `at` among its end's qualifiers, from 0 to their
/// count less one, -1 for the last; a place past the last is refused, not taken as the last.
fn qualifier_move(ctx: &mut C<'_>, p: &Params) -> R {
    let (q, end) = qualifier_of(ctx.doc, p, "qualifier")?;
    let all = qualifiers(ctx.doc, end);
    let last = all.len().saturating_sub(1);
    let from = all.iter().position(|x| *x == q).ok_or_else(|| refuse("qualifier", "it is not among its end's qualifiers"))?;
    let to = match p.integer("at").unwrap_or(-1) {
        -1 => last,
        at => usize::try_from(at)
            .ok()
            .filter(|k| *k <= last)
            .ok_or_else(|| refuse("at", format!("{at} is past the last qualifier; a place from 0 to {last}, or -1 for the last")))?,
    };
    let (Ok(from), Ok(to)) = (u32::try_from(from), u32::try_from(to)) else {
        return Err(refuse("at", format!("a place from 0 to {last}, or -1 for the last")));
    };
    ctx.doc.move_item(end, property::QUALIFIER, from, to).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, q)? }))
}

/// The qualifiers' commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.qualifier.add",
            "Add qualifier",
            &["Association", "Association class"],
            None,
            concat!(
                "association: string; the association's or association class's id\n",
                "end: integer, 0..1; the qualified end, 0 at the class it was drawn from, 1 at the other: the qualifier box is drawn beside that end's class, and the other end's multiplicity counts what one qualifier value picks\n",
                "name: string; the qualifier's name, free among the end's qualifiers whatever the case; an end holds at most 32\n",
                "type: string = \"\"; a type's id, or Integer, String, Boolean, Real or UnlimitedNatural, the built-in types kept in the package Types (never a type of the model's own named alike), or nothing\n",
                "multiplicity: string = \"\"; 1, 0..1, *, 1..*, 2..5, or nothing for one\n",
                "at: integer = -1, -1..1000000; where among the end's qualifiers, from 0 to their count, -1 for the end"
            ),
            has_associations,
            qualifier_add
        ),
        cmd!(
            "uml.qualifier.set",
            "Set qualifier",
            &["Association", "Association class"],
            None,
            concat!(
                "qualifier: string; the qualifier's id; every other field left out keeps what the qualifier has\n",
                "name: string?; the qualifier's name, free among its end's other qualifiers whatever the case; left out, kept\n",
                "type: string?; a type's id, or Integer, String, Boolean, Real or UnlimitedNatural, the built-in types kept in the package Types (never a type of the model's own named alike); \"\" for none; left out, kept\n",
                "multiplicity: string?; 1, 0..1, *, 1..*, 2..5; \"\" for none; left out, kept"
            ),
            has_associations,
            qualifier_set
        ),
        cmd!(
            "uml.qualifier.move",
            "Move qualifier",
            &["Association", "Association class"],
            None,
            "qualifier: string; the qualifier's id\nat: integer = -1, -1..1000000; its new place among its end's qualifiers, from 0 to their count less one, -1 for the last",
            has_associations,
            qualifier_move
        ),
    ]
}
