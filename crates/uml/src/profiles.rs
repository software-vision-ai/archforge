//! Profiles and stereotypes (AR-134, design page 08, section F): UML's own Profile, Stereotype,
//! Extension, ExtensionEnd and ProfileApplication, all in the generated schema, so nothing about
//! storage is new.
//!
//! A profile sits in a package beside its packages and classifiers, never inside a classifier, and
//! holds stereotypes. A stereotype extends metaclasses: each extension is UML's own shape, an
//! Extension in the profile whose member ends are a `base_<Metaclass>` attribute of the stereotype,
//! typed by the metaclass, and an ExtensionEnd the Extension owns, typed by the stereotype. The
//! metaclasses a profile uses are those its stereotypes' extensions name ([`metaclasses_of`]): no
//! `Profile::metaclassReference` is stored, since UML makes it an optional subset of the
//! profile's composite `elementImport`, a part a project's rows would then list twice, which the
//! reader refuses; the extensions say the same, and nothing is kept twice. The metaclasses are Classes in the hidden UML library (owner decision
//! 2026-11-06): a second, unnamed UML Model at the project's top level, beside the root package,
//! known by UML's own address [`UML_URI`] rather than by a name, so no project name clashes with
//! it. A metaclass is made there the first time a stereotype extends it, and never twice; the
//! library and its metaclasses are never shown in Models, type menus, the search or
//! `uml.type.list`, and are never renamed, moved or removed. Only the metaclasses Archforge draws
//! are offered ([`METACLASSES`]); any other is refused, saying so.
//!
//! A stereotype owns properties as a class owns attributes (`uml.attribute.*`), typed by a
//! primitive type, an enumeration or a data type of the model; a name starting `base_` is kept for
//! its extensions. A package applies profiles (`Package::profileApplication`); the stereotypes of a
//! profile applied to a package are available to the elements inside it, at any depth.
//!
//! A stereotype application (AR-135; owner decision 2026-11-06) is an InstanceSpecification
//! classified by the stereotype, with a Slot per value, and a core Link from it to the element it
//! applies to ([`APPLICATION_LINK`]). [`applications_of`] is the one place that finds them, so
//! every refusal here works the moment applications exist: unapplying a profile, removing a
//! stereotype or a profile, dropping an extension, moving a stereotype to another profile, moving
//! or removing what a use sits in, and removing a property whose values are set are refused while
//! they would leave an application without its definition or its scope, the uses counted and
//! named ([`Uses`]); nothing is dropped silently. An application goes with the element it applies
//! to when that is removed ([`applications_on`]).

use std::collections::BTreeSet;

use archforge_diagram::ProjectView;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, InUse, Params, Reason, Used, always, cmd};
use archforge_mof::{ClassId, Obj, Store, Value};
use serde_json::{Value as Json, json};

use crate::commands::{check_name, fail, name_of, refuse, set_name, short, text, uuid_of};
use crate::{
    ASSOCIATION, CLASS, DATA_TYPE, ENUMERATION, EXTENSION, EXTENSION_END, GENERALIZATION, INSTANCE_SPECIFICATION, INTERFACE, OPERATION, PACKAGE,
    PROFILE, PROFILE_APPLICATION, PROPERTY, STEREOTYPE, class, extension, extension_end, instance_specification, package, profile_application,
    property, slot, typed_element,
};

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

/// UML's own address: the hidden library of metaclasses is the top-level UML Model whose URI it is.
pub const UML_URI: &str = "http://www.omg.org/spec/UML/20161101";

/// The metaclasses a stereotype may extend, by UML's names, with the metaclass each stands for in
/// the store: those Archforge draws (design page 08, F1), in the order the menus offer them.
pub const METACLASSES: [(&str, ClassId); 9] = [
    ("Class", CLASS),
    ("Interface", INTERFACE),
    ("Enumeration", ENUMERATION),
    ("DataType", DATA_TYPE),
    ("Property", PROPERTY),
    ("Operation", OPERATION),
    ("Association", ASSOCIATION),
    ("Generalization", GENERALIZATION),
    ("Package", PACKAGE),
];

/// UML metaclasses Archforge does not draw yet: listed in the menus, disabled, and refused by the
/// commands with that reason.
pub const NOT_DRAWN: [&str; 8] = ["Activity", "Actor", "Artifact", "Component", "Interaction", "Node", "StateMachine", "UseCase"];

/// The start of the names a stereotype keeps for its extensions' ends (`base_Class`).
pub const BASE: &str = "base_";

/// The kind of the core Link from a stereotype application to the element it applies to; an
/// application is known by its source, an InstanceSpecification classified by a stereotype, so
/// the kind says only that the element is traced to it.
pub const APPLICATION_LINK: archforge_core::LinkKind = archforge_core::LinkKind::Trace;

/// The most stereotypes one profile holds.
pub const MAX_STEREOTYPES: usize = 256;
/// The most uses a refusal names one by one; past it they are counted.
const MAX_NAMED: usize = 2;
/// The deepest a container chain is followed: a damaged file could hold a loop.
const MAX_DEPTH: usize = 256;

/// Why a command is not available.
pub const NO_PROFILES: &str = "There is no profile yet.";
pub const NO_STEREOTYPES: &str = "There is no stereotype yet.";

