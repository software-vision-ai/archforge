//! Classifier templates and their parameters (AR-130): UML's `Classifier::ownedTemplateSignature`,
//! a RedefinableTemplateSignature whose ordered `ownedParameter` list holds
//! ClassifierTemplateParameters. A class, an interface or a data type becomes a template when it
//! gets its first parameter, and stops being one when its last goes: the signature is made with the
//! first and removed with the last, in the same step, since UML asks a signature for at least one.
//!
//! A parameter is the ClassifierTemplateParameter; what it stands for inside the template is its
//! `ownedParameteredElement`, a classifier of the parameter's kind (a Class for `class`, an
//! Interface for `interface`, a DataType, an Enumeration or a PrimitiveType), named as the
//! parameter is. The parameter's id is its identity: renaming it renames that classifier, moving it
//! reorders the signature, and both keep every reference, a binding's `formal` and a member's
//! type alike. Its kind changing gives it a classifier of the new kind under the same name, and
//! whatever was typed by the old one is typed by the new one in the same step. Value and Operation
//! parameters are listed as kinds UML has, refused as not supported yet, never turned into a
//! classifier parameter.
//!
//! A parameter may be constrained to one classifier (`constrainingClassifier`), whose metaclass
//! is of the parameter's kind, and may have a default (`default`), a classifier of the model of the
//! parameter's kind (UML's `TemplateParameter::must_be_compatible`) that is the constraining
//! classifier or specializes it. `allowSubstitutable` is kept false: an argument is the
//! constraining classifier or one that specializes it, and UML's `has_constraining_classifier`
//! holds with no constraint as well. The parameter's own classifier is not made to specialize the
//! constraining classifier: the constraint is kept in `constrainingClassifier` alone.
//!
//! A template parameter's classifier is the template's own: it is never placed on a diagram, listed
//! in a package, found by the search, renamed, moved or removed by the classifier commands, or
//! given members; only the template's own attributes and operations, and its operations'
//! parameters, are typed by it. Names are told apart among the signature's parameters whatever the
//! case, by the shared name rule ([`name_clash`]); a signature holds at most [`MAX_PARAMETERS`].
//!
//! Bindings (`TemplateBinding`, AR-131) name a parameter through a substitution's `formal`. A
//! parameter a binding substitutes is never removed, and its kind and constraint change only while
//! every argument bound to it still fits; its name and its default change freely, since bindings
//! follow the parameter, not its name. A template removed takes the bindings to it along.

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Obj, PropId, Store, TypeRef, Value};
use serde_json::{Value as Json, json};

use crate::commands::{fail, name_of, refuse, set_name, short, text, uuid_of};
use crate::{
    CLASS, CLASSIFIER_TEMPLATE_PARAMETER, DATA_TYPE, ENUMERATION, INTERFACE, PRIMITIVE_TYPE, REDEFINABLE_TEMPLATE_SIGNATURE, TEMPLATE_BINDING,
    TEMPLATE_PARAMETER_SUBSTITUTION, classifier, classifier_template_parameter, template_binding, template_parameter,
    template_parameter_substitution, template_signature,
};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// The most parameters one signature holds: its box is drawn on every frame, so its rows never grow
/// with what the model holds.
pub const MAX_PARAMETERS: usize = 32;

/// Why the template commands are not available.
pub const NO_TEMPLATES: &str = "There is no class, interface or data type yet.";

/// The classifiers that may be templates: a class, an interface and a data type, each matched by its
/// exact metaclass.
pub const TEMPLATES: [ClassId; 3] = [CLASS, INTERFACE, DATA_TYPE];

/// A parameter's kind: what its argument is. The first five are classifier parameters, each with
/// the metaclass its classifier is; Value and Operation are UML's other kinds, not supported yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Class,
    Interface,
    DataType,
    Enumeration,
    PrimitiveType,
    Value,
    Operation,
}

impl Kind {
    /// Every kind, the supported ones first, in the order a kind menu lists them.
    pub const ALL: [Kind; 7] = [Kind::Class, Kind::Interface, Kind::DataType, Kind::Enumeration, Kind::PrimitiveType, Kind::Value, Kind::Operation];

