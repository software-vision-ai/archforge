//! What the app's screenshot tool, its tests across every catalogue and its window tests reach in
//! UML's screens, behind the `snapshot` feature: the sample builders, which make the sample model
//! and stage UML's screens through the narrow context; the words UML hands its lookups through a
//! table ([`word_lists`]); and the items of UML's own modules they name, each under its module's
//! name. UML's modules stay private: only what is listed here is reached from outside.
//!
//! Each item is UML's own code unchanged: none draws, reads or decides anything of its own.

/// The class diagram's notation and kind, the open-sample builder and the sample builders over it,
/// the words of UML's figures, and the heights the canvas gives a card's members and literals.
pub mod uml_ui {
    pub use crate::uml_ui::{
        UmlKind, UmlUi, literals_height, members_height, open_sample, put_literals, put_nested_priority, put_payment_kinds, put_repository, texts,
    };
}

/// What UML keeps between frames, its menus in the shell's one open-menu slot, and how a
/// notation's state is read as UML's.
pub mod uml_state {
    pub use crate::uml_state::{UmlState, menus, uml, uml_mut};
}

/// The class diagram's classifier tool and its menu of kinds.
pub mod uml_classifier {
    pub use crate::uml_classifier::{Classifier, KINDS, TOOL};
}

/// Properties for a class, a member and a package: the member editor and the type and target
/// menus' choices.
pub mod uml_panel {
    pub use crate::uml_panel::{
        FINAL_ID, LEAF_ID, MEMBER_LEAF_ID, MemberEdit, MemberKind, TypeChoices, choices_for_menu, command_of, edit_of, keep_choices, keep_targets,
        leaf_refusal, member_leaf_refusal, open, package_parts, path_parts, take_targets, type_choices, type_notes, write_in_sight,
    };
}

/// A member typed in place on its card.
pub mod uml_inline {
    pub use crate::uml_inline::{details_ink, open};
}

/// The Relationship tool's kinds, an association's ends, the ends' editor as the model has it and
/// why a typed multiplicity does not read.
pub mod uml_relate {
    pub use crate::uml_relate::{EndModifiers, EndsEdit, Relate, connect, ends_edit, multiplicity_problem, reconnect};
}

/// N-ary associations: drawn end by end, their ends' editor and the sample that recommends one.
pub mod uml_nary {
    pub use crate::uml_nary::{DraftEnd, Nary, RemoveAsk, armed, click, end_types, leg_start, open, put_recommends};
}

/// Generalization sets: their Create rows and their shortcut.
pub mod uml_sets {
    pub use crate::uml_sets::{CREATE_ID, PANEL_CREATE_ID, shortcut};
}

/// A constraint's editor and its row's text.
pub mod uml_constraints {
    pub use crate::uml_constraints::{open, row_text};
}

/// A qualifier's editor and its commit.
pub mod uml_qualifiers {
    pub use crate::uml_qualifiers::{commit, open};
}

/// A template parameter's editor, a parameter in use and the editor's bin.
pub mod uml_templates {
    pub use crate::uml_templates::{InUse, open, remove_parameter};
}

/// A template binding: its draft's line, the editor of a new one, its refusal and the draft's
/// Apply.
pub mod uml_bindings {
    pub use crate::uml_bindings::{apply_draft, draft_line, open_new, refusal};
}

/// Nested classifiers: their section's ids, an addition and the owner menu's rows.
pub mod uml_nesting {
    pub use crate::uml_nesting::{NESTED_ADD_ID, NESTED_ROW_ID, OWNER_ID, add_nested, owner_rows};
}

/// Profiles: their sections' ids, the editors, the profile diagram, the refusals and the profiles
/// the sample builder puts in.
pub mod uml_profiles {
    pub use crate::uml_profiles::{
        APPLIED_ADD_ID, APPLIED_EDIT_ID, APPLIED_ROW_ID, EXTENDS_ADD_ID, EXTENDS_ROW_ID, EXTENSION_ID, ProfileEdit, ProfileRefusal, USES_ID,
        add_inside, open_profile_diagram, put_profiles, removal_refusal, unapply_profile,
    };
}

/// Stereotypes applied: their section's ids and state and the values compartments.
pub mod uml_stereotypes {
    pub use crate::uml_stereotypes::{ADD_ID, APPLIED_ID, SHOW_ID, Stereotypes, show_values};
}

/// A package's card: its thumbnail's look and preview.
pub mod uml_package {
    pub use crate::uml_package::{MiniLook, preview};
}

/// Package navigation: entering a package's diagram, leaving it, the share of the canvas that
/// enters, the zoom it lands at and the package a diagram belongs to.
pub mod uml_package_nav {
    pub use crate::uml_package_nav::{ENTER_COVER, LANDING, enter, leave, package_of_diagram};
}

/// Package navigation's way back.
pub mod uml_nav_route {
    pub use crate::uml_nav_route::{Route, Step};
}

/// The icons only UML draws that the classifier tool's faces and the Models tree show.
pub mod icons {
    pub use crate::ux::icons::{DATA_TYPE, PRIMITIVE_TYPE, TOOL_DATA_TYPE, TOOL_ENUMERATION, TOOL_PRIMITIVE_TYPE};
}

/// Every word one of UML's modules hands `tr` through a table rather than as a literal, which a
/// scan of the source cannot find, each list with the name of the module that says it (its file,
/// `src/<name>.rs`).
pub fn word_lists() -> Vec<(&'static str, Vec<&'static str>)> {
    macro_rules! lists {
        ($($name:ident),* $(,)?) => {
            vec![$((stringify!($name), crate::$name::texts().into_iter().collect::<Vec<&'static str>>())),*]
        };
    }
    lists!(uml_ui, uml_panel, uml_relate, uml_sets, uml_qualifiers, uml_templates, uml_bindings, uml_nary, uml_nesting, uml_profiles, uml_stereotypes)
}
