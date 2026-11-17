//! The notations a project holds, composed: each metamodel brings its slabs and the tables its
//! objects are stored in ([`PARTS`]), each notation its diagrams, skills and element hooks
//! ([`NOTATIONS`]), and a project is made from the lists, so one store, one database, one view and
//! one command table hold ER and UML side by side. The shell, the host and the agent's tools make
//! projects here and name no notation for it. Pure: builds for the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

pub mod differences;
mod labels;

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use archforge_diagram::default_diagram_binding::{DEFAULT_DIAGRAM, OWNER_ELEMENT};
use archforge_diagram::{DiagramView, Notation, ProjectView};
use archforge_mof::rows::{Cell, Rows};
use archforge_mof::schema::{Junction, Table};
use archforge_mof::{ClassId, CompositeSlabs, MetaError, Metamodel, NameKind, Slabs, Store, Universe, Uuid};

/// What a metamodel brings to a project's store.
pub struct Part {
    /// Its name, for messages.
    pub name: &'static str,
    pub metamodel: fn() -> Result<Arc<Metamodel>, MetaError>,
    /// The metamodel's index, and its objects' slabs, fresh.
    pub index: u16,
    pub slabs: fn() -> Box<dyn Slabs>,
    /// The tables and list tables its objects are stored in.
    pub tables: &'static [Table],
    pub junctions: &'static [Junction],
    /// The kind each of its classes is told apart within by name, among its owner's elements, for
    /// the check a project gets as it opens ([`name_kind`]); none for names that never clash
    /// there.
    pub name_kind: fn(ClassId) -> Option<NameKind>,
}

/// One kind for every named element of a notation: two named alike in one owner always clash.
const ONE_KIND: Option<NameKind> = Some(NameKind::Group(""));

/// What a project holds: the shared core and the diagrams first, then the notations. Their tables
/// are stored in this order, so a metamodel joins at the end.
pub const PARTS: &[Part] = &[
    Part {
        name: "core",
        metamodel: archforge_core::metamodel,
        index: archforge_core::INDEX,
        slabs: || Box::new(archforge_core::CoreSlabs::default()),
        tables: archforge_core::TABLES,
        junctions: archforge_core::JUNCTIONS,
        // Skills share the model's one list but are named uniquely per subject, by their own rule
        // (`archforge_core::skills::first_duplicate`).
        name_kind: |c| if c == archforge_core::SKILL { None } else { ONE_KIND },
    },
    Part {
        name: "diagram",
        metamodel: archforge_diagram::metamodel,
        index: archforge_diagram::INDEX,
        slabs: || Box::new(archforge_diagram::DiagramSlabs::default()),
        tables: archforge_diagram::TABLES,
        junctions: archforge_diagram::JUNCTIONS,
        // A diagram's name is a label: two diagrams may be named alike, as `diagram.add`, `diagram.rename`
        // and a package's own diagram, named like its package, let them be (AR-208).
        name_kind: |c| if c == archforge_diagram::DIAGRAM { None } else { ONE_KIND },
    },
    Part {
        name: "er",
        metamodel: archforge_er::metamodel,
        index: archforge_er::INDEX,
        slabs: || Box::new(archforge_er::ErSlabs::default()),
        tables: archforge_er::TABLES,
        junctions: archforge_er::JUNCTIONS,
        // ER's own rule, the one its commands ask too (AR-209).
        name_kind: archforge_er::names::kind,
    },
    Part {
        name: "uml",
        metamodel: archforge_uml::metamodel,
        index: archforge_uml::INDEX,
        slabs: || Box::new(archforge_uml::UmlSlabs::default()),
        tables: archforge_uml::TABLES,
        junctions: archforge_uml::JUNCTIONS,
        // UML's own rule, the one its commands ask too.
        name_kind: archforge_uml::names::kind,
    },
];

/// The notations a project's view asks about their diagrams and elements.
pub static NOTATIONS: &[&dyn Notation] = &[&archforge_er::ErNotation, &archforge_uml::UmlNotation];

/// A project's session: its store, the view and the commands.
pub type Session = archforge_engine::Session<Store, ProjectView>;

