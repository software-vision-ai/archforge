//! One view for a project of several notations: the selection and the open
//! diagram, and the notations the project holds, each a [`Notation`] that answers for its own
//! diagram kinds and for the elements of its metamodel. The diagram commands and the skill
//! commands ask the view; the view asks the notation that owns a diagram's kind or an element's
//! class. The shared commands here select, remove and describe elements of any notation.

use std::collections::BTreeSet;

use archforge_core::skills::SkillHost;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, always, cmd};
use archforge_mof::{ClassId, Invalid, Obj, PropId, Store, Uuid};
use serde_json::{Value as Json, json};

use crate::DiagramView;
use crate::figure::{Card, Line};

/// A node's width and height when its notation names none.
pub const DEFAULT_SIZE: (f64, f64) = (160.0, 100.0);

/// What a notation brings to a project's view: its diagram kinds and what they show, where its
/// elements may carry skills, and how its elements are removed and described. Every answer is about
/// the notation's own kinds and the classes of its own metamodel; the view asks no other.
pub trait Notation: Send + Sync + 'static {
    /// The notation's name, for messages.
    fn name(&self) -> &'static str;
    /// The index of the metamodel whose classes are the notation's elements.
    fn metamodel(&self) -> u16;
    /// Its diagram kinds, such as `er`.
    fn kinds(&self) -> &'static [&'static str] {
        &[]
    }
    /// Whether a diagram of `kind` must be of an element.
    fn needs_element(&self, kind: &str) -> bool {
        let _ = kind;
        false
    }
    /// Whether a diagram of `kind` may be of an element of `class`.
    fn is_of(&self, kind: &str, class: ClassId) -> bool {
        let _ = (kind, class);
        false
    }
    /// Whether a diagram of `kind` shows elements of `class` as nodes.
    fn shows(&self, kind: &str, class: ClassId) -> bool {
        let _ = (kind, class);
        false
    }
    /// Whether `diagram`, of one of its kinds, may show `element`, beyond its class.
    fn may_show(&self, store: &Store, diagram: Obj, element: Obj) -> bool {
        let _ = (store, diagram, element);
        true
    }
    /// Whether a diagram of `kind` shows elements of `class` as edges.
    fn connects(&self, kind: &str, class: ClassId) -> bool {
        let _ = (kind, class);
        false
    }
    /// The two ends of one of its relationship elements, source first.
    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        let _ = (store, element);
        None
    }
    /// The text slots a label of an element of `class` may show.
    fn text_slots(&self, class: ClassId) -> &'static [&'static str] {
        let _ = class;
        &[]
    }
    /// A node's width and height for an element of `class` when the user names none.
    fn default_size(&self, class: ClassId) -> (f64, f64) {
        let _ = class;
        DEFAULT_SIZE
    }
    /// The card a node of `element` shows on `diagram`, when the notation draws one; None draws a
    /// plain box with the element's name.
    fn card(&self, store: &Store, element: Obj, diagram: Obj) -> Option<Card> {
        let _ = (store, element, diagram);
        None
    }
    /// The nodes of `diagram` a node of `element` is tied to by dashed links worked out from the
    /// model, never stored: a constraint note's to its constrained elements' nodes (AR-133).
    fn links(&self, store: &Store, element: Obj, diagram: Obj) -> Vec<Obj> {
        let _ = (store, element, diagram);
        Vec::new()
    }
    /// The line an edge of `element` draws; None draws a plain solid line.
    fn line(&self, store: &Store, element: Obj) -> Option<Line> {
        let _ = (store, element);
        None
    }
    /// The text of `element`'s slot a label shows, such as its name.
    fn slot_text(&self, store: &Store, element: Obj, slot: &str) -> String {
        let _ = (store, element, slot);
        String::new()
    }
    /// The classes whose elements may be a skill's subject.
    fn skill_subjects(&self) -> &'static [ClassId] {
        &[]
    }
    /// The classes a skill may aim at, each with the word the user sees.
    fn skill_kinds(&self) -> &'static [(ClassId, &'static str)] {
        &[]
    }
    /// Classes that are kinds of their own when a skill is aimed, though no skill may name them
    /// yet: an element of one is not reached by a skill for a broader kind it conforms to. UML
    /// names none now that a primitive type is a skill kind of its own (AR-124).
    fn aim_only_kinds(&self) -> &'static [ClassId] {
        &[]
    }
    /// The class whose elements have a type a skill's pattern can match, and the property holding it.
    fn typed_class(&self) -> Option<(ClassId, PropId)> {
        None
    }
    /// Why `element`, of a skill subject or kind class, still carries no skill and is aimed at by
    /// none, when it does not.
    fn skill_problem(&self, store: &Store, element: Obj) -> Option<&'static str> {
        let _ = (store, element);
        None
    }
    /// Removes `objs`, live elements of its metamodel, with everything that goes with them; how
    /// many went. Refused, changing nothing the command keeps, when one of them cannot go.
    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<usize, CommandError> {
        let _ = (store, objs);
        Err(CommandError::about("ids", format!("{} elements cannot be removed yet", self.name())))
    }
    /// Sets the description of `obj`, an element of its metamodel; an empty text clears it.
    fn describe(&self, store: &mut Store, obj: Obj, text: &str) -> Result<(), CommandError> {
        let _ = (store, obj, text);
        Err(CommandError::about("element", format!("{} elements cannot be described yet", self.name())))
    }
    /// The description of `obj`, an element of its metamodel, as [`Notation::describe`] set it;
    /// nothing when it has none or the notation keeps none.
    fn description(&self, store: &Store, obj: Obj) -> String {
        let _ = (store, obj);
        String::new()
    }
}

