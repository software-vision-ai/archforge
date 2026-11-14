//! Stereotype applications and their values (AR-135, design page 08, F3 and F4; owner decisions
//! 2026-11-06). An application is UML's own: an InstanceSpecification classified by the stereotype,
//! with a Slot for each value set, and a core Link ([`crate::profiles::APPLICATION_LINK`]) from it to
//! the element it applies to. The instance sits at the project's top level, beside the hidden UML
//! library, as UML keeps stereotype instances apart from the model's packages: moving or handing up
//! what it applies to never moves it, and it goes only with its element
//! ([`crate::profiles::applications_on`]) or when it is unapplied. An application and its values
//! are model data, never a note or a tag.
//!
//! A stereotype applies to an element when it extends the element's metaclass, or one the
//! metaclass specializes (an association class is a Class and an Association), and when a package
//! around the element, the element itself when it is a package, applies the stereotype's profile
//! ([`offer`]). An element takes each stereotype once. UML's own metaclasses, a profile's own
//! elements and a template parameter's classifier take none.
//!
//! A value is typed as text and read by its property's type, as a default is
//! ([`crate::modifiers::parse_default`]): a whole number, a number, true or false, a whole number or
//! `*`, any text, one of an enumeration's literals (kept as an InstanceValue that refers to it), or
//! an expression's text for a data type or a primitive type of the model's own. As many values as
//! the property's multiplicity allows, each once while it is unique. A property left unset shows
//! its default; a value set to what the default says is still set: the two are told apart by the
//! Slot, which is there only for a value set explicitly. Clearing takes the Slot away.
//!
//! What the values depend on is protected where it could lose them: a property whose values are
//! set is not removed (AR-134), nor retyped, nor given a multiplicity a set value breaks
//! ([`reshape_problem`]); a literal a value names is not removed ([`literal_problem`]).
//!
//! Whether a class diagram shows the values of its cards' stereotypes is the diagram's own choice
//! (`Show stereotype values on this diagram`), kept as Archforge's marks are kept, by a core tag, as
//! the root package's is: the one tag named [`VALUES_TAG`] lists the diagrams that show their
//! values; turning the last one off takes the tag away. A diagram removed leaves the list as the
//! core leaves any tag; the tag, left on nothing, is the one the next diagram turned on reuses.

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, Reason, Unextended, always, cmd};
use archforge_mof::{Obj, Store, TypeRef, Uuid, Value};
use serde_json::{Value as Json, json};

use crate::commands::{fail, name_of, refuse, short, text, uuid_of};
use crate::modifiers::{DefaultError, DefaultForm, DefaultValue};
use crate::profiles::{self, METACLASSES};
use crate::{INSTANCE_SPECIFICATION, INSTANCE_VALUE, SLOT, instance_specification, instance_value, slot, typed_element};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;
/// Why something may not be: the English sentence, and the reason typed where it has one.
type Why = (String, Option<Reason>);

/// The most stereotypes one element carries.
pub const MAX_APPLIED: usize = 32;
/// The most values one property holds on one application.
pub const MAX_VALUES: usize = 256;
/// The name of the core tag that marks a class diagram showing its cards' stereotype values.
pub const VALUES_TAG: &str = "archforge.stereotypeValues";