/// The kind an element of `class` is told apart within by its name among its owner's elements, by
/// the rule of the notation the class is of ([`Part::name_kind`]): the check a project gets as it
/// opens refuses only two elements of one kind named alike, whatever the case, in one owner.
pub fn name_kind(class: ClassId) -> Option<NameKind> {
    PARTS.iter().find(|p| p.index == class.metamodel()).map_or(ONE_KIND, |p| (p.name_kind)(class))
}

/// A view of a project holding every notation.
pub fn view() -> ProjectView {
    ProjectView::new(NOTATIONS)
}

/// The universe of every notation a project holds.
pub fn universe() -> Result<Arc<Universe>, MetaError> {
    let metamodels = PARTS.iter().map(|n| (n.metamodel)()).collect::<Result<Vec<_>, _>>()?;
    Ok(Arc::new(Universe::new(metamodels)?))
}

/// The slabs of every notation, for a store over [`universe`].
fn slabs() -> Result<CompositeSlabs, String> {
    PARTS.iter().try_fold(CompositeSlabs::new(), |all, n| all.with(n.index, (n.slabs)()))
}

/// An empty store over every notation, with the host's ids.
pub fn store(ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Store, String> {
    let universe = universe().map_err(|e| e.to_string())?;
    Ok(Store::new(universe, Box::new(slabs()?), ids))
}

/// The tables a project stores its model in, every metamodel's in the order of [`PARTS`]. The
/// shell writes rows for them and the host stores them, so both take them from here.
pub fn storage() -> (Vec<Table>, Vec<Junction>) {
    let tables = PARTS.iter().flat_map(|n| n.tables).copied().collect();
    let junctions = PARTS.iter().flat_map(|n| n.junctions).copied().collect();
    (tables, junctions)
}

/// A diagram of a kind no notation of the project knows, such as one an earlier version stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDiagram {
    /// Its id (nil when the rows hold none), and its name, empty when it has none or the rows
    /// checked do not hold it.
    pub id: Uuid,
    pub name: String,
    /// Its kind as stored, empty for none.
    pub kind: String,
}

/// Why a model is refused as it opens: it holds diagrams of kinds no notation knows. It is never
/// shown in part or under another kind, and nothing in it is migrated, renamed or removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownKinds {
    /// Every such diagram, in the order read; never empty.
    pub diagrams: Vec<UnknownDiagram>,
    /// The kinds the project's notations know, in the order of [`NOTATIONS`].
    pub accepted: Vec<&'static str>,
}

impl fmt::Display for UnknownKinds {
    /// The reason in English, naming the first diagram; a screen words it through `tr`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(first) = self.diagrams.first() {
            let named = if first.name.is_empty() { String::new() } else { format!("{:?} ", first.name) };
            write!(f, "the diagram {named}({}) is of the kind {:?}, which no notation knows", first.id, first.kind)?;
        }
        let more = self.diagrams.len().saturating_sub(1);
        if more > 0 {
            write!(f, ", and so are {more} more")?;
        }
        write!(f, "; the kinds known are {}", self.accepted.join(", "))
    }
}

impl std::error::Error for UnknownKinds {}

/// Why a project's rows do not open as a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// Diagrams of kinds no notation knows: refused before anything is built from the rows.
    UnknownKinds(UnknownKinds),
    /// Default-diagram bindings that name nothing the project holds: refused before anything is
    /// built from the rows, never cleared or replaced. Never empty.
    BrokenBindings(Vec<BrokenBinding>),
    /// Any other reason, in English as the store gives it.
    Unreadable(String),
}

