//! The ER model in the project database: five `er_` tables and the `er_next` setting. Storing
//! rewrites the tables in one transaction and touches the working set only, so Save stays a Dolt
//! commit; loading reads the working set back with a cap on every table and on all the text read,
//! and refuses anything that does not make a valid model as damaged. Opening upgrades a format-1
//! project to these tables, in steps that a crash at any point leaves safe to repeat.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::{Duration, Instant};

use archforge_er::model::{MAX_ATTRIBUTES, MAX_DIAGRAMS, MAX_ENTITIES, MAX_KIND, MAX_NAME, MAX_PLACES, MAX_RELATIONSHIPS};
use archforge_er::{Attribute, Cardinality, Diagram, Entity, ErModel, Place, Relationship};
use mysql::prelude::Queryable;
use mysql::{Conn, TxOpts, Value};

use crate::{AUTHOR, DATABASE, Error, Hooks};

/// The setting that holds the counter new ids come from.
pub const NEXT: &str = "er_next";
/// How long a store may take before it is rolled back.
pub const STORE_BUDGET: Duration = Duration::from_secs(10);
/// The most text, in bytes, a valid model's rows can hold: each id at most 21 ASCII characters,
/// each name, kind and number at its cap in 4-byte characters. A load reads no more than this (and
/// a megabyte), so whatever a store wrote always loads.
const MAX_TEXT: usize = MAX_ENTITIES * (21 + 4 * MAX_NAME)
    + MAX_ATTRIBUTES * (21 + 21 + 3 + 4 * MAX_NAME + 4 * MAX_KIND + 3)
    + MAX_RELATIONSHIPS * (21 + 4 * MAX_NAME + 21 + 21 + 12 + 12)
    + MAX_DIAGRAMS * (21 + 4 * MAX_NAME)
    + MAX_PLACES * (21 + 21 + 4 * 24)
    + 20;
const TEXT_BUDGET: usize = MAX_TEXT + (1 << 20);
/// Rows per insert.
const CHUNK: usize = 500;
/// The widest id [`archforge_er::model::make_id`] makes is 21 characters.
const ID: usize = 24;
const NUMBER: usize = 40;

/// The five tables, in the order a store fills them, with their columns as
/// `information_schema.columns` reports them: name, type, nullable, key.
pub(crate) const TABLES: [(&str, &str, &[(&str, &str, bool, &str)]); 5] = [
    (
        "er_entities",
        "CREATE TABLE IF NOT EXISTS er_entities (id VARCHAR(24) NOT NULL PRIMARY KEY, name VARCHAR(128) NOT NULL)",
        &[("id", "varchar(24)", false, "PRI"), ("name", "varchar(128)", false, "")],
    ),
    (
        "er_attributes",
        "CREATE TABLE IF NOT EXISTS er_attributes (id VARCHAR(24) NOT NULL PRIMARY KEY, entity_id VARCHAR(24) NOT NULL, \
         position INT NOT NULL, name VARCHAR(128) NOT NULL, kind VARCHAR(64) NOT NULL, primary_key TINYINT NOT NULL, \
         nullable TINYINT NOT NULL, is_unique TINYINT NOT NULL)",
        &[
            ("id", "varchar(24)", false, "PRI"),
            ("entity_id", "varchar(24)", false, ""),
            ("position", "int", false, ""),
            ("name", "varchar(128)", false, ""),
            ("kind", "varchar(64)", false, ""),
            ("primary_key", "tinyint", false, ""),
            ("nullable", "tinyint", false, ""),
            ("is_unique", "tinyint", false, ""),
        ],
    ),
    (
        "er_relationships",
        "CREATE TABLE IF NOT EXISTS er_relationships (id VARCHAR(24) NOT NULL PRIMARY KEY, name VARCHAR(128) NULL, \
         from_id VARCHAR(24) NOT NULL, to_id VARCHAR(24) NOT NULL, from_end VARCHAR(16) NOT NULL, to_end VARCHAR(16) NOT NULL)",
        &[
            ("id", "varchar(24)", false, "PRI"),
            ("name", "varchar(128)", true, ""),
            ("from_id", "varchar(24)", false, ""),
            ("to_id", "varchar(24)", false, ""),
            ("from_end", "varchar(16)", false, ""),
            ("to_end", "varchar(16)", false, ""),
        ],
    ),
    (
        "er_diagrams",
        "CREATE TABLE IF NOT EXISTS er_diagrams (id VARCHAR(24) NOT NULL PRIMARY KEY, name VARCHAR(128) NOT NULL)",
        &[("id", "varchar(24)", false, "PRI"), ("name", "varchar(128)", false, "")],
    ),
    (
        "er_places",
        "CREATE TABLE IF NOT EXISTS er_places (diagram_id VARCHAR(24) NOT NULL, entity_id VARCHAR(24) NOT NULL, \
         x DOUBLE NOT NULL, y DOUBLE NOT NULL, width DOUBLE NOT NULL, height DOUBLE NOT NULL, \
         PRIMARY KEY (diagram_id, entity_id))",
        &[
            ("diagram_id", "varchar(24)", false, "PRI"),
            ("entity_id", "varchar(24)", false, "PRI"),
            ("x", "double", false, ""),
            ("y", "double", false, ""),
            ("width", "double", false, ""),
            ("height", "double", false, ""),
        ],
    ),
];