/// Why a command is not available.
pub const NO_APPLICATIONS: &str = "No stereotype is applied yet.";

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn refs(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

// Applications.

/// The stereotype that classifies `instance`, when it is a stereotype application.
pub fn stereotype_of(store: &Store, instance: Obj) -> Option<Obj> {
    if store.class_of(instance).ok() != Some(INSTANCE_SPECIFICATION) {
        return None;
    }
    refs(store, instance, instance_specification::CLASSIFIER).into_iter().find(|c| profiles::is_stereotype(store, *c))
}

/// Whether `o` is a stereotype application.
pub fn is_application(store: &Store, o: Obj) -> bool {
    stereotype_of(store, o).is_some()
}

/// Every stereotype of the model, in the order of the profiles and their stereotypes: the order an
/// element's stereotypes are listed in, so it is the same however they were applied.
pub fn all_stereotypes(store: &Store) -> Vec<Obj> {
    profiles::all_profiles(store).into_iter().flat_map(|p| profiles::stereotypes(store, p)).collect()
}

/// The stereotypes applied to `element`, each with its application, in the order of the profiles
/// and their stereotypes.
pub fn applied_to(store: &Store, element: Obj) -> Vec<(Obj, Obj)> {
    let mut out: Vec<(Obj, Obj)> = store
        .referrers(element)
        .into_iter()
        .filter(|(_, prop)| *prop == archforge_core::link::TARGET)
        .filter_map(|(l, _)| {
            let instance = one(store, l, archforge_core::link::SOURCE)?;
            Some((stereotype_of(store, instance)?, instance))
        })
        .collect();
    if out.len() > 1 {
        let order = all_stereotypes(store);
        out.sort_by_key(|(s, i)| (order.iter().position(|x| x == s).unwrap_or(usize::MAX), *i));
    }
    out
}

/// The application of `stereotype` to `element`, when it is applied.
pub fn application(store: &Store, element: Obj, stereotype: Obj) -> Option<Obj> {
    applied_to(store, element).into_iter().find(|(s, _)| *s == stereotype).map(|(_, i)| i)
}

/// The stereotypes on `element` as it shows them, `«Entity»` or `«Entity, Auditable»`; none when it
/// carries none. A stereotype's name is the model's, never translated.
pub fn keywords(store: &Store, element: Obj) -> Option<String> {
    let names: Vec<String> = applied_to(store, element).into_iter().map(|(s, _)| name_of(store, s)).collect();
    (!names.is_empty()).then(|| format!("«{}»", names.join(", ")))
}

/// What a member's row or a line writes before its text: `«Id» `, or nothing.
pub fn prefix(store: &Store, element: Obj) -> String {
    keywords(store, element).map(|k| format!("{k} ")).unwrap_or_default()
}

// What may be applied.

/// Why `element` takes no stereotype at all, when it takes none: UML's own metaclasses, a profile's
/// own elements, a stereotype application, a template parameter's classifier, and any element whose
/// metaclass no stereotype may extend.
pub fn element_problem(store: &Store, element: Obj) -> Option<String> {
    element_why(store, element).map(|(said, _)| said)
}

/// A refusal under `key` that says `why`, with its reason typed when it has one.
fn refusal(key: &str, (said, reason): Why) -> CommandError {
    let refused = refuse(key, said);
    match reason {
        Some(reason) => refused.because(reason),
        None => refused,
    }
}

/// [`element_problem`] with its reason typed for an interface to say; a template parameter's
/// classifier, an element gone and one without a metaclass have only their sentence.
fn element_why(store: &Store, element: Obj) -> Option<Why> {
    if !store.is_alive(element) {
        return Some(("it is gone".into(), None));
    }
    if let Some(refused) = profiles::library_refusal(store, "element", element) {
        return Some((refused.message, refused.reason));
    }
    if let Some(why) = crate::templates::parameter_element_problem(store, element) {
        return Some((why, None));
    }
    if is_application(store, element) {
        let id = uuid_of(store, element).unwrap_or_default();
        return Some((format!("{id} is a stereotype application; stereotypes apply to the model's elements"), Some(Reason::IsApplication)));
    }
    // A profile, a stereotype, an extension and what they hold define stereotypes: none is applied to them.
    let mut at = Some(element);
    for _ in 0..256 {
        let Some(o) = at else { break };
        if store.class_of(o).is_ok_and(|c| c == crate::PROFILE || c == crate::STEREOTYPE || c == crate::EXTENSION) {
            let name = name_or_id(store, element);
            return Some((format!("{name} belongs to a profile, which defines stereotypes and takes none"), Some(Reason::InProfile { name })));
        }
        at = store.container(o).ok().flatten().map(|(c, _)| c);
    }
    if !in_model(store, element) {
        return Some((format!("{} is no element of the model", uuid_of(store, element).unwrap_or_default()), Some(Reason::NotInModel)));
    }
    let Ok(class) = store.class_of(element) else { return Some(("it has no metaclass".into(), None)) };
    if METACLASSES.iter().any(|(_, kind)| store.universe().conforms(class, TypeRef::Class(*kind))) {
        return None;
    }
    let offered: Vec<&str> = METACLASSES.iter().map(|(n, _)| *n).collect();
    let name = name_or_id(store, element);
    Some((format!("{name} takes no stereotype: stereotypes extend {}", offered.join(", ")), Some(Reason::TakesNoStereotype { name })))
}

/// Whether `o` lies inside the project's Model.
fn in_model(store: &Store, o: Obj) -> bool {
    crate::commands::model(store).is_ok_and(|m| store.contains(m, o))
}

/// An element's name, or its id when it has none (a generalization).
fn name_or_id(store: &Store, o: Obj) -> String {
    let n = name_of(store, o);
    if n.is_empty() { uuid_of(store, o).unwrap_or_default() } else { n }
}

/// What applying a stereotype to an element would meet: nothing, or why it may not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// It may be applied.
    Open,
    /// It is applied already.
    Applied,
    /// It extends other metaclasses than the element's: their names, none when it extends nothing.
    Extends(Vec<String>),
    /// No package around the element applies its profile.
    OutOfScope,
}

