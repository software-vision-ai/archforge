//! Template bindings (AR-131): UML's TemplateBinding, owned by its bound element through
//! `TemplateableElement::templateBinding`, naming the template's signature, and holding one
//! TemplateParameterSubstitution per parameter it gives an argument (`formal` the parameter,
//! `actual` the argument). A binding is its own relationship: neither a dependency nor a
//! realization, never named, drawn as a dashed line from the bound element to the template with an
//! open arrowhead and the label `«bind» <T -> Order, Key -> Integer>`.
//!
//! The bound element is a class, an interface or a data type of the model's own, of the template's
//! own metaclass (a class binds a class template), never the template itself, never one that binds
//! that template already, and never one the template binds, directly or round a chain of bindings.
//! The template is a classifier with at least one parameter. Every argument fits its parameter as a
//! default must ([`crate::templates::fits`]): of the parameter's kind, the constraining classifier
//! or one that specializes it, a classifier of the model or a built-in type, never a template
//! parameter's own classifier. A parameter without a default needs an argument; one with a default
//! may be left out, and the binding then uses the default, whatever it is at the time. Each
//! parameter is substituted at most once.
//!
//! Nothing a binding names is left dangling. Renaming a parameter or an argument changes nothing
//! stored: the substitution names them by identity. A parameter a binding substitutes, or a
//! template's last while any binding names its signature, is not removed (AR-130,
//! [`crate::templates::removals_problem`]); a classifier a binding uses as an argument is not removed
//! either ([`argument_removal_problem`]), nor a generalization or a classifier an argument fits its
//! constraint only through ([`crate::templates::constraint_problem`]); the refusal names the
//! binding, which is changed or removed first. A template removed takes the bindings to it along, and a bound element
//! removed takes its own. A parameter without a default is not added to a template a binding names,
//! nor is a default cleared that a binding relies on ([`needs_argument_problem`]).

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, cmd};
use archforge_mof::{Obj, Store, TypeRef, Value};
use serde_json::{Map, Value as Json, json};