fn one(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Option<Obj> {
    store.get(o, prop).ok().and_then(|v| v.as_ref()).filter(|x| store.is_alive(*x))
}

fn refs(store: &Store, o: Obj, prop: archforge_mof::PropId) -> Vec<Obj> {
    store.list(o, prop).unwrap_or_default().iter().filter_map(Value::as_ref).filter(|x| store.is_alive(*x)).collect()
}

fn is(store: &Store, o: Obj, class: ClassId) -> bool {
    store.is_alive(o) && store.class_of(o).ok() == Some(class)
}

/// Whether `o` is a profile.
pub fn is_profile(store: &Store, o: Obj) -> bool {
    is(store, o, PROFILE)
}

/// Whether `o` is a stereotype.
pub fn is_stereotype(store: &Store, o: Obj) -> bool {
    is(store, o, STEREOTYPE)
}

// The hidden UML library.

/// The hidden UML library: the top-level UML Model whose URI is [`UML_URI`], if there is one yet.
pub fn library(store: &Store) -> Option<Obj> {
    let m = crate::commands::model(store).ok()?;
    refs(store, m, archforge_core::model::MEMBER).into_iter().find(|o| {
        is(store, *o, crate::MODEL) && store.get(*o, package::URI).ok().and_then(|v| v.as_str().map(str::to_string)).as_deref() == Some(UML_URI)
    })
}

/// Whether `o` is the library or lies inside it.
pub fn in_library(store: &Store, o: Obj) -> bool {
    library(store).is_some_and(|l| l == o || store.contains(l, o))
}

/// The library, made at the project's top level when there is none: unnamed, so it never clashes
/// with the root package's name, and known by its URI.
fn ensure_library(store: &mut Store) -> Result<Obj, CommandError> {
    if let Some(l) = library(store) {
        return Ok(l);
    }
    let m = crate::commands::model(store)?;
    let l = store.create(crate::MODEL).map_err(fail)?;
    store.set(l, package::URI, Value::str(UML_URI)).map_err(fail)?;
    store.push(m, archforge_core::model::MEMBER, Value::Ref(l)).map_err(fail)?;
    Ok(l)
}

/// Whether `o` is a metaclass: a class the library holds.
pub fn is_metaclass(store: &Store, o: Obj) -> bool {
    is(store, o, CLASS) && store.container(o).ok().flatten().is_some_and(|(l, _)| Some(l) == library(store))
}

/// The metaclass named `name` in the library, if it is made yet.
pub fn metaclass(store: &Store, name: &str) -> Option<Obj> {
    let l = library(store)?;
    refs(store, l, package::PACKAGED_ELEMENT).into_iter().find(|c| is(store, *c, CLASS) && name_of(store, *c) == name)
}

/// The metaclass of the store a metaclass's name stands for, when Archforge draws it.
pub fn metaclass_kind(name: &str) -> Option<ClassId> {
    METACLASSES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// The metaclass named `name`, made in the library the first time it is used, and never twice.
fn ensure_metaclass(store: &mut Store, name: &str) -> Result<Obj, CommandError> {
    if let Some(m) = metaclass(store, name) {
        return Ok(m);
    }
    let l = ensure_library(store)?;
    let c = store.create(CLASS).map_err(fail)?;
    set_name(store, c, name)?;
    store.push(l, package::PACKAGED_ELEMENT, Value::Ref(c)).map_err(fail)?;
    Ok(c)
}

/// Why a stereotype may not extend the metaclass `name`, when it may not: one Archforge does not
/// draw yet, or no metaclass at all; the refusal names the ones offered.
pub fn metaclass_problem(name: &str) -> Option<String> {
    if metaclass_kind(name).is_some() {
        return None;
    }
    let offered: Vec<&str> = METACLASSES.iter().map(|(n, _)| *n).collect();
    let what = if NOT_DRAWN.contains(&name) {
        format!("{name} is a UML metaclass Archforge does not draw yet")
    } else {
        format!("{:?} is not a metaclass Archforge draws", short(name))
    };
    Some(format!("{what}; a stereotype extends {}", offered.join(", ")))
}

/// Why `o` may not be changed, when it is the library or a metaclass: they are UML's own.
pub fn library_problem(store: &Store, o: Obj) -> Option<String> {
    if library(store) == Some(o) {
        return Some("it is the UML library, which holds the metaclasses stereotypes extend and is never changed".into());
    }
    is_metaclass(store, o).then(|| {
        format!(
            "{} is the UML metaclass {}, kept in the UML library, which is never changed",
            uuid_of(store, o).unwrap_or_default(),
            name_of(store, o)
        )
    })
}

// Profiles and stereotypes.

/// The profiles `package` holds, in its order.
pub fn profiles(store: &Store, package: Obj) -> Vec<Obj> {
    refs(store, package, package::PACKAGED_ELEMENT).into_iter().filter(|o| is_profile(store, *o)).collect()
}

/// Every profile `package` holds, in it or in a package inside it, at any depth.
pub fn profiles_within(store: &Store, package: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    let mut stack = vec![package];
    let mut seen = BTreeSet::new();
    while let Some(p) = stack.pop() {
        if !seen.insert(p) {
            continue;
        }
        out.extend(profiles(store, p));
        stack.extend(crate::commands::subpackages(store, p));
    }
    out
}

/// Every profile of the model, by the packages' order.
pub fn all_profiles(store: &Store) -> Vec<Obj> {
    crate::commands::all_packages(store).into_iter().flat_map(|p| profiles(store, p)).collect()
}

/// The package a profile sits in.
pub fn package_of(store: &Store, profile: Obj) -> Option<Obj> {
    store.container(profile).ok().flatten().map(|(p, _)| p).filter(|p| is(store, *p, PACKAGE))
}

/// The stereotypes `profile` holds, in its order.
pub fn stereotypes(store: &Store, profile: Obj) -> Vec<Obj> {
    refs(store, profile, package::PACKAGED_ELEMENT).into_iter().filter(|o| is_stereotype(store, *o)).collect()
}

/// The profile a stereotype sits in.
pub fn profile_of(store: &Store, stereotype: Obj) -> Option<Obj> {
    store.container(stereotype).ok().flatten().map(|(p, _)| p).filter(|p| is_profile(store, *p))
}

/// One extension of a stereotype: the Extension, the stereotype's `base_` attribute, the
/// ExtensionEnd, and the metaclass extended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extension {
    pub extension: Obj,
    pub base: Obj,
    pub end: Option<Obj>,
    pub metaclass: Obj,
}

/// Whether `p` is a stereotype's `base_` attribute: a property whose association is an Extension.
pub fn is_base(store: &Store, p: Obj) -> bool {
    is(store, p, PROPERTY) && one(store, p, property::ASSOCIATION).is_some_and(|a| is(store, a, EXTENSION))
}

/// A stereotype's extensions, in the order of its attributes.
pub fn extensions(store: &Store, stereotype: Obj) -> Vec<Extension> {
    refs(store, stereotype, class::OWNED_ATTRIBUTE)
        .into_iter()
        .filter(|p| is_base(store, *p))
        .filter_map(|base| {
            let ext = one(store, base, property::ASSOCIATION)?;
            let metaclass = one(store, base, typed_element::TYPE)?;
            Some(Extension { extension: ext, base, end: one(store, ext, extension::OWNED_END), metaclass })
        })
        .collect()
}

/// The names of the metaclasses a stereotype extends, in its order.
pub fn extended(store: &Store, stereotype: Obj) -> Vec<String> {
    extensions(store, stereotype).into_iter().map(|e| name_of(store, e.metaclass)).collect()
}

/// A stereotype's own properties, its `base_` attributes left out.
pub fn properties(store: &Store, stereotype: Obj) -> Vec<Obj> {
    crate::members::attributes(store, stereotype)
}

/// The stereotype and the metaclass an Extension joins, when `ext` is one.
pub fn extension_ends(store: &Store, ext: Obj) -> Option<(Obj, Obj)> {
    if !is(store, ext, EXTENSION) {
        return None;
    }
    let end = one(store, ext, extension::OWNED_END)?;
    let stereotype = one(store, end, extension_end::TYPE)?;
    let base = refs(store, stereotype, class::OWNED_ATTRIBUTE).into_iter().find(|p| one(store, *p, property::ASSOCIATION) == Some(ext))?;
    Some((stereotype, one(store, base, typed_element::TYPE)?))
}

/// The metaclasses a profile's stereotypes extend, each once, in the order of its stereotypes and
/// their extensions.
pub fn metaclasses_of(store: &Store, profile: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    for m in stereotypes(store, profile).into_iter().flat_map(|s| extensions(store, s)).map(|e| e.metaclass) {
        if !out.contains(&m) {
            out.push(m);
        }
    }
    out
}

/// Adds the extension of `stereotype` to the metaclass `name`, the Extension in its profile.
fn extend(store: &mut Store, stereotype: Obj, name: &str) -> Result<(), CommandError> {
    let profile = profile_of(store, stereotype).ok_or_else(|| refuse("stereotype", "it is in no profile"))?;
    let metaclass = ensure_metaclass(store, name)?;
    let ext = store.create(EXTENSION).map_err(fail)?;
    store.push(profile, package::PACKAGED_ELEMENT, Value::Ref(ext)).map_err(fail)?;
    // The stereotype's end, base_Class: one element of the metaclass, as UML has it.
    let base = store.create(PROPERTY).map_err(fail)?;
    set_name(store, base, &format!("{BASE}{name}"))?;
    store.set(base, typed_element::TYPE, Value::Ref(metaclass)).map_err(fail)?;
    store.push(stereotype, class::OWNED_ATTRIBUTE, Value::Ref(base)).map_err(fail)?;
    store.set(base, property::ASSOCIATION, Value::Ref(ext)).map_err(fail)?;
    // The extension's own end, typed by the stereotype: composite, 0..1, not required.
    let end = store.create(EXTENSION_END).map_err(fail)?;
    set_name(store, end, &format!("extension_{}", name_of(store, stereotype)))?;
    store.set(end, extension_end::TYPE, Value::Ref(stereotype)).map_err(fail)?;
    store.set(end, property::AGGREGATION, Value::Enum(2)).map_err(fail)?;
    store.set(ext, extension::OWNED_END, Value::Ref(end)).map_err(fail)?;
    if one(store, end, property::ASSOCIATION) != Some(ext) {
        store.set(end, property::ASSOCIATION, Value::Ref(ext)).map_err(fail)?;
    }
    for e in [base, end] {
        if !refs(store, ext, crate::association::MEMBER_END).contains(&e) {
            store.push(ext, crate::association::MEMBER_END, Value::Ref(e)).map_err(fail)?;
        }
    }
    crate::members::set_multiplicity(store, end, "0..1", "extends")
}

/// Takes the extension `e` away with its views, and its metaclass's cards from the profile's
/// diagrams when no other extension of the profile's stereotypes uses it.
fn unextend(store: &mut Store, e: Extension) -> Result<(), CommandError> {
    let profile = store.container(e.extension).ok().flatten().map(|(p, _)| p);
    archforge_diagram::remove_views_of(store, e.extension).map_err(fail)?;
    store.destroy(e.extension).map_err(fail)?;
    if store.is_alive(e.base) {
        store.destroy(e.base).map_err(fail)?;
    }
    if let Some(profile) = profile.filter(|p| is_profile(store, *p)) {
        drop_unused_cards(store, profile, None)?;
    }
    Ok(())
}

/// Takes the cards of metaclasses none of the profile's stereotypes (`leaving` left out) extends
/// any more off its diagrams.
fn drop_unused_cards(store: &mut Store, profile: Obj, leaving: Option<Obj>) -> Result<(), CommandError> {
    let used: BTreeSet<Obj> =
        stereotypes(store, profile).into_iter().filter(|s| Some(*s) != leaving).flat_map(|s| extensions(store, s)).map(|e| e.metaclass).collect();
    for d in diagrams_of(store, profile) {
        let nodes: Vec<Obj> = store
            .alive()
            .filter(|n| archforge_diagram::diagram_of(store, *n) == Some(d))
            .filter(|n| {
                store
                    .get(*n, archforge_diagram::node::ELEMENT)
                    .ok()
                    .and_then(|v| v.as_ref())
                    .is_some_and(|e| is_metaclass(store, e) && !used.contains(&e))
            })
            .collect();
        for n in nodes {
            if store.is_alive(n) {
                archforge_diagram::destroy_node_with_edges(store, n).map_err(fail)?;
            }
        }
    }
    Ok(())
}

/// The diagrams of `element` (a profile's own diagrams).
pub fn diagrams_of(store: &Store, element: Obj) -> Vec<Obj> {
    store
        .alive()
        .filter(|d| store.class_of(*d).ok() == Some(archforge_diagram::DIAGRAM))
        .filter(|d| store.get(*d, archforge_diagram::diagram::ELEMENT).ok().and_then(|v| v.as_ref()) == Some(element))
        .collect()
}

/// The nodes on diagram `d` that show `element`.
fn nodes_on(store: &Store, d: Obj, element: Obj) -> Vec<Obj> {
    archforge_diagram::views_of(store, element).2.into_iter().filter(|n| archforge_diagram::diagram_of(store, *n) == Some(d)).collect()
}

// Profile application.

/// The profiles `package` applies, each with its ProfileApplication, in its order.
pub fn applied(store: &Store, package: Obj) -> Vec<(Obj, Obj)> {
    refs(store, package, package::PROFILE_APPLICATION)
        .into_iter()
        .filter_map(|a| Some((a, one(store, a, profile_application::APPLIED_PROFILE)?)))
        .filter(|(_, p)| is_profile(store, *p))
        .collect()
}

/// The packages that apply `profile`, by the model's order.
pub fn applying(store: &Store, profile: Obj) -> Vec<Obj> {
    crate::commands::all_packages(store).into_iter().filter(|p| applied(store, *p).iter().any(|(_, a)| *a == profile)).collect()
}

/// The package an element sits in, at any depth: a package itself, else the nearest package its
/// containers lead to.
pub fn home_of(store: &Store, element: Obj) -> Option<Obj> {
    let mut at = element;
    for _ in 0..MAX_DEPTH {
        if is(store, at, PACKAGE) {
            return Some(at);
        }
        at = store.container(at).ok().flatten()?.0;
    }
    None
}

/// The packages around `element`, innermost first: its home package and every one above it.
fn scopes(store: &Store, element: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    let mut at = home_of(store, element);
    while let Some(p) = at.filter(|p| out.len() < MAX_DEPTH && !out.contains(p)) {
        out.push(p);
        at = crate::commands::package_of(store, p);
    }
    out
}

/// Whether the stereotypes of `profile` are available to `element`: a package around it, itself
/// when it is one, applies the profile.
pub fn in_scope(store: &Store, profile: Obj, element: Obj) -> bool {
    scopes(store, element).into_iter().any(|p| applied(store, p).iter().any(|(_, a)| *a == profile))
}

/// The stereotypes available to `element`: those of every profile applied to a package around it,
/// each once, whatever metaclass they extend (AR-135 filters them by metaclass).
pub fn available(store: &Store, element: Obj) -> Vec<Obj> {
    let mut out = Vec::new();
    for p in scopes(store, element) {
        for (_, profile) in applied(store, p) {
            for s in stereotypes(store, profile) {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
}

/// Whether a stereotype extending the metaclass `metaclass` applies to `element`: its metaclass
/// conforms to the one extended, as UML has it (an association class is a class too).
pub fn extends_kind(store: &Store, metaclass: Obj, element: Obj) -> bool {
    let (Some(kind), Ok(class)) = (metaclass_kind(&name_of(store, metaclass)), store.class_of(element)) else { return false };
    store.universe().conforms(class, archforge_mof::TypeRef::Class(kind))
}

// Applications and uses.

/// A stereotype application: the InstanceSpecification classified by the stereotype, and the
/// element its core Link traces it to, when it has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Application {
    pub instance: Obj,
    pub element: Option<Obj>,
}

/// The element an application traces to: the target of the core Link from it.
pub fn element_of(store: &Store, instance: Obj) -> Option<Obj> {
    store
        .referrers(instance)
        .into_iter()
        .filter(|(_, prop)| *prop == archforge_core::link::SOURCE)
        .find_map(|(l, _)| one(store, l, archforge_core::link::TARGET))
}

/// Every application of `stereotype`, in the order the store holds them: the one place that finds
/// them, so the refusals here and the commands of AR-135 agree.
pub fn applications_of(store: &Store, stereotype: Obj) -> Vec<Application> {
    store
        .referrers(stereotype)
        .into_iter()
        .filter(|(o, prop)| *prop == instance_specification::CLASSIFIER && is(store, *o, INSTANCE_SPECIFICATION))
        .map(|(instance, _)| Application { instance, element: element_of(store, instance) })
        .collect()
}

/// The uses a refusal names: per stereotype, the elements its applications sit on, and how many
/// trace to no element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Uses {
    pub by: Vec<(Obj, Vec<Obj>, usize)>,
}

impl Uses {
    /// How many applications in all.
    pub fn total(&self) -> usize {
        self.by.iter().map(|(_, e, loose)| e.len().saturating_add(*loose)).sum()
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// The elements used, each once, in order.
    pub fn elements(&self) -> Vec<Obj> {
        let mut out: Vec<Obj> = Vec::new();
        for (_, es, _) in &self.by {
            for e in es {
                if !out.contains(e) {
                    out.push(*e);
                }
            }
        }
        out
    }

    /// Adds `other`'s uses to these.
    fn merge(&mut self, other: Uses) {
        for (s, es, loose) in other.by {
            match self.by.iter_mut().find(|(x, _, _)| *x == s) {
                Some((_, mine, l)) => {
                    mine.extend(es);
                    *l = l.saturating_add(loose);
                }
                None => self.by.push((s, es, loose)),
            }
        }
    }

    fn add(&mut self, stereotype: Obj, a: Application) {
        let at = match self.by.iter().position(|(s, _, _)| *s == stereotype) {
            Some(i) => i,
            None => {
                self.by.push((stereotype, Vec::new(), 0));
                self.by.len() - 1
            }
        };
        if let Some((_, es, loose)) = self.by.get_mut(at) {
            match a.element {
                Some(e) => es.push(e),
                None => *loose = loose.saturating_add(1),
            }
        }
    }
}

/// The uses of `stereotypes` that `keep` keeps, each application once.
fn uses_where(store: &Store, stereotypes: &[Obj], keep: impl Fn(&Application) -> bool) -> Uses {
    let mut uses = Uses::default();
    for s in stereotypes {
        for a in applications_of(store, *s).into_iter().filter(|a| keep(a)) {
            uses.add(*s, a);
        }
    }
    uses
}

/// The applications on `objs` or on anything they hold: what goes with them when they are removed,
/// since an application means nothing without its element.
pub fn applications_on(store: &Store, objs: &[Obj]) -> Vec<Obj> {
    let profiles = all_profiles(store);
    let mut out = Vec::new();
    for s in profiles.into_iter().flat_map(|p| stereotypes(store, p)) {
        for a in applications_of(store, s) {
            if a.element.is_some_and(|e| objs.iter().any(|o| *o == e || store.contains(*o, e))) && !out.contains(&a.instance) {
                out.push(a.instance);
            }
        }
    }
    out
}

/// Every use of a stereotype.
pub fn stereotype_uses(store: &Store, stereotype: Obj) -> Uses {
    uses_where(store, &[stereotype], |_| true)
}

/// Every use of a profile's stereotypes.
pub fn profile_uses(store: &Store, profile: Obj) -> Uses {
    uses_where(store, &stereotypes(store, profile), |_| true)
}

/// The uses unapplying `profile` from `package` would strand: applications of its stereotypes on
/// elements inside `package` that no other package around them applies it to. An application
/// tracing to no element is never counted here: no scope holds it.
pub fn unapply_uses(store: &Store, package: Obj, profile: Obj) -> Uses {
    uses_where(store, &stereotypes(store, profile), |a| {
        a.element.is_some_and(|e| {
            let around = scopes(store, e);
            around.contains(&package) && !around.iter().any(|p| *p != package && applied(store, *p).iter().any(|(_, x)| *x == profile))
        })
    })
}

/// The uses that moving `moved` (a package or a classifier) into `to` would strand: applications on
/// elements inside `moved` whose profile no package around them applies any more, those `moved`
/// takes along and those around `to` counted.
pub fn moving_uses(store: &Store, moved: Obj, to: Obj) -> Uses {
    let mut uses = Uses::default();
    let after: Vec<Obj> = std::iter::once(to).chain(scopes(store, to)).collect();
    for profile in all_profiles(store) {
        for s in stereotypes(store, profile) {
            for a in applications_of(store, s) {
                let Some(e) = a.element.filter(|e| *e == moved || store.contains(moved, *e)) else { continue };
                if !in_scope(store, profile, e) {
                    continue;
                }
                // The packages that stay around it: those inside what moves, up to it, then `to`'s.
                let inside: Vec<Obj> = scopes(store, e).into_iter().take_while(|p| *p == moved || store.contains(moved, *p)).collect();
                let kept = inside.iter().chain(after.iter()).any(|p| applied(store, *p).iter().any(|(_, x)| *x == profile));
                if !kept {
                    uses.add(s, a);
                }
            }
        }
    }
    uses
}

/// The uses removing `package` would strand: applications on elements it hands up to its parent
/// whose profile only it applies there.
pub fn removal_uses(store: &Store, package: Obj) -> Uses {
    let mut uses = Uses::default();
    for (_, profile) in applied(store, package) {
        uses.merge(unapply_uses(store, package, profile));
    }
    uses
}

/// The uses dropping `stereotype`'s extension to `metaclass` would strand: applications on elements
/// of that metaclass no other extension of the stereotype covers.
pub fn extension_uses(store: &Store, stereotype: Obj, metaclass: Obj) -> Uses {
    let others: Vec<Obj> = extensions(store, stereotype).into_iter().map(|e| e.metaclass).filter(|m| *m != metaclass).collect();
    uses_where(store, &[stereotype], |a| {
        a.element.is_some_and(|e| extends_kind(store, metaclass, e) && !others.iter().any(|m| extends_kind(store, *m, e)))
    })
}

/// How many slots hold a value of the stereotype property `p`: its values set on applications.
pub fn values_set(store: &Store, p: Obj) -> usize {
    store.referrers(p).into_iter().filter(|(s, prop)| *prop == slot::DEFINING_FEATURE && is(store, *s, crate::SLOT)).count()
}

/// The uses as a refusal names them: per stereotype its name, its elements' names while there are
/// at most two of different names (else none, as they are counted), and how many in all.
pub fn uses_named(store: &Store, uses: &Uses) -> Vec<(String, Vec<String>, usize)> {
    let named = |(s, es, loose): &(Obj, Vec<Obj>, usize)| {
        let names: Vec<String> = es.iter().take(MAX_NAMED.saturating_add(1)).map(|e| name_of(store, *e)).collect();
        let alike = matches!(names.as_slice(), [a, b] if a.eq_ignore_ascii_case(b));
        let told = *loose == 0 && names.len() <= MAX_NAMED && !alike;
        (name_of(store, *s), if told { names } else { Vec::new() }, es.len().saturating_add(*loose))
    };
    uses.by.iter().map(named).collect()
}

/// The uses in words: `«Entity» on Order and Customer, «Id» on 2 elements`, each stereotype's
/// elements named while there are at most two, else counted.
pub fn uses_text(store: &Store, uses: &Uses) -> String {
    let said = |(s, names, n): (String, Vec<String>, usize)| match n {
        _ if !names.is_empty() => format!("«{s}» on {}", names.join(" and ")),
        1 => format!("«{s}» on 1 element"),
        n => format!("«{s}» on {n} elements"),
    };
    uses_named(store, uses).into_iter().map(said).collect::<Vec<_>>().join(", ")
}

/// A refusal that names what is in use and the uses, as `{what} is in use{where}: {uses}. Remove
/// those first.`, its reason typed.
fn in_use(store: &Store, key: &str, what: Used, uses: &Uses) -> CommandError {
    let said = match &what {
        Used::Element(name) => format!("{name} is in use"),
        Used::Moved(stereotype) => format!("«{stereotype}» is in use"),
        Used::ProfileIn { profile, package } => format!("{profile} is in use in {package}"),
        Used::Extension { stereotype, metaclass } => format!("«{stereotype}» on {metaclass} is in use"),
        Used::HeldProfile { package, profile } => format!("{package} holds the profile {profile}, which is in use"),
        Used::AppliedProfiles { package } => format!("{package} applies the profiles its elements use"),
    };
    let reason = Reason::InUse(Box::new(InUse { what, uses: uses_named(store, uses) }));
    refuse(key, format!("{said}: {}. Remove those first.", uses_text(store, uses))).because(reason)
}

/// The refusal of a move of `moved` that would strand `uses`, naming them.
pub fn stranding(store: &Store, moved: Obj, uses: &Uses) -> String {
    format!(
        "moving {} there takes stereotypes out of the packages that apply their profiles: {}; apply the profile there first",
        name_of(store, moved),
        uses_text(store, uses)
    )
}

/// Why `objs` may not be removed for their uses, when that is so: a profile or a stereotype whose
/// stereotypes are applied, a stereotype property whose values are set, a package whose profile
/// applications its contents need; and the library and its metaclasses, which are UML's own.
pub fn removal_problem(store: &Store, o: Obj) -> Option<CommandError> {
    if let Some(why) = library_problem(store, o) {
        return Some(refuse("ids", why));
    }
    let uses = match store.class_of(o).ok()? {
        c if c == PROFILE => profile_uses(store, o),
        c if c == STEREOTYPE => stereotype_uses(store, o),
        c if c == PACKAGE => {
            // A package with no parent takes what it holds along: a profile inside it goes, and
            // is checked for uses as a profile removed on its own is.
            if crate::commands::package_of(store, o).is_none() {
                for profile in profiles_within(store, o) {
                    let uses = profile_uses(store, profile);
                    if !uses.is_empty() {
                        let held = Used::HeldProfile { package: name_of(store, o), profile: name_of(store, profile) };
                        return Some(in_use(store, "ids", held, &uses));
                    }
                }
            }
            let uses = removal_uses(store, o);
            if uses.is_empty() {
                return None;
            }
            return Some(in_use(store, "ids", Used::AppliedProfiles { package: name_of(store, o) }, &uses));
        }
        c if c == PROPERTY => {
            if is_base(store, o) {
                return Some(refuse(
                    "ids",
                    format!("{} is a stereotype's extension end; drop the metaclass from what the stereotype extends instead", name_of(store, o)),
                ));
            }
            let n = values_set(store, o);
            if n == 0 {
                return None;
            }
            let on = if n == 1 { "1 element".to_string() } else { format!("{n} elements") };
            return Some(refuse("ids", format!("the values of {} are set on {on}; remove those first", name_of(store, o))));
        }
        _ => return None,
    };
    (!uses.is_empty()).then(|| in_use(store, "ids", Used::Element(name_of(store, o)), &uses))
}

/// Takes `profile`'s applications away from every package before it goes: an application needs its
/// profile. Its stereotypes' uses are refused before ([`removal_problem`]).
pub fn before_profile_removal(store: &mut Store, profile: Obj) -> Result<(), CommandError> {
    for p in applying(store, profile) {
        for (a, x) in applied(store, p) {
            if x == profile {
                store.destroy(a).map_err(fail)?;
            }
        }
    }
    Ok(())
}

/// Takes `stereotype`'s extensions away before it goes, with their lines, and the cards of the
/// metaclasses nothing else in its profile extends.
pub fn before_stereotype_removal(store: &mut Store, stereotype: Obj) -> Result<(), CommandError> {
    let profile = profile_of(store, stereotype);
    if let Some(p) = profile {
        drop_unused_cards(store, p, Some(stereotype))?;
    }
    for e in extensions(store, stereotype) {
        archforge_diagram::remove_views_of(store, e.extension).map_err(fail)?;
        store.destroy(e.extension).map_err(fail)?;
    }
    Ok(())
}

// Commands.

fn element(store: &Store, p: &Params, key: &str, class: ClassId, what: &str) -> Result<Obj, CommandError> {
    crate::commands::id_of(store, p, key, class, what)
}

/// The package under `key`, or the root package when it names none.
fn package_param(store: &mut Store, p: &Params, key: &str) -> Result<Obj, CommandError> {
    if text(p, key).is_empty() { crate::commands::ensure_root(store) } else { element(store, p, key, PACKAGE, "a package") }
}

/// The metaclass names under `key`, each once, each one Archforge draws.
fn metaclass_names(p: &Params, key: &str) -> Result<Option<Vec<String>>, CommandError> {
    let Some(v) = p.get(key) else { return Ok(None) };
    let items = v.as_array().ok_or_else(|| refuse(key, "a list of metaclass names"))?;
    if items.len() > METACLASSES.len() {
        return Err(refuse(key, format!("at most {} metaclasses", METACLASSES.len())));
    }
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let name = item.as_str().ok_or_else(|| refuse(key, "a list of metaclass names"))?;
        if let Some(why) = metaclass_problem(name) {
            return Err(refuse(key, why));
        }
        if out.iter().any(|n| n == name) {
            return Err(refuse(key, format!("{name} is named twice")));
        }
        out.push(name.to_string());
    }
    Ok(Some(out))
}

/// A stereotype's name: a name, free among what its profile holds.
fn check_stereotype_name(store: &Store, profile: Obj, name: &str, except: Option<Obj>) -> Result<(), CommandError> {
    check_name(store, Some(profile), STEREOTYPE, name, except)
}

/// `uml.profile.add`: a profile in a package, or in the root package, its name free among what the
/// package holds.
fn profile_add(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = package_param(ctx.doc, p, "package")?;
    let name = text(p, "name");
    check_name(ctx.doc, Some(pkg), PROFILE, name, None)?;
    let profile = ctx.doc.create(PROFILE).map_err(fail)?;
    set_name(ctx.doc, profile, name)?;
    ctx.doc.push(pkg, package::PACKAGED_ELEMENT, Value::Ref(profile)).map_err(fail)?;
    ctx.view.selection = vec![profile];
    Ok(json!({ "id": uuid_of(ctx.doc, profile)? }))
}

/// `uml.profile.rename`: a profile renamed; every package that applies it keeps applying it, and
/// every application of its stereotypes stays.
fn profile_rename(ctx: &mut C<'_>, p: &Params) -> R {
    let profile = element(ctx.doc, p, "profile", PROFILE, "a profile")?;
    let name = text(p, "name");
    check_name(ctx.doc, package_of(ctx.doc, profile), PROFILE, name, Some(profile))?;
    set_name(ctx.doc, profile, name)?;
    Ok(json!({ "id": uuid_of(ctx.doc, profile)? }))
}

/// `uml.stereotype.add`: a stereotype in a profile, extending the metaclasses named.
fn stereotype_add(ctx: &mut C<'_>, p: &Params) -> R {
    let profile = element(ctx.doc, p, "profile", PROFILE, "a profile")?;
    let name = text(p, "name");
    check_stereotype_name(ctx.doc, profile, name, None)?;
    if stereotypes(ctx.doc, profile).len() >= MAX_STEREOTYPES {
        return Err(refuse("profile", format!("a profile holds at most {MAX_STEREOTYPES} stereotypes")));
    }
    let extends = metaclass_names(p, "extends")?.unwrap_or_default();
    let s = ctx.doc.create(STEREOTYPE).map_err(fail)?;
    set_name(ctx.doc, s, name)?;
    ctx.doc.push(profile, package::PACKAGED_ELEMENT, Value::Ref(s)).map_err(fail)?;
    for m in &extends {
        extend(ctx.doc, s, m)?;
    }
    ctx.view.selection = vec![s];
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

/// `uml.stereotype.set`: a stereotype's name, profile and the metaclasses it extends, each kept when
/// left out. A rename keeps every application. Dropping a metaclass whose elements use the
/// stereotype, and moving a stereotype that is applied to another profile, are refused naming the
/// uses: the applications would lose their definition or their scope.
fn stereotype_set(ctx: &mut C<'_>, p: &Params) -> R {
    let s = element(ctx.doc, p, "stereotype", STEREOTYPE, "a stereotype")?;
    let from = profile_of(ctx.doc, s).ok_or_else(|| refuse("stereotype", "it is in no profile"))?;
    let to = match p.str("profile") {
        Some(_) => element(ctx.doc, p, "profile", PROFILE, "a profile")?,
        None => from,
    };
    let name = p.str("name").map_or_else(|| name_of(ctx.doc, s), str::to_string);
    check_stereotype_name(ctx.doc, to, &name, Some(s))?;
    let wanted = metaclass_names(p, "extends")?;
    // Everything checked before anything changes.
    if to != from {
        let uses = stereotype_uses(ctx.doc, s);
        if !uses.is_empty() {
            return Err(in_use(ctx.doc, "profile", Used::Moved(name_of(ctx.doc, s)), &uses));
        }
        if stereotypes(ctx.doc, to).len() >= MAX_STEREOTYPES {
            return Err(refuse("profile", format!("a profile holds at most {MAX_STEREOTYPES} stereotypes")));
        }
    }
    let had = extensions(ctx.doc, s);
    let dropped: Vec<Extension> = match &wanted {
        Some(names) => had.iter().copied().filter(|e| !names.contains(&name_of(ctx.doc, e.metaclass))).collect(),
        None => Vec::new(),
    };
    for e in &dropped {
        let uses = extension_uses(ctx.doc, s, e.metaclass);
        if !uses.is_empty() {
            let dropped = Used::Extension { stereotype: name_of(ctx.doc, s), metaclass: name_of(ctx.doc, e.metaclass) };
            return Err(in_use(ctx.doc, "extends", dropped, &uses));
        }
    }
    set_name(ctx.doc, s, &name)?;
    for e in had.iter().filter_map(|e| e.end) {
        set_name(ctx.doc, e, &format!("extension_{name}"))?;
    }
    if to != from {
        // Its extensions go with it.
        let exts = extensions(ctx.doc, s);
        ctx.doc.remove_value(from, package::PACKAGED_ELEMENT, &Value::Ref(s)).map_err(fail)?;
        ctx.doc.push(to, package::PACKAGED_ELEMENT, Value::Ref(s)).map_err(fail)?;
        for e in exts {
            ctx.doc.remove_value(from, package::PACKAGED_ELEMENT, &Value::Ref(e.extension)).map_err(fail)?;
            ctx.doc.push(to, package::PACKAGED_ELEMENT, Value::Ref(e.extension)).map_err(fail)?;
        }
        // Its cards on the old profile's diagrams go: they are of another profile now.
        for d in diagrams_of(ctx.doc, from) {
            for n in nodes_on(ctx.doc, d, s) {
                archforge_diagram::destroy_node_with_edges(ctx.doc, n).map_err(fail)?;
            }
        }
        drop_unused_cards(ctx.doc, from, None)?;
    }
    for e in dropped {
        unextend(ctx.doc, e)?;
    }
    if let Some(names) = wanted {
        let now = extended(ctx.doc, s);
        for n in names.iter().filter(|n| !now.contains(n)) {
            extend(ctx.doc, s, n)?;
        }
    }
    ctx.view.selection = vec![s];
    Ok(json!({ "id": uuid_of(ctx.doc, s)? }))
}

/// `uml.profile.apply`: a profile applied to a package, so the stereotypes are available to the
/// elements inside it; applying adds no stereotype by itself. A profile applied there already is
/// refused.
fn profile_apply(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = element(ctx.doc, p, "package", PACKAGE, "a package")?;
    let profile = element(ctx.doc, p, "profile", PROFILE, "a profile")?;
    if applied(ctx.doc, pkg).iter().any(|(_, x)| *x == profile) {
        return Err(refuse("profile", format!("{} applies {} already", name_of(ctx.doc, pkg), name_of(ctx.doc, profile))));
    }
    let a = ctx.doc.create(PROFILE_APPLICATION).map_err(fail)?;
    ctx.doc.set(a, profile_application::APPLIED_PROFILE, Value::Ref(profile)).map_err(fail)?;
    ctx.doc.push(pkg, package::PROFILE_APPLICATION, Value::Ref(a)).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, a)? }))
}

