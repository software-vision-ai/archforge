//! The engine's own refusals in the user's words (AR-440): a command not available now, by the
//! reason its availability check gives, with a catalogue row keyed by each reason, the shell's own
//! here and a notation's by its own hook (AR-786); a gesture under way, a session that could not be
//! put back after a failure, and undo or redo inside another change as plain sentences; and the
//! errors only a mistake in the program reaches (a parameter the engine refuses before the command
//! runs, a command that stopped on an internal error) as one sentence, so no engine text reaches
//! the user. The engine's own typed refusal, a change made for a revision of the model other than
//! the one it would run on (AR-574), is said here too, as is a diagram that stays because a
//! package's binding names it as its default (AR-578), and a default diagram's replacement refused
//! (AR-640, AR-641). Those only UML's commands give are UML's (`uml_said`).

use archforge_engine::{BindingFault, EngineError, Reason};

use crate::i18n::{fmt, tr};
use crate::notation_ui::NotationUi;

/// Why a command is not available now, in the user's words, by the reason its availability check
/// gives: the shell's own for the engine's, the skills' and the diagrams' commands, then the one
/// notation of `uis` whose command it is
/// ([`NotationUi::disabled`](crate::notation_ui::NotationUi::disabled)); none for a reason the
/// catalogues don't hold, which the engine's English then says.
pub fn disabled(uis: &[&'static dyn NotationUi], reason: &str) -> Option<String> {
    Some(match reason {
        archforge_engine::cmd::edit::NOTHING_TO_UNDO => tr("There is nothing to undo."),
        archforge_engine::cmd::edit::NOTHING_TO_REDO => tr("There is nothing to redo."),
        archforge_core::skills::NO_MODEL => tr("The project has no model root yet."),
        archforge_core::skills::NO_SKILLS => tr("There is no skill yet."),
        archforge_diagram::commands::NO_DIAGRAMS => tr("There is no diagram yet."),
        archforge_diagram::project::NO_ELEMENTS => tr("There is nothing to remove or describe yet."),
        _ => return uis.iter().find_map(|ui| ui.disabled(reason)),
    })
}

/// A refusal the engine or the diagrams' commands type in the user's words: a change made for
/// another revision of the model than the one it would run on, or for none, a diagram a package's
/// binding keeps, its own default or another element's diagram, with the choice to make first, and
/// a default diagram's replacement that is the diagram removed, is no other diagram of the same
/// kind of its package, or is named for a diagram that is no default (AR-640), or is given both as
/// another diagram and as a new one, or a new one's name that is no text (AR-641); none for another
/// reason.
pub(crate) fn reason(reason: &Reason) -> Option<String> {
    Some(match reason {
        Reason::StaleRevision { expected: Some(_), .. } => {
            tr("The model has changed since it was read, or another project was read. Read it again and retry. Nothing was changed.")
        }
        Reason::StaleRevision { expected: None, .. } => {
            tr("The change does not name the revision of the model it is for. Read the model and retry. Nothing was changed.")
        }
        Reason::DefaultDiagramOf { package, fault: None } => fmt(
            &tr("This diagram is the default diagram of {package}. Choose another class diagram as the default of {package} first."),
            &[("package", package)],
        ),
        Reason::DefaultDiagramOf { package, fault: Some(BindingFault::WrongOwner) } => fmt(
            &tr("{package} has another element's diagram as its default, so that diagram stays. Choose one of its own class diagrams first."),
            &[("package", package)],
        ),
        Reason::ReplacementIsRemoved => tr("A diagram cannot replace itself as it is removed. Choose another diagram as the replacement."),
        Reason::OneReplacement => tr("Choose either an existing diagram or a new one as the replacement, not both."),
        Reason::NameNotText => tr("A name has to be text."),
        Reason::ReplacementNotNeeded { diagram } => {
            fmt(&tr("{diagram} is not a default diagram, so it is removed without a replacement."), &[("diagram", diagram)])
        }
        Reason::NotReplacement { diagram, package } => fmt(
            &tr("{diagram} is not another diagram of {package} of the same kind, so it cannot replace the diagram being removed."),
            &[("diagram", diagram), ("package", package)],
        ),
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
