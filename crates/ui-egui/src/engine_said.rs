//! The engine's own refusals in the user's words (AR-440): a command not available now, by the
//! reason its availability check gives, with a catalogue row keyed by each reason; a gesture under
//! way, a session that could not be put back after a failure, and undo or redo inside another
//! change as plain sentences; and the errors only a mistake in the program reaches (a parameter the
//! engine refuses before the command runs, a command that stopped on an internal error) as one
//! sentence, so no engine text reaches the user. The engine's own typed refusal, a change made for a
//! revision of the model other than the one it would run on (AR-574), is said here too, as is a
//! package's default diagram that cannot be read for its unsound binding (AR-576), and a package
//! move or rename the root package or the package Types refuses (AR-563).

use archforge_engine::{BindingFault, EngineError, Reason};

use crate::i18n::{fmt, tr};

/// Why a command is not available now, in the user's words, by the reason its availability check
/// gives; none for a reason the catalogues don't hold, which the engine's English then says.
pub(crate) fn disabled(reason: &str) -> Option<String> {
    use archforge_uml::{
        applications, bindings, commands, constraints, generalization_sets, members, nary, nesting, profiles, qualifiers, templates,
    };
    Some(match reason {
        archforge_engine::cmd::edit::NOTHING_TO_UNDO => tr("There is nothing to undo."),
        archforge_engine::cmd::edit::NOTHING_TO_REDO => tr("There is nothing to redo."),
        archforge_core::skills::NO_MODEL => tr("The project has no model root yet."),
        archforge_core::skills::NO_SKILLS => tr("There is no skill yet."),
        archforge_diagram::commands::NO_DIAGRAMS => tr("There is no diagram yet."),
        archforge_diagram::project::NO_ELEMENTS => tr("There is nothing to remove or describe yet."),
        archforge_er::commands::NO_SCHEMAS => tr("There is no schema yet."),
        archforge_er::commands::NO_ENTITIES => tr("There is no entity yet."),
        commands::NO_PACKAGES => tr("There is no package yet."),
        members::NO_CLASSES => tr("There is no class, interface, enumeration, data type or primitive type yet."),
        members::NO_CLASS => tr("There is no class yet."),
        members::NO_ENUMERATIONS => tr("There is no enumeration yet."),
        nesting::NO_OWNERS => tr("There is no class or interface yet."),
        qualifiers::NO_ASSOCIATIONS => tr("There is no association or association class yet."),
        nary::NO_NARY => tr("There is no n-ary association yet."),
        generalization_sets::NO_GENERALIZATIONS => tr("There is no generalization yet."),
        generalization_sets::NO_SETS => tr("There is no generalization set yet."),
        constraints::NO_CONTEXTS => tr("There is no class, interface, enumeration, data type, primitive type, association class or package yet."),
        constraints::NO_CONSTRAINTS => tr("There is no constraint yet."),
        templates::NO_TEMPLATES => tr("There is no class, interface or data type yet."),
        templates::NO_PARAMETERS => tr("There is no template parameter yet."),
        bindings::NO_TEMPLATES => tr("There is no template yet: give a class, an interface or a data type a template parameter first."),
        bindings::NO_BINDINGS => tr("There is no template binding yet."),
        profiles::NO_PROFILES => tr("There is no profile yet."),
        profiles::NO_STEREOTYPES => tr("There is no stereotype yet."),
        applications::NO_APPLICATIONS => tr("No stereotype is applied yet."),
        _ => return None,
    })
}

/// A refusal the engine types itself in the user's words: a change made for another revision of the
/// model than the one it would run on, or for none, a package's default diagram that cannot be
/// read, by what is wrong with its binding and the choice that mends it, and a package move or
/// rename refused because the root package stays or the package Types keeps the built-in types;
/// none for another reason, or for a kind of element the catalogues don't name.
pub(crate) fn reason(reason: &Reason) -> Option<String> {
    let types = archforge_uml::members::TYPES;
    Some(match reason {
        Reason::RootPackageStays => tr("The model's root package holds all the other packages, so it stays where it is."),
        Reason::TypesPackageKept => fmt(
            &tr("The package {types} keeps the built-in types: while it holds them, it keeps its name and stays under the root package."),
            &[("types", types)],
        ),
        Reason::TypesPackageExists => {
            fmt(&tr("There is a package {types} already, which keeps the built-in types; a second one can't sit beside it."), &[("types", types)])
        }
        Reason::HoldsBuiltInName { kind, name } => fmt(
            &tr("The package holds {what} named {name}, a built-in type's name, which in {types} would mean that type. Rename {name} first."),
            &[("what", &crate::panels::a_kind(kind)?), ("name", name), ("types", types)],
        ),
        Reason::StaleRevision { expected: Some(_), .. } => {
            tr("The model has changed since it was read, or another project was read. Read it again and retry. Nothing was changed.")
        }
        Reason::StaleRevision { expected: None, .. } => {
            tr("The change does not name the revision of the model it is for. Read the model and retry. Nothing was changed.")
        }
        Reason::InvalidBinding(BindingFault::Missing) => {
            tr("The package has class diagrams, but none is its default. Choose one of them as its default diagram.")
        }
        Reason::InvalidBinding(BindingFault::Duplicate) => {
            tr("The package has more than one default diagram. Choose the one of its class diagrams that stays its default.")
        }
        Reason::InvalidBinding(BindingFault::DeadTarget) => {
            tr("The package's default diagram no longer exists. Choose one of its class diagrams as its default diagram.")
        }
        Reason::InvalidBinding(BindingFault::WrongKind) => {
            tr("The package's default diagram is not a class diagram. Choose one of its class diagrams as its default diagram.")
        }
        Reason::InvalidBinding(BindingFault::WrongOwner) => {
            tr("The package's default diagram belongs to another element. Choose one of its own class diagrams as its default diagram.")
        }
        _ => return None,
    })
}

/// An engine error that is neither a handler's refusal nor a command not available nor read-only,
/// in the user's words: a gesture under way, a session that could not be put back after a failure
/// and undo or redo inside another change each as its sentence, any other as an internal error,
/// its English kept for the log.
pub(crate) fn said(e: &EngineError) -> String {
    match e {
        EngineError::InteractionRunning => tr("Another change is still under way. Try again once it ends."),
        EngineError::Poisoned { .. } => tr("The model could not be put back as it was after a failure. Reopen the project to go on."),
        EngineError::NotInBatch { .. } => tr("Undo and redo cannot run as part of another change."),
        other => {
            log::warn!("an engine error said as an internal one: {other}");
            tr("This could not be done because of an internal error. Nothing was changed.")
        }
    }
}
