//! The model in the project database: the generated tables of the metamodels a
//! project holds, each made when rows first go there, so a project holds the tables of the
//! notations it uses and a table it lacks holds nothing; the model's rows written as one transaction that replaces
//! what the tables held, as the user works (owner decision); read back only from
//! tables of exactly the generated shape, row by row within budgets of rows, cells and bytes, never
//! sizing an allocation from what is read; and Save, a Dolt commit of everything written.
//!
//! The tables are described by the metamodels' generated code (`Table`, `Junction`); a project
//! records a fingerprint of that description. A project made by an earlier Archforge, whose tables
//! lack only tables and optional columns that come last, is brought up to date by adding them; any
//! other project made for other tables is refused rather than read wrongly.

use archforge_mof::rows::{Cell, Rows};
use archforge_mof::schema::{Junction, Table};
use archforge_mof::{DataKind, MAX_STRING, TypeRef, Universe, Uuid, Value};
use std::collections::BTreeMap;

use mysql::prelude::Queryable;

use crate::chat::{is_chat_table, is_commit};
use crate::{AUTHOR, DATABASE, Error, Project};

/// The most rows a table of the model may hold when read back: the store's own limit.
const MAX_TABLE_ROWS: usize = archforge_mof::store::MAX_OBJECTS;
/// The most cells a whole model may hold when read back.
const MAX_READ_CELLS: usize = 8_000_000;
/// The most memory the rows read back may take, counted as they are kept: each cell, each text's
/// bytes and header, each row's list. However the rows are spread over the tables, a reading stays
/// within it.
const MAX_READ_BYTES: usize = 384 << 20;
/// The most columns a model table may have when its shape is read.
const MAX_COLUMNS: usize = 64;
/// Rows inserted per statement at most.
const BATCH: usize = 500;
/// Bytes of values sent per statement at most: a quarter of the packet limit, so a statement of
/// long texts never exceeds it.
const BATCH_BYTES: usize = archforge_dolt::MAX_PACKET / 4;
/// The setting that holds the model tables' fingerprint.
const SCHEMA_KEY: &str = "model_schema";
/// The fingerprints of the model tables earlier Archforges made, oldest first: a project holding
/// one of them is brought up to date by adding what came since. Append-only: when the tables
/// change, the fingerprint they had goes here. A fingerprint not listed is another model's, or a
/// later Archforge's, and is refused.
pub const EARLIER_MODELS: &[&str] = &[
    // Before line routing: no Diagram.style, no Style.routing or Style.avoid.
    "7666551b88d9af5b",
    // Before UML: the core's, the diagrams' and ER's tables alone.
    "8c7ee9a29f10f219",
];
/// The table that names every model table rows have gone to: one the project lacks is then lost,
/// not unused, and the project is refused rather than read short (owner decision).
const IN_USE: &str = "model_tables";
/// The most tables a project's database may hold for an upgrade to look at.
pub(crate) const MAX_TABLES: usize = 4096;

/// The model's tables as the project stores them: the generated tables and junctions with the SQL
/// type of each junction's value, and their fingerprint.
pub struct ModelSchema {
    tables: Vec<Table>,
    junctions: Vec<(Junction, &'static str)>,
    fingerprint: String,
    /// For each column that may not be empty, the value a new object holds there, as SQL: what a
    /// column added to an earlier project's table gives the rows already in it.
    fills: Vec<(&'static str, &'static str, String)>,
}

impl ModelSchema {
    /// The schema of `tables` and `junctions` of the metamodels in `universe`.
    pub fn new(universe: &Universe, tables: Vec<Table>, junctions: Vec<Junction>) -> Result<ModelSchema, String> {
        // A name SQL takes: plain, and no longer than its 64 characters.
        let plain = |s: &str| !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        // Every table, the settings and the tables in use, within what an upgrade looks at.
        if tables.len() + junctions.len() > MAX_TABLES - 2 {
            return Err(format!("the model has more than {} tables", MAX_TABLES - 2));
        }
        let mut seen = std::collections::BTreeSet::new();
        for name in tables.iter().map(|t| t.name).chain(junctions.iter().map(|j| j.name)) {
            if name == "settings" || name == IN_USE || is_chat_table(name) || name.starts_with("chat_") {
                return Err(format!("a model table is named {name}, as the project's own"));
            }
            if !seen.insert(name) {
                return Err(format!("two tables are named {name}"));
            }
        }
        let mut text = String::new();
        let mut fills = Vec::new();
        for t in &tables {
            for c in t.columns.iter().filter(|c| c.sql.contains("NOT NULL")) {
                let Some(prop) = c.prop.and_then(|p| universe.prop(p)) else { continue };
                let fill = match (&prop.default, &prop.ty) {
                    (Some(Value::Bool(b)), _) => Some(i64::from(*b).to_string()),
                    (Some(Value::Int(i)), _) => Some(i.to_string()),
                    (Some(Value::Real(r)), _) if r.is_finite() => Some(r.to_string()),
                    (Some(Value::Enum(i)), _) => Some(i.to_string()),
                    (Some(_), _) => None,
                    // A new object holds zero, false or the first literal.
                    (None, TypeRef::Data(d)) => match universe.data_type(*d).map(|t| &t.kind) {
                        Some(DataKind::Boolean | DataKind::Integer | DataKind::Real | DataKind::Enumeration(_)) => Some("0".into()),
                        _ => None,
                    },
                    (None, _) => None,
                };
                if let Some(fill) = fill {
                    fills.push((t.name, c.name, fill));
                }
            }
            if !plain(t.name) || t.columns.iter().any(|c| !plain(c.name)) {
                return Err(format!("the table {:?} has a name that is not a plain identifier", t.name));
            }
            // What is written must read back: the reader looks at no more columns than this.
            if t.columns.len() > MAX_COLUMNS {
                return Err(format!("the table {} has more than {MAX_COLUMNS} columns", t.name));
            }
            text.push_str(t.name);
            for c in t.columns {
                text.push_str(&format!(" {}:{}", c.name, c.sql));
            }
            text.push('\n');
        }
        let mut with_types = Vec::with_capacity(junctions.len());
        for j in junctions {
            if !plain(j.name) {
                return Err(format!("the junction {:?} has a name that is not a plain identifier", j.name));
            }
            let sql = if j.reference {
                "BINARY(16) NOT NULL"
            } else {
                let prop = universe.prop(j.prop).ok_or_else(|| format!("{} has no property", j.name))?;
                match prop.ty {
                    TypeRef::Data(d) => match universe.data_type(d).map(|t| &t.kind) {
                        Some(DataKind::String) => "MEDIUMTEXT NOT NULL",
                        Some(DataKind::Boolean) => "TINYINT NOT NULL",
                        Some(DataKind::Integer | DataKind::UnlimitedNatural) => "BIGINT NOT NULL",
                        Some(DataKind::Enumeration(_)) => "INT NOT NULL",
                        Some(DataKind::Real) => "DOUBLE NOT NULL",
                        None => return Err(format!("{} has no data type", j.name)),
                    },
                    _ => "BINARY(16) NOT NULL",
                }
            };
            text.push_str(&format!("{} {}\n", j.name, sql));
            with_types.push((j, sql));
        }
        Ok(ModelSchema { tables, junctions: with_types, fingerprint: fingerprint(&text), fills })
    }

