//! Structural constraints (AR-133): a UML Constraint owned by its context, a classifier or a
//! package (`Namespace::ownedRule`), naming the elements it constrains (`constrainedElement`, in
//! their order) and stating its rule as an OpaqueExpression (`specification`) whose one body is
//! the expression and whose one language, when it has one, the language it is written in (none:
//! plain text). No table or column of its own: the metamodel's Constraint and OpaqueExpression hold
//! it, saved and read back with the rest of the model, and destroyed with its context.
//!
//! A constraint is stored, never evaluated: its expression is kept exactly as typed, in any
//! language, and nothing checks it against the model. The only refusals are an empty expression
//! and a reference to an element outside the model; ownership is checked too (a context is a
//! classifier of the model's own or a package).
//!
//! The content is the model's, shared by every diagram; how a diagram shows it is the diagram's
//! own: a node showing the constraint, of the kind [`BRACES`] (`{expression}` beside the element it
//! constrains, nested in that element's node so it moves with it) or [`NOTE`] (a constraint note,
//! tied to the nodes of its constrained elements by dashed links worked out from the model, never
//! stored). No view record is new: nodes are the diagrams' own, with a kind and a place.
//!
//! Deleting a constrained element drops it from the constraint's elements in the same step (the
//! core's destroy does); a constraint left constraining nothing goes with it ([`dropped_by`],
//! [`crate::commands::remove_elements`]).

use std::collections::BTreeSet;

use archforge_diagram::{ProjectView, diagram, node};
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, cmd};
use archforge_mof::{Obj, Store, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::commands::{fail, name_of, refuse, short, text, uuid_of};
use crate::{CONSTRAINT, OPAQUE_EXPRESSION, PACKAGE, constraint, named_element, namespace, opaque_expression};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The kind of a node that shows a constraint as `{expression}` beside the element it constrains.
pub const BRACES: &str = "uml.constraint.braces";
/// The kind of a node that shows a constraint as a constraint note, linked to its elements.
pub const NOTE: &str = "uml.constraint.note";
/// The most elements one constraint names.
pub const MAX_CONSTRAINED: usize = 256;
/// The longest expression the braces suit, in characters; a longer one, one of several lines or
/// one naming several elements reads best as a note.
pub const SHORT: usize = 40;
/// A constraint note's width when placed, and its least and greatest height.
pub const NOTE_WIDTH: f64 = 240.0;
const NOTE_MIN_HEIGHT: f64 = 96.0;
const NOTE_MAX_HEIGHT: f64 = 400.0;
/// The lines of an expression a note's height is worked out for at most.
const NOTE_LINES: usize = 16;
/// The braces' height, and the room around them.
const BRACES_HEIGHT: f64 = 20.0;
const BRACES_GAP: f64 = 4.0;
/// The room between the elements and a note placed beside them.
const NOTE_GAP: f64 = 40.0;
/// How wide one character of the braces' text is taken to be, and the widest braces.
const BRACES_CHAR: f64 = 6.5;
const BRACES_MAX_WIDTH: f64 = 480.0;

/// Why a command is not available.
pub const NO_CONTEXTS: &str = "There is no class, interface, enumeration, data type, primitive type, association class or package yet.";
/// What a context is, as a refusal names it.
const CONTEXTS: &str = "a class, an interface, an enumeration, a data type, a primitive type of the model's own, an association class or a package";
/// What a constraint may name, as a refusal names it.
const CONSTRAINABLE: &str =
    "a package, a classifier, an attribute or association end, an operation, a parameter, a literal or a relationship of this model";

/// How a diagram shows a constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// `{expression}` beside the element it constrains.
    Braces,
    /// A constraint note, linked to its elements.
    Note,
}