/// The points of an upgrade a test can fail at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeStep {
    /// The tables are made; the settings still say format 1.
    TablesMade,
    /// The settings say format 2 in the working set; nothing is committed.
    SettingsWritten,
    /// The tables and the settings are staged; the commit has not run.
    Added,
}

fn sql(what: &'static str) -> impl Fn(mysql::Error) -> Error {
    move |source| Error::Sql { what, source }
}

/// Makes the five tables and sets `er_next` to 1: a new project, inside its creation.
pub(crate) fn make_tables(c: &mut Conn) -> Result<(), Error> {
    for (_, create, _) in TABLES {
        c.query_drop(create).map_err(sql("cannot create the model's tables"))?;
    }
    c.exec_drop("INSERT INTO settings (`key`, `value`) VALUES (?, '1')", (NEXT,)).map_err(sql("cannot write the id counter"))
}

/// Stores `model` in the working set, replacing what the tables held; refuses an invalid model and
/// a store that takes longer than `budget`, leaving the tables as they were.
pub(crate) fn store(c: &mut Conn, model: &ErModel, counter: u64, budget: Duration) -> Result<(), Error> {
    model.check(counter).map_err(|e| Error::InvalidModel { why: e.0 })?;
    let start = Instant::now();
    let in_time = || if start.elapsed() > budget { Err(Error::StoreTooSlow { budget }) } else { Ok(()) };
    // Dropping the transaction before its commit rolls it back.
    let mut tx = c.start_transaction(TxOpts::default()).map_err(sql("cannot start storing the model"))?;
    for (table, _, _) in TABLES.iter().rev() {
        tx.query_drop(format!("DELETE FROM {table}")).map_err(sql("cannot clear the model's tables"))?;
    }
    in_time()?;
    let s = |v: &str| Value::from(v);
    let entities: Vec<Vec<Value>> = model.entities.values().map(|e| vec![s(&e.id), s(&e.name)]).collect();
    let attributes: Vec<Vec<Value>> = model
        .entities
        .values()
        .flat_map(|e| {
            e.attributes.iter().enumerate().map(|(i, a)| {
                vec![
                    s(&a.id),
                    s(&e.id),
                    Value::from(i as u64),
                    s(&a.name),
                    s(&a.kind),
                    Value::from(a.primary_key as u8),
                    Value::from(a.nullable as u8),
                    Value::from(a.unique as u8),
                ]
            })
        })
        .collect();
    let relationships: Vec<Vec<Value>> = model
        .relationships
        .values()
        .map(|r| {
            let name = r.name.as_deref().map_or(Value::NULL, s);
            vec![s(&r.id), name, s(&r.from), s(&r.to), s(r.from_end.as_str()), s(r.to_end.as_str())]
        })
        .collect();
    let diagrams: Vec<Vec<Value>> = model.diagrams.values().map(|d| vec![s(&d.id), s(&d.name)]).collect();
    let places: Vec<Vec<Value>> = model
        .diagrams
        .values()
        .flat_map(|d| {
            d.places.iter().map(|(e, p)| vec![s(&d.id), s(e), Value::from(p.x), Value::from(p.y), Value::from(p.width), Value::from(p.height)])
        })
        .collect();
    let columns = [
        "id, name",
        "id, entity_id, position, name, kind, primary_key, nullable, is_unique",
        "id, name, from_id, to_id, from_end, to_end",
        "id, name",
        "diagram_id, entity_id, x, y, width, height",
    ];
    for (((table, _, _), columns), rows) in TABLES.iter().zip(columns).zip([entities, attributes, relationships, diagrams, places]) {
        for chunk in rows.chunks(CHUNK) {
            let width = chunk.first().map_or(0, Vec::len);
            let row = format!("({})", vec!["?"; width].join(", "));
            let query = format!("INSERT INTO {table} ({columns}) VALUES {}", vec![row.as_str(); chunk.len()].join(", "));
            tx.exec_drop(query, chunk.concat()).map_err(sql("cannot store the model"))?;
            in_time()?;
        }
    }
    tx.exec_drop("REPLACE INTO settings (`key`, `value`) VALUES (?, ?)", (NEXT, counter.to_string())).map_err(sql("cannot store the id counter"))?;
    in_time()?;
    tx.commit().map_err(sql("cannot finish storing the model"))
}