/// `uml.profile.unapply`: a profile no longer applied to a package; refused, naming the uses, while
/// elements inside it use the profile's stereotypes and no other package around them applies it.
fn profile_unapply(ctx: &mut C<'_>, p: &Params) -> R {
    let pkg = element(ctx.doc, p, "package", PACKAGE, "a package")?;
    let profile = element(ctx.doc, p, "profile", PROFILE, "a profile")?;
    let Some((a, _)) = applied(ctx.doc, pkg).into_iter().find(|(_, x)| *x == profile) else {
        return Err(refuse("profile", format!("{} does not apply {}", name_of(ctx.doc, pkg), name_of(ctx.doc, profile))));
    };
    let uses = unapply_uses(ctx.doc, pkg, profile);
    if !uses.is_empty() {
        let unapplied = Used::ProfileIn { profile: name_of(ctx.doc, profile), package: name_of(ctx.doc, pkg) };
        return Err(in_use(ctx.doc, "profile", unapplied, &uses));
    }
    ctx.doc.destroy(a).map_err(fail)?;
    Ok(json!({ "id": uuid_of(ctx.doc, pkg)? }))
}

/// The uses as the reads give them.
fn uses_json(store: &Store, uses: &Uses) -> Json {
    let items: Vec<Json> = uses
        .by
        .iter()
        .map(|(s, es, loose)| {
            let elements: Vec<Json> =
                es.iter().map(|e| json!({ "id": uuid_of(store, *e).unwrap_or_default(), "name": name_of(store, *e) })).collect();
            json!({ "stereotype": uuid_of(store, *s).unwrap_or_default(), "name": name_of(store, *s), "elements": elements, "unplaced": loose })
        })
        .collect();
    json!({ "total": uses.total(), "uses": items })
}