impl Form {
    /// The node kind of the form.
    pub fn kind(self) -> &'static str {
        match self {
            Form::Braces => BRACES,
            Form::Note => NOTE,
        }
    }

    /// The form as the commands name it.
    pub fn word(self) -> &'static str {
        match self {
            Form::Braces => "braces",
            Form::Note => "note",
        }
    }
}

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn refs(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

/// Whether `o` is a constraint.
pub fn is_constraint(store: &Store, o: Obj) -> bool {
    store.is_alive(o) && store.class_of(o).ok() == Some(CONSTRAINT)
}

/// Whether `o` lies inside the project's model.
pub fn in_model(store: &Store, o: Obj) -> bool {
    store.is_alive(o) && crate::commands::model(store).is_ok_and(|m| store.contains(m, o))
}

/// Whether `o` may own constraints: a classifier of the model's own (a built-in type stays as it
/// is) or a package, inside the model.
pub fn is_context(store: &Store, o: Obj) -> bool {
    let Ok(class) = store.class_of(o) else { return false };
    let kind = (crate::is_classifier(class) && !crate::members::is_built_in(store, o)) || class == PACKAGE;
    kind && in_model(store, o)
}

/// Whether a constraint may name `o`: a package, a classifier, a property (an attribute or an
/// association's end), an operation, a parameter, a literal or a relationship, inside the model.
pub fn is_constrainable(store: &Store, o: Obj) -> bool {
    let Ok(class) = store.class_of(o) else { return false };
    let kind = crate::is_classifier(class)
        || crate::is_relationship(class)
        || [PACKAGE, crate::PROPERTY, crate::OPERATION, crate::PARAMETER, crate::ENUMERATION_LITERAL].contains(&class);
    kind && in_model(store, o)
}

/// The constraints `context` owns, in its order.
pub fn constraints_of(store: &Store, context: Obj) -> Vec<Obj> {
    let listed = store.reflect(context).ok().and_then(|r| r.list(namespace::OWNED_RULE)).unwrap_or(&[]);
    listed.iter().filter_map(Value::as_ref).filter(|c| is_constraint(store, *c)).collect()
}

/// The element that owns `c`, its context.
pub fn context_of(store: &Store, c: Obj) -> Option<Obj> {
    one(store, c, constraint::CONTEXT).or_else(|| store.container(c).ok().flatten().map(|(o, _)| o))
}

/// The elements `c` constrains, in its order.
pub fn constrained(store: &Store, c: Obj) -> Vec<Obj> {
    refs(store, c, constraint::CONSTRAINED_ELEMENT)
}

/// The OpaqueExpression that states `c`'s rule, when its specification is one.
fn opaque(store: &Store, c: Obj) -> Option<Obj> {
    one(store, c, constraint::SPECIFICATION).filter(|s| store.class_of(*s).ok() == Some(OPAQUE_EXPRESSION))
}

/// The first string of `o`'s list `prop`.
fn first_string(store: &Store, o: Obj, prop: archforge_mof::PropId) -> String {
    store.list(o, prop).ok().and_then(|l| l.first().and_then(|v| v.as_str().map(str::to_string))).unwrap_or_default()
}

/// `c`'s expression, exactly as it was typed; nothing for a specification that is no
/// OpaqueExpression (only a file holds one).
pub fn expression(store: &Store, c: Obj) -> String {
    opaque(store, c).map(|s| first_string(store, s, opaque_expression::BODY)).unwrap_or_default()
}

/// The language `c`'s expression is written in, as typed; nothing for plain text.
pub fn language(store: &Store, c: Obj) -> String {
    opaque(store, c).map(|s| first_string(store, s, opaque_expression::LANGUAGE)).unwrap_or_default()
}

/// `c`'s braces as a diagram shows them: its expression between braces, as written.
pub fn braces(store: &Store, c: Obj) -> String {
    format!("{{{}}}", expression(store, c))
}

/// Why `text` states no rule, when it states none: it is empty, or holds nothing but spaces. Any
/// other text is kept exactly as typed.
pub fn expression_problem(text: &str) -> Option<&'static str> {
    text.trim().is_empty().then_some("type an expression; a constraint without one states no rule")
}

/// How a diagram shows `c` unless asked otherwise: braces for a short expression of one line that
/// names one element, else a note.
pub fn suggested_form(store: &Store, c: Obj) -> Form {
    form_for(&expression(store, c), constrained(store, c).len())
}