/// What a load has read so far, against its text budget.
struct Reader<'a> {
    c: &'a mut Conn,
    path: &'a Path,
    text: usize,
}

impl Reader<'_> {
    /// The rows of `query`, every cell text or NULL, read one at a time against the text budget;
    /// more than `cap` rows is damage.
    fn rows(&mut self, query: &str, what: &str, cap: usize) -> Result<Vec<Vec<Option<String>>>, Error> {
        let path = self.path;
        let damaged = |why: String| Error::Damaged { path: path.to_path_buf(), why };
        let mut result = self.c.query_iter(format!("{query} LIMIT {}", cap + 1)).map_err(|e| damaged(format!("its {what} cannot be read: {e}")))?;
        let mut out = Vec::new();
        for row in result.by_ref() {
            let row = row.map_err(|e| damaged(format!("its {what} cannot be read: {e}")))?;
            if out.len() == cap {
                return Err(damaged(format!("it has more than {cap} {what}")));
            }
            let mut cells = Vec::with_capacity(row.len());
            for i in 0..row.len() {
                let cell = row
                    .get_opt::<Option<String>, usize>(i)
                    .and_then(Result::ok)
                    .ok_or_else(|| damaged(format!("a cell of its {what} is not text")))?;
                self.text = self.text.saturating_add(cell.as_ref().map_or(0, String::len));
                if self.text > TEXT_BUDGET {
                    return Err(damaged("its model holds more text than any valid model".into()));
                }
                cells.push(cell);
            }
            out.push(cells);
        }
        Ok(out)
    }
}

/// A cell read as text, capped one past its widest valid value so an oversized one is caught.
fn text(column: &str, max: usize) -> String {
    format!("LEFT(CAST({column} AS CHAR), {})", max + 1)
}