/// A stereotype as the reads give it.
fn stereotype_json(store: &Store, s: Obj) -> Json {
    let props: Vec<Json> = properties(store, s)
        .into_iter()
        .map(|a| json!({ "id": uuid_of(store, a).unwrap_or_default(), "name": name_of(store, a), "text": crate::figures::attribute_text(store, a) }))
        .collect();
    json!({
        "id": uuid_of(store, s).unwrap_or_default(), "name": name_of(store, s),
        "profile": profile_of(store, s).and_then(|p| uuid_of(store, p).ok()),
        "extends": extended(store, s), "properties": props, "uses": stereotype_uses(store, s).total(),
    })
}

/// `uml.profile.list`: every profile with its package, its stereotypes and the packages that apply it.
fn profile_list(ctx: &mut C<'_>, _: &Params) -> R {
    let store = &*ctx.doc;
    let items: Vec<Json> = all_profiles(store)
        .into_iter()
        .map(|profile| {
            let applied_to: Vec<String> = applying(store, profile).into_iter().filter_map(|p| uuid_of(store, p).ok()).collect();
            json!({
                "id": uuid_of(store, profile).unwrap_or_default(), "name": name_of(store, profile),
                "package": package_of(store, profile).and_then(|p| uuid_of(store, p).ok()),
                "description": crate::descriptions::description_of(store, profile),
                "stereotypes": stereotypes(store, profile).into_iter().map(|s| stereotype_json(store, s)).collect::<Vec<_>>(),
                "applied_to": applied_to,
            })
        })
        .collect();
    Ok(json!({ "profiles": items }))
}