/// [`suggested_form`] for an expression and a number of constrained elements.
pub fn form_for(expression: &str, elements: usize) -> Form {
    if elements <= 1 && !expression.contains('\n') && expression.chars().count() <= SHORT { Form::Braces } else { Form::Note }
}

/// The node of `diagram` that shows `c`, at any depth, and the form it shows it in.
pub fn shown_on(store: &Store, diagram: Obj, c: Obj) -> Option<(Obj, Form)> {
    let (_, _, nodes) = archforge_diagram::views_of(store, c);
    nodes.into_iter().filter(|n| archforge_diagram::diagram_of(store, *n) == Some(diagram)).find_map(|n| Some((n, form_of(store, n)?)))
}

/// The form a node showing a constraint has: its kind's, a note when it has no kind of either.
pub fn form_of(store: &Store, n: Obj) -> Option<Form> {
    let element = one(store, n, node::ELEMENT)?;
    if !is_constraint(store, element) {
        return None;
    }
    let kind = store.get(n, node::KIND).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    Some(if kind == BRACES { Form::Braces } else { Form::Note })
}

/// One constraint a removal changes: it loses some of its elements and keeps `remaining`, or
/// goes, when none remains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub constraint: Obj,
    pub remaining: Vec<Obj>,
}

/// The constraints that removing `objs` (as `element.remove` removes them, with the relationships
/// that go with classifiers) changes: each constraint that stays, since its context stays, but
/// names an element that goes, with what it keeps. A nested package hands what it holds up and
/// goes alone. Worked out before anything changes.
pub fn dropped_by(store: &Store, objs: &[Obj]) -> Vec<Dropped> {
    let mut roots: Vec<(Obj, bool)> = Vec::new();
    let mut classes: Vec<Obj> = Vec::new();
    for o in objs.iter().copied().filter(|o| store.is_alive(*o)) {
        let class = store.class_of(o).ok();
        // A nested package's contents move up to its parent: only the package goes.
        let nested = class == Some(PACKAGE) && crate::commands::package_of(store, o).is_some();
        roots.push((o, !nested));
        match class {
            Some(c) if crate::is_classifier(c) => classes.push(o),
            Some(c) if c == PACKAGE && !nested => classes.extend(crate::commands::classifiers_within(store, o)),
            _ => {}
        }
    }
    roots.extend(crate::commands::going_with(store, &classes).into_iter().map(|r| (r, true)));
    let goes = |e: Obj| roots.iter().any(|(r, deep)| *r == e || (*deep && store.contains(*r, e)));
    let mut out = Vec::new();
    for c in store.alive().filter(|c| store.class_of(*c).ok() == Some(CONSTRAINT)) {
        // A constraint inside what goes, its context's own among them, goes with it anyway.
        if goes(c) {
            continue;
        }
        let elements = constrained(store, c);
        if elements.iter().any(|e| goes(*e)) {
            out.push(Dropped { constraint: c, remaining: elements.into_iter().filter(|e| !goes(*e)).collect() });
        }
    }
    out
}

// Commands.

fn has_contexts(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| is_context(f.doc, o)) { Ok(()) } else { Err(NO_CONTEXTS) }
}

fn has_constraints(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| is_constraint(f.doc, o)) { Ok(()) } else { Err("There is no constraint yet.") }
}

/// The live object the id under `key` names, or, for one given as `id` alone, that.
fn object(store: &Store, key: &str, id: &str) -> Result<Obj, CommandError> {
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{:?} is not an id", short(id)))) };
    store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {} in this model", short(id))))
}

/// The context under `key`.
fn context_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let o = object(store, key, text(p, key))?;
    if !is_context(store, o) {
        return Err(refuse(key, format!("{} owns no constraint; {CONTEXTS} does", text(p, key))));
    }
    Ok(o)
}

/// The constraint under `key`.
fn constraint_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let o = object(store, key, text(p, key))?;
    if !is_constraint(store, o) || !in_model(store, o) {
        return Err(refuse(key, format!("{} is not a constraint of this model", text(p, key))));
    }
    Ok(o)
}