impl fmt::Display for OpenError {
    /// The reason in English; a screen words it through `tr`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::UnknownKinds(e) => e.fmt(f),
            OpenError::BrokenBindings(all) => {
                if let Some(first) = all.first() {
                    first.fmt(f)?;
                }
                match all.len().saturating_sub(1) {
                    0 => Ok(()),
                    more => write!(f, ", and so do {more} more"),
                }
            }
            OpenError::Unreadable(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<String> for OpenError {
    /// A reason the store or the commands give in English.
    fn from(why: String) -> OpenError {
        OpenError::Unreadable(why)
    }
}

impl From<OpenError> for String {
    /// The reason in English, for callers that show text alone.
    fn from(e: OpenError) -> String {
        e.to_string()
    }
}

/// The table a project stores its diagrams in, under this name in every shape its tables have had.
const DIAGRAMS: &str = "diagram_diagram";

/// Refused when any of `diagrams` is of a kind none of [`NOTATIONS`] knows, naming them all.
fn known_kinds(diagrams: impl Iterator<Item = UnknownDiagram>) -> Result<(), UnknownKinds> {
    let accepted = view().kinds();
    let diagrams: Vec<UnknownDiagram> = diagrams.filter(|d| !accepted.contains(&d.kind.as_str())).collect();
    if diagrams.is_empty() {
        return Ok(());
    }
    Err(UnknownKinds { diagrams, accepted })
}

/// Refuses a store holding diagrams whose kinds none of [`NOTATIONS`] knows.
pub fn check_kinds(store: &Store) -> Result<(), UnknownKinds> {
    let text = |d, prop| store.get(d, prop).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    known_kinds(store.alive().filter(|o| store.class_of(*o).ok() == Some(archforge_diagram::DIAGRAM)).map(|d| UnknownDiagram {
        id: store.uuid(d).unwrap_or(Uuid([0; 16])),
        name: text(d, archforge_diagram::diagram::NAME),
        kind: text(d, archforge_diagram::diagram::KIND),
    }))
}

/// [`check_kinds`] over rows read in the shape `tables`, before anything is built from them:
/// only the diagram table's `id`, `kind` and, when the shape has it, `name` columns are read, found
/// by name, so the rows of an earlier shape, or of the diagram table alone, are checked before the
/// project's tables are brought up to date. A shape without the diagram table holds no diagram; a
/// diagram whose kind column is missing or holds no text is of no kind any notation knows.
pub fn check_kinds_in_rows(rows: &Rows, tables: &[Table]) -> Result<(), UnknownKinds> {
    let Some((at, table)) = tables.iter().enumerate().find(|(_, t)| t.name == DIAGRAMS) else { return Ok(()) };
    let column = |name: &str| table.columns.iter().position(|c| c.name == name);
    let (id, name, kind) = (column("id"), column("name"), column("kind"));
    let text = |row: &[Cell], at: Option<usize>| match at.and_then(|i| row.get(i)) {
        Some(Cell::Text(t)) => t.to_string(),
        _ => String::new(),
    };
    known_kinds(rows.tables.get(at).into_iter().flatten().map(|row| UnknownDiagram {
        id: match id.and_then(|i| row.get(i)) {
            Some(Cell::Id(u)) => *u,
            _ => Uuid([0; 16]),
        },
        name: text(row, name),
        kind: text(row, kind),
    }))
}

/// The end of a default-diagram binding that names nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingEnd {
    /// `ownerElement`, the element whose default the binding names.
    Owner,
    /// `defaultDiagram`, the diagram it names: a dead target.
    Diagram,
}

/// A default-diagram binding whose `end` names nothing the project holds (Decision 12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenBinding {
    /// The binding's id.
    pub binding: Uuid,
    pub end: BindingEnd,
    /// The id the end names, none when it names none.
    pub names: Option<Uuid>,
}

impl fmt::Display for BrokenBinding {
    /// The reason in English; a screen words it through `tr`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let end = match self.end {
            BindingEnd::Owner => "owner",
            BindingEnd::Diagram => "default diagram",
        };
        match self.names {
            Some(id) => write!(f, "the default-diagram binding {} names the {end} {id}, which the project does not hold", self.binding),
            None => write!(f, "the default-diagram binding {} names no {end}", self.binding),
        }
    }
}

impl std::error::Error for BrokenBinding {}