/// `uml.stereotype.get`: a stereotype with its profile, the metaclasses it extends, its properties
/// and how many elements use it.
fn stereotype_get(ctx: &mut C<'_>, p: &Params) -> R {
    let s = element(ctx.doc, p, "stereotype", STEREOTYPE, "a stereotype")?;
    Ok(stereotype_json(ctx.doc, s))
}

/// `uml.profile.uses`: the uses of a profile's stereotypes, or, with a package, those unapplying it
/// there would strand.
fn profile_uses_cmd(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let profile = element(store, p, "profile", PROFILE, "a profile")?;
    let uses = if text(p, "package").is_empty() {
        profile_uses(store, profile)
    } else {
        unapply_uses(store, element(store, p, "package", PACKAGE, "a package")?, profile)
    };
    Ok(uses_json(store, &uses))
}

/// `uml.package.profiles`: the profiles a package applies, and every profile of the model it could.
fn package_profiles(ctx: &mut C<'_>, p: &Params) -> R {
    let store = &*ctx.doc;
    let pkg = element(store, p, "package", PACKAGE, "a package")?;
    let item = |profile: Obj| json!({ "id": uuid_of(store, profile).unwrap_or_default(), "name": name_of(store, profile) });
    let applied: Vec<Json> = applied(store, pkg).into_iter().map(|(_, x)| item(x)).collect();
    let all: Vec<Json> = all_profiles(store).into_iter().map(item).collect();
    Ok(json!({ "package": uuid_of(store, pkg)?, "applied": applied, "profiles": all }))
}