use crate::commands::{a_what, fail, name_of, refuse, short, uuid_of};
use crate::templates::{self, Kind};
use crate::{
    PRIMITIVE_TYPE, TEMPLATE_BINDING, TEMPLATE_PARAMETER_SUBSTITUTION, template_binding, template_parameter_substitution, templateable_element,
};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The longest label a binding's line carries: it is built every frame, so it never grows with
/// what the model holds.
const MAX_LABEL: usize = 200;

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn list(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

/// Whether `o` is a live template binding.
pub fn is_binding(store: &Store, o: Obj) -> bool {
    store.is_alive(o) && store.class_of(o).ok() == Some(TEMPLATE_BINDING)
}

/// The bindings `bound` owns, in its order.
pub fn bindings_of(store: &Store, bound: Obj) -> Vec<Obj> {
    list(store, bound, templateable_element::TEMPLATE_BINDING).into_iter().filter(|b| is_binding(store, *b)).collect()
}

/// The template a binding binds: the classifier whose signature it names.
pub fn template_of(store: &Store, binding: Obj) -> Option<Obj> {
    let sig = one(store, binding, template_binding::SIGNATURE)?;
    match store.container(sig).ok()?? {
        (c, prop) if prop == crate::classifier::OWNED_TEMPLATE_SIGNATURE => Some(c),
        _ => None,
    }
}

/// A binding's substitutions, in its order.
pub fn substitutions(store: &Store, binding: Obj) -> Vec<Obj> {
    list(store, binding, template_binding::PARAMETER_SUBSTITUTION)
        .into_iter()
        .filter(|s| store.class_of(*s).ok() == Some(TEMPLATE_PARAMETER_SUBSTITUTION))
        .collect()
}

/// The substitution of `binding` for parameter `p`, when it gives `p` an argument.
pub fn substitution_for(store: &Store, binding: Obj, p: Obj) -> Option<Obj> {
    substitutions(store, binding).into_iter().find(|s| one(store, *s, template_parameter_substitution::FORMAL) == Some(p))
}

/// What a binding gives parameter `p`: its own argument, or, left out, the parameter's default,
/// with whether it is the default; none when it gives nothing and there is no default.
pub fn argument(store: &Store, binding: Obj, p: Obj) -> Option<(Obj, bool)> {
    match substitution_for(store, binding, p).and_then(|s| templates::actual_of(store, s)) {
        Some(a) => Some((a, false)),
        None => templates::default_of(store, p).map(|d| (d, true)),
    }
}

/// The template's parameters a binding gives no argument and that have no default: what it still
/// needs.
pub fn missing(store: &Store, binding: Obj) -> Vec<Obj> {
    let Some(t) = template_of(store, binding) else { return Vec::new() };
    templates::parameters(store, t).into_iter().filter(|p| argument(store, binding, *p).is_none()).collect()
}

/// The text inside the binding's angle brackets: each parameter with what the binding gives it, in
/// the signature's order, `T -> Order, Key -> Integer`; a parameter given nothing is left out.
pub fn substitution_text(store: &Store, binding: Obj) -> String {
    let Some(t) = template_of(store, binding) else { return String::new() };
    templates::parameters(store, t)
        .into_iter()
        .take(templates::MAX_PARAMETERS)
        .filter_map(|p| argument(store, binding, p).map(|(a, _)| format!("{} -> {}", templates::parameter_name(store, p), name_of(store, a))))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The label of a binding's line, `«bind» <T -> Order, Key -> Integer>`, cut to a bounded length.
/// The arrow is written `->`, as UML's notation has it and every message of the shell says it.
pub fn label(store: &Store, binding: Obj) -> String {
    let text = format!("«bind» <{}>", substitution_text(store, binding));
    if text.chars().count() <= MAX_LABEL { text } else { text.chars().take(MAX_LABEL - 1).chain(['…']).collect() }
}

/// The substitutions whose argument is `c`, each with its binding: what removing `c` would leave
/// dangling.
pub fn argument_uses(store: &Store, c: Obj) -> Vec<(Obj, Obj)> {
    let mut out: Vec<(Obj, Obj)> = store
        .referrers(c)
        .into_iter()
        .filter(|(s, prop)| *prop == template_parameter_substitution::ACTUAL && store.class_of(*s).ok() == Some(TEMPLATE_PARAMETER_SUBSTITUTION))
        .filter(|(s, _)| store.is_alive(*s))
        .filter_map(|(s, _)| Some((templates::binding_of(store, s)?, s)))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Whether a classifier of `class` may bind a template: a class, an interface or a data type, the
/// kinds that may be templates.
pub fn can_bind(class: archforge_mof::ClassId) -> bool {
    templates::can_be_template(class)
}

/// Whether `from` binds `to`, directly or through the templates its templates bind; a seen-set
/// bounds the walk.
pub fn binds(store: &Store, from: Obj, to: Obj) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![from];
    while let Some(c) = stack.pop() {
        if !seen.insert(c) {
            continue;
        }
        for b in bindings_of(store, c) {
            if let Some(t) = template_of(store, b) {
                if t == to {
                    return true;
                }
                stack.push(t);
            }
        }
    }
    false
}

/// Why `bound` may not bind `template`, when it may not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// Only a class, an interface or a data type of the model's own binds a template.
    NotBindable,
    /// The template has no parameter: it is no template.
    NotATemplate,
    /// A class binds a class template, an interface an interface template, a data type a data type
    /// template.
    OtherKind,
    /// A template does not bind itself.
    Itself,
    /// The bound element binds that template already.
    Again,
    /// The template binds the bound element already, directly or round other bindings: a cycle.
    Cycle,
}

impl Problem {
    /// The parameter of `uml.template-binding.add` a refusal names.
    pub fn key(self) -> &'static str {
        match self {
            Problem::NotBindable => "bound",
            _ => "template",
        }
    }

    /// Why, as a refusal says it, with the ids of the two.
    pub fn said(self, store: &Store, bound: Obj, template: Obj) -> String {
        let id = |o: Obj| uuid_of(store, o).unwrap_or_default();
        match self {
            Problem::NotBindable => {
                format!("{} is {}; only a class, an interface or a data type of the model's own binds a template", id(bound), a_what(store, bound))
            }
            Problem::NotATemplate => {
                format!("{} ({}) has no template parameter, so it is no template; give it one first", id(template), name_of(store, template))
            }
            Problem::OtherKind => format!(
                "{} is {} and {} is {}; a template is bound by a classifier of its own kind",
                id(bound),
                a_what(store, bound),
                id(template),
                a_what(store, template)
            ),
            Problem::Itself => format!("{}: a template does not bind itself", id(bound)),
            Problem::Again => format!(
                "{} binds {} already; one binding gives each parameter its argument, so change that one",
                name_of(store, bound),
                name_of(store, template)
            ),
            Problem::Cycle => format!(
                "{} binds {} already, directly or through other bindings, which would make a cycle",
                name_of(store, template),
                name_of(store, bound)
            ),
        }
    }
}