    /// The kind's name as the commands take it.
    pub fn key(self) -> &'static str {
        match self {
            Kind::Class => "class",
            Kind::Interface => "interface",
            Kind::DataType => "data-type",
            Kind::Enumeration => "enumeration",
            Kind::PrimitiveType => "primitive-type",
            Kind::Value => "value",
            Kind::Operation => "operation",
        }
    }

    /// The kind as the signature writes it, UML's metaclass: `Class`, `DataType`.
    pub fn keyword(self) -> &'static str {
        match self {
            Kind::Class => "Class",
            Kind::Interface => "Interface",
            Kind::DataType => "DataType",
            Kind::Enumeration => "Enumeration",
            Kind::PrimitiveType => "PrimitiveType",
            Kind::Value => "Value",
            Kind::Operation => "Operation",
        }
    }

    /// The metaclass a parameter of this kind stands for, None for a kind not supported yet.
    pub fn metaclass(self) -> Option<ClassId> {
        match self {
            Kind::Class => Some(CLASS),
            Kind::Interface => Some(INTERFACE),
            Kind::DataType => Some(DATA_TYPE),
            Kind::Enumeration => Some(ENUMERATION),
            Kind::PrimitiveType => Some(PRIMITIVE_TYPE),
            Kind::Value | Kind::Operation => None,
        }
    }

    /// Whether a parameter may be of this kind now.
    pub fn supported(self) -> bool {
        self.metaclass().is_some()
    }

    /// The kind named `key`, as the commands take it.
    pub fn from_key(key: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.key() == key)
    }

    /// The kind whose classifier is of metaclass `class`.
    pub fn of_class(class: ClassId) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.metaclass() == Some(class))
    }
}