/// `uml.metaclass.list`: the metaclasses a stereotype may extend, and those Archforge does not draw
/// yet, which it may not.
fn metaclass_list(_: &mut C<'_>, _: &Params) -> R {
    let offered: Vec<&str> = METACLASSES.iter().map(|(n, _)| *n).collect();
    Ok(json!({ "offered": offered, "not_drawn": NOT_DRAWN }))
}

/// The room between two cards a profile's diagram places side by side, and between its rows.
const GAP: f64 = 50.0;
const ROW_GAP: f64 = 280.0;
/// The height of a placed metaclass's card (its header alone), and of a stereotype's per property.
const METACLASS_HEIGHT: f64 = 66.0;
const PROPERTY_ROW: f64 = 20.0;

/// Fills `d`, a diagram of `profile`, with what it lacks: a card for every metaclass the profile's
/// stereotypes extend, in a row, and one for every stereotype, in a row under them, below whatever
/// it shows already, which stays where it is; then a line for every extension whose two cards it
/// shows and does not join yet. Gives how many cards and lines it added.
fn fill_diagram(store: &mut Store, d: Obj, profile: Obj) -> Result<usize, CommandError> {
    use archforge_diagram::{EDGE, NODE, diagram, edge, node};
    let on_d = |st: &Store, e: Obj| archforge_diagram::views_of(st, e).2.into_iter().find(|n| archforge_diagram::diagram_of(st, *n) == Some(d));
    let real = |st: &Store, n: Obj, prop| match st.get(n, prop) {
        Ok(Value::Real(r)) => r,
        _ => 0.0,
    };
    let top_nodes = refs(store, d, diagram::NODE);
    let below = top_nodes
        .iter()
        .map(|n| real(store, *n, node::Y) + real(store, *n, node::HEIGHT))
        .fold(None, |m: Option<f64>, y| Some(m.map_or(y, |m| m.max(y))));
    let top = below.map_or(0.0, |b| b + 80.0);
    let (w, _) = crate::CLASS_SIZE;
    let metaclasses: Vec<Obj> = metaclasses_of(store, profile).into_iter().filter(|m| on_d(store, *m).is_none()).collect();
    let stereotypes: Vec<(Obj, f64)> = stereotypes(store, profile)
        .into_iter()
        .filter(|s| on_d(store, *s).is_none())
        .map(|s| (s, METACLASS_HEIGHT + PROPERTY_ROW * properties(store, s).len().max(1) as f64 + 16.0))
        .collect();
    let mut added = 0;
    let mut place = |st: &mut Store, e: Obj, x: f64, y: f64, h: f64| -> Result<(), CommandError> {
        let n = st.create(NODE).map_err(fail)?;
        st.set(n, node::ELEMENT, Value::Ref(e)).map_err(fail)?;
        st.set(n, node::X, Value::Real(x)).map_err(fail)?;
        st.set(n, node::Y, Value::Real(y)).map_err(fail)?;
        st.set(n, node::WIDTH, Value::Real(w)).map_err(fail)?;
        st.set(n, node::HEIGHT, Value::Real(h)).map_err(fail)?;
        st.push(d, diagram::NODE, Value::Ref(n)).map_err(fail)?;
        added += 1;
        Ok(())
    };
    for (i, m) in metaclasses.iter().enumerate() {
        place(store, *m, i as f64 * (w + GAP), top, METACLASS_HEIGHT)?;
    }
    let row = if metaclasses.is_empty() { top } else { top + ROW_GAP };
    for (i, (s, h)) in stereotypes.iter().enumerate() {
        place(store, *s, i as f64 * (w + GAP), row, *h)?;
    }
    for e in stereotypes_of_extensions(store, profile) {
        let joined = archforge_diagram::views_of(store, e.extension).1.into_iter().any(|x| archforge_diagram::diagram_of(store, x) == Some(d));
        let ends = e.end.and_then(|end| one(store, end, extension_end::TYPE)).and_then(|s| Some((on_d(store, s)?, on_d(store, e.metaclass)?)));
        if let (false, Some((from, to))) = (joined, ends) {
            let x = store.create(EDGE).map_err(fail)?;
            store.set(x, edge::ELEMENT, Value::Ref(e.extension)).map_err(fail)?;
            store.set(x, edge::SOURCE, Value::Ref(from)).map_err(fail)?;
            store.set(x, edge::TARGET, Value::Ref(to)).map_err(fail)?;
            store.push(d, diagram::EDGE, Value::Ref(x)).map_err(fail)?;
            added += 1;
        }
    }
    Ok(added)
}

