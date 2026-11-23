//! One view for a project of several notations: the selection and the open
//! diagram, and the notations the project holds, each a [`Notation`] that answers for its own
//! diagram kinds and for the elements of its metamodel. The diagram commands and the skill
//! commands ask the view; the view asks the notation that owns a diagram's kind or an element's
//! class. The shared commands here select, remove and describe elements of any notation.

use std::collections::BTreeSet;

use archforge_core::skills::SkillHost;
use archforge_engine::{CommandError, CommandSpec, Ctx, Facts, Params, Reason, always, cmd};
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
    /// Whether the first diagram of `kind` of an element of `class` becomes that element's default
    /// as it is made, a binding naming it (Decision 10, AR-579). None does by default.
    fn binds_default(&self, kind: &str, class: ClassId) -> bool {
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
    /// The element that owns `element` as a nested member, its node tied to `element`'s by a line
    /// ending in ⊕ at it where a diagram shows both, worked out from the model, never stored: a
    /// nested classifier's owner (AR-132). None for an element nested in nothing.
    fn nested_in(&self, store: &Store, element: Obj) -> Option<Obj> {
        let _ = (store, element);
        None
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
    /// What a click on a figure of `element` selects: an n-ary association's end, whose leg was
    /// clicked, its association, the diamond and every leg being the one element (AR-128). The
    /// element itself by default.
    fn selects_as(&self, store: &Store, element: Obj) -> Obj {
        let _ = store;
        element
    }
    /// The element whose line draws selected while `element` is selected, beside `element`'s own
    /// figure: a qualifier's association, the qualifier sitting in its line's box, so it marks the
    /// line, as a selected member marks its own card (AR-127). The element's card, where it has one
    /// too, is not marked. None by default.
    fn line_selected_with(&self, store: &Store, element: Obj) -> Option<Obj> {
        let _ = (store, element);
        None
    }
    /// Whether a selected line of `element` shows the two end handles that move its ends: by
    /// default when no node shows the element, a card's element having no line of its own.
    fn line_handles(&self, store: &Store, element: Obj) -> bool {
        no_node_shows(store, element)
    }
    /// Whether the notation draws `element` as a card on a diagram of one of its kinds, whether or
    /// not one shows it now: by default when one of its kinds shows the element's class as nodes.
    fn drawn_as_card(&self, store: &Store, element: Obj) -> bool {
        store.class_of(element).is_ok_and(|class| self.kinds().iter().any(|k| self.shows(k, class)))
    }
    /// Whether a node of `element` is drawn, and hit, under the diagram's other figures, such as a
    /// card its preview fills, so the figures placed over it stay in sight. None is by default.
    fn drawn_behind(&self, store: &Store, element: Obj) -> bool {
        let _ = (store, element);
        false
    }
    /// Whether a node of `element` holds a label drawn with the lines it names rather than a card
    /// of its own, which a press looks through where it draws nothing. None does by default.
    fn is_label(&self, store: &Store, element: Obj) -> bool {
        let _ = (store, element);
        false
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
    /// Removes `objs`, live elements of its metamodel, with everything that goes with them; what
    /// went and what a removed package handed up ([`Removed`]). Refused, changing nothing the
    /// command keeps, when one of them cannot go; by default every one, the reason typed.
    fn remove(&self, store: &mut Store, objs: &[Obj]) -> Result<Removed, CommandError> {
        let _ = (store, objs);
        let notation = self.name();
        Err(CommandError::about("ids", format!("{notation} elements cannot be removed yet")).because(Reason::NotRemovable { notation }))
    }
    /// Sets the description of `obj`, an element of its metamodel; an empty text clears it. By
    /// default refused, the reason typed.
    fn describe(&self, store: &mut Store, obj: Obj, text: &str) -> Result<(), CommandError> {
        let _ = (store, obj, text);
        let notation = self.name();
        Err(CommandError::about("element", format!("{notation} elements cannot be described yet")).because(Reason::NotDescribable { notation }))
    }
    /// The description of `obj`, an element of its metamodel, as [`Notation::describe`] set it;
    /// nothing when it has none or the notation keeps none.
    fn description(&self, store: &Store, obj: Obj) -> String {
        let _ = (store, obj);
        String::new()
    }
    /// What is wrong in `store` with the notation's own elements as a project opens: reported,
    /// never repaired, so the model opens as it was stored, in an order of the notation's choosing
    /// that does not change with the handles. None by default.
    fn open_problems(&self, store: &Store) -> Vec<OpenProblem> {
        let _ = store;
        Vec::new()
    }
}

/// Whether no diagram node shows `element`: whether a selected line of it shows its end handles,
/// unless its notation says otherwise ([`Notation::line_handles`]).
fn no_node_shows(store: &Store, element: Obj) -> bool {
    crate::views_of(store, element).2.is_empty()
}

/// Something wrong in a project's model that a notation reports as the project opens
/// ([`Notation::open_problems`]), told the same way by every notation: which one, the element it
/// concerns, a code for it, the objects it names and what it is in English.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenProblem {
    /// The name of the notation that reports it, as [`Notation::name`] gives it.
    pub notation: &'static str,
    /// The element the problem concerns; none when it concerns no live element.
    pub owner: Option<Obj>,
    /// The problem's kind, in the notation's own words, the same in every version: `snake_case`.
    pub code: &'static str,
    /// The other objects the problem names, in the notation's order.
    pub objects: Vec<Obj>,
    /// The problem in English, for the command line; an interface says it in its own words.
    pub message: String,
}

/// What a notation's removal took away and handed up (AR-675), which `element.remove` answers
/// beside `removed`: how many objects went in all, how many elements and diagrams a removed package
/// handed up are still in the model once it is done, and how many default-diagram bindings and
/// relationships went. A notation counts its relationships by its own metamodel, every kind in one
/// field, so a kind removed with a package later lands there without a field of its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Removed {
    /// How many objects went, the bindings and the relationships among them.
    pub gone: usize,
    /// How many elements a removed package handed up are still in the model once the removal is done.
    pub moved_elements: usize,
    /// How many diagrams a removed package handed up are still in the model once the removal is done.
    pub moved_diagrams: usize,
    /// How many default-diagram bindings went.
    pub bindings: usize,
    /// How many relationships went, of every kind the notation has, each once.
    pub relationships: usize,
}