/// The elements under `key`, an array of ids, each in the model, none twice, at most
/// [`MAX_CONSTRAINED`]; an empty array names `context` alone.
fn constrained_param(store: &Store, p: &Params, key: &str, context: Obj) -> Result<Vec<Obj>, CommandError> {
    let items = p.get(key).and_then(Json::as_array).cloned().unwrap_or_default();
    if items.len() > MAX_CONSTRAINED {
        return Err(refuse(key, format!("a constraint names at most {MAX_CONSTRAINED} elements; {} were given", items.len())));
    }
    if items.is_empty() {
        return Ok(vec![context]);
    }
    let mut out: Vec<Obj> = Vec::with_capacity(items.len());
    for item in &items {
        let id = item.as_str().ok_or_else(|| refuse(key, "every element is an id, as a string"))?;
        let o = object(store, key, id)?;
        if !is_constrainable(store, o) {
            return Err(refuse(key, format!("{} is outside this model; a constraint names {CONSTRAINABLE}", short(id))));
        }
        if out.contains(&o) {
            return Err(refuse(key, format!("{} is named twice; name each element once", short(id))));
        }
        out.push(o);
    }
    Ok(out)
}

/// The expression under `key`, refused when it states no rule.
fn expression_param<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    let e = text(p, key);
    match expression_problem(e) {
        Some(why) => Err(refuse(key, why)),
        None => Ok(e),
    }
}

/// Puts `elements` in `c`'s constrained elements, in their order, in place of what it had.
fn set_constrained(st: &mut Store, c: Obj, elements: &[Obj]) -> Result<(), archforge_mof::Invalid> {
    while !st.list(c, constraint::CONSTRAINED_ELEMENT)?.is_empty() {
        st.remove_at(c, constraint::CONSTRAINED_ELEMENT, 0)?;
    }
    for e in elements {
        st.push(c, constraint::CONSTRAINED_ELEMENT, Value::Ref(*e))?;
    }
    Ok(())
}

/// Puts `expression` and `language` in `c`'s specification, an OpaqueExpression with one body and
/// at most one language, made in place of a specification of another kind.
fn set_specification(st: &mut Store, c: Obj, expression: &str, language: &str) -> Result<(), archforge_mof::Invalid> {
    let spec = match opaque(st, c) {
        Some(s) => s,
        None => {
            if let Some(old) = one(st, c, constraint::SPECIFICATION) {
                st.destroy(old)?;
            }
            let s = st.create(OPAQUE_EXPRESSION)?;
            st.set(c, constraint::SPECIFICATION, Value::Ref(s))?;
            s
        }
    };
    for prop in [opaque_expression::BODY, opaque_expression::LANGUAGE] {
        while !st.list(spec, prop)?.is_empty() {
            st.remove_at(spec, prop, 0)?;
        }
    }
    st.push(spec, opaque_expression::BODY, Value::str(expression))?;
    if !language.is_empty() {
        st.push(spec, opaque_expression::LANGUAGE, Value::str(language))?;
    }
    Ok(())
}

/// The braces' width for `text`.
fn braces_width(text: &str) -> f64 {
    let chars = text.chars().count().min(1000) as f64;
    (chars * BRACES_CHAR + 8.0).clamp(archforge_diagram::rules::MIN_SIZE, BRACES_MAX_WIDTH)
}

/// A note's height for `c`'s name and expression.
fn note_height(expression: &str) -> f64 {
    let lines = expression.lines().count().clamp(1, NOTE_LINES) as f64;
    (44.0 + 18.0 * lines + 14.0).clamp(NOTE_MIN_HEIGHT, NOTE_MAX_HEIGHT)
}

/// The node on `d` that shows `e` or, for a member, its classifier: where braces for it sit.
fn node_for(store: &Store, d: Obj, e: Obj) -> Option<Obj> {
    let mut at = Some(e);
    for _ in 0..4 {
        let o = at?;
        let (_, _, nodes) = archforge_diagram::views_of(store, o);
        if let Some(n) = nodes.into_iter().find(|n| archforge_diagram::diagram_of(store, *n) == Some(d) && form_of(store, *n).is_none()) {
            return Some(n);
        }
        at = store.container(o).ok().flatten().map(|(c, _)| c);
    }
    None
}