/// Every extension of the profile's stereotypes, in their order.
fn stereotypes_of_extensions(store: &Store, profile: Obj) -> Vec<Extension> {
    stereotypes(store, profile).into_iter().flat_map(|s| extensions(store, s)).collect()
}

/// `uml.profile.diagram`: the profile's diagram, made when it has none and named after it, filled
/// with every stereotype, every metaclass they extend and the extensions between them, what it
/// showed already kept where it was.
fn profile_diagram(ctx: &mut C<'_>, p: &Params) -> R {
    let profile = element(ctx.doc, p, "profile", PROFILE, "a profile")?;
    let (d, made) = match diagrams_of(ctx.doc, profile).first() {
        Some(d) => (*d, false),
        None => {
            let m = crate::commands::model(ctx.doc)?;
            let name: String = format!("{} profile", name_of(ctx.doc, profile)).chars().take(archforge_diagram::rules::MAX_NAME).collect();
            let d = archforge_diagram::commands::new_diagram(
                ctx.doc,
                crate::CLASS_DIAGRAM,
                &name,
                Some(profile),
                Some((m, archforge_core::model::MEMBER)),
            )
            .map_err(fail)?;
            (d, true)
        }
    };
    let added = fill_diagram(ctx.doc, d, profile)?;
    Ok(json!({ "id": uuid_of(ctx.doc, d)?, "made": made, "added": added }))
}