/// Why `bound` may not bind `template`, when it may not. `except` is a binding being changed,
/// which never clashes with itself.
pub fn problem(store: &Store, bound: Obj, template: Obj, except: Option<Obj>) -> Option<Problem> {
    let class = store.class_of(bound).ok();
    if !class.is_some_and(can_bind)
        || crate::members::is_built_in(store, bound)
        || templates::is_parameter_element(store, bound)
        || !crate::constraints::in_model(store, bound)
    {
        return Some(Problem::NotBindable);
    }
    if !templates::is_template(store, template) {
        return Some(Problem::NotATemplate);
    }
    if bound == template {
        return Some(Problem::Itself);
    }
    if store.class_of(template).ok() != class {
        return Some(Problem::OtherKind);
    }
    if bindings_of(store, bound).into_iter().any(|b| Some(b) != except && template_of(store, b) == Some(template)) {
        return Some(Problem::Again);
    }
    binds(store, template, bound).then_some(Problem::Cycle)
}

/// An argument as a call names it, before anything is written: a classifier of the model, or a
/// built-in type by its name, which may not be made yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    Classifier(Obj),
    BuiltIn(&'static str),
}

/// The built-in type `name` names when it is made already.
fn built_in(store: &Store, name: &str) -> Option<Obj> {
    let pkg = crate::members::types_package(store)?;
    crate::commands::primitive_types(store, pkg).into_iter().find(|t| name_of(store, *t) == name && crate::members::is_built_in(store, *t))
}

/// The argument `value` names: a built-in type's name (made yet or not) or the id of a classifier of
/// the model's own, never a template parameter's classifier.
pub fn arg_of(store: &Store, key: &str, value: &str) -> Result<Arg, CommandError> {
    let value = value.trim();
    if let Some(b) = crate::members::BUILT_IN.iter().find(|b| b.eq_ignore_ascii_case(value)) {
        return Ok(built_in(store, b).map_or(Arg::BuiltIn(b), Arg::Classifier));
    }
    let Some(uuid) = archforge_mof::Uuid::parse(value) else {
        return Err(refuse(
            key,
            format!("{:?} is neither a classifier's id nor one of the built-in types {}", short(value), crate::members::BUILT_IN.join(", ")),
        ));
    };
    let c = store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {}", short(value))))?;
    if let Some(why) = templates::parameter_element_problem(store, c) {
        return Err(refuse(key, why));
    }
    if !store.class_of(c).is_ok_and(crate::is_classifier) {
        return Err(refuse(key, format!("{value} is {}, and an argument is a classifier", a_what(store, c))));
    }
    if !crate::constraints::in_model(store, c) {
        return Err(refuse(key, format!("{value} is not in this model")));
    }
    Ok(Arg::Classifier(c))
}

/// Whether `arg` may be the argument of parameter `p`: a built-in type not made yet is a primitive
/// type that specializes nothing, so it fits only an unconstrained parameter whose kind takes a
/// primitive type.
pub fn arg_fits(store: &Store, p: Obj, arg: Arg) -> bool {
    let Some(kind) = templates::kind_of(store, p) else { return false };
    let constraint = templates::constraint_of(store, p);
    match arg {
        Arg::Classifier(c) => templates::fits(store, c, kind, constraint),
        Arg::BuiltIn(_) => constraint.is_none() && kind.metaclass().is_some_and(|m| store.universe().conforms(PRIMITIVE_TYPE, TypeRef::Class(m))),
    }
}

/// What an argument of `kind` is, in a refusal.
fn kind_words(kind: Option<Kind>) -> &'static str {
    match kind {
        Some(Kind::Class) => "a class",
        Some(Kind::Interface) => "an interface",
        Some(Kind::DataType) => "a data type (a primitive type or an enumeration is one too)",
        Some(Kind::Enumeration) => "an enumeration",
        Some(Kind::PrimitiveType) => "a primitive type",
        _ => "a classifier of its kind",
    }
}