/// The default-diagram bindings in rows read in the shape `tables` whose owner or diagram is no
/// row of any table, or which name none, in the order read, an owner before a diagram; read before
/// anything is built from the rows. Rows without the binding table, or of a shape without its
/// ends, hold none.
pub fn check_bindings_in_rows(rows: &Rows, tables: &[Table]) -> Result<(), Vec<BrokenBinding>> {
    let Some((at, table)) = tables.iter().enumerate().find(|(_, t)| t.class == archforge_diagram::DEFAULT_DIAGRAM_BINDING) else { return Ok(()) };
    let column = |prop| table.columns.iter().position(|c| c.prop == Some(prop));
    let (Some(owner), Some(diagram)) = (column(OWNER_ELEMENT), column(DEFAULT_DIAGRAM)) else { return Ok(()) };
    let id_of = |t: &Table| t.columns.iter().position(|c| c.prop.is_none());
    let named = |row: &[Cell], i: Option<usize>| match i.and_then(|i| row.get(i)) {
        Some(Cell::Id(u)) => Some(*u),
        _ => None,
    };
    let bindings = rows.tables.get(at).map(Vec::as_slice).unwrap_or_default();
    let ends = [(BindingEnd::Owner, owner), (BindingEnd::Diagram, diagram)];
    // Only the ids the bindings name are looked for among every table's rows.
    let wanted: HashSet<Uuid> = bindings.iter().flat_map(|row| ends.map(|(_, i)| named(row, Some(i)))).flatten().collect();
    let held: HashSet<Uuid> = tables
        .iter()
        .zip(&rows.tables)
        .flat_map(|(t, table_rows)| table_rows.iter().filter_map(move |row| named(row, id_of(t))))
        .filter(|u| wanted.contains(u))
        .collect();
    // A row without an id is refused as the rows are built.
    let broken: Vec<BrokenBinding> = bindings
        .iter()
        .filter_map(|row| named(row, id_of(table)).map(|binding| ends.map(|(end, i)| BrokenBinding { binding, end, names: named(row, Some(i)) })))
        .flatten()
        .filter(|b| !b.names.is_some_and(|u| held.contains(&u)))
        .collect();
    if broken.is_empty() { Ok(()) } else { Err(broken) }
}

/// The commands: the edit area, the shared element commands, every notation's own, the diagrams'
/// and the skills'.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Store, ProjectView>>, String> {
    let mut specs = archforge_engine::cmd::edit::specs();
    specs.extend(archforge_diagram::project::specs());
    specs.extend(archforge_er::commands::specs());
    specs.extend(archforge_uml::commands::specs());
    specs.extend(archforge_uml::members::specs());
    specs.extend(archforge_uml::relations::specs());
    specs.extend(archforge_uml::constraints::specs());
    specs.extend(archforge_uml::generalization_sets::specs());
    specs.extend(archforge_uml::qualifiers::specs());
    specs.extend(archforge_uml::nary::specs());
    specs.extend(archforge_uml::nesting::specs());
    specs.extend(archforge_uml::templates::specs());
    specs.extend(archforge_uml::profiles::specs());
    specs.extend(archforge_uml::bindings::specs());
    specs.extend(archforge_uml::applications::specs());
    specs.extend(archforge_diagram::commands::specs::<ProjectView>());
    specs.extend(archforge_core::skills::specs::<ProjectView>());
    archforge_engine::Registry::new(specs)
}

/// The id counter a new project's session starts at: ids below it are the store's own.
const FIRST_COUNTER: u64 = 1;