/// The view of a project: the selection and the open diagram, which undo never touches, and the
/// notations the project holds, which answer for their diagrams and elements.
#[derive(Clone, Default)]
pub struct ProjectView {
    pub selection: Vec<Obj>,
    pub open_diagram: Option<Obj>,
    notations: &'static [&'static dyn Notation],
}

impl ProjectView {
    /// A view of a project holding `notations`.
    pub fn new(notations: &'static [&'static dyn Notation]) -> ProjectView {
        ProjectView { selection: Vec::new(), open_diagram: None, notations }
    }

    /// The notations the project holds.
    pub fn notations(&self) -> &'static [&'static dyn Notation] {
        self.notations
    }

    /// The notation whose diagrams are of `kind`.
    pub fn of_kind(&self, kind: &str) -> Option<&'static dyn Notation> {
        self.notations.iter().copied().find(|n| n.kinds().contains(&kind))
    }

    /// The notation whose metamodel has `class`.
    pub fn of_class(&self, class: ClassId) -> Option<&'static dyn Notation> {
        self.notations.iter().copied().find(|n| n.metamodel() == class.metamodel())
    }

    /// The notation of the element `obj`.
    pub fn of_element(&self, store: &Store, obj: Obj) -> Option<&'static dyn Notation> {
        store.class_of(obj).ok().and_then(|c| self.of_class(c))
    }
}

impl PartialEq for ProjectView {
    fn eq(&self, other: &ProjectView) -> bool {
        self.selection == other.selection && self.open_diagram == other.open_diagram && std::ptr::eq(self.notations, other.notations)
    }
}

impl std::fmt::Debug for ProjectView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.notations.iter().map(|n| n.name()).collect();
        f.debug_struct("ProjectView")
            .field("selection", &self.selection)
            .field("open_diagram", &self.open_diagram)
            .field("notations", &names)
            .finish()
    }
}

impl archforge_engine::View<Store> for ProjectView {
    fn retain_valid(&mut self, doc: &Store) {
        self.selection.retain(|o| doc.is_alive(*o));
        if self.open_diagram.is_some_and(|d| !doc.is_alive(d)) {
            self.open_diagram = None;
        }
    }
}

impl DiagramView for ProjectView {
    fn open_diagram(&self) -> Option<Obj> {
        self.open_diagram
    }

    fn set_open_diagram(&mut self, diagram: Option<Obj>) {
        self.open_diagram = diagram;
    }

    fn selection_mut(&mut self) -> &mut Vec<Obj> {
        &mut self.selection
    }