/// Why `arg` does not fit parameter `p`, as a refusal says it.
fn misfit(store: &Store, p: Obj, arg: Arg) -> String {
    let name = match arg {
        Arg::Classifier(c) => name_of(store, c),
        Arg::BuiltIn(b) => b.to_string(),
    };
    let kind = templates::kind_of(store, p);
    match templates::constraint_of(store, p) {
        Some(c) => format!(
            "{name} does not fit {}: its argument is {} and must be {} or specialize it",
            templates::parameter_name(store, p),
            kind_words(kind),
            name_of(store, c)
        ),
        None => format!("{name} does not fit {}: its argument is {}", templates::parameter_name(store, p), kind_words(kind)),
    }
}

/// The parameters of `template` a call's `arguments` object names, each with the argument it is
/// given, or none to go back to the default; read and checked against the template before anything
/// is written. A parameter is named by its id or its name, and named once.
fn arguments(store: &Store, template: Obj, given: &Map<String, Json>) -> Result<Vec<(Obj, Option<Arg>)>, CommandError> {
    let params = templates::parameters(store, template);
    if given.len() > templates::MAX_PARAMETERS {
        return Err(refuse("arguments", format!("{} arguments, and a template has at most {}", given.len(), templates::MAX_PARAMETERS)));
    }
    let mut out: Vec<(Obj, Option<Arg>)> = Vec::new();
    for (key, value) in given {
        let p =
            params.iter().copied().find(|p| uuid_of(store, *p).is_ok_and(|u| u == *key) || templates::parameter_name(store, *p) == *key).ok_or_else(
                || {
                    let names: Vec<String> = params.iter().map(|p| templates::parameter_name(store, *p)).collect();
                    refuse(
                        "arguments",
                        format!("{} has no template parameter {:?}; its parameters are {}", name_of(store, template), short(key), names.join(", ")),
                    )
                },
            )?;
        if out.iter().any(|(q, _)| *q == p) {
            return Err(refuse(
                "arguments",
                format!("{} is given twice; a binding substitutes each parameter once", templates::parameter_name(store, p)),
            ));
        }
        let Some(value) = value.as_str() else {
            return Err(refuse("arguments", format!("the argument for {} is no string: a classifier's id or a built-in type's name", short(key))));
        };
        let arg = if value.trim().is_empty() { None } else { Some(arg_of(store, "arguments", value)?) };
        // A built-in type not made yet is made when the binding is written; what would stop that
        // is said now, before anything is written.
        if let Some(Arg::BuiltIn(b)) = arg
            && let Some(pkg) = crate::members::types_package(store)
            && let Some(c) = crate::commands::classifiers(store, pkg).into_iter().find(|c| name_of(store, *c).eq_ignore_ascii_case(b))
        {
            return Err(refuse(
                "arguments",
                format!(
                    "the package {} holds {} named {}, where the built-in type {b} is kept; rename it to use the built-in type",
                    crate::members::TYPES,
                    a_what(store, c),
                    name_of(store, c)
                ),
            ));
        }
        if let Some(a) = arg
            && !arg_fits(store, p, a)
        {
            return Err(refuse("arguments", misfit(store, p, a)));
        }
        if arg.is_none() && templates::default_of(store, p).is_none() {
            return Err(refuse(
                "arguments",
                format!("{} has no default, so a binding gives it an argument; it cannot go back to a default", templates::parameter_name(store, p)),
            ));
        }
        out.push((p, arg));
    }
    Ok(out)
}

/// Why a binding of `template` that gives the arguments `given` (and, for a binding being changed,
/// `kept` substitutions' parameters) leaves a parameter without an argument and without a default:
/// the parameters named, and what to do.
fn missing_problem(store: &Store, template: Obj, given: &[(Obj, Option<Arg>)], kept: &[Obj]) -> Option<String> {
    let lacking: Vec<String> = templates::parameters(store, template)
        .into_iter()
        .filter(|p| templates::default_of(store, *p).is_none())
        .filter(|p| !given.iter().any(|(q, a)| q == p && a.is_some()) && !kept.contains(p))
        .map(|p| format!("{} ({})", templates::parameter_name(store, p), kind_words(templates::kind_of(store, p))))
        .collect();
    (!lacking.is_empty())
        .then(|| format!("{} needs an argument for {}, which has no default; give it in arguments", name_of(store, template), lacking.join(", ")))
}