/// Whether `stereotype` may be applied to `element` (F4): applied already, then extending the
/// element's metaclass, then its profile applied to a package around it. The element itself is
/// asked first ([`element_problem`]).
pub fn offer(store: &Store, stereotype: Obj, element: Obj) -> Offer {
    if application(store, element, stereotype).is_some() {
        return Offer::Applied;
    }
    let exts = profiles::extensions(store, stereotype);
    if !exts.iter().any(|e| profiles::extends_kind(store, e.metaclass, element)) {
        return Offer::Extends(exts.into_iter().map(|e| name_of(store, e.metaclass)).collect());
    }
    match profiles::profile_of(store, stereotype) {
        Some(p) if profiles::in_scope(store, p, element) => Offer::Open,
        _ => Offer::OutOfScope,
    }
}

/// Every stereotype of the model with what applying it to `element` would meet, in the order of the
/// profiles and their stereotypes: what the picker lists (F4).
pub fn choices(store: &Store, element: Obj) -> Vec<(Obj, Offer)> {
    all_stereotypes(store).into_iter().map(|s| (s, offer(store, s, element))).collect()
}

/// Why `stereotype` may not be applied to `element`, as a refusal says it; none when it may.
pub fn apply_problem(store: &Store, stereotype: Obj, element: Obj) -> Option<String> {
    apply_why(store, stereotype, element).map(|(said, _)| said)
}

/// [`apply_problem`] with its reason typed for an interface to say.
fn apply_why(store: &Store, stereotype: Obj, element: Obj) -> Option<Why> {
    if let Some(why) = element_why(store, element) {
        return Some(why);
    }
    let (s, e) = (name_of(store, stereotype), name_or_id(store, element));
    let unextended = |extends: &[String]| {
        let metaclass = profiles::metaclass_name(store, element);
        Reason::Unextended(Box::new(Unextended { stereotype: s.clone(), metaclass, extends: extends.to_vec() }))
    };
    let (said, reason) = match offer(store, stereotype, element) {
        Offer::Open if applied_to(store, element).len() >= MAX_APPLIED => (
            format!("{e} carries {MAX_APPLIED} stereotypes, the most one element carries"),
            Reason::TooMany { kind: "applied stereotype", max: MAX_APPLIED },
        ),
        Offer::Open => return None,
        Offer::Applied => {
            (format!("«{s}» is applied to {e} already"), Reason::StereotypeApplied { stereotype: s.as_str().into(), element: e.as_str().into() })
        }
        Offer::Extends(m) if m.is_empty() => (format!("«{s}» extends no metaclass yet; add one to what it extends first"), unextended(&m)),
        Offer::Extends(m) => (format!("«{s}» extends {}, and {e} is {}", m.join(", "), crate::commands::a_what(store, element)), unextended(&m)),
        Offer::OutOfScope => {
            let profile = profiles::profile_of(store, stereotype).map(|p| name_of(store, p)).unwrap_or_default();
            let home = profiles::home_of(store, element).map(|p| name_of(store, p)).unwrap_or_default();
            let said =
                format!("«{s}» is of the profile {profile}, which no package around {e} applies; apply it to {home} or a package around it first");
            (said, Reason::ProfileOutOfScope { profile: profile.as_str().into(), package: home.as_str().into() })
        }
    };
    Some((said, Some(reason)))
}

/// Applies `stereotype` to `element`: a new InstanceSpecification classified by it at the
/// project's top level, and the core Link from it to the element. Checked first.
pub fn apply(store: &mut Store, stereotype: Obj, element: Obj) -> Result<Obj, CommandError> {
    if let Some(why) = apply_why(store, stereotype, element) {
        return Err(refusal("stereotype", why));
    }
    let model = crate::commands::model(store)?;
    let instance = store.create(INSTANCE_SPECIFICATION).map_err(fail)?;
    store.push(instance, instance_specification::CLASSIFIER, Value::Ref(stereotype)).map_err(fail)?;
    store.push(model, archforge_core::model::MEMBER, Value::Ref(instance)).map_err(fail)?;
    archforge_core::link(store, model, profiles::APPLICATION_LINK, instance, element, None).map_err(fail)?;
    Ok(instance)
}

// Values.

/// The Slot of `instance` for `property`, when a value of it is set.
pub fn slot_of(store: &Store, instance: Obj, property: Obj) -> Option<Obj> {
    refs(store, instance, instance_specification::SLOT).into_iter().find(|s| one(store, *s, slot::DEFINING_FEATURE) == Some(property))
}

/// The values of `property` set on `instance`, in their order; empty when none is set.
pub fn values(store: &Store, instance: Obj, property: Obj) -> Vec<Obj> {
    slot_of(store, instance, property).map(|s| refs(store, s, slot::VALUE)).unwrap_or_default()
}