fn one(store: &Store, o: Obj, prop: PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn list(store: &Store, o: Obj, prop: PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

/// Whether a classifier of `class` may be a template.
pub fn can_be_template(class: ClassId) -> bool {
    TEMPLATES.contains(&class)
}

/// The template signature of `c`, when it has one.
pub fn signature(store: &Store, c: Obj) -> Option<Obj> {
    one(store, c, classifier::OWNED_TEMPLATE_SIGNATURE)
}

/// The template parameters of `c`, in its signature's order; none when it is no template.
pub fn parameters(store: &Store, c: Obj) -> Vec<Obj> {
    signature(store, c)
        .map(|s| list(store, s, template_signature::OWNED_PARAMETER))
        .unwrap_or_default()
        .into_iter()
        .filter(|p| store.class_of(*p).ok() == Some(CLASSIFIER_TEMPLATE_PARAMETER))
        .collect()
}

/// Whether `c` is a template: it has a parameter.
pub fn is_template(store: &Store, c: Obj) -> bool {
    !parameters(store, c).is_empty()
}

/// Whether `o` is a classifier's template parameter.
pub fn is_parameter(store: &Store, o: Obj) -> bool {
    template_of(store, o).is_some()
}

/// The classifier whose template parameter `p` is.
pub fn template_of(store: &Store, p: Obj) -> Option<Obj> {
    if store.class_of(p).ok() != Some(CLASSIFIER_TEMPLATE_PARAMETER) {
        return None;
    }
    let (sig, prop) = store.container(p).ok()??;
    if prop != template_signature::OWNED_PARAMETER {
        return None;
    }
    match store.container(sig).ok()?? {
        (c, prop) if prop == classifier::OWNED_TEMPLATE_SIGNATURE => Some(c),
        _ => None,
    }
}

/// The classifier template parameter `p` stands for inside its template.
pub fn element_of(store: &Store, p: Obj) -> Option<Obj> {
    one(store, p, template_parameter::OWNED_PARAMETERED_ELEMENT).or_else(|| one(store, p, classifier_template_parameter::PARAMETERED_ELEMENT))
}

/// The template parameter whose classifier `o` is, when it is one: what it is owned by.
pub fn parameter_of(store: &Store, o: Obj) -> Option<Obj> {
    match store.container(o).ok()?? {
        (p, prop) if prop == template_parameter::OWNED_PARAMETERED_ELEMENT && store.class_of(p).ok() == Some(CLASSIFIER_TEMPLATE_PARAMETER) => {
            Some(p)
        }
        _ => None,
    }
}

/// Whether `o` is a template parameter's classifier: the template's own, never one of the model's
/// classifiers to list, place or edit as such.
pub fn is_parameter_element(store: &Store, o: Obj) -> bool {
    parameter_of(store, o).is_some()
}

/// Why `o` is not one of the model's classifiers to use by its id, when it is a template
/// parameter's: what the classifier commands refuse.
pub fn parameter_element_problem(store: &Store, o: Obj) -> Option<String> {
    let p = parameter_of(store, o)?;
    let template = template_of(store, p).map(|t| name_of(store, t)).unwrap_or_default();
    Some(format!(
        "{} is the template parameter {} of {template}, which only types {template}'s own members; its name, kind, constraint and default are set with uml.template-parameter.set on the parameter {}",
        uuid_of(store, o).unwrap_or_default(),
        name_of(store, o),
        uuid_of(store, p).unwrap_or_default()
    ))
}

/// Why `ty` may not type a member of `within` (none for an element outside any classifier): it is
/// a template parameter's classifier and `within` is not its template.
pub fn type_scope_problem(store: &Store, ty: Obj, within: Option<Obj>) -> Option<String> {
    let p = parameter_of(store, ty)?;
    let template = template_of(store, p)?;
    if within == Some(template) {
        return None;
    }
    Some(format!(
        "{} is the template parameter {} of {}, which types only that template's own attributes, operations and parameters",
        uuid_of(store, ty).unwrap_or_default(),
        name_of(store, ty),
        name_of(store, template)
    ))
}

/// The kind of template parameter `p`: its classifier's.
pub fn kind_of(store: &Store, p: Obj) -> Option<Kind> {
    element_of(store, p).and_then(|e| store.class_of(e).ok()).and_then(Kind::of_class)
}

/// The classifier template parameter `p` is constrained to, when it is.
pub fn constraint_of(store: &Store, p: Obj) -> Option<Obj> {
    list(store, p, classifier_template_parameter::CONSTRAINING_CLASSIFIER).into_iter().next()
}

/// The default of template parameter `p`, when it has one.
pub fn default_of(store: &Store, p: Obj) -> Option<Obj> {
    one(store, p, template_parameter::DEFAULT)
}

/// The name of template parameter `p`: its classifier's.
pub fn parameter_name(store: &Store, p: Obj) -> String {
    element_of(store, p).map(|e| name_of(store, e)).unwrap_or_default()
}

/// A parameter as the signature box and Properties write it: `T : Class`, `Key : DataType =
/// Integer`, a constraint after `>` (`T : Class > Entity`), as UML's notation has it.
pub fn parameter_text(store: &Store, p: Obj) -> String {
    let mut out = parameter_name(store, p);
    if let Some(k) = kind_of(store, p) {
        out.push_str(&format!(" : {}", k.keyword()));
    }
    if let Some(c) = constraint_of(store, p) {
        out.push_str(&format!(" > {}", name_of(store, c)));
    }
    if let Some(d) = default_of(store, p) {
        out.push_str(&format!(" = {}", name_of(store, d)));
    }
    out.chars().take(200).collect()
}

/// The rows of `c`'s template signature box, one parameter each in their order; empty for none.
pub fn signature_rows(store: &Store, c: Obj) -> Vec<String> {
    parameters(store, c).into_iter().take(MAX_PARAMETERS).map(|p| parameter_text(store, p)).collect()
}

/// The template parameter of `template`, other than `except`, that a parameter named `name` would
/// clash with. Each parameter's classifier has an owner of its own, its parameter, so the shared
/// rule ([`crate::names::kind`], AR-208) is asked across the signature: two parameters clash when
/// their classifiers are of one kind (every classifier is one) and their names differ only in
/// case. The commands and the shell ask this one rule.
pub fn name_clash(store: &Store, template: Obj, name: &str, except: Option<Obj>) -> Option<Obj> {
    let wanted = name.to_lowercase();
    let kind = crate::names::kind(CLASS);
    parameters(store, template).into_iter().find(|p| {
        Some(*p) != except
            && element_of(store, *p).and_then(|e| store.class_of(e).ok()).and_then(crate::names::kind) == kind
            && parameter_name(store, *p).to_lowercase() == wanted
    })
}

/// The substitutions of bindings that substitute template parameter `p`: what a binding keeps of
/// it, through `formal`.
pub fn substitutions(store: &Store, p: Obj) -> Vec<Obj> {
    let mut out: Vec<Obj> = store
        .referrers(p)
        .into_iter()
        .filter(|(s, prop)| *prop == template_parameter_substitution::FORMAL && store.class_of(*s).ok() == Some(TEMPLATE_PARAMETER_SUBSTITUTION))
        .map(|(s, _)| s)
        .filter(|s| store.is_alive(*s))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The binding a substitution belongs to.
pub fn binding_of(store: &Store, substitution: Obj) -> Option<Obj> {
    one(store, substitution, template_parameter_substitution::TEMPLATE_BINDING)
}

/// The element a binding binds: what owns it.
pub fn bound_element(store: &Store, binding: Obj) -> Option<Obj> {
    one(store, binding, template_binding::BOUND_ELEMENT)
}

/// The argument a substitution gives its parameter.
pub fn actual_of(store: &Store, substitution: Obj) -> Option<Obj> {
    one(store, substitution, template_parameter_substitution::ACTUAL)
}

/// The bindings that substitute template parameter `p`, each once, with the bound element and the
/// argument each gives it: `OrderRepository` binds `Key` to `Integer`.
pub fn users(store: &Store, p: Obj) -> Vec<(Obj, Option<Obj>, Option<Obj>)> {
    let mut out: Vec<(Obj, Option<Obj>, Option<Obj>)> = Vec::new();
    for s in substitutions(store, p) {
        let Some(b) = binding_of(store, s) else { continue };
        if out.iter().all(|(x, _, _)| *x != b) {
            out.push((b, bound_element(store, b), actual_of(store, s)));
        }
    }
    out
}

/// The bindings to template `c`: those whose signature is its signature.
pub fn bindings_to(store: &Store, c: Obj) -> Vec<Obj> {
    let Some(sig) = signature(store, c) else { return Vec::new() };
    let mut out: Vec<Obj> = store
        .referrers(sig)
        .into_iter()
        .filter(|(b, prop)| *prop == template_binding::SIGNATURE && store.class_of(*b).ok() == Some(TEMPLATE_BINDING))
        .map(|(b, _)| b)
        .filter(|b| store.is_alive(*b))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// A binding's use of a parameter as a refusal says it: `OrderRepository binds Key to Integer`.
fn use_text(store: &Store, p: Obj, (binding, bound, actual): (Obj, Option<Obj>, Option<Obj>)) -> String {
    let bound = bound.map(|b| name_of(store, b)).unwrap_or_else(|| uuid_of(store, binding).unwrap_or_default());
    match actual {
        Some(a) => format!("{bound} binds {} to {}", parameter_name(store, p), name_of(store, a)),
        None => format!("{bound} binds {}", parameter_name(store, p)),
    }
}

/// Why template parameter `p` may not be removed, when a binding substitutes it: the bindings
/// named, and what to do first.
pub fn removal_problem(store: &Store, p: Obj) -> Option<String> {
    let users = users(store, p);
    let first = *users.first()?;
    let more = users.len().saturating_sub(1);
    let others = match more {
        0 => String::new(),
        1 => " and 1 other binding".to_string(),
        n => format!(" and {n} other bindings"),
    };
    Some(format!("{}{others}; change or remove that binding first, then remove {}", use_text(store, p, first), parameter_name(store, p)))
}

/// Whether `arg` may be the argument, or the default, of a parameter of `kind` constrained to
/// `constraint`: of the kind's metaclass or one under it (UML's `isCompatibleWith`), and the
/// constraining classifier itself or one that specializes it
/// (`constraining_classifiers_constrain_args`).
pub fn fits(store: &Store, arg: Obj, kind: Kind, constraint: Option<Obj>) -> bool {
    let (Some(meta), Ok(class)) = (kind.metaclass(), store.class_of(arg)) else { return false };
    store.universe().conforms(class, TypeRef::Class(meta)) && constraint.is_none_or(|c| crate::relations::specializes(store, arg, c))
}

/// What an argument of `kind` is, in a refusal: a class, a data type.
fn kind_words(kind: Kind) -> &'static str {
    match kind {
        Kind::Class => "a class",
        Kind::Interface => "an interface",
        Kind::DataType => "a data type",
        Kind::Enumeration => "an enumeration",
        Kind::PrimitiveType => "a primitive type",
        Kind::Value => "a value",
        Kind::Operation => "an operation",
    }
}

/// The kind under `key`, refused when it names none or one not supported yet: never turned into
/// another kind.
fn kind_param(p: &Params, key: &str) -> Result<Kind, CommandError> {
    let typed = text(p, key);
    let kind = Kind::from_key(typed)
        .ok_or_else(|| refuse(key, format!("{:?} is no parameter kind; one of {}", short(typed), Kind::ALL.map(Kind::key).join(", "))))?;
    if !kind.supported() {
        return Err(refuse(
            key,
            format!(
                "{} template parameters are not supported yet and are never turned into another kind; the kinds supported are class, interface, data-type, enumeration and primitive-type",
                kind.keyword()
            ),
        ));
    }
    Ok(kind)
}

/// The classifier `value` names for a parameter's constraint or default (`key`): empty for none, a
/// built-in type's name, or the id of a classifier of the model's own, never a template
/// parameter's.
fn classifier_param(store: &mut Store, key: &str, value: &str) -> Result<Option<Obj>, CommandError> {
    let Some(c) = crate::members::resolve_type(store, key, value, None)? else { return Ok(None) };
    if !store.class_of(c).is_ok_and(crate::is_classifier) {
        return Err(refuse(key, format!("{} is not a class, an interface, a data type, an enumeration or a primitive type", short(value))));
    }
    if crate::constraints::in_model(store, c) { Ok(Some(c)) } else { Err(refuse(key, format!("{} is not in this model", short(value)))) }
}

/// The constraint of a parameter of `kind`: a classifier whose metaclass is of the kind, so an
/// argument can be it or specialize it.
fn check_constraint(store: &Store, kind: Kind, constraint: Option<Obj>) -> Result<(), CommandError> {
    let Some(c) = constraint else { return Ok(()) };
    let Some(meta) = kind.metaclass() else { return Ok(()) };
    if !store.class_of(c).is_ok_and(|k| store.universe().conforms(k, TypeRef::Class(meta))) {
        return Err(refuse(
            "constraint",
            format!(
                "{} is {}, and an argument of {} parameter is {}: it could neither be it nor specialize it; constrain it to {} instead",
                name_of(store, c),
                crate::commands::a_what(store, c),
                kind_words(kind),
                kind_words(kind),
                kind_words(kind)
            ),
        ));
    }
    Ok(())
}

/// The default of a parameter of `kind` constrained to `constraint`: of the kind, and the
/// constraining classifier or one that specializes it.
fn check_default(store: &Store, kind: Kind, constraint: Option<Obj>, default: Option<Obj>) -> Result<(), CommandError> {
    let Some(d) = default else { return Ok(()) };
    let Some(meta) = kind.metaclass() else { return Ok(()) };
    if !store.class_of(d).is_ok_and(|k| store.universe().conforms(k, TypeRef::Class(meta))) {
        return Err(refuse(
            "default",
            format!(
                "{} is {}, and a default of {} parameter is {}",
                name_of(store, d),
                crate::commands::a_what(store, d),
                kind_words(kind),
                kind_words(kind)
            ),
        ));
    }
    if let Some(c) = constraint
        && !crate::relations::specializes(store, d, c)
    {
        return Err(refuse(
            "default",
            format!(
                "{} neither is {} nor specializes it, and the parameter is constrained to {}",
                name_of(store, d),
                name_of(store, c),
                name_of(store, c)
            ),
        ));
    }
    Ok(())
}

/// A parameter's name in `template`: a name, none of its other parameters' whatever the case.
fn check_name(store: &Store, template: Obj, name: &str, except: Option<Obj>) -> Result<(), CommandError> {
    if let Some(problem) = archforge_core::skills::name_problem(name, crate::commands::MAX_NAME) {
        return Err(refuse("name", format!("the name {problem}")));
    }
    if let Some(other) = name_clash(store, template, name, except) {
        return Err(refuse(
            "name",
            format!(
                "{} has a template parameter named {} already, {}, and {:?} matches it whatever the case; choose a name none of its other parameters has",
                name_of(store, template),
                parameter_name(store, other),
                uuid_of(store, other)?,
                short(name)
            ),
        ));
    }
    Ok(())
}

/// The class, interface or data type under `key`, which may be a template.
fn template_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let c = crate::commands::classifier_of(store, p, key)?;
    let class = store.class_of(c).map_err(fail)?;
    if !can_be_template(class) {
        return Err(refuse(
            key,
            format!("{} is {} and is no template; a class, an interface or a data type is", text(p, key), crate::commands::a_what(store, c)),
        ));
    }
    Ok(c)
}

/// The template parameter under `key`, with its template.
fn parameter_param(store: &Store, p: &Params, key: &str) -> Result<(Obj, Obj), CommandError> {
    let typed = text(p, key);
    let obj = archforge_mof::Uuid::parse(typed)
        .and_then(|u| store.by_uuid(u))
        .filter(|o| store.is_alive(*o))
        .ok_or_else(|| refuse(key, format!("there is no template parameter {}", short(typed))))?;
    // The parameter's classifier stands for it.
    let param = parameter_of(store, obj).unwrap_or(obj);
    let template = template_of(store, param).ok_or_else(|| refuse(key, format!("{} is no template parameter of a classifier", short(typed))))?;
    Ok((param, template))
}

/// Why the bindings of parameter `p` would no longer fit it as `kind` constrained to `constraint`:
/// the first argument that does not, named with its binding.
fn bindings_problem(store: &Store, p: Obj, kind: Kind, constraint: Option<Obj>) -> Option<String> {
    let bad = users(store, p).into_iter().find(|(_, _, actual)| actual.is_some_and(|a| !fits(store, a, kind, constraint)))?;
    let why = match constraint {
        Some(c) => format!("as {} constrained to {} it would not fit", kind_words(kind), name_of(store, c)),
        None => format!("it is not {}", kind_words(kind)),
    };
    Some(format!("{}, and {why}; change or remove that binding first", use_text(store, p, bad)))
}

/// Gives template parameter `p` a classifier of `kind` named `name` in place of `old`, typing by the
/// new one whatever was typed by the old one, and removes the old one.
fn change_kind(store: &mut Store, p: Obj, old: Obj, kind: Kind, name: &str) -> Result<Obj, CommandError> {
    let meta = kind.metaclass().ok_or_else(|| refuse("kind", format!("{} parameters are not supported yet", kind.keyword())))?;
    let new = store.create(meta).map_err(fail)?;
    set_name(store, new, name)?;
    // What refers to the old classifier from outside the parameter, read before it moves.
    let referrers: Vec<(Obj, PropId)> = store.referrers(old).into_iter().filter(|(src, _)| *src != p && !store.contains(old, *src)).collect();
    store.set(p, template_parameter::OWNED_PARAMETERED_ELEMENT, Value::Ref(new)).map_err(fail)?;
    store.set(p, classifier_template_parameter::PARAMETERED_ELEMENT, Value::Ref(new)).map_err(fail)?;
    for (src, prop) in referrers {
        let many = store.universe().prop(prop).is_some_and(|q| q.is_many());
        if many {
            let values = store.list(src, prop).map_err(fail)?;
            if let Some(i) = values.iter().position(|v| v.as_ref() == Some(old)) {
                let i = u32::try_from(i).map_err(|_| refuse("kind", "a list too long to change"))?;
                store.remove_at(src, prop, i).map_err(fail)?;
                store.insert(src, prop, i, Value::Ref(new)).map_err(fail)?;
            }
        } else if store.get(src, prop).ok().and_then(|v| v.as_ref()) == Some(old) {
            store.set(src, prop, Value::Ref(new)).map_err(fail)?;
        }
    }
    if store.is_alive(old) {
        store.destroy(old).map_err(fail)?;
    }
    Ok(new)
}

/// Sets parameter `p`'s constraint and default.
fn set_constraint_and_default(store: &mut Store, p: Obj, constraint: Option<Obj>, default: Option<Obj>) -> Result<(), CommandError> {
    for old in list(store, p, classifier_template_parameter::CONSTRAINING_CLASSIFIER) {
        store.remove_value(p, classifier_template_parameter::CONSTRAINING_CLASSIFIER, &Value::Ref(old)).map_err(fail)?;
    }
    if let Some(c) = constraint {
        store.push(p, classifier_template_parameter::CONSTRAINING_CLASSIFIER, Value::Ref(c)).map_err(fail)?;
    }
    store.set(p, template_parameter::DEFAULT, default.map_or(Value::Null, Value::Ref)).map_err(fail)?;
    Ok(())
}

/// `uml.template-parameter.add`: a template parameter of a class, an interface or a data type, its
/// signature made with the first; its name free among the template's parameters whatever the
/// case, its kind supported, its constraint of its kind and its default fitting both, at the place
/// `at` asks for (from 0 to their count) or, at -1, at the end.
fn parameter_add(ctx: &mut C<'_>, p: &Params) -> R {
    let template = template_param(ctx.doc, p, "classifier")?;
    let count = parameters(ctx.doc, template).len();
    if count >= MAX_PARAMETERS {
        return Err(refuse(
            "classifier",
            format!(
                "{} has {count} template parameters already, and a template has at most {MAX_PARAMETERS}; remove one before adding another",
                name_of(ctx.doc, template)
            ),
        ));
    }
    let name = text(p, "name").to_string();
    check_name(ctx.doc, template, &name, None)?;
    let kind = kind_param(p, "kind")?;
    let at = match p.integer("at").unwrap_or(-1) {
        -1 => count,
        at => usize::try_from(at)
            .ok()
            .filter(|k| *k <= count)
            .ok_or_else(|| refuse("at", format!("{at} is past the end of {count} parameters; a place from 0 to {count}, or -1 for the end")))?,
    };
    let at = u32::try_from(at).map_err(|_| refuse("at", format!("{at} is too far; a place from 0 to {count}, or -1 for the end")))?;
    let constraint = classifier_param(ctx.doc, "constraint", text(p, "constraint"))?;
    check_constraint(ctx.doc, kind, constraint)?;
    let default = classifier_param(ctx.doc, "default", text(p, "default"))?;
    check_default(ctx.doc, kind, constraint, default)?;
    let meta = kind.metaclass().ok_or_else(|| refuse("kind", "not supported yet"))?;
    let sig = match signature(ctx.doc, template) {
        Some(s) => s,
        None => {
            let s = ctx.doc.create(REDEFINABLE_TEMPLATE_SIGNATURE).map_err(fail)?;
            ctx.doc.set(template, classifier::OWNED_TEMPLATE_SIGNATURE, Value::Ref(s)).map_err(fail)?;
            s
        }
    };
    let param = ctx.doc.create(CLASSIFIER_TEMPLATE_PARAMETER).map_err(fail)?;
    // An argument is the constraining classifier or one that specializes it; with no constraint UML
    // asks for false (`has_constraining_classifier`).
    ctx.doc.set(param, classifier_template_parameter::ALLOW_SUBSTITUTABLE, Value::Bool(false)).map_err(fail)?;
    let element = ctx.doc.create(meta).map_err(fail)?;
    set_name(ctx.doc, element, &name)?;
    ctx.doc.set(param, template_parameter::OWNED_PARAMETERED_ELEMENT, Value::Ref(element)).map_err(fail)?;
    ctx.doc.set(param, classifier_template_parameter::PARAMETERED_ELEMENT, Value::Ref(element)).map_err(fail)?;
    set_constraint_and_default(ctx.doc, param, constraint, default)?;
    ctx.doc.insert(sig, template_signature::OWNED_PARAMETER, at, Value::Ref(param)).map_err(fail)?;
    keep_order(ctx.doc, sig)?;
    Ok(json!({ "id": uuid_of(ctx.doc, param)?, "element": uuid_of(ctx.doc, element)?, "classifier": uuid_of(ctx.doc, template)? }))
}

/// Keeps the signature's `parameter` list in the order of its `ownedParameter` list: the store
/// adds a new owned parameter at the end of the list it subsets.
fn keep_order(store: &mut Store, sig: Obj) -> Result<(), CommandError> {
    let owned = store.list(sig, template_signature::OWNED_PARAMETER).map_err(fail)?;
    for (to, v) in owned.iter().enumerate() {
        let now = store.list(sig, template_signature::PARAMETER).map_err(fail)?;
        let Some(from) = now.iter().position(|x| x == v) else { continue };
        if from != to {
            let (Ok(from), Ok(to)) = (u32::try_from(from), u32::try_from(to)) else { continue };
            store.move_item(sig, template_signature::PARAMETER, from, to).map_err(fail)?;
        }
    }
    Ok(())
}

/// `uml.template-parameter.set`: a template parameter's name, kind, constraint and default
/// changed, everything checked before anything changes; a field left out keeps what the parameter
/// has, an empty constraint or default clears it. Its identity stays: the parameter's id, and a
/// rename keeps its classifier too; a kind changed gives it a classifier of the new kind, typing
/// by it whatever was typed by the old one. A kind or a constraint a binding's argument would no
/// longer fit is refused, naming the binding.
fn parameter_set(ctx: &mut C<'_>, p: &Params) -> R {
    let (param, template) = parameter_param(ctx.doc, p, "parameter")?;
    let element = element_of(ctx.doc, param).ok_or_else(|| refuse("parameter", "the parameter stands for no classifier"))?;
    let name = match p.str("name") {
        Some(n) => n.to_string(),
        None => parameter_name(ctx.doc, param),
    };
    check_name(ctx.doc, template, &name, Some(param))?;
    let was = kind_of(ctx.doc, param).ok_or_else(|| refuse("parameter", "the parameter's classifier is of no kind a parameter has"))?;
    let kind = match p.str("kind") {
        Some(_) => kind_param(p, "kind")?,
        None => was,
    };
    let constraint = match p.str("constraint") {
        Some(c) => classifier_param(ctx.doc, "constraint", c)?,
        None => constraint_of(ctx.doc, param),
    };
    check_constraint(ctx.doc, kind, constraint)?;
    let default = match p.str("default") {
        Some(d) => classifier_param(ctx.doc, "default", d)?,
        None => default_of(ctx.doc, param),
    };
    check_default(ctx.doc, kind, constraint, default)?;
    if (kind != was || constraint != constraint_of(ctx.doc, param))
        && let Some(why) = bindings_problem(ctx.doc, param, kind, constraint)
    {
        return Err(refuse(if kind != was { "kind" } else { "constraint" }, why));
    }
    let element = if kind != was { change_kind(ctx.doc, param, element, kind, &name)? } else { element };
    set_name(ctx.doc, element, &name)?;
    set_constraint_and_default(ctx.doc, param, constraint, default)?;
    Ok(json!({ "id": uuid_of(ctx.doc, param)?, "element": uuid_of(ctx.doc, element)? }))
}

/// `uml.template-parameter.move`: a template parameter to the place `at` among its template's,
/// from 0 to their count less one, -1 for the last; a place past the last is refused, not taken as
/// the last.
fn parameter_move(ctx: &mut C<'_>, p: &Params) -> R {
    let (param, template) = parameter_param(ctx.doc, p, "parameter")?;
    let sig = signature(ctx.doc, template).ok_or_else(|| refuse("parameter", "its template has no signature"))?;
    let all = ctx.doc.list(sig, template_signature::OWNED_PARAMETER).map_err(fail)?;
    let last = all.len().saturating_sub(1);
    let from = all.iter().position(|x| x.as_ref() == Some(param)).ok_or_else(|| refuse("parameter", "it is not among its template's parameters"))?;
    let to = match p.integer("at").unwrap_or(-1) {
        -1 => last,
        at => usize::try_from(at)
            .ok()
            .filter(|k| *k <= last)
            .ok_or_else(|| refuse("at", format!("{at} is past the last parameter; a place from 0 to {last}, or -1 for the last")))?,
    };
    let (Ok(from), Ok(to)) = (u32::try_from(from), u32::try_from(to)) else {
        return Err(refuse("at", format!("a place from 0 to {last}, or -1 for the last")));
    };
    ctx.doc.move_item(sig, template_signature::OWNED_PARAMETER, from, to).map_err(fail)?;
    keep_order(ctx.doc, sig)?;
    Ok(json!({ "id": uuid_of(ctx.doc, param)? }))
}

/// Template parameter `p` as the reads give it: its id, name, kind, its classifier's id, its
/// constraint and default, and the bindings that substitute it.
pub fn parameter_json(store: &Store, p: Obj) -> Json {
    let named = |o: Option<Obj>| o.map_or(Json::Null, |o| json!({ "id": uuid_of(store, o).unwrap_or_default(), "name": name_of(store, o) }));
    json!({
        "id": uuid_of(store, p).unwrap_or_default(),
        "name": parameter_name(store, p),
        "kind": kind_of(store, p).map(Kind::key),
        "element": element_of(store, p).and_then(|e| uuid_of(store, e).ok()),
        "constraint": named(constraint_of(store, p)),
        "default": named(default_of(store, p)),
        "bound_by": users(store, p).into_iter().map(|(b, bound, actual)| json!({
            "binding": uuid_of(store, b).unwrap_or_default(),
            "bound": named(bound),
            "actual": named(actual),
        })).collect::<Vec<_>>(),
    })
}

/// `c`'s template parameters as the reads give them, in their order.
pub fn parameters_json(store: &Store, c: Obj) -> Json {
    Json::Array(parameters(store, c).into_iter().map(|p| parameter_json(store, p)).collect())
}

/// `uml.template-parameter.list`: a classifier's template parameters, in their order.
fn parameter_list(ctx: &mut C<'_>, p: &Params) -> R {
    let c = crate::commands::classifier_of(ctx.doc, p, "classifier")?;
    Ok(json!({ "template": is_template(ctx.doc, c), "parameters": parameters_json(ctx.doc, c) }))
}

/// Removes template parameters `params` (refused before anything changes while a binding
/// substitutes one), each signature left with none going with its last: what `element.remove`
/// does with them.
pub(crate) fn remove(store: &mut Store, params: &[Obj]) -> Result<usize, CommandError> {
    for p in params {
        if let Some(why) = removal_problem(store, *p) {
            return Err(refuse("ids", why));
        }
    }
    let mut gone = 0;
    for p in params {
        if !store.is_alive(*p) {
            continue;
        }
        let template = template_of(store, *p);
        gone += store.destroy(*p).map_err(fail)?;
        if let Some(t) = template
            && parameters(store, t).is_empty()
            && let Some(sig) = signature(store, t)
        {
            gone += store.destroy(sig).map_err(fail)?;
        }
    }
    Ok(gone)
}

fn has_templates(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).is_ok_and(can_be_template)) { Ok(()) } else { Err(NO_TEMPLATES) }
}

