//! The model in the project database (archforge-kjz.3): the generated tables of the metamodels a
//! project holds, created when missing; the model's rows written as one transaction that replaces
//! what the tables held, as the user works (owner decision archforge-gm2.17); read back only from
//! tables of exactly the generated shape, row by row within budgets of rows, cells and bytes, never
//! sizing an allocation from what is read; and Save, a Dolt commit of everything written.
//!
//! The tables are described by the metamodels' generated code (`Table`, `Junction`); a project
//! records a fingerprint of that description, and a project made for another one is refused rather
//! than read wrongly.

use archforge_mof::rows::{Cell, Rows};
use archforge_mof::schema::{Junction, Table};
use archforge_mof::{DataKind, MAX_STRING, TypeRef, Universe, Uuid};
use mysql::prelude::Queryable;

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

/// The model's tables as the project stores them: the generated tables and junctions with the SQL
/// type of each junction's value, and their fingerprint.
pub struct ModelSchema {
    tables: Vec<Table>,
    junctions: Vec<(Junction, &'static str)>,
    fingerprint: String,
}

impl ModelSchema {
    /// The schema of `tables` and `junctions` of the metamodels in `universe`.
    pub fn new(universe: &Universe, tables: Vec<Table>, junctions: Vec<Junction>) -> Result<ModelSchema, String> {
        let plain = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        let mut seen = std::collections::BTreeSet::new();
        for name in tables.iter().map(|t| t.name).chain(junctions.iter().map(|j| j.name)) {
            if !seen.insert(name) {
                return Err(format!("two tables are named {name}"));
            }
        }
        let mut text = String::new();
        for t in &tables {
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
        Ok(ModelSchema { tables, junctions: with_types, fingerprint: fingerprint(&text) })
    }

    pub fn tables(&self) -> &[Table] {
        &self.tables
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

fn sql(what: &'static str) -> impl Fn(mysql::Error) -> Error {
    move |source| Error::Sql { what, source }
}

impl Project {
    fn model_connection(&self) -> Result<mysql::Conn, Error> {
        let mut c = self.server().connect().map_err(Error::Server)?;
        c.query_drop(format!("USE `{DATABASE}`")).map_err(sql("cannot use the project database"))?;
        Ok(c)
    }

    /// Makes the model's tables the project lacks and records their fingerprint; a project made for
    /// other tables is refused.
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
                return Err(Error::OtherModel { path: self.path().to_path_buf(), found: f, expected: schema.fingerprint.clone() });
            }
            Some(_) => {}
            None => {
                c.exec_drop("INSERT INTO settings (`key`, `value`) VALUES (?, ?)", (SCHEMA_KEY, &schema.fingerprint))
                    .map_err(sql("cannot record the model's fingerprint"))?;
            }
        }
        for t in &schema.tables {
            let columns: Vec<String> = t.columns.iter().map(|col| format!("`{}` {}", col.name, col.sql)).collect();
            c.query_drop(format!("CREATE TABLE IF NOT EXISTS `{}` ({})", t.name, columns.join(", "))).map_err(sql("cannot create a model table"))?;
        }
        for (j, value) in &schema.junctions {
            c.query_drop(format!(
                "CREATE TABLE IF NOT EXISTS `{}` (`owner` BINARY(16) NOT NULL, `position` INT NOT NULL, `value` {value}, PRIMARY KEY (`owner`, `position`))",
                j.name
            ))
            .map_err(sql("cannot create a model list table"))?;
        }
        Ok(())
    }

