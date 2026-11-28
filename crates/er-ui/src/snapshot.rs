//! What the screenshot tool, the app's tests and the shell's tests reach in ER's screens, behind
//! the `snapshot` feature: the words ER hands its lookups through a table ([`word_lists`]) and the
//! items of ER's own modules they name, each under its module's name. ER's modules stay private:
//! only what is listed here is reached from outside.
//!
//! Each item is ER's own code unchanged: none draws, reads or decides anything of its own.

/// The ER diagram's notation.
pub mod er_ui {
    pub use crate::er_ui::{ErUi, texts};
}

/// The ER diagram's kind: its tools and gestures.
pub mod er_kind {
    pub use crate::er_kind::ErKind;
}

/// ER's refusals in the user's words.
pub mod er_said {
    pub use crate::er_said::said;
}

/// ER's tables on its diagrams: their width, the height of one without columns, the height its
/// columns need, and the tables of a diagram grown to fit them.
pub mod er_place {
    pub use crate::er_place::{ENTITY_EMPTY_HEIGHT, ENTITY_WIDTH, content_height, fit_tables};
}

/// What ER keeps between frames, its menus in the shell's one open-menu slot, and how a notation's
/// state is read as ER's.
pub mod er_state {
    pub use crate::er_state::{ErState, er, er_mut, menus};
}

/// A table in Properties: the column editor, opened on a column or a new one, and its base types.
pub mod er_table {
    pub use crate::er_table::{BASE_TYPES, ColumnEdit, column_edit, column_edit_mut, open_column};
}

/// A schema's domains and enum types in Properties: their editor and its kinds.
pub mod er_types {
    pub use crate::er_types::{TypeKind, open};
}

/// A relationship's Properties: the role editor.
pub mod er_relation {
    pub use crate::er_relation::RoleEdit;
}

/// A table's indexes in Properties: their editor and its rows.
pub mod er_indexes {
    pub use crate::er_indexes::{open, rows};
}

/// A schema's sequences and views in Properties: their editor and its kinds.
pub mod er_objects {
    pub use crate::er_objects::{ObjectKind, open};
}

/// A table's keys in Properties: the key editor and its rows.
pub mod er_keys {
    pub use crate::er_keys::{KeyEdit, open, rows};
}

/// A table's check constraints in Properties: their editor.
pub mod er_checks {
    pub use crate::er_checks::{CheckEdit, open};
}

/// The Relationship tool and what it makes: a relationship connected, its kind, whether it has a
/// foreign key, and the mapping table it offers.
pub mod er_connect {
    pub use crate::er_connect::{MappingOffer, RelKind, connect, has_foreign_key, offer_mapping};
}

/// The Entity tool's icon, which the shell's tests give a sketch kind's tool.
pub mod icons {
    pub use crate::ux::icons::TOOL_ENTITY;
}

/// Every word one of ER's modules hands `tr` through a table rather than as a literal, which a
/// scan of the source cannot find, each list with the name of the module that says it (its file,
/// `src/<name>.rs`).
pub fn word_lists() -> Vec<(&'static str, Vec<&'static str>)> {
    macro_rules! lists {
        ($($name:ident),* $(,)?) => {
            vec![$((stringify!($name), crate::$name::texts().into_iter().collect::<Vec<&'static str>>())),*]
        };
    }
    lists!(er_ui, er_kind, er_connect, er_relation, er_table, er_keys, er_checks, er_indexes, er_types, er_subtypes, er_objects)
}