fn has_parameters(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(CLASSIFIER_TEMPLATE_PARAMETER)) {
        Ok(())
    } else {
        Err("There is no template parameter yet.")
    }
}

/// The parameter lines the add and set commands share.
macro_rules! kind_line {
    () => {
        "class|interface|data-type|enumeration|primitive-type|value|operation; what an argument is: a class, an interface, a data type, an enumeration or a primitive type; value and operation are UML's other kinds, refused as not supported yet, never turned into another kind"
    };
}

/// The template commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.template-parameter.add",
            "Add template parameter",
            &["Class", "Interface", "Data type"],
            None,
            concat!(
                "classifier: string; the class's, interface's or data type's id; it becomes a template with its first parameter\n",
                "name: string; the parameter's name, free among the template's parameters whatever the case; a template has at most 32\n",
                "kind: string = \"class\", ",
                kind_line!(),
                "\nconstraint: string = \"\"; a classifier's id, or a built-in type's name, of the parameter's kind: every argument is it or specializes it; nothing for none\n",
                "default: string = \"\"; a classifier's id, or a built-in type's name, of the parameter's kind and fitting its constraint: the argument a binding that leaves the parameter out uses; nothing for none\n",
                "at: integer = -1, -1..1000000; where among the template's parameters, from 0 to their count, -1 for the end"
            ),
            has_templates,
            parameter_add
        ),
        cmd!(
            "uml.template-parameter.set",
            "Set template parameter",
            &["Class", "Interface", "Data type"],
            None,
            concat!(
                "parameter: string; the template parameter's id (its classifier's id names it too); every other field left out keeps what it has\n",
                "name: string?; its name, free among the template's other parameters whatever the case; renaming keeps every binding and member typed by it; left out, kept\n",
                "kind: string?, ",
                kind_line!(),
                "; a binding's argument that would not fit the new kind refuses it; left out, kept\n",
                "constraint: string?; a classifier's id, or a built-in type's name, of the parameter's kind; \"\" for none; a binding's argument that would not fit refuses it; left out, kept\n",
                "default: string?; a classifier's id, or a built-in type's name, of the parameter's kind and fitting its constraint; \"\" for none; left out, kept"
            ),
            has_parameters,
            parameter_set
        ),
        cmd!(
            "uml.template-parameter.move",
            "Move template parameter",
            &["Class", "Interface", "Data type"],
            None,
            "parameter: string; the template parameter's id\nat: integer = -1, -1..1000000; its new place among its template's parameters, from 0 to their count less one, -1 for the last; bindings follow the parameter, not its place",
            has_parameters,
            parameter_move
        ),
        cmd!(
            query "uml.template-parameter.list",
            "List template parameters",
            &["Class", "Interface", "Data type"],
            None,
            "classifier: string; the classifier's id",
            always,
            parameter_list
        ),
    ]
}