/// Loads the model and its counter from the working set.
pub(crate) fn load(c: &mut Conn, path: &Path) -> Result<(ErModel, u64), Error> {
    // The columns are checked first: the caps below and reading a DOUBLE as it is rely on them.
    for (table, _, columns) in TABLES {
        if !columns_match(c, table, columns)? {
            return Err(Error::Damaged { path: path.to_path_buf(), why: format!("its table {table} is not the one Archforge makes") });
        }
    }
    let mut r = Reader { c, path, text: 0 };
    let id = |col: &str| text(col, ID);
    let entities = r.rows(&format!("SELECT {}, {} FROM er_entities ORDER BY id", id("id"), text("name", MAX_NAME)), "entities", MAX_ENTITIES)?;
    let attributes = r.rows(
        &format!(
            "SELECT {}, {}, {}, {}, {}, {}, {}, {} FROM er_attributes ORDER BY entity_id, position",
            id("id"),
            id("entity_id"),
            text("position", NUMBER),
            text("name", MAX_NAME),
            text("kind", MAX_KIND),
            text("primary_key", NUMBER),
            text("nullable", NUMBER),
            text("is_unique", NUMBER)
        ),
        "attributes",
        MAX_ATTRIBUTES,
    )?;
    let relationships = r.rows(
        &format!(
            "SELECT {}, {}, {}, {}, {}, {} FROM er_relationships ORDER BY id",
            id("id"),
            text("name", MAX_NAME),
            id("from_id"),
            id("to_id"),
            text("from_end", NUMBER),
            text("to_end", NUMBER)
        ),
        "relationships",
        MAX_RELATIONSHIPS,
    )?;
    let diagrams = r.rows(&format!("SELECT {}, {} FROM er_diagrams ORDER BY id", id("id"), text("name", MAX_NAME)), "diagrams", MAX_DIAGRAMS)?;
    let places = r.rows(
        &format!(
            // A DOUBLE comes as dolt's shortest round-trip text; cast to text it would be written
            // out in full decimals (5e-324 takes 300 digits).
            "SELECT {}, {}, x, y, width, height FROM er_places ORDER BY diagram_id, entity_id",
            id("diagram_id"),
            id("entity_id")
        ),
        "places",
        MAX_PLACES,
    )?;
    let next = r.rows(&format!("SELECT {} FROM settings WHERE `key` = '{NEXT}'", text("`value`", NUMBER)), "id counter", 1)?;

    let damaged = |why: String| Error::Damaged { path: path.to_path_buf(), why };
    let need = |cell: &Option<String>, what: &str| cell.clone().ok_or_else(|| damaged(format!("{what} is empty")));
    let flag = |cell: &Option<String>, what: &str| match cell.as_deref() {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        _ => Err(damaged(format!("{what} is not 0 or 1"))),
    };
    let number = |cell: &Option<String>, what: &str| -> Result<f64, Error> {
        need(cell, what)?.parse::<f64>().map_err(|_| damaged(format!("{what} is not a number")))
    };
    let end = |cell: &Option<String>, what: &str| -> Result<Cardinality, Error> {
        cell.as_deref().and_then(Cardinality::parse).ok_or_else(|| damaged(format!("{what} is not a cardinality")))
    };

    let mut model = ErModel::default();
    for row in &entities {
        let id = need(&row[0], "an entity id")?;
        let entity = Entity { id: id.clone(), name: need(&row[1], "an entity name")?, attributes: Vec::new() };
        if model.entities.insert(id.clone(), entity).is_some() {
            return Err(damaged(format!("the entity {id:?} is there twice")));
        }
    }
    for row in &attributes {
        let id = need(&row[0], "an attribute id")?;
        let owner = need(&row[1], "the entity of an attribute")?;
        let position: usize =
            need(&row[2], "an attribute position")?.parse().map_err(|_| damaged(format!("the position of {id:?} is not a number")))?;
        let Some(entity) = model.entities.get_mut(&owner) else {
            return Err(damaged(format!("the attribute {id:?} belongs to the missing entity {owner:?}")));
        };
        if position != entity.attributes.len() {
            return Err(damaged(format!("the attribute positions of {owner:?} do not run from 0 without a gap")));
        }
        entity.attributes.push(Attribute {
            id: id.clone(),
            name: need(&row[3], "an attribute name")?,
            kind: need(&row[4], "an attribute kind")?,
            primary_key: flag(&row[5], &format!("the key flag of {id:?}"))?,
            nullable: flag(&row[6], &format!("the nullable flag of {id:?}"))?,
            unique: flag(&row[7], &format!("the unique flag of {id:?}"))?,
        });
    }
    for row in &relationships {
        let id = need(&row[0], "a relationship id")?;
        let relationship = Relationship {
            id: id.clone(),
            name: row[1].clone(),
            from: need(&row[2], &format!("the start of {id:?}"))?,
            to: need(&row[3], &format!("the end of {id:?}"))?,
            from_end: end(&row[4], &format!("the start of {id:?}"))?,
            to_end: end(&row[5], &format!("the end of {id:?}"))?,
        };
        if model.relationships.insert(id.clone(), relationship).is_some() {
            return Err(damaged(format!("the relationship {id:?} is there twice")));
        }
    }
    for row in &diagrams {
        let id = need(&row[0], "a diagram id")?;
        let diagram = Diagram { id: id.clone(), name: need(&row[1], "a diagram name")?, places: BTreeMap::new() };
        if model.diagrams.insert(id.clone(), diagram).is_some() {
            return Err(damaged(format!("the diagram {id:?} is there twice")));
        }
    }
    for row in &places {
        let diagram = need(&row[0], "the diagram of a place")?;
        let entity = need(&row[1], "the entity of a place")?;
        let what = format!("the place of {entity:?} in {diagram:?}");
        let place = Place { x: number(&row[2], &what)?, y: number(&row[3], &what)?, width: number(&row[4], &what)?, height: number(&row[5], &what)? };
        let Some(d) = model.diagrams.get_mut(&diagram) else { return Err(damaged(format!("{what} is in the missing diagram {diagram:?}"))) };
        if d.places.insert(entity, place).is_some() {
            return Err(damaged(format!("{what} is there twice")));
        }
    }
    let next = next.first().and_then(|row| row[0].clone()).ok_or_else(|| damaged(format!("it has no {NEXT} setting")))?;
    let next: u64 = next.parse().map_err(|_| damaged(format!("its {NEXT} setting is not a number")))?;
    model.check(next).map_err(|e| damaged(e.0))?;
    Ok((model, next))
}