    pub fn tables(&self) -> &[Table] {
        &self.tables
    }

    /// The fingerprint a project made for these tables records.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// The junctions without their value types, as `rows` takes them.
    pub fn junctions(&self) -> Vec<Junction> {
        self.junctions.iter().map(|(j, _)| *j).collect()
    }
}

/// A short stable fingerprint of a text (FNV-1a, 64 bits, in hex): enough to tell one table
/// description from another, not a security measure.
fn fingerprint(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

pub(crate) fn sql(what: &'static str) -> impl Fn(mysql::Error) -> Error {
    move |source| Error::Sql { what, source }
}

impl Project {
    pub(crate) fn model_connection(&self) -> Result<mysql::Conn, Error> {
        let mut c = self.server().connect().map_err(Error::Server)?;
        c.query_drop(format!("USE `{DATABASE}`")).map_err(sql("cannot use the project database"))?;
        Ok(c)
    }

    /// A connection to the version `commit` through Dolt's revision database: it sees that commit
    /// and nothing else, and the server itself refuses every write over it, so the working set
    /// cannot be touched. The name goes into a `USE` only once it has the shape of a hash, and only
    /// a commit on the project's own line of work is opened ([`Error::NotAVersion`] otherwise). A
    /// commit never changes, so what is read over the connection is one consistent state however
    /// many statements the reading takes and whatever is written meanwhile.
    fn revision_connection(&self, commit: &str) -> Result<mysql::Conn, Error> {
        let refused = || Error::NotAVersion { commit: commit.chars().take(40).collect() };
        if !is_commit(commit) {
            return Err(refused());
        }
        let mut c = self.model_connection()?;
        // One of the line's own commits: the merge base of it and the head is itself. A hash Dolt
        // does not know is the server's refusal; a connection lost here is not a wrong version.
        let base: Option<mysql::Row> = match c.exec_first("SELECT LEFT(CAST(DOLT_MERGE_BASE(?, 'HEAD') AS CHAR), 64)", (commit,)) {
            Ok(base) => base,
            Err(mysql::Error::MySqlError(e)) => {
                log::warn!("the commit {commit} of {} could not be looked up: {e}", self.path().display());
                return Err(refused());
            }
            Err(e) => return Err(sql("cannot look up the version")(e)),
        };
        if base.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok).as_deref() != Some(commit) {
            return Err(refused());
        }
        c.query_drop(format!("USE `{DATABASE}/{commit}`")).map_err(sql("cannot open the version"))?;
        Ok(c)
    }

    /// Records the model tables' fingerprint, bringing a project made by an earlier Archforge up to
    /// date; a project made for other tables is refused. The tables themselves are made as rows go
    /// there.
    pub fn prepare_model(&self, schema: &ModelSchema) -> Result<(), Error> {
        let mut c = self.model_connection()?;
        let held: Option<mysql::Row> = c
            .exec_first("SELECT LEFT(CAST(`value` AS CHAR), 64) FROM settings WHERE `key` = ?", (SCHEMA_KEY,))
            .map_err(sql("cannot read the model's fingerprint"))?;
        // Read as a row, not a string: a value that is not text is refused, not a panic.
        let held = match held {
            None => None,
            Some(row) => Some(
                row.get_opt::<String, usize>(0)
                    .and_then(Result::ok)
                    .ok_or_else(|| Error::Damaged { path: self.path().to_path_buf(), why: "the model's fingerprint is not text".into() })?,
            ),
        };
        match held {
            Some(f) if f != schema.fingerprint => {
                // Made by an earlier Archforge, as its fingerprint says: the columns added since come
                // last and are empty or hold what a new object holds, so adding them keeps every row
                // as it was. Anything else is another model, or a later Archforge's.
                let other = || Error::OtherModel { path: self.path().to_path_buf(), found: f.clone(), expected: schema.fingerprint.clone() };
                if !EARLIER_MODELS.contains(&f.as_str()) {
                    return Err(other());
                }
                let added = match upgrade(&mut c, schema) {
                    Ok(added) => added,
                    Err(Refusal::Shape(why)) => {
                        log::warn!("{} is not upgraded: {why}", self.path().display());
                        return Err(other());
                    }
                    Err(Refusal::Read(why)) => return Err(Error::Damaged { path: self.path().to_path_buf(), why }),
                };
                for statement in &added {
                    c.query_drop(statement).map_err(sql("cannot add a column to a model table"))?;
                }
                // An earlier Archforge made every table it had: those it left are the ones in use.
                let held = tables_held(&mut c, schema).map_err(|why| Error::Damaged { path: self.path().to_path_buf(), why })?;
                create_in_use(&mut c)?;
                let used: Vec<&str> =
                    held.iter().filter(|(name, base)| **base && schema.is_model_table(name)).map(|(name, _)| name.as_str()).collect();
                record_in_use(&mut c, &used)?;
                c.exec_drop("UPDATE settings SET `value` = ? WHERE `key` = ?", (&schema.fingerprint, SCHEMA_KEY))
                    .map_err(sql("cannot record the model's fingerprint"))?;
                log::info!("{} upgraded: {} columns added", self.path().display(), added.len());
            }
            Some(_) => {}
            None => {
                // Model tables a project already holds without a fingerprint are in use too.
                let held = tables_held(&mut c, schema).map_err(|why| Error::Damaged { path: self.path().to_path_buf(), why })?;
                create_in_use(&mut c)?;
                let used: Vec<&str> =
                    held.iter().filter(|(name, base)| **base && schema.is_model_table(name)).map(|(name, _)| name.as_str()).collect();
                record_in_use(&mut c, &used)?;
                c.exec_drop("INSERT INTO settings (`key`, `value`) VALUES (?, ?)", (SCHEMA_KEY, &schema.fingerprint))
                    .map_err(sql("cannot record the model's fingerprint"))?;
            }
        }
        Ok(())
    }

    /// Replaces the model's rows with `rows`, all of them or none.
    pub fn write_model(&self, schema: &ModelSchema, rows: &Rows) -> Result<(), Error> {
        if rows.tables.len() != schema.tables.len() || rows.junctions.len() != schema.junctions.len() {
            return Err(Error::Damaged { path: self.path().to_path_buf(), why: "the rows do not match the model's tables".into() });
        }
        let mut c = self.model_connection()?;
        let damaged = |why: String| Error::Damaged { path: self.path().to_path_buf(), why };
        let mut held = shapes(&mut c, schema).map_err(damaged)?;
        let recorded = in_use(&mut c, &held).map_err(damaged)?;
        // The tables rows go to for the first time are made first: made and left empty, should the
        // writing fail, they hold what a table the project lacks holds.
        for (t, table_rows) in schema.tables.iter().zip(&rows.tables) {
            if !table_rows.is_empty() && !held.contains_key(t.name) {
                let columns: Vec<String> = t.columns.iter().map(|col| format!("`{}` {}", col.name, col.sql)).collect();
                c.query_drop(format!("CREATE TABLE IF NOT EXISTS `{}` ({})", t.name, columns.join(", ")))
                    .map_err(sql("cannot create a model table"))?;
                held.insert(t.name.to_string(), Shape::Columns(Vec::new()));
            }
        }
        for ((j, value), list_rows) in schema.junctions.iter().zip(&rows.junctions) {
            if !list_rows.is_empty() && !held.contains_key(j.name) {
                c.query_drop(format!(
                    "CREATE TABLE IF NOT EXISTS `{}` (`owner` BINARY(16) NOT NULL, `position` INT NOT NULL, `value` {value}, PRIMARY KEY (`owner`, `position`))",
                    j.name
                ))
                .map_err(sql("cannot create a model list table"))?;
                held.insert(j.name.to_string(), Shape::Columns(Vec::new()));
            }
        }
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the model"))?;
        // The tables rows go to are recorded as in use with the rows, never apart from them.
        let names = schema.tables.iter().map(|t| t.name).zip(rows.tables.iter().map(Vec::is_empty));
        let names = names.chain(schema.junctions.iter().map(|(j, _)| j.name).zip(rows.junctions.iter().map(Vec::is_empty)));
        let new: Vec<&str> = names.filter(|(name, empty)| !empty && !recorded.contains(*name)).map(|(name, _)| name).collect();
        record_in_use(&mut tx, &new)?;
        for (t, table_rows) in schema.tables.iter().zip(&rows.tables) {
            if !held.contains_key(t.name) {
                continue;
            }
            tx.query_drop(format!("DELETE FROM `{}`", t.name)).map_err(sql("cannot clear a model table"))?;
            let names: Vec<String> = t.columns.iter().map(|col| format!("`{}`", col.name)).collect();
            let one = format!("({})", vec!["?"; t.columns.len()].join(", "));
            for chunk in batches(table_rows, |row| row.iter().map(cell_bytes).sum()) {
                let statement = format!("INSERT INTO `{}` ({}) VALUES {}", t.name, names.join(", "), vec![one.as_str(); chunk.len()].join(", "));
                let params: Vec<mysql::Value> = chunk.iter().flatten().map(to_sql).collect();
                tx.exec_drop(statement, params).map_err(sql("cannot write a model table"))?;
            }
        }
        for ((j, _), list_rows) in schema.junctions.iter().zip(&rows.junctions) {
            if !held.contains_key(j.name) {
                continue;
            }
            tx.query_drop(format!("DELETE FROM `{}`", j.name)).map_err(sql("cannot clear a model list table"))?;
            for chunk in batches(list_rows, |(_, _, value)| 20 + cell_bytes(value)) {
                let statement =
                    format!("INSERT INTO `{}` (`owner`, `position`, `value`) VALUES {}", j.name, vec!["(?, ?, ?)"; chunk.len()].join(", "));
                let mut params: Vec<mysql::Value> = Vec::with_capacity(chunk.len() * 3);
                for (owner, at, value) in chunk {
                    params.push(mysql::Value::Bytes(owner.0.to_vec()));
                    params.push(mysql::Value::Int(i64::from(*at)));
                    params.push(to_sql(value));
                }
                tx.exec_drop(statement, params).map_err(sql("cannot write a model list table"))?;
            }
        }
        tx.commit().map_err(sql("cannot finish writing the model"))
    }

    /// The model's rows as the project holds them. Every table it holds must have exactly its
    /// generated shape (a base table with the generated columns and types, so no cell is longer
    /// than its type allows), and a table it lacks holds nothing; the rows are read one at a time
    /// within the budgets of rows per table and of cells and bytes for the whole model.
    pub fn read_model(&self, schema: &ModelSchema) -> Result<Rows, Error> {
        self.read_rows(&mut self.model_connection()?, schema)
    }

    /// The model's rows as the version `commit` holds them, read as [`Project::read_model`] reads
    /// the working set, under the same budgets and from that commit's own table shapes. Nothing is
    /// written and nothing moves: unsaved work, the head and the branch stay as they were. Refused
    /// with [`Error::NotAVersion`] when `commit` is not a commit on the project's line of work, and
    /// as [`Error::Damaged`] when that commit's tables are not exactly the generated shape or it
    /// holds no list of the tables in use, which the commit that created the project does not.
    pub fn read_model_at(&self, schema: &ModelSchema, commit: &str) -> Result<Rows, Error> {
        self.read_rows(&mut self.revision_connection(commit)?, schema)
    }

    /// The model's rows as the database `c` is using holds them: the reading behind
    /// [`Project::read_model`] and [`Project::read_model_at`].
    fn read_rows(&self, c: &mut mysql::Conn, schema: &ModelSchema) -> Result<Rows, Error> {
        let damaged = |why: String| Error::Damaged { path: self.path().to_path_buf(), why };
        let shapes = shapes(c, schema).map_err(damaged)?;
        for name in in_use(c, &shapes).map_err(damaged)? {
            if !schema.is_model_table(&name) {
                return Err(damaged(format!("the model uses the table {name:?}, which this Archforge does not know")));
            }
            if !shapes.contains_key(&name) {
                return Err(damaged(format!("the table {name:?}, which the model uses, is missing")));
            }
        }
        let mut budget = Budget::default();
        let mut rows = Rows::default();
        for t in &schema.tables {
            let expected: Vec<(&str, &str)> = t.columns.iter().map(|col| (col.name, col.sql)).collect();
            if !check_shape(&shapes, t.name, &expected).map_err(damaged)? {
                rows.tables.push(Vec::new());
                continue;
            }
            let names: Vec<String> = t.columns.iter().map(|col| format!("`{}`", col.name)).collect();
            let mut out = Vec::new();
            let read = c
                .query_iter(format!("SELECT {} FROM `{}` LIMIT {}", names.join(", "), t.name, MAX_TABLE_ROWS + 1))
                .map_err(|e| damaged(format!("the table {} cannot be read: {e}", t.name)))?;
            for row in read {
                let row = row.map_err(|e| damaged(format!("the table {} cannot be read: {e}", t.name)))?;
                if out.len() == MAX_TABLE_ROWS {
                    return Err(damaged(format!("the table {} has more than {MAX_TABLE_ROWS} rows", t.name)));
                }
                budget.row().map_err(damaged)?;
                let mut cells = Vec::with_capacity(t.columns.len());
                for (i, col) in t.columns.iter().enumerate() {
                    let v = row.as_ref(i).cloned().unwrap_or(mysql::Value::NULL);
                    budget.take(&v).map_err(damaged)?;
                    cells.push(from_sql(&v, col.sql).map_err(|why| damaged(format!("{}.{}: {why}", t.name, col.name)))?);
                }
                out.push(cells);
            }
            rows.tables.push(out);
        }
        for (j, value_sql) in &schema.junctions {
            if !check_shape(&shapes, j.name, &[("owner", "BINARY(16) NOT NULL"), ("position", "INT NOT NULL"), ("value", value_sql)])
                .map_err(damaged)?
            {
                rows.junctions.push(Vec::new());
                continue;
            }
            let mut out = Vec::new();
            let read = c
                .query_iter(format!("SELECT `owner`, `position`, `value` FROM `{}` LIMIT {}", j.name, MAX_TABLE_ROWS + 1))
                .map_err(|e| damaged(format!("the table {} cannot be read: {e}", j.name)))?;
            for row in read {
                let row = row.map_err(|e| damaged(format!("the table {} cannot be read: {e}", j.name)))?;
                if out.len() == MAX_TABLE_ROWS {
                    return Err(damaged(format!("the table {} has more than {MAX_TABLE_ROWS} rows", j.name)));
                }
                let get = |i: usize| row.as_ref(i).cloned().unwrap_or(mysql::Value::NULL);
                let (owner, at, value) = (get(0), get(1), get(2));
                for v in [&owner, &at, &value] {
                    budget.take(v).map_err(damaged)?;
                }
                let Cell::Id(owner) = from_sql(&owner, "BINARY(16)").map_err(|why| damaged(format!("{}: {why}", j.name)))? else {
                    return Err(damaged(format!("{} has a row without its owner", j.name)));
                };
                let at = match from_sql(&at, "INT NOT NULL") {
                    Ok(Cell::Int(i)) => u32::try_from(i).ok(),
                    _ => None,
                }
                .ok_or_else(|| damaged(format!("{} has a position that is not a number in range", j.name)))?;
                let value = from_sql(&value, value_sql).map_err(|why| damaged(format!("{}: {why}", j.name)))?;
                out.push((owner, at, value));
            }
            rows.junctions.push(out);
        }
        Ok(rows)
    }

    /// Whether the project holds changes not yet committed. The conversations are left out: they
    /// are committed in chat commits of their own, and a chat message never makes the project
    /// unsaved (owner decision, AR-206).
    pub fn has_unsaved(&self) -> Result<bool, Error> {
        let mut c = self.model_connection()?;
        Ok(!unsaved_tables(&mut c, self.path())?.is_empty())
    }

    /// Save: a Dolt commit of every table written except the conversations', with `message`. False
    /// when there was nothing to commit, which is a save all the same. The tables are staged by
    /// name, never with `-A`, so a turn still running is never committed half-way by a Save: its rows
    /// wait for the turn's own chat commit.
    pub fn commit(&self, message: &str) -> Result<bool, Error> {
        let mut c = self.model_connection()?;
        let tables = unsaved_tables(&mut c, self.path())?;
        if tables.is_empty() {
            self.changes_saved();
            return Ok(false);
        }
        // Whatever an interrupted step left staged is unstaged first, so only these tables go in.
        c.query_drop("CALL DOLT_RESET()").map_err(sql("cannot prepare the save"))?;
        stage(&mut c, &tables).map_err(sql("cannot prepare the save"))?;
        // The commit's reply is not read: converting a row that is not one string would panic.
        c.exec_drop("CALL DOLT_COMMIT('-m', ?, '--author', ?)", (message, AUTHOR)).map_err(sql("cannot save the project"))?;
        self.changes_saved();
        Ok(true)
    }

    /// The messages of the newest `limit` commits of the project, newest first.
    pub fn newest_commit_messages(&self, limit: u32) -> Result<Vec<String>, Error> {
        let mut c = self.model_connection()?;
        // In the log's own order: commits made within the same instant have no order by date.
        let query = format!("SELECT message FROM dolt_log LIMIT {limit}");
        let rows: Vec<mysql::Row> = c.query(query).map_err(sql("cannot read the project's history"))?;
        rows.iter()
            .map(|r| match r.as_ref(0) {
                Some(mysql::Value::Bytes(b)) => std::str::from_utf8(b).ok().map(str::to_string),
                _ => None,
            })
            .map(|m| {
                m.ok_or_else(|| Error::Damaged { path: self.path().to_path_buf(), why: "a commit in its history has no readable message".into() })
            })
            .collect()
    }

    /// Don't save: puts the model back to its last save, throwing away every change written since
    /// (owner decision): the tables it had are reset, and those made since (a notation's table first
    /// used after the save) are removed. A project never saved goes back to how it was created; its
    /// model's tables are made again when rows next go there.
    ///
    /// The conversations are kept (owner decision, AR-206): whatever of them waits is committed in a
    /// chat commit first, so the reset goes back to a commit that holds them, and Don't save is
    /// refused when that fails or leaves any of them uncommitted. Then each turn of this session
    /// that changed the model goes back to the state its changes had at the last save, or becomes
    /// discarded when it ran since, stamped `now`, in a chat commit of its own.
    pub fn discard(&self, now: &str) -> Result<(), Error> {
        let mut c = self.model_connection()?;
        self.keep_chats(&mut c)?;
        c.query_drop("CALL DOLT_RESET('--hard')").map_err(sql("cannot put the project back to its last save"))?;
        c.query_drop("CALL DOLT_CLEAN()").map_err(sql("cannot remove the tables made since the last save"))?;
        self.chats_after_discard(&mut c, now);
        Ok(())
    }
}

/// The most rows of `dolt_status` read: each table may be listed staged and not staged.
const MAX_STATUS: usize = 2 * MAX_TABLES + 2;

/// The tables `dolt_status` lists as changed, by name: a renamed table under both its names.
pub(crate) fn changed_tables(c: &mut impl Queryable, path: &std::path::Path) -> Result<std::collections::BTreeSet<String>, Error> {
    let rows: Vec<mysql::Row> = c
        .query(format!("SELECT LEFT(CAST(table_name AS CHAR), 160) FROM dolt_status LIMIT {}", MAX_STATUS + 1))
        .map_err(sql("cannot read the project's status"))?;
    if rows.len() > MAX_STATUS {
        return Err(Error::Damaged { path: path.to_path_buf(), why: format!("its status lists more than {MAX_STATUS} tables") });
    }
    let mut names = std::collections::BTreeSet::new();
    for row in &rows {
        let Some(name) = row.get_opt::<String, usize>(0).and_then(Result::ok) else {
            return Err(Error::Damaged { path: path.to_path_buf(), why: "its status names a table that does not read".into() });
        };
        match name.split_once(" -> ") {
            Some((from, to)) => {
                names.insert(from.to_string());
                names.insert(to.to_string());
            }
            None => {
                names.insert(name);
            }
        }
    }
    Ok(names)
}

/// The changed tables a Save commits: every one but the conversations'.
fn unsaved_tables(c: &mut impl Queryable, path: &std::path::Path) -> Result<Vec<String>, Error> {
    Ok(changed_tables(c, path)?.into_iter().filter(|name| !is_chat_table(name)).collect())
}

/// Stages exactly `tables`, by name.
pub(crate) fn stage(c: &mut impl Queryable, tables: &[String]) -> Result<(), mysql::Error> {
    for chunk in tables.chunks(BATCH) {
        c.exec_drop(format!("CALL DOLT_ADD({})", vec!["?"; chunk.len()].join(", ")), chunk.to_vec())?;
    }
    Ok(())
}

/// Splits `rows` into runs of at most [`BATCH`] rows and [`BATCH_BYTES`] bytes (a run holds at
/// least one row, which the packet limit still fits: a row is at most a few megabytes).
fn batches<T>(rows: &[T], bytes: impl Fn(&T) -> usize) -> Vec<&[T]> {
    let mut out = Vec::new();
    let (mut start, mut size) = (0, 0);
    for (i, row) in rows.iter().enumerate() {
        let b = bytes(row);
        if i > start && (i - start == BATCH || size + b > BATCH_BYTES) {
            out.push(&rows[start..i]);
            (start, size) = (i, 0);
        }
        size += b;
    }
    if start < rows.len() {
        out.push(&rows[start..]);
    }
    out
}

/// The bytes a cell takes in a statement.
fn cell_bytes(cell: &Cell) -> usize {
    match cell {
        Cell::Text(t) => t.len() + 4,
        Cell::Id(_) => 20,
        _ => 8,
    }
}

/// What a reading has taken so far of the whole model's budgets.
#[derive(Default)]
struct Budget {
    cells: usize,
    bytes: usize,
}

impl Budget {
    /// A row's list of cells.
    fn row(&mut self) -> Result<(), String> {
        self.bytes += std::mem::size_of::<Vec<Cell>>();
        self.check()
    }

