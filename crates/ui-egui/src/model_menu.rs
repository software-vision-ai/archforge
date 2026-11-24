//! The Model menu: a row for each thing the diagram tools make, arming the tool its toolbar button
//! arms, so the canvas then makes it through the same commands. Note and Link note are for every
//! diagram; the other rows are those the open diagram's kind answers
//! ([`crate::kinds::DiagramKind::menu_rows`]). Rows answer as the table of actions does.

use crate::Shell;
use crate::actions::{Action, free};
use crate::canvas::Tool;
use crate::home::off_words;
use crate::i18n::tr;
use crate::kinds::{DiagramKind, active_kind};
use crate::state::Explorer;

/// A row of diagrams of `$kind` (any, for `None`) that arms a tool through `$set`, labelled
/// `$word`: a diagram kind's rows are made with it, as the shared ones are.
macro_rules! menu_row {
    ($kind:expr, $id:literal, $word:expr, $set:expr) => {
        $crate::actions::Action {
            id: $id,
            label: |_| $word,
            available: |s| $crate::model_menu::can_pick(s, $kind),
            run: |s| $crate::model_menu::pick(s, $kind, $set),
            keys: None,
            search: None,
        }
    };
}
pub(crate) use menu_row;

/// The rows of every diagram, last in the menu as they are on the toolbar.
static SHARED: &[Action] = &[
    menu_row!(None, "model.note", tr("Note"), |s| s.canvas.tool = Tool::Note),
    menu_row!(None, "model.link", tr("Link note"), |s| s.canvas.tool = Tool::Link),
];

/// Every row of every kind of `kinds` (a shell's, as the table of actions asks), then the shared ones.
pub(crate) fn rows(kinds: &[&'static dyn DiagramKind]) -> impl Iterator<Item = &'static Action> {
    kinds.iter().flat_map(|kind| kind.menu_rows().iter().copied().flatten()).chain(SHARED)
}

/// The rows of a diagram of `kind` (none: no diagram), by section: the kind's own, then the shared ones.
fn sections_of(kind: Option<&dyn DiagramKind>) -> impl Iterator<Item = &'static [Action]> {
    kind.map_or(&[][..], |k| k.menu_rows()).iter().copied().chain([SHARED])
}

/// The ids of the menu's rows now, by section: the open diagram's kind's, then the shared ones.
pub fn sections(shell: &Shell) -> Vec<Vec<&'static str>> {
    sections_of(active_kind(shell)).map(|section| section.iter().map(|a| a.id).collect()).collect()
}

/// The rows the menu can show at once, one list per kind, for the access letters' tests.
#[cfg(test)]
pub(crate) fn every_menu() -> Vec<Vec<&'static str>> {
    crate::kinds::KINDS.iter().map(|kind| sections_of(Some(*kind)).flatten().map(|a| a.id).collect()).collect()
}

/// Whether a version's details or a changed element covers the canvas.
pub(crate) fn covered(shell: &Shell) -> bool {
    shell.state.explorer == Explorer::SourceControl && (shell.versions.details.is_some() || shell.versions.element.is_some())
}

/// Whether a tool can be armed: a diagram of `kind` (any, for `None`) on the canvas of a project
/// that can change, with no version's details or changed element over it and nothing over the window.
pub(crate) fn can_pick(shell: &Shell, kind: Option<&str>) -> bool {
    let shown = shell.active.is_some() && active_kind(shell).is_some_and(|k| kind.is_none_or(|wanted| k.kind() == wanted));
    shown && !covered(shell) && free(shell) && shell.session.as_ref().is_some_and(|s| !s.read_only())
}

/// Arms a tool through `set`, with the kind a notation's tool makes, closing the toolbar's popover.
/// Asks again: a chosen row runs a frame later.
pub(crate) fn pick(shell: &mut Shell, kind: Option<&str>, set: fn(&mut Shell)) {
    if can_pick(shell, kind) {
        set(shell);
        shell.canvas.popover = None;
        shell.canvas.menu_focus = None;
    }
}

/// Why the menu's rows are off, in words: the first thing that keeps them off.
pub(crate) fn why_off(shell: &Shell) -> String {
    if shell.session.is_none() || shell.active.is_none() {
        tr("Open or go back to a project first")
    } else if shell.looking_only() {
        off_words(shell)
    } else if shell.session.as_ref().is_some_and(|s| s.read_only()) {
        tr("The project is busy")
    } else {
        tr("Show a diagram first")
    }
}