    fn kinds(&self) -> Vec<&'static str> {
        self.notations.iter().flat_map(|n| n.kinds().iter().copied()).collect()
    }

    /// Diagrams are members of the project's Model, beside what they show.
    fn home(&self, store: &Store) -> Result<Option<(Obj, PropId)>, String> {
        store
            .alive()
            .find(|o| store.class_of(*o).ok() == Some(archforge_core::MODEL))
            .map(|m| Some((m, archforge_core::model::MEMBER)))
            .ok_or_else(|| "the project has no model root".to_string())
    }

    fn is_of(&self, kind: &str, class: ClassId) -> bool {
        self.of_kind(kind).is_some_and(|n| n.is_of(kind, class))
    }

    fn needs_element(&self, kind: &str) -> bool {
        self.of_kind(kind).is_some_and(|n| n.needs_element(kind))
    }

    fn shows(&self, kind: &str, class: ClassId) -> bool {
        self.of_kind(kind).is_some_and(|n| n.shows(kind, class))
    }

    fn may_show(&self, store: &Store, diagram: Obj, element: Obj) -> bool {
        let kind = store.get(diagram, crate::diagram::KIND).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        self.of_kind(&kind).is_some_and(|n| n.may_show(store, diagram, element))
    }

    fn connects(&self, kind: &str, class: ClassId) -> bool {
        self.of_kind(kind).is_some_and(|n| n.connects(kind, class))
    }

    fn ends(&self, store: &Store, element: Obj) -> Option<(Obj, Obj)> {
        self.of_element(store, element)?.ends(store, element)
    }

    fn text_slots(&self, class: ClassId) -> &'static [&'static str] {
        self.of_class(class).map(|n| n.text_slots(class)).unwrap_or(&[])
    }

    fn default_size(&self, class: ClassId) -> (f64, f64) {
        self.of_class(class).map(|n| n.default_size(class)).unwrap_or(DEFAULT_SIZE)
    }
}

impl SkillHost for ProjectView {
    fn selection_mut(&mut self) -> &mut Vec<Obj> {
        &mut self.selection
    }

    /// Every notation's subjects, and a diagram (owner decision).
    fn skill_subjects(&self) -> Vec<ClassId> {
        self.notations.iter().flat_map(|n| n.skill_subjects().iter().copied()).chain([crate::DIAGRAM]).collect()
    }

    /// Every notation's kinds a skill may name.
    fn skill_kinds(&self) -> Vec<(ClassId, &'static str)> {
        self.notations.iter().flat_map(|n| n.skill_kinds().iter().copied()).collect()
    }

    /// Every notation's kinds that count when a skill is aimed but that no skill names.
    fn aim_only_kinds(&self) -> Vec<ClassId> {
        self.notations.iter().flat_map(|n| n.aim_only_kinds().iter().copied()).collect()
    }

    fn typed_classes(&self) -> Vec<(ClassId, PropId)> {
        self.notations.iter().filter_map(|n| n.typed_class()).collect()
    }

    fn skill_problem(&self, store: &Store, element: Obj) -> Option<&'static str> {
        self.of_element(store, element)?.skill_problem(store, element)
    }

    /// A diagram inherits the skills of the element it shows (owner decision); one that shows
    /// none, or a diagram or something inside itself (a crafted file), those of what holds it.
    fn level_above(&self, store: &Store, element: Obj) -> Option<Obj> {
        if store.class_of(element).ok() != Some(crate::DIAGRAM) {
            return None;
        }
        store
            .get(element, crate::diagram::ELEMENT)
            .ok()
            .and_then(|v| v.as_ref())
            .filter(|o| store.is_alive(*o) && store.class_of(*o).ok() != Some(crate::DIAGRAM) && !store.contains(element, *o))
    }
}

type C<'a> = Ctx<'a, Store, ProjectView>;
type R = Result<Json, CommandError>;

fn refuse(key: &str, why: impl Into<String>) -> CommandError {
    CommandError::about(key, why)
}

fn fail(e: Invalid) -> CommandError {
    CommandError::new(e.to_string())
}

fn short(s: &str) -> String {
    s.chars().take(40).collect()
}

