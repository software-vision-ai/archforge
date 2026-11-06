//! The notations a project holds, composed: each metamodel brings its slabs and the tables its
//! objects are stored in ([`PARTS`]), each notation its diagrams, skills and element hooks
//! ([`NOTATIONS`]), and a project is made from the lists, so one store, one database, one view and
//! one command table hold ER and UML side by side. The shell, the host and the agent's tools make
//! projects here and name no notation for it. Pure: builds for the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

use std::sync::Arc;

use archforge_diagram::{Notation, ProjectView};
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
    specs.extend(archforge_uml::nesting::specs());
    specs.extend(archforge_uml::templates::specs());
    specs.extend(archforge_uml::profiles::specs());
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
/// saved as it is. `ids` mints the ids of the elements made from now on.
pub fn session_from_rows(rows: &archforge_mof::rows::Rows, ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Session, String> {
    session_from_rows_repaired(rows, ids, |_| {})
}

/// [`session_from_rows`] with `repair` run on the model read before the session starts: for a
/// layout the reader puts right (a box too small for what it shows) without an undo step; it is
/// written with the next change.
pub fn session_from_rows_repaired(
    rows: &archforge_mof::rows::Rows,
    ids: Box<dyn FnMut() -> Uuid + Send>,
    repair: impl FnOnce(&mut Store),
) -> Result<Session, String> {
    let registry = registry()?;
    let (tables, junctions) = storage();
    // Built without an undo log, so a model of any size the read budgets let through opens.
    let mut st = archforge_mof::rows::restore_into_empty(store(ids)?, rows, &tables, &junctions).map_err(|e| e.to_string())?;
    repair(&mut st);
    // Only elements of one kind clash by name in one owner (AR-208); skills by their own rule.
    if let Some(problem) = st.first_problem_by(name_kind) {
        return Err(format!("the model does not hold together: {problem:?}"));
    }
    if let Some(problem) = archforge_core::skills::first_duplicate(&st) {
        return Err(format!("the model does not hold together: {problem:?}"));
    }
    if !st.alive().any(|o| st.class_of(o).ok() == Some(archforge_core::MODEL)) {
        return Err("the project has no model root".into());
    }
    let mut session = Session::with_view(registry, st, FIRST_COUNTER, archforge_engine::Options::default(), view());
    session.mark_saved().map_err(|e| e.to_string())?;
    Ok(session)
}