/// Writes the argument of parameter `p` into `binding`: a new substitution, or the one it has
/// changed, or, with none, the substitution removed so the default is used.
fn put_argument(store: &mut Store, binding: Obj, p: Obj, arg: Option<Arg>) -> Result<(), CommandError> {
    let actual = match arg {
        Some(Arg::Classifier(c)) => Some(c),
        Some(Arg::BuiltIn(b)) => crate::members::resolve_type(store, "arguments", b, None)?,
        None => None,
    };
    let existing = substitution_for(store, binding, p);
    match (existing, actual) {
        (Some(s), Some(a)) => store.set(s, template_parameter_substitution::ACTUAL, Value::Ref(a)).map_err(fail)?,
        (Some(s), None) => {
            store.destroy(s).map_err(fail)?;
        }
        (None, Some(a)) => {
            let s = store.create(TEMPLATE_PARAMETER_SUBSTITUTION).map_err(fail)?;
            store.set(s, template_parameter_substitution::FORMAL, Value::Ref(p)).map_err(fail)?;
            store.set(s, template_parameter_substitution::ACTUAL, Value::Ref(a)).map_err(fail)?;
            store.push(binding, template_binding::PARAMETER_SUBSTITUTION, Value::Ref(s)).map_err(fail)?;
        }
        (None, None) => {}
    }
    Ok(())
}

/// The class, interface or data type under `key` that may be a template or bind one.
fn classifier_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    crate::commands::classifier_of(store, p, key)
}

/// The arguments object under `key`, empty when it is left out.
fn arguments_param(p: &Params, key: &str) -> Result<Map<String, Json>, CommandError> {
    match p.get(key) {
        None | Some(Json::Null) => Ok(Map::new()),
        Some(Json::Object(m)) => Ok(m.clone()),
        Some(other) => Err(refuse(key, format!("{} is no object of parameter names or ids mapped to arguments", short(&other.to_string())))),
    }
}

/// The binding under `key`.
fn binding_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    crate::commands::id_of(store, p, key, TEMPLATE_BINDING, "a template binding")
}

/// `uml.template-binding.add`: `bound` binds `template`, giving its parameters the arguments named;
/// a parameter with a default may be left out and then uses it. Everything is checked before the
/// binding is written, as one undo step.
fn binding_add(ctx: &mut C<'_>, p: &Params) -> R {
    let bound = classifier_param(ctx.doc, p, "bound")?;
    let template = classifier_param(ctx.doc, p, "template")?;
    if let Some(problem) = problem(ctx.doc, bound, template, None) {
        return Err(refuse(problem.key(), problem.said(ctx.doc, bound, template)));
    }
    let given = arguments(ctx.doc, template, &arguments_param(p, "arguments")?)?;
    if let Some(why) = missing_problem(ctx.doc, template, &given, &[]) {
        return Err(refuse("arguments", why));
    }
    let sig = templates::signature(ctx.doc, template).ok_or_else(|| refuse("template", "the template has no signature"))?;
    let b = ctx.doc.create(TEMPLATE_BINDING).map_err(fail)?;
    ctx.doc.set(b, template_binding::SIGNATURE, Value::Ref(sig)).map_err(fail)?;
    ctx.doc.push(bound, templateable_element::TEMPLATE_BINDING, Value::Ref(b)).map_err(fail)?;
    // In the signature's order, so the substitutions read as the label does.
    let order = templates::parameters(ctx.doc, template);
    let mut given = given;
    given.sort_by_key(|(q, _)| order.iter().position(|x| x == q));
    for (q, arg) in given {
        put_argument(ctx.doc, b, q, arg)?;
    }
    ctx.view.selection = vec![b];
    Ok(binding_json(ctx.doc, b))
}