fn has_profiles(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(PROFILE)) { Ok(()) } else { Err(NO_PROFILES) }
}

fn has_stereotypes(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.doc.class_of(o).ok() == Some(STEREOTYPE)) { Ok(()) } else { Err(NO_STEREOTYPES) }
}

/// The profiles' and stereotypes' commands.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(
            "uml.profile.add",
            "Add profile",
            &["Profile"],
            None,
            "name: string; the profile's name, free among what its package holds whatever the case\npackage: string = \"\"; the id of the package it goes into, or nothing for the model's root package",
            always,
            profile_add
        ),
        cmd!(
            "uml.profile.rename",
            "Rename profile",
            &["Profile"],
            None,
            "profile: string; the profile's id\nname: string; the new name, free among what its package holds; the packages that apply it and every application of its stereotypes stay",
            has_profiles,
            profile_rename
        ),
        cmd!(
            "uml.profile.apply",
            "Apply profile",
            &["Profile", "Package"],
            None,
            "package: string; the id of the package that applies it; the elements inside it, at any depth, may then use the profile's stereotypes\nprofile: string; the profile's id, not applied there yet",
            has_profiles,
            profile_apply
        ),
        cmd!(
            "uml.profile.unapply",
            "Unapply profile",
            &["Profile", "Package"],
            None,
            "package: string; the id of the package that applies it\nprofile: string; the profile's id; refused, naming the uses, while elements inside the package use its stereotypes and no package around them applies it too",
            has_profiles,
            profile_unapply
        ),
        cmd!(
            "uml.stereotype.add",
            "Add stereotype",
            &["Stereotype"],
            None,
            "profile: string; the id of the profile that holds it\nname: string; its name, free among what the profile holds whatever the case\nextends: array = []; the metaclasses it extends, by UML's names: Class, Interface, Enumeration, DataType, Property, Operation, Association, Generalization or Package, each once",
            has_profiles,
            stereotype_add
        ),
        cmd!(
            "uml.stereotype.set",
            "Change stereotype",
            &["Stereotype"],
            None,
            concat!(
                "stereotype: string; the stereotype's id\n",
                "name: string?; its new name, free among what its profile holds; every application stays; left out, kept\n",
                "profile: string?; the id of the profile it moves to, with its extensions; refused while elements use it; left out, kept\n",
                "extends: array?; every metaclass it extends, by UML's names (Class, Interface, Enumeration, DataType, Property, Operation, Association, Generalization, Package), replacing those it extended; dropping one whose elements use the stereotype is refused, naming them; left out, kept"
            ),
            has_stereotypes,
            stereotype_set
        ),
        cmd!(
            "uml.profile.diagram",
            "Profile's diagram",
            &["Profile", "Diagram"],
            None,
            "profile: string; the profile's id; its class diagram, made when it has none, filled with every stereotype, every metaclass they extend and the extension lines between them, in rows under what it shows already, which stays",
            has_profiles,
            profile_diagram
        ),
        cmd!(query "uml.profile.list", "List profiles", &["Profile"], None, "", always, profile_list),
        cmd!(
            query "uml.stereotype.get",
            "Get stereotype",
            &["Stereotype"],
            None,
            "stereotype: string; the stereotype's id; its profile, the metaclasses it extends, its properties and how many elements use it",
            has_stereotypes,
            stereotype_get
        ),
        cmd!(
            query "uml.profile.uses",
            "List profile uses",
            &["Profile"],
            None,
            "profile: string; the profile's id\npackage: string = \"\"; a package that applies it: only the uses unapplying it there would strand; nothing for every use",
            has_profiles,
            profile_uses_cmd
        ),
        cmd!(
            query "uml.package.profiles",
            "List applied profiles",
            &["Profile", "Package"],
            None,
            "package: string; the package's id; the profiles it applies, and every profile it could",
            crate::commands::has_packages,
            package_profiles
        ),
        cmd!(query "uml.metaclass.list", "List metaclasses", &["Stereotype"], None, "", always, metaclass_list),
    ]
}