/// Whether a value of `property` is set on `instance`, explicitly: a default is never set.
pub fn is_set(store: &Store, instance: Obj, property: Obj) -> bool {
    !values(store, instance, property).is_empty()
}

/// The values of `property` set on `instance` as they are typed.
pub fn value_texts(store: &Store, instance: Obj, property: Obj) -> Vec<String> {
    values(store, instance, property).into_iter().map(|v| crate::modifiers::spec_text(store, v)).collect()
}

/// The values of `property` set on `instance` as a card writes them, a String's in quotes, several
/// separated by commas; none when none is set.
pub fn shown(store: &Store, instance: Obj, property: Obj) -> Option<String> {
    let all: Vec<String> = values(store, instance, property).into_iter().map(|v| crate::modifiers::spec_shown(store, v)).collect();
    (!all.is_empty()).then(|| all.join(", "))
}

/// Why typed values do not fit their property.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueProblem {
    /// More than [`MAX_VALUES`].
    TooMany,
    /// A value of nothing but spaces.
    Empty,
    /// One that does not read in its type.
    Malformed(DefaultError),
    /// Fewer or more than the multiplicity allows: the count given, and the bounds.
    Count(usize, u64, Option<u64>),
    /// The same value twice in a unique property: the value as typed.
    Twice(String),
}

impl ValueProblem {
    /// Why, as a refusal says it.
    pub fn said(&self, store: &Store, property: Obj) -> String {
        let p = name_of(store, property);
        match self {
            ValueProblem::TooMany => format!("{p} holds at most {MAX_VALUES} values"),
            ValueProblem::Empty => "a value is empty; clear the property to show its default".into(),
            ValueProblem::Malformed(e) => match e {
                DefaultError::TooLong => format!("a value is longer than {} characters", crate::modifiers::MAX_DEFAULT),
                DefaultError::Malformed(v, form) => format!("{v:?} is no value of {p}: {}", accepted(store, *form)),
            },
            ValueProblem::Count(n, lower, upper) => {
                let values = if *n == 1 { "value" } else { "values" };
                format!("the multiplicity of {p}, [{}], does not allow {n} {values}", bounds_text(*lower, *upper))
            }
            ValueProblem::Twice(v) => format!("{p} is unique and holds {v:?} twice"),
        }
    }
}

/// What a value of `form` is, as a refusal says it.
pub fn accepted(store: &Store, form: DefaultForm) -> String {
    match form {
        DefaultForm::Integer => "an Integer is a whole number, such as 42 or -7".into(),
        DefaultForm::Real => "a Real is a number, such as 2.5 or -1e3".into(),
        DefaultForm::Boolean => "a Boolean is true or false".into(),
        DefaultForm::UnlimitedNatural => "an UnlimitedNatural is a whole number of 0 or more, or *".into(),
        DefaultForm::String | DefaultForm::Opaque => "any text".into(),
        DefaultForm::Enumeration(e) => {
            let names: Vec<String> = crate::commands::literals(store, e).into_iter().map(|l| name_of(store, l)).collect();
            if names.is_empty() {
                return format!("the enumeration {} has no literals yet", name_of(store, e));
            }
            let shown: Vec<String> = names.iter().take(12).cloned().collect();
            let more = names.len().saturating_sub(shown.len());
            let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
            format!("a value of {} is one of its literals: {}{tail}", name_of(store, e), shown.join(", "))
        }
    }
}

/// Bounds as a multiplicity writes them: `1`, `0..1`, `*`, `1..*`, `2..5`.
pub fn bounds_text(lower: u64, upper: Option<u64>) -> String {
    match upper {
        None if lower == 0 => "*".into(),
        None => format!("{lower}..*"),
        Some(u) if u == lower => u.to_string(),
        Some(u) => format!("{lower}..{u}"),
    }
}

/// How values of `property` are read: by its type, as its default is.
pub fn form_of(store: &Store, property: Obj) -> DefaultForm {
    DefaultForm::of(store, one(store, property, typed_element::TYPE))
}

/// The values typed in `texts`, read by `property`'s type, as many as its multiplicity allows and
/// each once while it is unique. An empty list clears the property, so its default shows.
pub fn read_values(store: &Store, property: Obj, texts: &[String]) -> Result<Vec<DefaultValue>, ValueProblem> {
    if texts.len() > MAX_VALUES {
        return Err(ValueProblem::TooMany);
    }
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let form = form_of(store, property);
    let mut out: Vec<DefaultValue> = Vec::with_capacity(texts.len());
    for t in texts {
        let v = crate::modifiers::parse_default(store, form, t).map_err(ValueProblem::Malformed)?.ok_or(ValueProblem::Empty)?;
        let (_, _, _, unique) = crate::modifiers::flags(store, property);
        if unique && out.contains(&v) {
            return Err(ValueProblem::Twice(short(t.trim())));
        }
        out.push(v);
    }
    let (lower, upper) = crate::modifiers::bounds(store, property);
    let n = u64::try_from(out.len()).unwrap_or(u64::MAX);
    if n < lower || upper.is_some_and(|u| n > u) {
        return Err(ValueProblem::Count(out.len(), lower, upper));
    }
    Ok(out)
}