/// The nodes on `d` of `c`'s elements, each once, in its order: what a note links to.
pub fn linked_nodes(store: &Store, d: Obj, c: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    for e in constrained(store, c) {
        if let Some(n) = node_for(store, d, e)
            && !out.contains(&n)
        {
            out.push(n);
        }
    }
    out
}

/// Shows `c` on the class diagram `d` in `form`, at `at` or where it reads: braces nested in the
/// node of the first of its elements the diagram shows, under it; a note right of its elements'
/// nodes. A node showing it already in another form gives way, in the same step. Refused when
/// braces have no element of `c` to sit by.
pub fn show(store: &mut Store, d: Obj, c: Obj, form: Form, at: Option<(f64, f64)>) -> Result<Obj, CommandError> {
    let was = shown_on(store, d, c);
    let rect = |n: Obj| archforge_diagram::grid::bounds_of(store, n);
    let anchor = constrained(store, c).into_iter().chain(context_of(store, c)).find_map(|e| node_for(store, d, e));
    let expr = expression(store, c);
    let (parent, size, place) = match form {
        Form::Braces => {
            let Some(anchor) = anchor else {
                return Err(refuse(
                    "as",
                    format!("braces sit by an element they constrain, and this diagram shows none of {}'s; show it as a note", name_of(store, c)),
                ));
            };
            let r = rect(anchor);
            ((anchor, node::CHILD), (braces_width(&format!("{{{expr}}}")), BRACES_HEIGHT), (r.x, r.y + r.height + BRACES_GAP))
        }
        Form::Note => {
            let around: Vec<archforge_diagram::Rect> = linked_nodes(store, d, c).into_iter().map(rect).collect();
            let place = match around.split_first() {
                Some((first, rest)) => {
                    let right = rest.iter().fold(first.x + first.width, |m, r| m.max(r.x + r.width));
                    let top = rest.iter().fold(first.y, |m, r| m.min(r.y));
                    (right + NOTE_GAP, top)
                }
                None => (0.0, 0.0),
            };
            ((d, diagram::NODE), (NOTE_WIDTH, note_height(&expr)), place)
        }
    };
    let (x, y) = at.unwrap_or(place);
    for (key, v) in [("x", x), ("y", y)] {
        if let Some(why) = archforge_diagram::rules::coord_problem(v) {
            return Err(refuse(key, format!("the {key} {why}")));
        }
    }
    // The same form where it is already, with no place asked: nothing changes.
    if let Some((n, f)) = was
        && f == form
        && at.is_none()
    {
        return Ok(n);
    }
    store
        .transaction(|st| {
            if let Some((n, _)) = was {
                archforge_diagram::destroy_node_with_edges(st, n)?;
            }
            let n = st.create(archforge_diagram::NODE)?;
            st.set(n, node::ELEMENT, Value::Ref(c))?;
            st.set(n, node::KIND, Value::str(form.kind()))?;
            st.set(n, node::X, Value::Real(x))?;
            st.set(n, node::Y, Value::Real(y))?;
            st.set(n, node::WIDTH, Value::Real(size.0))?;
            st.set(n, node::HEIGHT, Value::Real(size.1))?;
            st.push(parent.0, parent.1, Value::Ref(n))?;
            Ok(n)
        })
        .map_err(fail)
}

/// Fits every node of braces showing `c` to its expression, on every diagram.
fn fit_braces(st: &mut Store, c: Obj) -> Result<(), archforge_mof::Invalid> {
    let width = braces_width(&braces(st, c));
    let (_, _, nodes) = archforge_diagram::views_of(st, c);
    for n in nodes {
        if form_of(st, n) == Some(Form::Braces) {
            st.set(n, node::WIDTH, Value::Real(width))?;
        }
    }
    Ok(())
}

