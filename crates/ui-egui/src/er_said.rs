//! The ER commands' typed refusals in the user's words (AR-438), which ER's own hook says
//! (`NotationUi::said`, AR-786): only the reasons ER's commands alone give, and only the ER kinds,
//! by the words the interface uses for them; a kind not listed is left to the engine's English, and
//! a reason the engine, the diagrams or UML give too is the shell's own (`panels::said`). The
//! reasons an ER command is not available now are said here too, as are the ER kinds a refusal
//! names (AR-785). A word none of a choice's words is said under the choice's label. A key's
//! columns (AR-492): none, a column of another table, one that may be null in a primary key, a key
//! a foreign key points at, and no primary key; a column twice by its name (AR-544).
//! Mapping tables (AR-505): a relationship that does not resolve into one, a table that does not
//! collapse back into a relationship, one that cannot be associative, by the rule it breaks, and
//! a mapping table a step would break, with why in the same words.
//! The key a foreign key points at (AR-501): one of another table, one without columns, one it
//! points at already, and identifying relationships that would run in a cycle. A foreign key's
//! columns (AR-502): none, too many, not as many as its key's, one twice and types that differ,
//! also for a foreign key a step would break.
//! Subtypes (AR-506): a table made its own subtype, one of another schema, one with a supertype
//! already, a circle, a chain too deep, and a part of a subtype's key changed on its own;
//! identifying keys in a cycle and a discriminator of another table take the key rules' rows.
//! A relationship (AR-504): identifying or a key to point at without its foreign key, a foreign key
//! for many to many, a table with itself without a role, and a role without a letter.
//! Found inside a step
//! (AR-493): a key that would break, with why in the same words, and keys built on each other too
//! deep. A column's generation (AR-494): the rule it breaks, also for a column on
//! a domain that would break, and a key or identity column asked to be nullable. A column's own
//! (AR-495): a default or collation its domain gives, an unlink of one that follows no column, and
//! a place past its table's last; a default none of an enum type's literals is the shell's row. A
//! column's or a domain's size (AR-496): the rule it breaks, with the type and the size. Checks
//! (AR-499): a domain's checks too many, one named twice and an item of another shape; an item's
//! key the list does not take and an empty expression are the shell's rows.
//! An index's columns (AR-500): none, too many, one twice, an item of another shape, and an index
//! column with both a column and an expression or with neither.
//! A removal (AR-508): an element removed where it is edited, and a part of a subtype's key
//! without its subtype.
//! What a column is typed by (AR-497): a type name that holds its own size, a domain and an enum
//! type together, and a type name or a size beside either.
//!
//! Domains and enum types (AR-498): a type that columns still take, with the first of them (a list
//! the engine cut ending in an ellipsis) and their count, and an enum type's literals: none, too
//! many, one that is no text or no name, or one twice.
//!
//! A foreign key's key, actions and relationship (AR-503): set null on a column that may never be
//! null, a relationship that does not join its two tables, and a column outside the primary key of
//! one that follows an identifying relationship; a key that is none and one without columns are
//! said by their own rows, also for a foreign key a step would break.

use archforge_engine::{GenerationRule, MappingRule, NotOneOf, Reason, SizeRule, WordPlace};

use crate::i18n::{fmt, tr, tr_ctx, trn};
use crate::notation_ui::NotationUi;

/// A kind of ER element as a refusal names it, with its article where the language has one, as
/// `archforge_er`'s commands type it; none for another kind.
pub(crate) fn a_kind(kind: &str) -> Option<String> {
    Some(match kind {
        "schema" => tr_ctx("refusal", "a schema"),
        "table" => tr_ctx("refusal", "a table"),
        "column" => tr_ctx("refusal", "a column"),
        "relationship" => tr_ctx("refusal", "a relationship"),
        "domain" => tr_ctx("refusal", "a domain"),
        "enum type" => tr_ctx("refusal", "an enum type"),
        "sequence" => tr_ctx("refusal", "a sequence"),
        "view" => tr_ctx("refusal", "a view"),
        "primary key" => tr_ctx("refusal", "a primary key"),
        "unique constraint" => tr_ctx("refusal", "a unique constraint"),
        "foreign key" => tr_ctx("refusal", "a foreign key"),
        "check" => tr_ctx("refusal", "a check"),
        "index" => tr_ctx("refusal", "an index"),
        "subtype" => tr_ctx("refusal", "a subtype"),
        "key" => tr_ctx("refusal", "a key"),
        "key column" => tr_ctx("refusal", "a key column"),
        _ => return None,
    })
}

/// A key's columns, as a list refused names them.
const KEY_COLUMN: &str = "key column";