/// Sets the values of `property` on `instance` to `values`, replacing those set; none clears it.
pub fn set_values(store: &mut Store, instance: Obj, property: Obj, values: &[DefaultValue]) -> Result<(), CommandError> {
    if let Some(s) = slot_of(store, instance, property) {
        store.destroy(s).map_err(fail)?;
    }
    if values.is_empty() {
        return Ok(());
    }
    let s = store.create(SLOT).map_err(fail)?;
    store.set(s, slot::DEFINING_FEATURE, Value::Ref(property)).map_err(fail)?;
    for v in values {
        let spec = crate::modifiers::make_spec(store, v)?;
        store.push(s, slot::VALUE, Value::Ref(spec)).map_err(fail)?;
    }
    store.push(instance, instance_specification::SLOT, Value::Ref(s)).map(|_| ()).map_err(fail)
}

/// The rows a card's values compartment lists for `element`: per stereotype with a value set, its
/// `«Entity»` and a row `table = "orders"` per property set, in the properties' order. A property
/// left unset shows nothing there: its default is the stereotype's, not the element's.
pub fn value_rows(store: &Store, element: Obj) -> Vec<(String, Vec<String>)> {
    applied_to(store, element)
        .into_iter()
        .filter_map(|(s, i)| {
            let rows: Vec<String> =
                profiles::properties(store, s).into_iter().filter_map(|p| Some(format!("{} = {}", name_of(store, p), shown(store, i, p)?))).collect();
            (!rows.is_empty()).then(|| (format!("«{}»", name_of(store, s)), rows))
        })
        .collect()
}

// What the values depend on.

/// Why `property`, a stereotype's, may not take the type `ty` and the bounds `bounds` while values
/// of it are set: a new type would read them otherwise, and bounds that a set value breaks would
/// lose it. None for any other property, or when nothing is set.
pub fn reshape_problem(store: &Store, property: Obj, ty: Option<Obj>, bounds: (u64, Option<u64>), unique: bool) -> Option<String> {
    let s = store.container(property).ok().flatten().map(|(c, _)| c).filter(|c| profiles::is_stereotype(store, *c))?;
    let applications: Vec<Obj> =
        profiles::applications_of(store, s).into_iter().map(|a| a.instance).filter(|i| is_set(store, *i, property)).collect();
    if applications.is_empty() {
        return None;
    }
    let p = name_of(store, property);
    let on = |n: usize| if n == 1 { "1 element".to_string() } else { format!("{n} elements") };
    if ty != one(store, property, typed_element::TYPE) {
        return Some(format!("the values of {p} are set on {}; clear them before giving it another type", on(applications.len())));
    }
    let (lower, upper) = bounds;
    let broken = applications
        .iter()
        .filter(|i| {
            let vs: Vec<Obj> = values(store, **i, property);
            let n = u64::try_from(vs.len()).unwrap_or(u64::MAX);
            let twice = unique
                && vs
                    .iter()
                    .enumerate()
                    .any(|(k, a)| vs.iter().skip(k + 1).any(|b| crate::modifiers::spec_text(store, *a) == crate::modifiers::spec_text(store, *b)));
            n < lower || upper.is_some_and(|u| n > u) || twice
        })
        .count();
    (broken > 0).then(|| format!("the values set on {} do not fit {p} [{}]; change those values first", on(broken), bounds_text(lower, upper)))
}

/// Why a removal would lose a value, `goes` telling what the removal takes away: a literal a
/// value names, on an application whose element stays. None when none would.
pub fn literal_problem(store: &Store, goes: impl Fn(Obj) -> bool) -> Option<String> {
    for v in store.alive().filter(|o| store.class_of(*o).ok() == Some(INSTANCE_VALUE)) {
        let Some(l) = one(store, v, instance_value::INSTANCE).filter(|l| store.class_of(*l).ok() == Some(crate::ENUMERATION_LITERAL)) else {
            continue;
        };
        if !goes(l) {
            continue;
        }
        let Some((s, _)) = store.container(v).ok().flatten() else { continue };
        let Some((instance, _)) = store.container(s).ok().flatten() else { continue };
        let Some(stereotype) = stereotype_of(store, instance) else { continue };
        let element = profiles::element_of(store, instance);
        if element.is_some_and(&goes) {
            continue;
        }
        let property = one(store, s, slot::DEFINING_FEATURE).map(|p| name_of(store, p)).unwrap_or_default();
        let on = element.map(|e| name_or_id(store, e)).unwrap_or_default();
        return Some(format!(
            "the literal {} is the value of {property} in «{}» on {on}; set another value there first",
            name_of(store, l),
            name_of(store, stereotype)
        ));
    }
    None
}