/// `uml.template-binding.set`: a binding's bound element, template or arguments changed, everything
/// checked before anything changes. Another template drops the old one's substitutions and takes
/// the arguments given for the new one; the same template keeps the arguments not named.
fn binding_set(ctx: &mut C<'_>, p: &Params) -> R {
    let b = binding_param(ctx.doc, p, "binding")?;
    let old_bound = templates::bound_element(ctx.doc, b).ok_or_else(|| refuse("binding", "the binding binds no element"))?;
    let old_template = template_of(ctx.doc, b).ok_or_else(|| refuse("binding", "the binding names no template"))?;
    let bound = match p.str("bound") {
        Some(_) => classifier_param(ctx.doc, p, "bound")?,
        None => old_bound,
    };
    let template = match p.str("template") {
        Some(_) => classifier_param(ctx.doc, p, "template")?,
        None => old_template,
    };
    if let Some(problem) = problem(ctx.doc, bound, template, Some(b)) {
        return Err(refuse(if problem == Problem::NotBindable { "bound" } else { "template" }, problem.said(ctx.doc, bound, template)));
    }
    let given = arguments(ctx.doc, template, &arguments_param(p, "arguments")?)?;
    let same = template == old_template;
    let kept: Vec<Obj> = if same {
        substitutions(ctx.doc, b).into_iter().filter_map(|s| one(ctx.doc, s, template_parameter_substitution::FORMAL)).collect()
    } else {
        Vec::new()
    };
    if let Some(why) = missing_problem(ctx.doc, template, &given, &kept) {
        return Err(refuse(if same { "arguments" } else { "template" }, why));
    }
    move_binding(ctx.doc, b, old_bound, bound, (!same).then_some(template))?;
    let order = templates::parameters(ctx.doc, template);
    let mut given = given;
    given.sort_by_key(|(q, _)| order.iter().position(|x| x == q));
    for (q, arg) in given {
        put_argument(ctx.doc, b, q, arg)?;
    }
    if bound != old_bound || !same {
        crate::relations::follow_ends(ctx.doc, b)?;
    }
    ctx.view.selection = vec![b];
    Ok(binding_json(ctx.doc, b))
}

/// Moves binding `b` from `old` to `bound` when they differ, and, with `template`, to that
/// template's signature, its substitutions of the old template's parameters removed.
fn move_binding(store: &mut Store, b: Obj, old: Obj, bound: Obj, template: Option<Obj>) -> Result<(), CommandError> {
    if bound != old {
        store.remove_value(old, templateable_element::TEMPLATE_BINDING, &Value::Ref(b)).map_err(fail)?;
        store.push(bound, templateable_element::TEMPLATE_BINDING, Value::Ref(b)).map_err(fail)?;
    }
    if let Some(t) = template {
        for s in substitutions(store, b) {
            store.destroy(s).map_err(fail)?;
        }
        let sig = templates::signature(store, t).ok_or_else(|| refuse("template", "the template has no signature"))?;
        store.set(b, template_binding::SIGNATURE, Value::Ref(sig)).map_err(fail)?;
    }
    Ok(())
}

/// One end of binding `b` moved, as `uml.relationship.reconnect` asks: the source to another bound
/// element, keeping its arguments; the target to another template, which then needs every
/// parameter without a default to have an argument, given with `uml.template-binding.set`.
pub(crate) fn reconnect(store: &mut Store, b: Obj, source: bool, c: Obj) -> Result<(), CommandError> {
    let old_bound = templates::bound_element(store, b).ok_or_else(|| refuse("relationship", "the binding binds no element"))?;
    let old_template = template_of(store, b).ok_or_else(|| refuse("relationship", "the binding names no template"))?;
    let (bound, template) = if source { (c, old_template) } else { (old_bound, c) };
    if let Some(problem) = problem(store, bound, template, Some(b)) {
        return Err(refuse("class", problem.said(store, bound, template)));
    }
    if template != old_template
        && let Some(why) = missing_problem(store, template, &[], &[])
    {
        return Err(refuse("class", format!("{why}: set the template and its arguments together with uml.template-binding.set")));
    }
    move_binding(store, b, old_bound, bound, (template != old_template).then_some(template))
}

/// Why removing the classifiers `classes` would leave a binding's argument dangling, when a binding
/// that does not go with them (`going`) uses one: the binding named, and what to do first.
pub fn argument_removal_problem(store: &Store, classes: &[Obj], going: &[Obj]) -> Option<String> {
    for c in classes {
        for (b, s) in argument_uses(store, *c) {
            if going.contains(&b) {
                continue;
            }
            let p = one(store, s, template_parameter_substitution::FORMAL);
            let bound = templates::bound_element(store, b).map(|x| name_of(store, x)).unwrap_or_default();
            let template = template_of(store, b).map(|x| name_of(store, x)).unwrap_or_default();
            return Some(format!(
                "{bound} binds {template} with {} -> {}, so {} is its argument; change or remove that binding ({}) first, then remove {}",
                p.map(|p| templates::parameter_name(store, p)).unwrap_or_default(),
                name_of(store, *c),
                name_of(store, *c),
                uuid_of(store, b).unwrap_or_default(),
                name_of(store, *c)
            ));
        }
    }
    None
}