fn constraint_add(ctx: &mut C<'_>, p: &Params) -> R {
    let context = context_param(ctx.doc, p, "context")?;
    let expression = expression_param(p, "expression")?;
    let elements = constrained_param(ctx.doc, p, "constrained", context)?;
    let (name, language) = (text(p, "name"), text(p, "language"));
    let c = ctx
        .doc
        .transaction(|st| {
            let c = st.create(CONSTRAINT)?;
            if !name.is_empty() {
                st.set(c, named_element::NAME, Value::str(name))?;
            }
            set_specification(st, c, expression, language)?;
            set_constrained(st, c, &elements)?;
            st.push(context, namespace::OWNED_RULE, Value::Ref(c))?;
            Ok(c)
        })
        .map_err(fail)?;
    ctx.view.selection = vec![c];
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn constraint_set(ctx: &mut C<'_>, p: &Params) -> R {
    let c = constraint_param(ctx.doc, p, "constraint")?;
    let context = context_of(ctx.doc, c).ok_or_else(|| refuse("constraint", "the constraint has no context"))?;
    let expression = match p.str("expression") {
        Some(_) => Some(expression_param(p, "expression")?.to_string()),
        None => None,
    };
    let elements = match p.get("constrained") {
        Some(_) => Some(constrained_param(ctx.doc, p, "constrained", context)?),
        None => None,
    };
    let name = p.str("name").map(str::to_string);
    let language = p.str("language").map(str::to_string);
    let (now_expression, now_language) = (self::expression(ctx.doc, c), self::language(ctx.doc, c));
    ctx.doc
        .transaction(|st| {
            if let Some(name) = &name {
                st.set(c, named_element::NAME, if name.is_empty() { Value::Null } else { Value::str(name.as_str()) })?;
            }
            if expression.is_some() || language.is_some() {
                let e = expression.clone().unwrap_or(now_expression);
                let l = language.clone().unwrap_or(now_language);
                set_specification(st, c, &e, &l)?;
                fit_braces(st, c)?;
            }
            if let Some(elements) = &elements {
                set_constrained(st, c, elements)?;
            }
            Ok(())
        })
        .map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn constraint_move(ctx: &mut C<'_>, p: &Params) -> R {
    let c = constraint_param(ctx.doc, p, "constraint")?;
    let context = context_of(ctx.doc, c).ok_or_else(|| refuse("constraint", "the constraint has no context"))?;
    let all: Vec<Obj> = ctx.doc.list(context, namespace::OWNED_RULE).unwrap_or_default().iter().filter_map(Value::as_ref).collect();
    let last = all.len().saturating_sub(1);
    let from = all.iter().position(|x| *x == c).ok_or_else(|| refuse("constraint", "the constraint is not among its context's"))?;
    let at = p.integer("at").unwrap_or(-1);
    let to = usize::try_from(at)
        .ok()
        .filter(|i| *i <= last)
        .ok_or_else(|| refuse("at", format!("{at} is past the last constraint; a place from 0 to {last}")))?;
    let (Ok(from), Ok(to)) = (u32::try_from(from), u32::try_from(to)) else {
        return Err(refuse("at", format!("{at} is too far; a place from 0 to {last}")));
    };
    ctx.doc.move_item(context, namespace::OWNED_RULE, from, to).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, c)? }))
}

fn constraint_show(ctx: &mut C<'_>, p: &Params) -> R {
    let d = object(ctx.doc, "diagram", text(p, "diagram"))?;
    let kind = ctx.doc.get(d, diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    if ctx.doc.class_of(d).ok() != Some(archforge_diagram::DIAGRAM) || kind != crate::CLASS_DIAGRAM {
        return Err(refuse("diagram", format!("{} is not a class diagram", text(p, "diagram"))));
    }
    let c = constraint_param(ctx.doc, p, "constraint")?;
    let form = match text(p, "as") {
        "braces" => Form::Braces,
        _ => Form::Note,
    };
    let at = match (p.number("x"), p.number("y")) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => return Err(refuse("x", "give both x and y, or neither for where it reads")),
    };
    let n = show(ctx.doc, d, c, form, at)?;
    ctx.view.selection = vec![n];
    Ok(json!({ "id": uuid_of(ctx.doc, n)? }))
}