// The diagram's choice.

/// The tag that lists the diagrams showing their stereotype values, when there is one.
fn values_tag(store: &Store) -> Option<Obj> {
    let model = crate::commands::model(store).ok()?;
    refs(store, model, archforge_core::model::TAG)
        .into_iter()
        .find(|t| store.get(*t, archforge_core::tag::NAME).ok().and_then(|v| v.as_str().map(str::to_string)).as_deref() == Some(VALUES_TAG))
}

/// Whether `diagram` shows its cards' stereotype values.
pub fn values_shown(store: &Store, diagram: Obj) -> bool {
    values_tag(store).is_some_and(|t| refs(store, t, archforge_core::tag::ELEMENT).contains(&diagram))
}

/// Turns showing the stereotype values on `diagram` on or off.
fn show_values(store: &mut Store, diagram: Obj, on: bool) -> Result<(), CommandError> {
    if values_shown(store, diagram) == on {
        return Ok(());
    }
    let tag = values_tag(store);
    match (tag, on) {
        (Some(t), true) => store.push(t, archforge_core::tag::ELEMENT, Value::Ref(diagram)).map(|_| ()).map_err(fail),
        (None, true) => {
            let model = crate::commands::model(store)?;
            let t = store.create(archforge_core::TAG).map_err(fail)?;
            store.set(t, archforge_core::tag::NAME, Value::str(VALUES_TAG)).map_err(fail)?;
            store.push(t, archforge_core::tag::ELEMENT, Value::Ref(diagram)).map_err(fail)?;
            store.push(model, archforge_core::model::TAG, Value::Ref(t)).map(|_| ()).map_err(fail)
        }
        (Some(t), false) => {
            store.remove_value(t, archforge_core::tag::ELEMENT, &Value::Ref(diagram)).map_err(fail)?;
            if refs(store, t, archforge_core::tag::ELEMENT).is_empty() {
                store.destroy(t).map_err(fail)?;
            }
            Ok(())
        }
        (None, false) => Ok(()),
    }
}

// Commands.

/// The model's element the id under `key` names.
fn element_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let id = text(p, key);
    let Some(uuid) = Uuid::parse(id) else { return Err(refuse(key, format!("{:?} is not an id", short(id)))) };
    store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))
}

/// The stereotype the id under `key` names.
fn stereotype_param(store: &Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    let s = element_param(store, p, key)?;
    if !profiles::is_stereotype(store, s) {
        return Err(refuse(key, format!("{} is not a stereotype", text(p, key))));
    }
    Ok(s)
}

/// The property of `stereotype` under `key`: its id, or its name whatever the case.
fn property_param(store: &Store, p: &Params, key: &str, stereotype: Obj) -> Result<Obj, CommandError> {
    let wanted = text(p, key);
    let all = profiles::properties(store, stereotype);
    let by_id = Uuid::parse(wanted).and_then(|u| store.by_uuid(u)).filter(|o| all.contains(o));
    let by_name = || all.iter().copied().find(|a| name_of(store, *a).eq_ignore_ascii_case(wanted));
    by_id.or_else(by_name).ok_or_else(|| {
        let names: Vec<String> = all.iter().map(|a| name_of(store, *a)).collect();
        let offered = if names.is_empty() { "it has none".to_string() } else { format!("it has {}", names.join(", ")) };
        refuse(key, format!("«{}» has no property {:?}; {offered}", name_of(store, stereotype), short(wanted)))
    })
}

/// The application of the stereotype under `stereotype` to the element under `element`, refused
/// when it is not applied there.
fn application_param(store: &Store, p: &Params) -> Result<(Obj, Obj, Obj), CommandError> {
    let element = element_param(store, p, "element")?;
    let stereotype = stereotype_param(store, p, "stereotype")?;
    let instance = application(store, element, stereotype)
        .ok_or_else(|| refuse("stereotype", format!("«{}» is not applied to {}", name_of(store, stereotype), name_or_id(store, element))))?;
    Ok((element, stereotype, instance))
}

/// `uml.stereotype.apply`: a stereotype applied to an element (F3, F4).
fn apply_cmd(ctx: &mut C<'_>, p: &Params) -> R {
    let element = element_param(ctx.doc, p, "element")?;
    let stereotype = stereotype_param(ctx.doc, p, "stereotype")?;
    if let Some(why) = element_why(ctx.doc, element) {
        return Err(refusal("element", why));
    }
    let instance = apply(ctx.doc, stereotype, element)?;
    Ok(json!({ "id": uuid_of(ctx.doc, instance)?, "element": uuid_of(ctx.doc, element)? }))
}