    /// A cell as it is kept: the cell, and a text's bytes and shared header.
    fn take(&mut self, v: &mysql::Value) -> Result<(), String> {
        self.cells += 1;
        self.bytes += std::mem::size_of::<Cell>()
            + match v {
                mysql::Value::Bytes(b) => b.len() + 16,
                _ => 0,
            };
        self.check()
    }

    fn check(&self) -> Result<(), String> {
        if self.cells > MAX_READ_CELLS || self.bytes > MAX_READ_BYTES {
            return Err(format!("the model is larger than Archforge reads ({MAX_READ_CELLS} values, {} MB)", MAX_READ_BYTES >> 20));
        }
        Ok(())
    }
}

/// The statements that bring a project made by an earlier Archforge up to `schema`: a column the
/// generated tables have that a table lacks is added when it comes after the columns the table
/// has, empty or holding what a new object holds; a table missing is made when rows go there.
/// Refused when the project holds a table Archforge does not know, a table whose columns are not
/// the first ones of its generated shape, or a list table of another shape: a project of another
/// model, or of a later Archforge.
fn upgrade(c: &mut mysql::Conn, schema: &ModelSchema) -> Result<Vec<String>, Refusal> {
    let shapes = shapes(c, schema).map_err(Refusal::Read)?;
    if let Some(name) = shapes.iter().find_map(|(name, shape)| matches!(shape, Shape::Unknown).then_some(name)) {
        return Err(Refusal::Shape(format!("it holds the table {name:?}, which this Archforge does not know")));
    }
    let mut added = Vec::new();
    for t in &schema.tables {
        let found = match shapes.get(t.name) {
            None => continue,
            Some(Shape::Columns(found)) => found,
            Some(_) => return Err(Refusal::Shape(format!("{} is not a table", t.name))),
        };
        let expected: Vec<(&str, &str)> = t.columns.iter().map(|col| (col.name, col.sql)).collect();
        if found.len() > expected.len() || !found.iter().zip(&expected).all(|(f, e)| same_column(f, e)) {
            return Err(Refusal::Shape(format!("the table {} has columns this Archforge does not write", t.name)));
        }
        for (name, sql) in &expected[found.len()..] {
            if !sql.contains("NOT NULL") {
                added.push(format!("ALTER TABLE `{}` ADD COLUMN `{name}` {sql}", t.name));
                continue;
            }
            // The rows already there take what a new object holds.
            let Some((_, _, fill)) = schema.fills.iter().find(|(table, column, _)| *table == t.name && column == name) else {
                return Err(Refusal::Shape(format!("the table {} lacks the column {name}, which has no value to give the rows it holds", t.name)));
            };
            added.push(format!("ALTER TABLE `{}` ADD COLUMN `{name}` {sql} DEFAULT {fill}", t.name));
        }
    }
    for (j, value) in &schema.junctions {
        check_shape(&shapes, j.name, &[("owner", "BINARY(16) NOT NULL"), ("position", "INT NOT NULL"), ("value", value)]).map_err(Refusal::Shape)?;
    }
    Ok(added)
}

/// Why a project is not upgraded: its tables are not an earlier shape of the model's, or they
/// could not be read.
enum Refusal {
    Shape(String),
    Read(String),
}

/// A column as `information_schema` gives it: its name, data type and length, whether it is plain
/// (not generated), and whether it may be empty.
pub(crate) type Found = (Option<String>, Option<String>, Option<i64>, bool, Option<bool>);

/// Whether a column found is the one declared `(name, sql)`; a column declared with a length, such
/// as `BINARY(16)`, must have exactly that length.
pub(crate) fn same_column((name, ty, length, plain, nullable): &Found, (want_name, want_sql): &(&str, &str)) -> bool {
    let want = data_type(want_sql);
    let want_length = want_sql.split_once('(').and_then(|(_, rest)| rest.split_once(')')).and_then(|(n, _)| n.parse::<i64>().ok());
    *plain
        && name.as_deref() == Some(*want_name)
        && ty.as_deref() == Some(want.as_str())
        && want_length.is_none_or(|n| *length == Some(n))
        && *nullable == Some(!want_sql.contains("NOT NULL"))
}

impl ModelSchema {
    /// Whether `name` is one of the model's tables or list tables.
    fn is_model_table(&self, name: &str) -> bool {
        self.tables.iter().any(|t| t.name == name) || self.junctions.iter().any(|(j, _)| j.name == name)
    }