/// The elements the ids under `key` name, each once.
fn id_list(store: &Store, p: &Params, key: &str) -> Result<Vec<Obj>, CommandError> {
    let items = p.get(key).and_then(Json::as_array).map(Vec::as_slice).unwrap_or_default();
    if items.len() > archforge_mof::store::MAX_LIST {
        return Err(refuse(key, format!("at most {} ids", archforge_mof::store::MAX_LIST)));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        let id = v.as_str().ok_or_else(|| refuse(key, "every id must be a string"))?;
        let uuid = Uuid::parse(id).ok_or_else(|| refuse(key, format!("{:?} is not an id", short(id))))?;
        let obj = store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refuse(key, format!("there is no element {}", short(id))))?;
        if !seen.insert(obj) {
            return Err(refuse(key, format!("{id} is listed twice")));
        }
        out.push(obj);
    }
    Ok(out)
}

fn uuid_of(store: &Store, obj: Obj) -> Result<String, CommandError> {
    store.uuid(obj).map(|u| u.to_string()).map_err(fail)
}

/// Whether the project holds an element of one of its notations.
fn has_elements(f: &Facts<'_, Store, ProjectView>) -> Result<(), &'static str> {
    if f.doc.alive().any(|o| f.view.of_element(f.doc, o).is_some()) { Ok(()) } else { Err(NO_ELEMENTS) }
}

/// Why removing and describing are not available.
pub const NO_ELEMENTS: &str = "There is nothing to remove or describe yet.";

fn view_select(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(ctx.doc, p, "ids")?;
    ctx.view.selection = ids;
    Ok(Json::Null)
}

/// Removes elements of any notation at once (the selection's delete): each notation removes its
/// own, in the order the project holds them.
fn element_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(ctx.doc, p, "ids")?;
    if ids.is_empty() {
        return Err(refuse("ids", "nothing to remove"));
    }
    let mut by_notation: Vec<Vec<Obj>> = vec![Vec::new(); ctx.view.notations.len()];
    for o in &ids {
        let class = ctx.doc.class_of(*o).map_err(fail)?;
        let Some(at) = ctx.view.notations.iter().position(|n| n.metamodel() == class.metamodel()) else {
            return Err(refuse("ids", format!("{} is not an element of a notation", uuid_of(ctx.doc, *o)?)));
        };
        by_notation[at].push(*o);
    }
    let mut gone = 0;
    for (n, objs) in ctx.view.notations.iter().zip(by_notation) {
        // What an earlier notation's removal took along is gone already.
        let objs: Vec<Obj> = objs.into_iter().filter(|o| ctx.doc.is_alive(*o)).collect();
        if !objs.is_empty() {
            gone += n.remove(ctx.doc, &objs)?;
        }
    }
    Ok(json!({ "removed": gone }))
}

/// `element.describe`: sets an element's description through its notation, or clears it.
fn element_describe(ctx: &mut C<'_>, p: &Params) -> R {
    let id = p.str("element").unwrap_or_default();
    let uuid = Uuid::parse(id).ok_or_else(|| refuse("element", format!("{:?} is not an id", short(id))))?;
    let o = ctx.doc.by_uuid(uuid).filter(|o| ctx.doc.is_alive(*o)).ok_or_else(|| refuse("element", format!("there is no element {}", short(id))))?;
    let n = ctx.view.of_element(ctx.doc, o).ok_or_else(|| refuse("element", format!("{id} is not an element of a notation")))?;
    n.describe(ctx.doc, o, p.str("text").unwrap_or_default())?;
    Ok(json!({ "id": uuid_of(ctx.doc, o)? }))
}

/// The shared commands: select, remove and describe elements of any notation.
pub fn specs() -> Vec<CommandSpec<Store, ProjectView>> {
    vec![
        cmd!(noundo "view.select", "Select", &["View"], None, "ids: array = []; the ids selected", always, view_select),
        cmd!("element.remove", "Remove", &["Edit"], Some("Delete"), "ids: array; the ids of what goes", has_elements, element_remove),
        cmd!(
            "element.describe",
            "Describe",
            &["Edit"],
            None,
            "element: string; the id of an ER element, or of a UML package, classifier, attribute, operation, literal or relationship\ntext: string = \"\"; its description, or nothing to clear it",
            has_elements,
            element_describe
        ),
    ]
}