impl std::ops::AddAssign for Removed {
    /// Adds what another notation's removal in the same call took and handed up.
    fn add_assign(&mut self, other: Removed) {
        self.gone = self.gone.saturating_add(other.gone);
        self.moved_elements = self.moved_elements.saturating_add(other.moved_elements);
        self.moved_diagrams = self.moved_diagrams.saturating_add(other.moved_diagrams);
        self.bindings = self.bindings.saturating_add(other.bindings);
        self.relationships = self.relationships.saturating_add(other.relationships);
    }
}

/// The default-diagram bindings and the relationships that stand before a removal starts, for
/// [`Standing::removed`] to count those it took once it is done, whatever step of it took them. It
/// looks at every live object once: a removal's own steps, and those later kinds add, need not
/// count what they take.
pub struct Standing {
    bindings: Vec<Obj>,
    relationships: Vec<Obj>,
}

impl Standing {
    /// What stands in `store` now: its live bindings, and its live objects of a class `relationship`
    /// holds for, the notation's relationships.
    pub fn before(store: &Store, relationship: impl Fn(ClassId) -> bool) -> Standing {
        let (mut bindings, mut relationships) = (Vec::new(), Vec::new());
        for o in store.alive() {
            match store.class_of(o) {
                Ok(c) if c == crate::DEFAULT_DIAGRAM_BINDING => bindings.push(o),
                Ok(c) if relationship(c) => relationships.push(o),
                _ => {}
            }
        }
        Standing { bindings, relationships }
    }