/// `uml.stereotype.unapply`: a stereotype taken off an element with its values; the stereotype and
/// its definition stay.
fn unapply_cmd(ctx: &mut C<'_>, p: &Params) -> R {
    let (element, _, instance) = application_param(ctx.doc, p)?;
    archforge_core::destroy_with_links(ctx.doc, instance).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, element)? }))
}

/// The texts under `values`: a list of strings, or one string for one value.
fn texts_param(p: &Params) -> Result<Vec<String>, CommandError> {
    match p.get("values") {
        Some(Json::Array(items)) => {
            if items.len() > MAX_VALUES {
                return Err(refuse("values", format!("at most {MAX_VALUES} values")));
            }
            items.iter().map(|i| i.as_str().map(str::to_string).ok_or_else(|| refuse("values", "a list of values, each as text"))).collect()
        }
        _ => Err(refuse("values", "a list of values, each as text")),
    }
}

/// `uml.stereotype.set-value`: the values of one of a stereotype's properties on an element it is
/// applied to, read by the property's type; an empty list clears them, so the default shows. The
/// element's cards on the diagrams that show values grow to show its rows (AR-241).
fn set_value_cmd(ctx: &mut C<'_>, p: &Params) -> R {
    let (element, stereotype, instance) = application_param(ctx.doc, p)?;
    let property = property_param(ctx.doc, p, "property", stereotype)?;
    let texts = texts_param(p)?;
    let values = read_values(ctx.doc, property, &texts).map_err(|e| refuse("values", e.said(ctx.doc, property)))?;
    set_values(ctx.doc, instance, property, &values)?;
    for n in archforge_diagram::views_of(ctx.doc, element).2 {
        if archforge_diagram::diagram_of(ctx.doc, n).is_some_and(|d| values_shown(ctx.doc, d)) {
            crate::figures::grow_node(ctx.doc, element, n).map_err(fail)?;
        }
    }
    Ok(json!({ "id": uuid_of(ctx.doc, element)?, "values": value_texts(ctx.doc, instance, property) }))
}

/// A property of an application as the reads give it: its values set (empty when none is), whether
/// it is set, and its default.
fn property_json(store: &Store, instance: Obj, property: Obj) -> Json {
    let ty = one(store, property, typed_element::TYPE).map(|t| name_of(store, t));
    let (lower, upper) = crate::modifiers::bounds(store, property);
    let default = crate::modifiers::default_text(store, property);
    json!({
        "id": uuid_of(store, property).unwrap_or_default(), "name": name_of(store, property), "type": ty,
        "multiplicity": bounds_text(lower, upper), "set": is_set(store, instance, property),
        "values": value_texts(store, instance, property),
        "default": if default.is_empty() { Json::Null } else { json!(default) },
    })
}

/// `uml.stereotype.list-applied`: the stereotypes applied to an element, each with its profile and
/// its properties' values, set or defaulted; with a diagram, whether it shows the values.
fn list_applied(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let element = element_param(store, p, "element")?;
    let items: Vec<Json> = applied_to(store, element)
        .into_iter()
        .map(|(s, i)| {
            json!({
                "stereotype": uuid_of(store, s).unwrap_or_default(), "name": name_of(store, s), "id": uuid_of(store, i).unwrap_or_default(),
                "profile": profiles::profile_of(store, s).map(|x| name_of(store, x)),
                "properties": profiles::properties(store, s).into_iter().map(|a| property_json(store, i, a)).collect::<Vec<_>>(),
            })
        })
        .collect();
    let mut out = json!({ "element": uuid_of(store, element)?, "keywords": keywords(store, element), "applied": items });
    if !text(p, "diagram").is_empty() {
        let d = element_param(store, p, "diagram")?;
        out["values_shown"] = json!(values_shown(store, d));
    }
    Ok(out)
}

/// `uml.stereotype.applicable`: every stereotype of the model with whether it may be applied to an
/// element and why not, as the picker lists them (F4).
fn applicable_cmd(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let element = element_param(store, p, "element")?;
    if let Some(why) = element_problem(store, element) {
        return Ok(json!({ "element": uuid_of(store, element)?, "takes": false, "why": why, "stereotypes": [] }));
    }
    let items: Vec<Json> = choices(store, element)
        .into_iter()
        .map(|(s, offer)| {
            let (state, why) = match &offer {
                Offer::Open => ("open", None),
                Offer::Applied => ("applied", None),
                Offer::Extends(_) | Offer::OutOfScope => ("refused", apply_problem(store, s, element)),
            };
            json!({
                "id": uuid_of(store, s).unwrap_or_default(), "name": name_of(store, s),
                "profile": profiles::profile_of(store, s).map(|x| name_of(store, x)), "state": state, "why": why,
            })
        })
        .collect();
    Ok(json!({ "element": uuid_of(store, element)?, "takes": true, "stereotypes": items }))
}