    /// Replaces the model's rows with `rows`, all of them or none.
    pub fn write_model(&self, schema: &ModelSchema, rows: &Rows) -> Result<(), Error> {
        if rows.tables.len() != schema.tables.len() || rows.junctions.len() != schema.junctions.len() {
            return Err(Error::Damaged { path: self.path().to_path_buf(), why: "the rows do not match the model's tables".into() });
        }
        let mut c = self.model_connection()?;
        let mut tx = c.start_transaction(mysql::TxOpts::default()).map_err(sql("cannot start writing the model"))?;
        for (t, table_rows) in schema.tables.iter().zip(&rows.tables) {
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

    /// The model's rows as the project holds them. Every table must have exactly its generated
    /// shape (a base table with the generated columns and types, so no cell is longer than its type
    /// allows); the rows are read one at a time within the budgets of rows per table and of cells
    /// and bytes for the whole model.
    pub fn read_model(&self, schema: &ModelSchema) -> Result<Rows, Error> {
        let damaged = |why: String| Error::Damaged { path: self.path().to_path_buf(), why };
        let mut c = self.model_connection()?;
        let mut budget = Budget::default();
        let mut rows = Rows::default();
        for t in &schema.tables {
            let expected: Vec<(&str, &str)> = t.columns.iter().map(|col| (col.name, col.sql)).collect();
            check_shape(&mut c, t.name, &expected).map_err(&damaged)?;
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
                budget.row().map_err(&damaged)?;
                let mut cells = Vec::with_capacity(t.columns.len());
                for (i, col) in t.columns.iter().enumerate() {
                    let v = row.as_ref(i).cloned().unwrap_or(mysql::Value::NULL);
                    budget.take(&v).map_err(&damaged)?;
                    cells.push(from_sql(&v, col.sql).map_err(|why| damaged(format!("{}.{}: {why}", t.name, col.name)))?);
                }
                out.push(cells);
            }
            rows.tables.push(out);
        }
        for (j, value_sql) in &schema.junctions {
            check_shape(&mut c, j.name, &[("owner", "BINARY(16)"), ("position", "INT"), ("value", value_sql)]).map_err(&damaged)?;
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
                    budget.take(v).map_err(&damaged)?;
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

    /// Whether the project holds changes not yet committed.
    pub fn has_unsaved(&self) -> Result<bool, Error> {
        let mut c = self.model_connection()?;
        let any: Option<mysql::Row> = c.query_first("SELECT 1 FROM dolt_status LIMIT 1").map_err(sql("cannot read the project's status"))?;
        Ok(any.is_some())
    }

    /// Save: a Dolt commit of everything written, with `message`. False when there was nothing to
    /// commit, which is a save all the same.
    pub fn commit(&self, message: &str) -> Result<bool, Error> {
        if !self.has_unsaved()? {
            return Ok(false);
        }
        let mut c = self.model_connection()?;
        // The commit's reply is not read: converting a row that is not one string would panic.
        c.exec_drop("CALL DOLT_COMMIT('-Am', ?, '--author', ?)", (message, AUTHOR)).map_err(sql("cannot save the project"))?;
        Ok(true)
    }
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

/// The type name `information_schema` gives a column declared `sql`: its first word, lower case.
fn data_type(sql: &str) -> String {
    sql.split([' ', '(']).next().unwrap_or_default().to_ascii_lowercase()
}

/// Refuses a table that is not a base table with exactly the `expected` columns, in order, of the
/// declared types; a binary id column must hold exactly 16 bytes.
fn check_shape(c: &mut mysql::Conn, table: &str, expected: &[(&str, &str)]) -> Result<(), String> {
    let kind: Option<mysql::Row> = c
        .exec_first("SELECT table_type FROM information_schema.tables WHERE table_schema = ? AND table_name = ?", (DATABASE, table))
        .map_err(|e| format!("the table {table} cannot be looked at: {e}"))?;
    if kind.and_then(|r| r.get_opt::<String, usize>(0)).and_then(Result::ok).as_deref() != Some("BASE TABLE") {
        return Err(format!("{table} is not a table"));
    }
    let columns: Vec<mysql::Row> = c
        .exec(
            format!(
                "SELECT LEFT(CAST(column_name AS CHAR), 64), LEFT(CAST(data_type AS CHAR), 32), character_octet_length, \
                 LEFT(CAST(COALESCE(extra, '') AS CHAR), 64), LEFT(CAST(COALESCE(generation_expression, '') AS CHAR), 64) \
                 FROM information_schema.columns WHERE table_schema = ? AND table_name = ? ORDER BY ordinal_position LIMIT {}",
                MAX_COLUMNS + 1
            ),
            (DATABASE, table),
        )
        .map_err(|e| format!("the columns of {table} cannot be looked at: {e}"))?;
    let found: Vec<(Option<String>, Option<String>, Option<i64>, bool)> = columns
        .iter()
        .map(|r| {
            let text = |i: usize| r.get_opt::<String, usize>(i).and_then(Result::ok);
            // A generated column computes its value on every read: none is written.
            let plain = text(3).is_some_and(|e| e.trim().is_empty()) && text(4).is_some_and(|g| g.trim().is_empty());
            (text(0), text(1).map(|t| t.to_ascii_lowercase()), r.get_opt::<i64, usize>(2).and_then(Result::ok), plain)
        })
        .collect();
    let same = found.len() == expected.len()
        && found.iter().zip(expected).all(|((name, ty, octets, plain), (want_name, want_sql))| {
            let want = data_type(want_sql);
            *plain && name.as_deref() == Some(*want_name) && ty.as_deref() == Some(want.as_str()) && (want != "binary" || *octets == Some(16))
        });
    if same { Ok(()) } else { Err(format!("the table {table} does not have the shape Archforge writes")) }
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
