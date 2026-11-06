//! Which ER elements may share a name in one owner (AR-209), as SQL names them: a name is unique,
//! whatever its case, among the elements of one kind in one owner. The kinds:
//!
//! - a schema: one name per schema in the model, which is one database;
//! - a relation or a type: tables, domains, enum types, sequences and views share one name per
//!   schema, as relations and types do in SQL;
//! - a column: one name per column in its table;
//! - a relationship: one name per relationship in its schema (an unnamed one takes none);
//! - a constraint: the keys, foreign keys, checks and indexes of a table, one kind. A primary key
//!   and a unique constraint share one name property, so the check on open refuses two of them
//!   named alike in one table, and a foreign key, a check or an index two of its own class; the
//!   commands ask more of them, one name among every constraint of the schema
//!   (`rules::constraint_name_free`);
//! - a domain's check: one name per check in its domain;
//! - any other ER metaclass is a kind of its own.
//!
//! [`kind`] is the one rule. The project's check as it opens (`archforge_notations`) asks it for
//! every ER element, and every command that names or adds a schema, a table, a type, a sequence, a
//! view, a column or a relationship asks [`clash`], which asks the same rule of one name; a
//! domain's checks, all given at once, are compared among themselves (`rules::first_twice`), and a
//! constraint by the wider rule of its schema. So an edit the commands let through leaves a project
//! that opens again.

use archforge_mof::{ClassId, NameKind, Obj, Store};

use crate::{CHECK_CONSTRAINT, DOMAIN, ENTITY, ENUM_TYPE, FOREIGN_KEY, PRIMARY_KEY, SEQUENCE, TABLE_INDEX, UNIQUE_CONSTRAINT, VIEW};

/// The kind tables, domains, enum types, sequences and views are: one namespace per schema.
pub const RELATION: NameKind = NameKind::Group("relation");

/// The kind keys, foreign keys, checks and indexes are.
pub const CONSTRAINT: NameKind = NameKind::Group("constraint");

/// The kind an ER element of `class` is told apart within by its name, among its owner's elements.
pub fn kind(class: ClassId) -> Option<NameKind> {
    if [ENTITY, DOMAIN, ENUM_TYPE, SEQUENCE, VIEW].contains(&class) {
        return Some(RELATION);
    }
    if [PRIMARY_KEY, UNIQUE_CONSTRAINT, FOREIGN_KEY, CHECK_CONSTRAINT, TABLE_INDEX].contains(&class) {
        return Some(CONSTRAINT);
    }
    // A schema, a column, a relationship, a domain's check, and any other metaclass: each a kind
    // of its own.
    Some(NameKind::Class(class))
}

/// The element `owner` holds, other than `except`, that an element of `class` named `name` would
/// clash with there by [`kind`]: one of the same kind whose name differs only in case. None for an
/// empty name.
pub fn clash(store: &Store, owner: Obj, class: ClassId, name: &str, except: Option<Obj>) -> Option<Obj> {
    store.name_clash(owner, class, name, except, kind)
}