    /// Whether `name` is a table the project holds: the model's, the settings, the tables in use or
    /// the conversations'.
    fn knows(&self, name: &str) -> bool {
        name == "settings" || name == IN_USE || is_chat_table(name) || self.is_model_table(name)
    }
}

/// Makes the table of the model's tables in use.
fn create_in_use(c: &mut impl Queryable) -> Result<(), Error> {
    c.query_drop(format!("CREATE TABLE IF NOT EXISTS `{IN_USE}` (`name` VARCHAR(64) NOT NULL PRIMARY KEY)"))
        .map_err(sql("cannot create the table of the model's tables"))
}

/// Records `names` as model tables in use; a name recorded already stays as it is.
fn record_in_use(c: &mut impl Queryable, names: &[&str]) -> Result<(), Error> {
    for chunk in names.chunks(BATCH) {
        c.exec_drop(
            format!("INSERT INTO `{IN_USE}` (`name`) VALUES {} ON DUPLICATE KEY UPDATE `name` = `name`", vec!["(?)"; chunk.len()].join(", ")),
            chunk.to_vec(),
        )
        .map_err(sql("cannot record the model's tables"))?;
    }
    Ok(())
}

/// The model tables rows have gone to, as the project records them in a table of exactly the shape
/// Archforge made.
fn in_use(c: &mut mysql::Conn, shapes: &BTreeMap<String, Shape>) -> Result<std::collections::BTreeSet<String>, String> {
    if !check_shape(shapes, IN_USE, &[("name", "VARCHAR(64) NOT NULL")])? {
        return Err("the list of the model's tables in use is missing".into());
    }
    let rows: Vec<mysql::Row> = c
        .query(format!("SELECT LEFT(CAST(`name` AS CHAR), 64) FROM `{IN_USE}` LIMIT {}", MAX_TABLES + 1))
        .map_err(|e| format!("the model's tables in use cannot be read: {e}"))?;
    if rows.len() > MAX_TABLES {
        return Err(format!("it records more than {MAX_TABLES} tables in use"));
    }
    rows.iter().map(|r| r.get_opt::<String, usize>(0).and_then(Result::ok).ok_or_else(|| "a table in use cannot be read".to_string())).collect()
}

/// The tables the project's database holds: each name, and whether it is a base table. Refused
/// when a name is one of the model's in another case: the database finds a table under any case,
/// so it would be read as missing and cleared as the model's.
fn tables_held(c: &mut mysql::Conn, schema: &ModelSchema) -> Result<BTreeMap<String, bool>, String> {
    tables_held_by(c, &|name| schema.knows(name))
}

/// [`tables_held`] for the tables `knows` names as Archforge's, in the database `c` is using: the
/// project's working set, or the commit a revision connection shows.
pub(crate) fn tables_held_by(c: &mut mysql::Conn, knows: &dyn Fn(&str) -> bool) -> Result<BTreeMap<String, bool>, String> {
    let names: Vec<mysql::Row> = c
        .query(format!(
            "SELECT LEFT(CAST(table_name AS CHAR), 64), LEFT(CAST(table_type AS CHAR), 32) FROM information_schema.tables WHERE table_schema = DATABASE() LIMIT {}",
            MAX_TABLES + 1
        ))
        .map_err(|e| format!("the tables cannot be listed: {e}"))?;
    if names.len() > MAX_TABLES {
        return Err(format!("it holds more than {MAX_TABLES} tables"));
    }
    let mut held = BTreeMap::new();
    for row in &names {
        let text = |i: usize| row.get_opt::<String, usize>(i).and_then(Result::ok);
        let name = text(0).ok_or("a table's name cannot be read")?;
        // Archforge names its tables in plain lower-case ASCII; the database finds one under any case.
        if !name.is_ascii() || (!knows(&name) && knows(&name.to_ascii_lowercase())) {
            return Err(format!("it holds the table {name:?}, named as one of Archforge's in another case or script"));
        }
        held.insert(name, text(1).as_deref() == Some("BASE TABLE"));
    }
    Ok(held)
}

/// What the project holds under a table's name.
enum Shape {
    /// A base table of the model, with its columns in order.
    Columns(Vec<Found>),
    /// A view, or anything else that is not a base table.
    NotATable,
    /// A table the model does not have (its columns are not looked at).
    Unknown,
    /// The settings or a conversations' table: the project's own, not the model's (the
    /// conversations' shapes are looked at apart, so a damaged one never blocks the model).
    Own,
}

/// The shape of every table the project's database holds, looked at in two queries however many
/// tables the model has: what one query per table would cost grows with every table the project
/// holds.
fn shapes(c: &mut mysql::Conn, schema: &ModelSchema) -> Result<BTreeMap<String, Shape>, String> {
    let mut shapes = BTreeMap::new();
    let mut model = Vec::new();
    for (name, base) in tables_held(c, schema)? {
        let shape = if name == "settings" || is_chat_table(&name) {
            Shape::Own
        } else if !schema.knows(&name) {
            Shape::Unknown
        } else if !base {
            Shape::NotATable
        } else {
            model.push(name.clone());
            Shape::Columns(Vec::new())
        };
        shapes.insert(name, shape);
    }
    for (table, found) in columns_of(c, model)? {
        if let Some(Shape::Columns(found_columns)) = shapes.get_mut(&table) {
            *found_columns = found;
        }
    }
    Ok(shapes)
}

/// The columns of `tables` in the database `c` is using, in order, looked at in one query: each
/// table may have at most [`MAX_COLUMNS`], and a table that has more is refused.
pub(crate) fn columns_of(c: &mut mysql::Conn, tables: Vec<String>) -> Result<BTreeMap<String, Vec<Found>>, String> {
    let mut out: BTreeMap<String, Vec<Found>> = tables.iter().map(|t| (t.clone(), Vec::new())).collect();
    if tables.is_empty() {
        return Ok(out);
    }
    // No table of Archforge's has more columns than this; a table that has is refused, and so is a
    // database whose tables have more in all than the reading looks at.
    let most = tables.len() * (MAX_COLUMNS + 1);
    let columns = c
        .exec_iter(
            format!(
                "SELECT LEFT(CAST(table_name AS CHAR), 64), LEFT(CAST(column_name AS CHAR), 64), LEFT(CAST(data_type AS CHAR), 32), \
                 character_maximum_length, LEFT(CAST(COALESCE(extra, '') AS CHAR), 64), LEFT(CAST(COALESCE(generation_expression, '') AS CHAR), 64), \
                 LEFT(CAST(is_nullable AS CHAR), 8) \
                 FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name IN ({}) ORDER BY table_name, ordinal_position LIMIT {}",
                vec!["?"; tables.len()].join(", "),
                most + 1
            ),
            tables,
        )
        .map_err(|e| format!("the columns of the tables cannot be looked at: {e}"))?;
    for (n, r) in columns.enumerate() {
        let r = r.map_err(|e| format!("the columns of the tables cannot be looked at: {e}"))?;
        if n == most {
            return Err("the tables have more columns than Archforge writes".into());
        }
        let text = |i: usize| r.get_opt::<String, usize>(i).and_then(Result::ok);
        // A generated column computes its value on every read: none is written.
        let plain = text(4).is_some_and(|e| e.trim().is_empty()) && text(5).is_some_and(|g| g.trim().is_empty());
        let nullable = match text(6).as_deref() {
            Some("YES") => Some(true),
            Some("NO") => Some(false),
            _ => None,
        };
        let found = (text(1), text(2).map(|t| t.to_ascii_lowercase()), r.get_opt::<i64, usize>(3).and_then(Result::ok), plain, nullable);
        match text(0).and_then(|table| out.get_mut(&table)) {
            Some(found_columns) if found_columns.len() < MAX_COLUMNS => found_columns.push(found),
            Some(_) => return Err("a table has more columns than Archforge writes".into()),
            None => return Err("a column of the tables cannot be read".into()),
        }
    }
    Ok(out)
}

/// The type name `information_schema` gives a column declared `sql`: its first word, lower case.
fn data_type(sql: &str) -> String {
    sql.split([' ', '(']).next().unwrap_or_default().to_ascii_lowercase()
}

/// Whether the project holds the table `table`; refused when it holds it other than as a base table
/// with exactly the `expected` columns, in order, of the declared types (a binary id column must
/// hold exactly 16 bytes).
fn check_shape(shapes: &BTreeMap<String, Shape>, table: &str, expected: &[(&str, &str)]) -> Result<bool, String> {
    match shapes.get(table) {
        None => Ok(false),
        Some(Shape::Columns(found)) if found.len() == expected.len() && found.iter().zip(expected).all(|(f, e)| same_column(f, e)) => Ok(true),
        Some(Shape::Columns(_)) => Err(format!("the table {table} does not have the shape Archforge writes")),
        Some(_) => Err(format!("{table} is not a table")),
    }
}

fn to_sql(cell: &Cell) -> mysql::Value {
    match cell {
        Cell::Null => mysql::Value::NULL,
        Cell::Bool(b) => mysql::Value::Int(i64::from(*b)),
        Cell::Int(i) => mysql::Value::Int(*i),
        Cell::Real(r) => mysql::Value::Double(*r),
        Cell::Text(t) => mysql::Value::Bytes(t.as_bytes().to_vec()),
        Cell::Id(u) => mysql::Value::Bytes(u.0.to_vec()),
    }
}

/// A database value read back as a cell of a column of `sql` type; anything else is refused.
fn from_sql(v: &mysql::Value, sql: &str) -> Result<Cell, String> {
    if *v == mysql::Value::NULL {
        return Ok(Cell::Null);
    }
    let int = || match v {
        mysql::Value::Int(i) => Some(*i),
        mysql::Value::UInt(u) => i64::try_from(*u).ok(),
        // Text protocol rows carry numbers as text.
        mysql::Value::Bytes(b) => std::str::from_utf8(b).ok().and_then(|s| s.parse().ok()),
        _ => None,
    };
    if sql.starts_with("BINARY(16)") {
        return match v {
            mysql::Value::Bytes(b) if b.len() == 16 => {
                let mut a = [0u8; 16];
                a.copy_from_slice(b);
                Ok(Cell::Id(Uuid(a)))
            }
            _ => Err("an id is not 16 bytes".into()),
        };
    }
    if sql.starts_with("MEDIUMTEXT") {
        return match v {
            mysql::Value::Bytes(b) if b.len() <= MAX_STRING * 4 => {
                let s = std::str::from_utf8(b).map_err(|_| "a text is not UTF-8".to_string())?;
                if s.chars().count() > MAX_STRING {
                    return Err(format!("a text is longer than {MAX_STRING} characters"));
                }
                Ok(Cell::Text(s.into()))
            }
            _ => Err("a text is too long or not text".into()),
        };
    }
    if sql.starts_with("TINYINT") {
        return match int() {
            Some(0) => Ok(Cell::Bool(false)),
            Some(1) => Ok(Cell::Bool(true)),
            _ => Err("a flag is neither 0 nor 1".into()),
        };
    }
    if sql.starts_with("DOUBLE") {
        let r = match v {
            mysql::Value::Double(d) => Some(*d),
            mysql::Value::Float(f) => Some(f64::from(*f)),
            mysql::Value::Bytes(b) => std::str::from_utf8(b).ok().and_then(|s| s.parse().ok()),
            _ => None,
        };
        return r.filter(|r| r.is_finite()).map(Cell::Real).ok_or_else(|| "a real number is not finite".into());
    }
    int().map(Cell::Int).ok_or_else(|| "a number is not a whole number".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_survive_the_trip_through_database_values() {
        let id = Uuid::parse("019a2bc3-2280-7000-8000-00000000000d").unwrap_or_else(|| panic!("an id"));
        for (cell, sql) in [
            (Cell::Null, "MEDIUMTEXT NULL"),
            (Cell::Bool(true), "TINYINT NOT NULL"),
            (Cell::Int(-1), "BIGINT NOT NULL"),
            (Cell::Int(42), "INT NOT NULL"),
            (Cell::Real(2.5), "DOUBLE NOT NULL"),
            (Cell::Text("Kunde · 顧客".into()), "MEDIUMTEXT NULL"),
            (Cell::Id(id), "BINARY(16) NULL"),
        ] {
            assert_eq!(from_sql(&to_sql(&cell), sql), Ok(cell), "{sql}");
        }
    }

    #[test]
    fn values_of_the_wrong_kind_or_size_are_refused() {
        assert!(from_sql(&mysql::Value::Bytes(vec![1, 2, 3]), "BINARY(16) NULL").is_err());
        assert!(from_sql(&mysql::Value::Bytes(vec![0xff, 0xfe]), "MEDIUMTEXT NULL").is_err());
        assert!(from_sql(&mysql::Value::Bytes(vec![b'a'; MAX_STRING * 4 + 1]), "MEDIUMTEXT NULL").is_err());
        assert!(from_sql(&mysql::Value::Int(2), "TINYINT NOT NULL").is_err());
        assert!(from_sql(&mysql::Value::Double(f64::NAN), "DOUBLE NOT NULL").is_err());
        assert!(from_sql(&mysql::Value::Bytes(b"twelve".to_vec()), "INT NOT NULL").is_err());
    }

    #[test]
    fn the_fingerprint_tells_two_descriptions_apart_and_is_stable() {
        assert_eq!(fingerprint("a b"), fingerprint("a b"));
        assert_ne!(fingerprint("a b"), fingerprint("a c"));
        assert_eq!(fingerprint("").len(), 16);
    }
}