/// A constraint as the reads give it.
pub fn constraint_json(store: &Store, c: Obj) -> Json {
    let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
    let elements: Vec<Json> = constrained(store, c).into_iter().map(|e| json!({ "id": id(e), "name": name_of(store, e) })).collect();
    let shown: Vec<Json> = archforge_diagram::views_of(store, c)
        .2
        .into_iter()
        .filter_map(|n| Some(json!({ "diagram": id(archforge_diagram::diagram_of(store, n)?), "as": form_of(store, n)?.word(), "node": id(n) })))
        .collect();
    json!({
        "id": id(c),
        "name": name_of(store, c),
        "expression": expression(store, c),
        "language": language(store, c),
        "constrained": elements,
        "shown": shown,
        "evaluated": false,
    })
}

fn constraint_list(ctx: &mut C<'_>, p: &Params) -> R {
    let context = context_param(ctx.doc, p, "context")?;
    let items: Vec<Json> = constraints_of(ctx.doc, context).into_iter().map(|c| constraint_json(ctx.doc, c)).collect();
    Ok(json!({ "constraints": items }))
}

/// The constraint commands; removing goes through `element.remove`.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.constraint.add",
            "Add constraint",
            &["Constraint"],
            None,
            concat!(
                "context: string; the id of the element that owns it: a class, an interface, an enumeration, a data type, a primitive type of the model's own, an association class or a package\n",
                "expression: string; the rule, kept exactly as typed and never evaluated; anything but empty\n",
                "name: string = \"\"; its name, or nothing\n",
                "language: string = \"\"; the language the expression is written in, as typed (OCL, English, any other), or nothing for plain text\n",
                "constrained: array = []; the ids of the elements it constrains, in order, each of this model (a package, a classifier, an attribute or end, an operation, a parameter, a literal or a relationship), at most 256; [] for the context alone"
            ),
            has_contexts,
            constraint_add
        ),
        cmd!(
            "uml.constraint.set",
            "Set constraint",
            &["Constraint"],
            None,
            concat!(
                "constraint: string; the constraint's id\n",
                "expression: string?; the rule, kept exactly as typed and never evaluated; anything but empty; left out, kept\n",
                "name: string?; its name, \"\" for none; left out, kept\n",
                "language: string?; the language, as typed, \"\" for plain text; left out, kept\n",
                "constrained: array?; the ids of the elements it constrains, in order, each of this model, at most 256, [] for its context alone; left out, kept"
            ),
            has_constraints,
            constraint_set
        ),
        cmd!(
            "uml.constraint.move",
            "Move constraint",
            &["Constraint"],
            None,
            "constraint: string; the constraint's id\nat: integer = -1, -1..1000000; its new place among its context's constraints, from 0",
            has_constraints,
            constraint_move
        ),
        cmd!(
            "uml.constraint.show",
            "Show constraint on diagram",
            &["Constraint", "Diagram"],
            None,
            concat!(
                "diagram: string; the class diagram's id\n",
                "constraint: string; the constraint's id\n",
                "as: string = \"note\", braces|note; braces beside the first of its elements the diagram shows, for short text, or a note linked to its elements, for long text; it replaces how the diagram showed it\n",
                "x: number?; the left edge, with y; left out, where it reads\n",
                "y: number?; the top edge, with x"
            ),
            has_constraints,
            constraint_show
        ),
        cmd!(
            query "uml.constraint.list",
            "List constraints",
            &["Constraint"],
            None,
            "context: string; the id of the element that owns them",
            has_contexts,
            constraint_list
        ),
    ]
}

/// The constraints `dropped` leaves constraining nothing: they go with what they constrained.
pub(crate) fn orphans(dropped: &[Dropped]) -> BTreeSet<Obj> {
    dropped.iter().filter(|d| d.remaining.is_empty()).map(|d| d.constraint).collect()
}