/// `uml.stereotype.show-values`: whether a class diagram shows its cards' stereotype values, each in
/// a compartment of its own under the members. Turned on, every card of the diagram with values set
/// grows to show them (AR-241); a card without values is left as it is.
fn show_values_cmd(ctx: &mut C<'_>, p: &Params) -> R {
    let d = element_param(ctx.doc, p, "diagram")?;
    let kind = ctx.doc.get(d, archforge_diagram::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string));
    if ctx.doc.class_of(d).ok() != Some(archforge_diagram::DIAGRAM) || kind.as_deref() != Some(crate::CLASS_DIAGRAM) {
        return Err(refuse("diagram", format!("{} is not a class diagram", text(p, "diagram"))));
    }
    let on = p.bool("show").unwrap_or(false);
    show_values(ctx.doc, d, on)?;
    if on {
        let store: &Store = ctx.doc;
        let valued: Vec<(Obj, Obj)> = store
            .alive()
            .filter(|n| store.class_of(*n).ok() == Some(archforge_diagram::NODE) && archforge_diagram::diagram_of(store, *n) == Some(d))
            .filter_map(|n| Some((n, one(store, n, archforge_diagram::node::ELEMENT)?)))
            .filter(|(_, e)| !value_rows(store, *e).is_empty())
            .collect();
        for (n, e) in valued {
            crate::figures::grow_node(ctx.doc, e, n).map_err(fail)?;
        }
    }
    Ok(json!({ "id": uuid_of(ctx.doc, d)?, "show": values_shown(ctx.doc, d) }))
}

fn has_stereotypes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| profiles::is_stereotype(f.doc, o)) { Ok(()) } else { Err(profiles::NO_STEREOTYPES) }
}

fn has_applications(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| is_application(f.doc, o)) { Ok(()) } else { Err(NO_APPLICATIONS) }
}

/// The applications' commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.stereotype.apply",
            "Apply stereotype",
            &["Stereotype"],
            None,
            concat!(
                "element: string; the id of the element it applies to: a class, an interface, an enumeration, a data type, a primitive type, an association class, an attribute, an association end, an operation, an association, a generalization or a package, of the metaclass the stereotype extends or one specializing it\n",
                "stereotype: string; the stereotype's id; its profile is applied to the element's package or a package around it; each stereotype once on an element"
            ),
            has_stereotypes,
            apply_cmd
        ),
        cmd!(
            "uml.stereotype.unapply",
            "Unapply stereotype",
            &["Stereotype"],
            None,
            "element: string; the id of the element it is applied to\nstereotype: string; the stereotype's id; its values on the element go with it, and the stereotype stays",
            has_applications,
            unapply_cmd
        ),
        cmd!(
            "uml.stereotype.set-value",
            "Set stereotype value",
            &["Stereotype"],
            None,
            concat!(
                "element: string; the id of the element the stereotype is applied to\n",
                "stereotype: string; the stereotype's id\n",
                "property: string; the id of one of the stereotype's properties, or its name\n",
                "values: array; the values as text, read by the property's type: an Integer's a whole number, a Real's a number, a Boolean's true or false, an UnlimitedNatural's a whole number of 0 or more or *, a String's any text, an enumeration's one of its literals' names, any other type's an expression's text; as many as its multiplicity allows, each once while it is unique, at most 256; [] clears them, so the property shows its default"
            ),
            has_applications,
            set_value_cmd
        ),
        cmd!(
            query "uml.stereotype.list-applied",
            "List applied stereotypes",
            &["Stereotype"],
            None,
            "element: string; the element's id; its stereotypes, each with its profile and every property's values, set says whether they were set or the default shows\ndiagram: string = \"\"; a class diagram's id, to say whether it shows the values",
            always,
            list_applied
        ),
        cmd!(
            query "uml.stereotype.applicable",
            "List applicable stereotypes",
            &["Stereotype"],
            None,
            "element: string; the element's id; every stereotype of the model, open when it may be applied, applied when it is, refused with why: it extends another metaclass, or its profile is applied to no package around the element",
            has_stereotypes,
            applicable_cmd
        ),
        cmd!(
            "uml.stereotype.show-values",
            "Show stereotype values",
            &["Stereotype", "Diagram"],
            None,
            "diagram: string; a class diagram's id\nshow: bool; on, its cards list the values set on their stereotypes in a compartment per stereotype; off, they do not; every other diagram keeps its own choice",
            has_stereotypes,
            show_values_cmd
        ),
    ]
}
