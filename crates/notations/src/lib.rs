//! The notations a project holds, composed: each brings its metamodel, its slabs and the tables its
//! objects are stored in, and a project is made from the list, so one store, one database and one
//! command table hold ER and UML side by side. The shell, the host and the agent's tools make
//! projects here and name no notation for it. Pure: builds for the portable target.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unreachable, clippy::todo, clippy::unimplemented)]

use std::sync::Arc;

use archforge_mof::schema::{Junction, Table};
use archforge_mof::{CompositeSlabs, MetaError, Metamodel, Slabs, Store, Universe, Uuid};

/// What a metamodel brings to a project.
pub struct Notation {
    /// Its name, for messages.
    pub name: &'static str,
    pub metamodel: fn() -> Result<Arc<Metamodel>, MetaError>,
    /// The metamodel's index, and its objects' slabs, fresh.
    pub index: u16,
    pub slabs: fn() -> Box<dyn Slabs>,
    /// The tables and list tables its objects are stored in.
    pub tables: &'static [Table],
    pub junctions: &'static [Junction],
}

/// What a project holds: the shared core and the diagrams first, then the notations. Their tables
/// are stored in this order, so a metamodel joins at the end.
pub const NOTATIONS: &[Notation] = &[
    Notation {
        name: "core",
        metamodel: archforge_core::metamodel,
        index: archforge_core::INDEX,
        slabs: || Box::new(archforge_core::CoreSlabs::default()),
        tables: archforge_core::TABLES,
        junctions: archforge_core::JUNCTIONS,
    },
    Notation {
        name: "diagram",
        metamodel: archforge_diagram::metamodel,
        index: archforge_diagram::INDEX,
        slabs: || Box::new(archforge_diagram::DiagramSlabs::default()),
        tables: archforge_diagram::TABLES,
        junctions: archforge_diagram::JUNCTIONS,
    },
    Notation {
        name: "er",
        metamodel: archforge_er::metamodel,
        index: archforge_er::INDEX,
        slabs: || Box::new(archforge_er::ErSlabs::default()),
        tables: archforge_er::TABLES,
        junctions: archforge_er::JUNCTIONS,
    },
    Notation {
        name: "uml",
        metamodel: archforge_uml::metamodel,
        index: archforge_uml::INDEX,
        slabs: || Box::new(archforge_uml::UmlSlabs::default()),
        tables: archforge_uml::TABLES,
        junctions: archforge_uml::JUNCTIONS,
    },
];

/// A project's session: its store, the view and the commands. The view is ER's until the project
/// has one view across notations.
pub type Session = archforge_er::Session;

/// The universe of every notation a project holds.
pub fn universe() -> Result<Arc<Universe>, MetaError> {
    let metamodels = NOTATIONS.iter().map(|n| (n.metamodel)()).collect::<Result<Vec<_>, _>>()?;
    Ok(Arc::new(Universe::new(metamodels)?))
}

/// The slabs of every notation, for a store over [`universe`].
fn slabs() -> Result<CompositeSlabs, String> {
    NOTATIONS.iter().try_fold(CompositeSlabs::new(), |all, n| all.with(n.index, (n.slabs)()))
}

/// An empty store over every notation, with the host's ids.
pub fn store(ids: Box<dyn FnMut() -> Uuid + Send>) -> Result<Store, String> {
    let universe = universe().map_err(|e| e.to_string())?;
    Ok(Store::new(universe, Box::new(slabs()?), ids))
}

/// The tables a project stores its model in, every notation's in the order of [`NOTATIONS`]. The
/// shell writes rows for them and the host stores them, so both take them from here.
pub fn storage() -> (Vec<Table>, Vec<Junction>) {
    let tables = NOTATIONS.iter().flat_map(|n| n.tables).copied().collect();
    let junctions = NOTATIONS.iter().flat_map(|n| n.junctions).copied().collect();
    (tables, junctions)
}

/// The commands of every notation, the diagrams' and the skills'.
pub fn registry() -> Result<Arc<archforge_engine::Registry<Store, archforge_er::ErView>>, String> {
    archforge_er::registry()
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
    Ok(Session::new(registry, st, FIRST_COUNTER, archforge_engine::Options::default()))
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
    let mut st = store(ids)?;
    let (tables, junctions) = storage();
    archforge_mof::rows::restore(&mut st, rows, &tables, &junctions).map_err(|e| e.to_string())?;
    repair(&mut st);
    if let Some(problem) = st.validate_all().first() {
        return Err(format!("the model does not hold together: {problem:?}"));
    }
    if !st.alive().any(|o| st.class_of(o).ok() == Some(archforge_core::MODEL)) {
        return Err("the project has no model root".into());
    }
    let mut session = Session::new(registry, st, FIRST_COUNTER, archforge_engine::Options::default());
    session.mark_saved().map_err(|e| e.to_string())?;
    Ok(session)
}