/// Why a parameter of `template` named `name` with no default (`has_default` false) would leave a
/// binding of the template without an argument: a new parameter (`except` none), or one whose
/// default is being cleared (`except` the parameter, whose own substitutions give it one).
pub fn needs_argument_problem(store: &Store, template: Obj, name: &str, has_default: bool, except: Option<Obj>) -> Option<String> {
    if has_default {
        return None;
    }
    let b = templates::bindings_to(store, template).into_iter().find(|b| except.is_none_or(|p| substitution_for(store, *b, p).is_none()))?;
    let bound = templates::bound_element(store, b).map(|x| name_of(store, x)).unwrap_or_default();
    Some(format!(
        "{bound} binds {} and gives {name} no argument, so {name} needs a default; give it one, or give that binding ({}) an argument for it first",
        name_of(store, template),
        uuid_of(store, b).unwrap_or_default()
    ))
}

/// A binding as the reads give it: its id, bound element, template, its label, and every parameter
/// of the template with its argument, whether that is the default, or none.
pub fn binding_json(store: &Store, b: Obj) -> Json {
    let named = |o: Option<Obj>| o.map_or(Json::Null, |o| json!({ "id": uuid_of(store, o).unwrap_or_default(), "name": name_of(store, o) }));
    let template = template_of(store, b);
    let arguments: Vec<Json> = template
        .map(|t| templates::parameters(store, t))
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let arg = argument(store, b, p);
            json!({
                "parameter": { "id": uuid_of(store, p).unwrap_or_default(), "name": templates::parameter_name(store, p) },
                "argument": named(arg.map(|(a, _)| a)),
                "default": arg.is_some_and(|(_, d)| d),
            })
        })
        .collect();
    json!({
        "id": uuid_of(store, b).unwrap_or_default(),
        "kind": "template_binding",
        "bound": named(templates::bound_element(store, b)),
        "template": named(template),
        "label": label(store, b),
        "arguments": arguments,
    })
}

/// `uml.template-binding.list`: the bindings a classifier owns and those to it, as a template.
fn binding_list(ctx: &mut C<'_>, p: &Params) -> R {
    let c = crate::commands::classifier_of(ctx.doc, p, "classifier")?;
    let store = &*ctx.doc;
    Ok(json!({
        "binds": bindings_of(store, c).into_iter().map(|b| binding_json(store, b)).collect::<Vec<_>>(),
        "bound_by": templates::bindings_to(store, c).into_iter().map(|b| binding_json(store, b)).collect::<Vec<_>>(),
    }))
}

fn has_templates(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| templates::is_template(f.doc, o)) {
        Ok(())
    } else {
        Err("There is no template yet: give a class, an interface or a data type a template parameter first.")
    }
}

fn has_bindings(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| is_binding(f.doc, o)) { Ok(()) } else { Err("There is no template binding yet.") }
}

/// The parameter line both commands share for the arguments.
macro_rules! argument_line {
    () => {
        "each template parameter, by its name or id, mapped to its argument: a classifier's id or a built-in type's name, of the parameter's kind and the parameter's constraint or one that specializes it; never a template parameter's own classifier"
    };
}

/// The binding commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.template-binding.add",
            "Add template binding",
            &["Template binding"],
            None,
            concat!(
                "bound: string; the id of the class, interface or data type that binds the template, of the template's own kind; never the template itself, one that binds it already, or one the template binds\n",
                "template: string; the id of the template: a class, an interface or a data type with template parameters\n",
                "arguments: object = {}; ",
                argument_line!(),
                "; a parameter without a default needs one, a parameter with a default left out uses it"
            ),
            has_templates,
            binding_add
        ),
        cmd!(
            "uml.template-binding.set",
            "Set template binding",
            &["Template binding"],
            None,
            concat!(
                "binding: string; the template binding's id; every field left out keeps what it has\n",
                "bound: string?; the id of the class, interface or data type that binds the template instead, keeping the arguments; left out, kept\n",
                "template: string?; the id of another template to bind: the old template's arguments go, and those given in arguments are the new one's; left out, kept\n",
                "arguments: object?; ",
                argument_line!(),
                "; \"\" takes a parameter back to its default, refused for one without a default; parameters not named keep their arguments"
            ),
            has_bindings,
            binding_set
        ),
        cmd!(
            query "uml.template-binding.list",
            "List template bindings",
            &["Template binding"],
            None,
            "classifier: string; the classifier's id: the bindings it owns as the bound element (binds), and those to it as a template (bound_by)",
            has_bindings,
            binding_list
        ),
    ]
}