/// Whether the working set holds changes Save has not committed: any `er_` table differs from HEAD,
/// or `settings` does in anything but the counter.
pub(crate) fn uncommitted(c: &mut Conn, path: &Path) -> Result<bool, Error> {
    let changed = status(c, path, "table_name LIKE 'er\\_%' OR table_name = 'settings'")?;
    if changed.iter().any(|t| t.starts_with("er_")) {
        return Ok(true);
    }
    if !changed.iter().any(|t| t == "settings") {
        return Ok(false);
    }
    let working = settings_map(c, path, "settings")?;
    let head = settings_map(c, path, "settings AS OF 'HEAD'")?;
    let without_next = |m: BTreeMap<String, String>| m.into_iter().filter(|(k, _)| k != NEXT).collect::<BTreeMap<_, _>>();
    Ok(without_next(working) != without_next(head))
}

/// The tables `dolt_status` lists where `filter` holds; a name that does not read is an error,
/// never dropped.
fn status(c: &mut Conn, path: &Path, filter: &str) -> Result<BTreeSet<String>, Error> {
    let rows: Vec<mysql::Row> = c
        .query(format!("SELECT LEFT(CAST(table_name AS CHAR), 256) FROM dolt_status WHERE {filter} LIMIT 10001"))
        .map_err(sql("cannot read what changed"))?;
    rows.into_iter()
        .map(|r| {
            r.get_opt::<String, usize>(0)
                .and_then(Result::ok)
                .ok_or(Error::Damaged { path: path.to_path_buf(), why: "a changed table's name does not read".into() })
        })
        .collect()
}

fn settings_map(c: &mut Conn, path: &Path, from: &str) -> Result<BTreeMap<String, String>, Error> {
    let mut r = Reader { c, path, text: 0 };
    let rows = r.rows(
        &format!("SELECT {}, {} FROM {from} ORDER BY `key`", text("`key`", crate::MAX_KEY), text("`value`", crate::MAX_VALUE)),
        "settings",
        crate::MAX_ROWS,
    )?;
    Ok(rows.into_iter().filter_map(|row| Some((row[0].clone()?, row[1].clone()?))).collect())
}

/// The format HEAD's settings say, if it says one.
fn head_format(c: &mut Conn) -> Result<Option<String>, Error> {
    let row: Option<mysql::Row> = c
        .query_first("SELECT LEFT(CAST(`value` AS CHAR), 41) FROM settings AS OF 'HEAD' WHERE `key` = 'format'")
        .map_err(sql("cannot read the committed format"))?;
    Ok(row.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok))
}

/// Whether `table` has exactly `columns`, as `information_schema.columns` reports them.
fn columns_match(c: &mut Conn, table: &str, columns: &[(&str, &str, bool, &str)]) -> Result<bool, Error> {
    let rows: Vec<mysql::Row> = c
        .exec(
            "SELECT LEFT(CAST(column_name AS CHAR), 65), LEFT(CAST(column_type AS CHAR), 65), LEFT(CAST(is_nullable AS CHAR), 4), \
             LEFT(CAST(column_key AS CHAR), 4) FROM information_schema.columns WHERE table_schema = ? AND table_name = ? \
             ORDER BY ordinal_position LIMIT 100",
            (DATABASE, table),
        )
        .map_err(sql("cannot read the model's columns"))?;
    let cell = |r: &mysql::Row, i: usize| r.get_opt::<String, usize>(i).and_then(Result::ok).unwrap_or_default();
    let found: Vec<(String, String, String, String)> = rows.iter().map(|r| (cell(r, 0), cell(r, 1), cell(r, 2), cell(r, 3))).collect();
    let expected: Vec<(String, String, String, String)> = columns
        .iter()
        .map(|(n, t, null, key)| (n.to_string(), t.to_string(), if *null { "YES" } else { "NO" }.to_string(), key.to_string()))
        .collect();
    Ok(found == expected)
}