/// An enum type's literals, as a list refused names them.
const LITERAL: &str = "enum literal";

/// A foreign key's columns, as its rules name them.
const FOREIGN_KEY_COLUMN: &str = "foreign key column";

/// An index's columns, as a list refused names them.
const INDEX_COLUMN: &str = "index column";

/// A subtype, as the refusals of a table's supertype name the relationship and the kind.
const SUBTYPE: &str = "subtype";

/// Why an ER command is not available now, in the user's words, by the reason its availability
/// check gives; none for another reason.
pub(crate) fn disabled(reason: &str) -> Option<String> {
    Some(match reason {
        archforge_er::commands::NO_SCHEMAS => tr("There is no schema yet."),
        archforge_er::commands::NO_ENTITIES => tr("There is no entity yet."),
        _ => return None,
    })
}

/// An ER command's refusal in the user's words, a reason it holds (why a key, a column or a literal
/// would break) said by the notations of `uis`; none for a reason or a kind this doesn't say.
pub(crate) fn said(uis: &[&'static dyn NotationUi], reason: &Reason) -> Option<String> {
    Some(match reason {
        Reason::IdEmpty { kind } => fmt(&tr("No id is given. Give the id of {what}."), &[("what", &a_kind(kind)?)]),
        Reason::NotA { kind, name } => fmt(&tr("{name} is not {what}."), &[("name", name), ("what", &a_kind(kind)?)]),
        Reason::IdsExpected { kind: KEY_COLUMN } => tr("Give the key's columns as a list of ids."),
        Reason::TooMany { kind: KEY_COLUMN, max } => fmt(&tr("A key holds at most {max} columns."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: KEY_COLUMN, name } => fmt(&tr("{name} appears twice in the key. Give each column once."), &[("name", name)]),
        Reason::TooFew { kind: KEY_COLUMN, min: 1 } => tr("A key needs at least one column."),
        Reason::NotColumnOf { column, table } => fmt(&tr("{column} is not a column of {table}."), &[("column", column), ("table", table)]),
        Reason::NullableKeyColumn { name } => fmt(&tr("{name} may be null, which a primary key column never is."), &[("name", name)]),
        Reason::KeyInUse { foreign_key, table } => fmt(
            &tr("The foreign key {foreign_key} points at this key of {table}. Change or remove it first."),
            &[("foreign_key", foreign_key), ("table", table)],
        ),
        Reason::NoPrimaryKey { table } => fmt(&tr("{table} has no primary key."), &[("table", table)]),
        Reason::KeyNotOf { table } => fmt(&tr("The key is not one of {table}'s keys."), &[("table", table)]),
        Reason::NoKeyColumns { table } => fmt(&tr("The key of {table} has no columns to refer to."), &[("table", table)]),
        Reason::IdentifyingCycle { child, parent } => {
            fmt(&tr("Identifying relationships would run in a cycle between {child} and {parent}."), &[("child", child), ("parent", parent)])
        }
        Reason::Itself { relationship: SUBTYPE, .. } => tr("A table cannot be a subtype of itself."),
        Reason::SchemasDiffer { relationship: SUBTYPE } => tr("The two tables are in different schemas. A subtype is in its supertype's schema."),
        Reason::HasSupertype { table } => fmt(&tr("{table} has a supertype already. A table has one supertype."), &[("table", table)]),
        Reason::Cycle { relationship: SUBTYPE } => tr("The supertype is a subtype of this table already, which would close a circle."),
        Reason::TooDeep { kind: SUBTYPE, max } => fmt(&tr("Subtypes would run deeper than {max} levels here."), &[("max", &max.to_string())]),
        Reason::InSubtypeKey => tr("This is part of a subtype's key. Remove the subtype or change the supertype's mapping instead."),
        Reason::IdentifyingWithoutForeignKey => tr("An identifying relationship needs its foreign key; turn the switch on first"),
        Reason::KeyWithoutForeignKey => tr("Only a relationship with its foreign key points at a key."),
        Reason::ManyToManyForeignKey => tr("A many-to-many relationship takes no foreign key. Add a mapping table instead."),
        Reason::RoleNeeded => tr("A relationship of a table with itself needs a role at the end its key points to, such as parent."),
        Reason::RoleWithoutLetter => tr("A role needs a letter or a digit, since it names key columns."),
        Reason::Again { relationship: "foreign key" } => tr("The foreign key points at this key already. Name the columns to change them."),
        Reason::TooFew { kind: FOREIGN_KEY_COLUMN, min: 1 } => tr("A foreign key needs at least one column."),
        Reason::TooMany { kind: FOREIGN_KEY_COLUMN, max } => fmt(&tr("A foreign key holds at most {max} columns."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: FOREIGN_KEY_COLUMN, name } => fmt(&tr("{name} appears twice in the foreign key."), &[("name", name)]),
        Reason::ColumnCount { table, key, .. } => {
            fmt(&tr("The foreign key needs as many columns as the key of {table}, which has {n}."), &[("table", table), ("n", &key.to_string())])
        }
        Reason::TypesDiffer { column, target, types } => return types_said(column, target, types),
        Reason::SetNullNotNullable { column } => fmt(&tr("Set null needs nullable columns, but {column} may never be null."), &[("column", column)]),
        Reason::RelationshipNotJoining { table, references } => {
            fmt(&tr("The relationship does not join {table} and {references}."), &[("table", table), ("references", references)])
        }
        Reason::IdentifyingOutsideKey { column } => {
            fmt(&tr("An identifying relationship's foreign key columns are part of the primary key, and {column} is not."), &[("column", column)])
        }
        Reason::WouldBreak { kind: "primary key" | "unique constraint", name, why } => {
            fmt(&tr("The key {name} would break: {why}"), &[("name", name), ("why", &crate::panels::said(uis, why)?)])
        }
        Reason::WouldBreak { kind: "foreign key", name, why } => {
            fmt(&tr("The foreign key {name} would break: {why}"), &[("name", name), ("why", &crate::panels::said(uis, why)?)])
        }
        Reason::TooDeep { kind: "key", max } => {
            fmt(&tr("Keys built on each other run deeper than {max} levels here, or in a loop."), &[("max", &max.to_string())])
        }
        Reason::RemovedWhereEdited => tr("This element is removed where it is edited."),
        Reason::SubtypeKeyPart => tr("This is part of a subtype's key, which goes only with its subtype."),
        Reason::Mapping(rule) => mapping_said(rule),
        Reason::WouldBreak { kind: "column", name, why } => {
            fmt(&tr("The column {name} would break: {why}"), &[("name", name), ("why", &crate::panels::said(uis, why)?)])
        }
        Reason::WouldBreak { kind: "table", name, why } => {
            fmt(&tr("The table {name} would break: {why}"), &[("name", name), ("why", &crate::panels::said(uis, why)?)])
        }
        Reason::Sequence(rule) => sequence_said(rule),
        Reason::ColumnOfOtherSchema { column } => {
            fmt(&tr("{column} is a column of another schema. A sequence's owner is a column of its schema."), &[("column", column)])
        }
        Reason::InNoTable { column } => fmt(&tr("The column {column} belongs to no table."), &[("column", column)]),
        Reason::NotIntegerType { column, ty } if ty.is_empty() => {
            fmt(&tr("{column} has no type. A sequence's owner has a whole-number type."), &[("column", column)])
        }
        Reason::NotIntegerType { column, ty } => {
            fmt(&tr("{column} is of type {type}. A sequence's owner has a whole-number type."), &[("column", column), ("type", ty)])
        }
        Reason::Generation(rule) => generation_said(*rule),
        Reason::NotNullable => tr("A key or identity column may never be null."),
        Reason::SizeMisfit { rule, ty, value } => size_said(*rule, ty, *value),
        Reason::SizeInTypeName => tr("A type name holds no size; give the length, precision or scale instead."),
        Reason::DomainAndEnum => tr("A column takes a domain or an enum type, not both."),
        Reason::DomainGives { what: "type" } => tr("A column on a domain takes the domain's type."),
        Reason::DomainGives { what: "size" } => tr("A column on a domain takes the domain's size."),
        Reason::DomainGives { what: "default" } => tr("A column on a domain takes the domain's default."),
        Reason::DomainGives { what: "collation" } => tr("A column on a domain takes the domain's collation."),
        Reason::EnumTypeTakesNo { what: "type" } => tr("A column of an enum type takes no type name of its own."),
        Reason::EnumTypeTakesNo { what: "size" } => tr("A column of an enum type takes no size."),
        Reason::FollowsNoColumn { name } => fmt(&tr("{name} follows no column."), &[("name", name)]),
        Reason::PastLastColumn { columns } => trn("The table has {n} column.", "The table has {n} columns.", u64::try_from(*columns).ok()?),
        Reason::TooMany { kind: "check", max } => fmt(&tr("Give at most {max} checks."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: "check", name } => fmt(&tr("Two checks are named {name}. Give each check its own name."), &[("name", name)]),
        Reason::ItemsExpected { kind: "check" } => tr("Give each check as text, or as an object with an expression and a name."),
        Reason::TooFew { kind: INDEX_COLUMN, min: 1 } => tr("An index needs at least one column."),
        Reason::TooMany { kind: INDEX_COLUMN, max } => fmt(&tr("An index holds at most {max} columns."), &[("max", &max.to_string())]),
        Reason::NamedTwice { kind: INDEX_COLUMN, name } => fmt(&tr("{name} is in the index twice. Give each column once."), &[("name", name)]),
        Reason::ItemsExpected { kind: INDEX_COLUMN } => {
            tr("Give each index column as a column's id, or as an object with an attribute or an expression.")
        }
        Reason::AttributeAndExpression { column } => {
            fmt(&tr("{column} is given with an expression too. An index column sorts by a column or by an expression."), &[("column", column)])
        }
        Reason::NoAttributeNorExpression => tr("An index column needs a column or an expression to sort by."),
        Reason::NotOneOf(no) => return not_one_of_said(no),
        Reason::TypeInUse(used) => {
            let n = u64::try_from(used.count).ok()?;
            let template = trn("{n} column takes {name}: {columns}. Retype it first.", "{n} columns take {name}: {columns}. Retype them first.", n);
            // The engine names the first columns only; a list it cut ends in an ellipsis.
            let cut = if used.count > used.columns.len() { ", …" } else { "" };
            fmt(&template, &[("name", &used.name), ("columns", &format!("{}{cut}", used.columns.join(", ")))])
        }
        Reason::NamesExpected { kind: LITERAL } => tr("Give the literals as a list of names."),
        Reason::TooFew { kind: LITERAL, min: 1 } => tr("An enum type needs at least one literal."),
        Reason::TooMany { kind: LITERAL, max } => fmt(&tr("An enum type has at most {max} literals."), &[("max", &max.to_string())]),
        Reason::LiteralName { name, .. } if name.is_empty() => tr("A literal can't be empty."),
        Reason::LiteralName { name, why } => {
            fmt(&tr("The literal \"{name}\" is not a valid name. {why}"), &[("name", name), ("why", &crate::panels::said(uis, why)?)])
        }
        Reason::NamedTwice { kind: LITERAL, name } => fmt(&tr("The literal \"{name}\" is given twice. Give each literal once."), &[("name", name)]),
        _ => return None,
    })
}

/// The rule a mapping table, or a relationship to become one, breaks, in the user's words (AR-505);
/// a table with fewer than two foreign keys in its primary key has none or one.
fn mapping_said(rule: &MappingRule) -> String {
    match rule {
        MappingRule::NotManyToMany => tr("Only a many-to-many relationship becomes a mapping table."),
        MappingRule::LacksEnd => tr("The relationship lacks one of its two tables."),
        MappingRule::TwoForeignKeys => tr("A mapping table has exactly two foreign keys."),
        MappingRule::NoRelationship { foreign_key } => {
            fmt(&tr("The foreign key {foreign_key} stands for no relationship."), &[("foreign_key", foreign_key)])
        }
        MappingRule::NoKey { foreign_key } => fmt(&tr("The foreign key {foreign_key} points at no key."), &[("foreign_key", foreign_key)]),
        MappingRule::OwnRelationship => tr("The table has a relationship of its own, so it does not collapse into a relationship."),
        MappingRule::OwnColumns => tr("The table has columns of its own, so it does not collapse into a relationship."),
        MappingRule::ColumnsOutsideKey => tr("Not all of the table's columns are in its primary key, so it does not collapse into a relationship."),
        MappingRule::UniqueConstraint => tr("The table has a unique constraint, so it does not collapse into a relationship."),
        MappingRule::Check => tr("The table has a check, so it does not collapse into a relationship."),
        MappingRule::Index => tr("The table has an index, so it does not collapse into a relationship."),
        MappingRule::KeysInPrimaryKey { table, keys: 0 } => {
            fmt(&tr("{table} has no foreign key within its primary key. A mapping table needs two."), &[("table", table)])
        }
        MappingRule::KeysInPrimaryKey { table, .. } => {
            fmt(&tr("{table} has only one foreign key within its primary key. A mapping table needs two."), &[("table", table)])
        }
    }
}

/// The rule a sequence's numbers break, in the user's words (AR-507).
fn sequence_said(rule: &archforge_engine::SequenceRule) -> String {
    use archforge_engine::SequenceRule;
    let (min, max) = (i64::MIN.to_string(), i64::MAX.to_string());
    let unread = |template: String, value: &str| fmt(&template, &[("value", value), ("min", &min), ("max", &max)]);
    match rule {
        SequenceRule::IncrementZero => tr("The increment can't be 0. A sequence moves by at least 1."),
        SequenceRule::MinimumUnread { value } => unread(tr("The minimum \"{value}\" is not a whole number between {min} and {max}."), value),
        SequenceRule::MaximumUnread { value } => unread(tr("The maximum \"{value}\" is not a whole number between {min} and {max}."), value),
        SequenceRule::MaximumNotAbove { maximum, minimum } => fmt(
            &tr("The maximum {maximum} is not greater than the minimum {minimum}."),
            &[("maximum", &maximum.to_string()), ("minimum", &minimum.to_string())],
        ),
        SequenceRule::StartBelow { start, minimum } => {
            fmt(&tr("The start {start} is below the minimum {minimum}."), &[("start", &start.to_string()), ("minimum", &minimum.to_string())])
        }
        SequenceRule::StartAbove { start, maximum } => {
            fmt(&tr("The start {start} is above the maximum {maximum}."), &[("start", &start.to_string()), ("maximum", &maximum.to_string())])
        }
    }
}

/// A foreign key's column and the key's column it matches, of different types, in the user's words
/// (AR-502), a column without a type said to have none; none for two without, which never differ.
fn types_said(column: &str, target: &str, types: &[Option<Box<str>>; 2]) -> Option<String> {
    let (own, theirs) = (types[0].as_deref().unwrap_or_default(), types[1].as_deref().unwrap_or_default());
    let args = [("column", column), ("type", own), ("target", target), ("target_type", theirs)];
    Some(match types {
        [Some(_), Some(_)] => fmt(&tr("{column} is of type {type}, but {target} is of type {target_type}."), &args),
        [None, Some(_)] => fmt(&tr("{column} has no type, but {target} is of type {target_type}."), &args),
        [Some(_), None] => fmt(&tr("{column} is of type {type}, but {target} has no type."), &args),
        [None, None] => return None,
    })
}

/// The rule a column's generation breaks, in the user's words (AR-494).
fn generation_said(rule: GenerationRule) -> String {
    match rule {
        GenerationRule::ExpressionNeedsComputed => tr("Only a computed column takes an expression."),
        GenerationRule::ComputedNeedsExpression => tr("A computed column needs an expression."),
        GenerationRule::ComputedTakesNoDefault => tr("A computed column takes no default."),
        GenerationRule::IdentityNeedsInteger => tr("An identity column needs an integer type."),
        GenerationRule::IdentityNotNullable => tr("An identity column may never be null."),
        GenerationRule::IdentityTakesNoDefault => tr("An identity column takes no default."),
        GenerationRule::IdentityTakesNoExpression => tr("An identity column takes no expression."),
    }
}

/// A size that does not fit its type, by the rule it breaks, in the user's words (AR-496); a column
/// without a type name of its own takes no size at all.
fn size_said(rule: SizeRule, ty: &str, value: i64) -> String {
    let value = value.to_string();
    let args = [("type", ty), ("value", value.as_str())];
    match rule {
        SizeRule::OutOfRange => fmt(&tr("The size {value} is not between 1 and 65535."), &args),
        SizeRule::TakesNoLength | SizeRule::TakesNoPrecision | SizeRule::TakesNoScale if ty.is_empty() => {
            tr("A column without a type name of its own takes no size.")
        }
        SizeRule::TakesNoLength => fmt(&tr("{type} takes no length."), &args),
        SizeRule::TakesNoPrecision => fmt(&tr("{type} takes no precision."), &args),
        SizeRule::TakesNoScale => fmt(&tr("{type} takes no scale."), &args),
        SizeRule::BinaryTooLong => fmt(&tr("A BINARY holds at most 255 bytes, not {value}."), &args),
        SizeRule::ScaleOverPrecision => fmt(&tr("The scale {value} is more than the precision."), &args),
        SizeRule::ScaleWithoutPrecision => tr("A scale needs a precision."),
    }
}

/// A word that is none of an ER choice's words, under the label the interface shows the choice by,
/// with the words it takes: an index column's direction or nulls (AR-491), an empty word (the
/// commands trim it, so one of only spaces too) said as no choice given; none for a place that is
/// not ER's.
fn not_one_of_said(no: &NotOneOf) -> Option<String> {
    let field = match no.what {
        WordPlace::IndexDirection => tr("Direction"),
        WordPlace::IndexNulls => tr("Nulls"),
        _ => return None,
    };
    let words = no.known.join(", ");
    let args = [("field", field.as_str()), ("word", no.word.as_str()), ("words", words.as_str())];
    if no.word.is_empty() {
        return Some(fmt(&tr("{field}: no choice is given. The choices are {words}."), &args));
    }
    Some(fmt(&tr("{field}: {word} is not one of the choices. The choices are {words}."), &args))
}