/// The session of a project that has nothing yet: the project's Model root named `name`. `ids`
/// mints the elements' ids; the host supplies it, since only the host may read the clock.
pub fn new_project_session(name: &str, ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Session, String> {
    let registry = registry()?;
    let mut st = store(ids)?;
    let model = st.create(archforge_core::MODEL).map_err(|e| e.to_string())?;
    st.set(model, archforge_core::named_element::NAME, archforge_mof::Value::str(name)).map_err(|e| e.to_string())?;
    Ok(Session::with_view(registry, st, FIRST_COUNTER, archforge_engine::Options::default(), view()))
}

/// The session of a project read back from its rows: the store rebuilt from them and checked,
/// saved as it is; refused, with nothing built, when a diagram is of a kind no notation knows
/// ([`check_kinds_in_rows`]) or a binding names nothing ([`check_bindings_in_rows`]). `ids` mints
/// the ids of the elements made from now on.
pub fn session_from_rows(rows: &archforge_mof::rows::Rows, ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Session, OpenError> {
    session_from_rows_repaired(rows, ids, |_| {})
}

/// [`session_from_rows`] with `repair` run on the model read before the session starts: for a
/// layout the reader puts right (a box too small for what it shows) without an undo step; it is
/// written with the next change.
pub fn session_from_rows_repaired(
    rows: &archforge_mof::rows::Rows,
    ids: Box<dyn FnMut() -> Uuid + Send>,
    repair: impl FnOnce(&mut Store),
) -> Result<Session, OpenError> {
    session_from_rows_diagnosed(rows, ids, repair).map(|opened| opened.session)
}

/// A project's session as it opens, with what is wrong with its default-diagram bindings.
pub struct Opened {
    pub session: Session,
    /// The binding problems of the session's store, after any repair, as
    /// [`archforge_uml::defaults::problems`] gives them: reported, never repaired, so the bindings
    /// open as they were stored.
    pub problems: Vec<archforge_uml::defaults::Diagnosis>,
}

/// [`session_from_rows_repaired`], with the binding problems of the model opened. A version opened
/// to be looked at reports those of its own revision. No repair changes a binding, its ids compared
/// whatever store the repair leaves: one that would is refused.
pub fn session_from_rows_diagnosed(
    rows: &archforge_mof::rows::Rows,
    ids: Box<dyn FnMut() -> Uuid + Send>,
    repair: impl FnOnce(&mut Store),
) -> Result<Opened, OpenError> {
    let registry = registry()?;
    let (tables, junctions) = storage();
    // A diagram no notation knows refuses the model before anything is built from it.
    check_kinds_in_rows(rows, &tables).map_err(OpenError::UnknownKinds)?;
    // So does a binding that names nothing; one that names the wrong thing is reported.
    check_bindings_in_rows(rows, &tables).map_err(OpenError::BrokenBindings)?;
    // Built without an undo log, so a model of any size the read budgets let through opens.
    let mut st = archforge_mof::rows::restore_into_empty(store(ids)?, rows, &tables, &junctions).map_err(|e| e.to_string())?;
    // Every binding and its ends by their ids, in id order: a repair may hand back another store,
    // whose handles name other objects.
    let bound = |st: &Store| {
        let end = |b, prop| st.get(b, prop).ok().and_then(|v| v.as_ref()).and_then(|o| st.uuid(o).ok());
        let mut all: Vec<_> = archforge_diagram::defaults::bindings(st)
            .into_iter()
            .map(|b| (st.uuid(b).ok(), end(b, OWNER_ELEMENT), end(b, DEFAULT_DIAGRAM)))
            .collect();
        all.sort();
        all
    };
    let stored = bound(&st);
    repair(&mut st);
    // The repair changes layout alone; a kind or a binding it would change is refused all the same.
    check_kinds(&st).map_err(OpenError::UnknownKinds)?;
    if bound(&st) != stored {
        return Err(OpenError::Unreadable("the repair as the project opened changed a default-diagram binding".into()));
    }
    // Only elements of one kind clash by name in one owner (AR-208); skills by their own rule.
    if let Some(problem) = st.first_problem_by(name_kind) {
        return Err(format!("the model does not hold together: {problem:?}").into());
    }
    if let Some(problem) = archforge_core::skills::first_duplicate(&st) {
        return Err(format!("the model does not hold together: {problem:?}").into());
    }
    if !st.alive().any(|o| st.class_of(o).ok() == Some(archforge_core::MODEL)) {
        return Err(OpenError::Unreadable("the project has no model root".into()));
    }
    // Read from the store the session gets, so they hold whatever the repair changed around a binding.
    let problems = archforge_uml::defaults::problems(&st);
    let mut session = Session::with_view(registry, st, FIRST_COUNTER, archforge_engine::Options::default(), view());
    session.mark_saved().map_err(|e| e.to_string())?;
    Ok(Opened { session, problems })
}