    /// What the removal took, `gone` objects in all: the bindings and the relationships that stood
    /// before it and are gone from `store` now. It hands nothing up; a notation whose removal does
    /// sets the moved counts itself.
    pub fn removed(&self, store: &Store, gone: usize) -> Removed {
        let went = |objs: &[Obj]| objs.iter().filter(|o| !store.is_alive(**o)).count();
        Removed { gone, bindings: went(&self.bindings), relationships: went(&self.relationships), ..Removed::default() }
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

    /// What a click on a figure of `element` selects, as its notation answers
    /// ([`Notation::selects_as`]); the element itself when no notation holds it.
    pub fn selects_as(&self, store: &Store, element: Obj) -> Obj {
        self.of_element(store, element).map_or(element, |n| n.selects_as(store, element))
    }

    /// The element whose line draws selected while `element` is selected, as its notation answers
    /// ([`Notation::line_selected_with`]); none when no notation holds it.
    pub fn line_selected_with(&self, store: &Store, element: Obj) -> Option<Obj> {
        self.of_element(store, element)?.line_selected_with(store, element)
    }

    /// Whether a selected line of `element` shows its end handles, as its notation answers
    /// ([`Notation::line_handles`]); by the default rule, when no node shows it, where no notation
    /// holds it.
    pub fn line_handles(&self, store: &Store, element: Obj) -> bool {
        self.of_element(store, element).map_or_else(|| no_node_shows(store, element), |n| n.line_handles(store, element))
    }

    /// Whether `element`'s notation draws it as a card ([`Notation::drawn_as_card`]); not when no
    /// notation holds it.
    pub fn drawn_as_card(&self, store: &Store, element: Obj) -> bool {
        self.of_element(store, element).is_some_and(|n| n.drawn_as_card(store, element))
    }

    /// Whether diagram node `node` is drawn, and hit, under the diagram's other figures, as the
    /// notation of the live element it shows answers ([`Notation::drawn_behind`]); not when it
    /// shows none.
    pub fn node_drawn_behind(&self, store: &Store, node: Obj) -> bool {
        self.node_element(store, node).is_some_and(|(n, e)| n.drawn_behind(store, e))
    }

    /// Whether diagram node `node` holds a label, as the notation of the live element it shows
    /// answers ([`Notation::is_label`]); not when it shows none.
    pub fn node_is_label(&self, store: &Store, node: Obj) -> bool {
        self.node_element(store, node).is_some_and(|(n, e)| n.is_label(store, e))
    }

    /// The live element diagram node `node` shows, with its notation, when one holds it.
    fn node_element(&self, store: &Store, node: Obj) -> Option<(&'static dyn Notation, Obj)> {
        let element = store.get(node, crate::node::ELEMENT).ok().and_then(|v| v.as_ref())?;
        Some((self.of_element(store, element)?, element))
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

    /// As the notation whose diagrams are of `kind` says; none for a kind no notation has.
    fn binds_default(&self, kind: &str, class: ClassId) -> bool {
        self.of_kind(kind).is_some_and(|n| n.binds_default(kind, class))
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

/// The kind the shared commands' refusals of an id name: an element of any kind.
pub const ELEMENT: &str = "element";
/// The kind a [`Reason::NotA`] names for an element of no notation the project holds, as a diagram.
pub const NOTATION_ELEMENT: &str = "element of a notation";

/// The live element `id` names, given under `key`; refused with its reason typed: no id, or one
/// malformed or of no element.
fn element_in(store: &Store, key: &str, id: &str) -> Result<Obj, CommandError> {
    let refused =
        |said: String| refuse(key, said).because(if id.is_empty() { Reason::IdEmpty { kind: ELEMENT } } else { Reason::NoElement { id: short(id) } });
    let uuid = Uuid::parse(id).ok_or_else(|| refused(format!("{:?} is not an id", short(id))))?;
    store.by_uuid(uuid).filter(|o| store.is_alive(*o)).ok_or_else(|| refused(format!("there is no element {}", short(id))))
}

/// The refusal under `key` of `o`, an element of no notation the project holds, said as `said`.
fn of_no_notation(store: &Store, key: &str, o: Obj, said: String) -> CommandError {
    refuse(key, said).because(Reason::NotA { kind: NOTATION_ELEMENT, name: crate::commands::name_or_id(store, o) })
}

/// The elements the ids under `key` name, each once; every refusal typed: an item that is no
/// string, no id, of no element, or given twice. A list over the store's limit is refused untyped,
/// as the engine's limit on a call's size refuses one that long first.
fn id_list(store: &Store, p: &Params, key: &str) -> Result<Vec<Obj>, CommandError> {
    let items = p.get(key).and_then(Json::as_array).map(Vec::as_slice).unwrap_or_default();
    if items.len() > archforge_mof::store::MAX_LIST {
        return Err(refuse(key, format!("at most {} ids", archforge_mof::store::MAX_LIST)));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        let id = v.as_str().ok_or_else(|| refuse(key, "every id must be a string").because(Reason::IdsExpected { kind: ELEMENT }))?;
        let obj = element_in(store, key, id)?;
        if !seen.insert(obj) {
            return Err(refuse(key, format!("{id} is listed twice")).because(Reason::NamedTwice { kind: ELEMENT, name: short(id) }));
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
/// own, in the order the project holds them. It answers how many objects went (`removed`) and, added
/// up over the notations ([`Removed`]), how many elements and diagrams a removed package handed up
/// (`moved_elements`, `moved_diagrams`) and how many bindings and relationships went (AR-675). An
/// empty list, and an element of no notation, refused with the reason typed.
fn element_remove(ctx: &mut C<'_>, p: &Params) -> R {
    let ids = id_list(ctx.doc, p, "ids")?;
    if ids.is_empty() {
        return Err(refuse("ids", "nothing to remove").because(Reason::TooFew { kind: ELEMENT, min: 1 }));
    }
    let mut by_notation: Vec<Vec<Obj>> = vec![Vec::new(); ctx.view.notations.len()];
    for o in &ids {
        let class = ctx.doc.class_of(*o).map_err(fail)?;
        let Some(at) = ctx.view.notations.iter().position(|n| n.metamodel() == class.metamodel()) else {
            return Err(of_no_notation(ctx.doc, "ids", *o, format!("{} is not an element of a notation", uuid_of(ctx.doc, *o)?)));
        };
        by_notation[at].push(*o);
    }
    let mut removed = Removed::default();
    for (n, objs) in ctx.view.notations.iter().zip(by_notation) {
        // What an earlier notation's removal took along is gone already.
        let objs: Vec<Obj> = objs.into_iter().filter(|o| ctx.doc.is_alive(*o)).collect();
        if !objs.is_empty() {
            removed += n.remove(ctx.doc, &objs)?;
        }
    }
    Ok(json!({
        "removed": removed.gone,
        "moved_elements": removed.moved_elements,
        "moved_diagrams": removed.moved_diagrams,
        "bindings": removed.bindings,
        "relationships": removed.relationships,
    }))
}

/// `element.describe`: sets an element's description through its notation, or clears it. No id,
/// one of no element and an element of no notation refused with the reason typed.
fn element_describe(ctx: &mut C<'_>, p: &Params) -> R {
    let id = p.str("element").unwrap_or_default();
    let o = element_in(ctx.doc, "element", id)?;
    let n = ctx.view.of_element(ctx.doc, o).ok_or_else(|| of_no_notation(ctx.doc, "element", o, format!("{id} is not an element of a notation")))?;
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