/// Refuses a project whose database holds triggers, stored procedures or events: Archforge's own
/// writes would set them off, and they could change what it stores or what Save commits.
pub(crate) fn no_scripts(c: &mut Conn, path: &Path) -> Result<(), Error> {
    for (what, table, column) in
        [("triggers", "triggers", "trigger_schema"), ("stored procedures", "routines", "routine_schema"), ("events", "events", "event_schema")]
    {
        let any: Option<mysql::Row> = c
            .exec_first(format!("SELECT 1 FROM information_schema.{table} WHERE {column} = ? LIMIT 1"), (DATABASE,))
            .map_err(sql("cannot list the project's scripts"))?;
        if any.is_some() {
            return Err(Error::Scripted { path: path.to_path_buf(), what });
        }
    }
    Ok(())
}

/// Brings a project to format 2, or finishes an upgrade a crash interrupted; `format` is what the
/// working set says. Gives true when it upgraded.
pub(crate) fn upgrade(c: &mut Conn, path: &Path, format: u32, hooks: &Hooks) -> Result<bool, Error> {
    let damaged = |why: String| Error::Damaged { path: path.to_path_buf(), why };
    let hook = |step: UpgradeStep| match &hooks.upgrade {
        Some(h) => h(step).map_err(|why| Error::Damaged { path: path.to_path_buf(), why }),
        None => Ok(()),
    };
    let finishing = match format {
        1 => false,
        2 if head_format(c)?.as_deref() == Some("1") => true,
        _ => return Ok(false),
    };
    // A commit takes whatever is staged, so anything else staged would go into the upgrade's commit.
    let ours: BTreeSet<&str> = TABLES.iter().map(|t| t.0).chain(["settings"]).collect();
    let foreign: Vec<String> = status(c, path, "staged = 1")?.into_iter().filter(|t| !ours.contains(t.as_str())).collect();
    if !foreign.is_empty() {
        return Err(Error::StagedChanges { path: path.to_path_buf(), tables: foreign });
    }
    if !finishing {
        for (table, create, columns) in TABLES {
            c.query_drop(create).map_err(sql("cannot create the model's tables"))?;
            if !columns_match(c, table, columns)? {
                return Err(damaged(format!("its table {table} is not the one Archforge makes")));
            }
            let any: Option<mysql::Row> = c.query_first(format!("SELECT 1 FROM {table} LIMIT 1")).map_err(sql("cannot read the model's rows"))?;
            if any.is_some() {
                return Err(damaged(format!("its table {table} holds rows before the upgrade made it")));
            }
        }
        hook(UpgradeStep::TablesMade)?;
        let mut tx = c.start_transaction(TxOpts::default()).map_err(sql("cannot start the upgrade"))?;
        tx.exec_drop("REPLACE INTO settings (`key`, `value`) VALUES (?, '1')", (NEXT,)).map_err(sql("cannot write the id counter"))?;
        tx.query_drop("UPDATE settings SET `value` = '2' WHERE `key` = 'format'").map_err(sql("cannot write the format"))?;
        tx.commit().map_err(sql("cannot finish the upgrade's settings"))?;
        hook(UpgradeStep::SettingsWritten)?;
    } else {
        // Finishing what a crash left: the tables are checked again before they are committed.
        for (table, _, columns) in TABLES {
            if !columns_match(c, table, columns)? {
                return Err(damaged(format!("its table {table} is not the one Archforge makes")));
            }
        }
    }
    let names: Vec<String> = ours.iter().map(|t| format!("'{t}'")).collect();
    c.query_drop(format!("CALL DOLT_ADD({})", names.join(", "))).map_err(sql("cannot stage the upgrade"))?;
    hook(UpgradeStep::Added)?;
    // The commit's reply is not read: converting a row that is not one string would panic.
    c.query_drop(format!("CALL DOLT_COMMIT('-m', 'Upgrade the project to format 2', '--author', '{AUTHOR}')"))
        .map_err(sql("cannot commit the upgrade"))?;
    Ok(true)
}
