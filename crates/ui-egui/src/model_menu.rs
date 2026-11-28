//! The Model menu: a row for each thing the diagram tools make, arming the tool its toolbar button
//! arms, so the canvas then makes it through the same commands. Note and Link note are for every
//! diagram; the other rows are those the open diagram's kind answers, as data
//! ([`crate::kinds::DiagramKind::menu_rows`]), from which the menu makes its entries: each answers
//! the table of actions by its id ([`label`], [`available`], [`run`]).

use crate::Shell;
use crate::actions::free;
use crate::canvas::Tool;
use crate::home::off_words;
use crate::i18n::tr;
use crate::kinds::{DiagramKind, ModelRow, active_kind};
use crate::state::Explorer;

/// The rows of every diagram, last in the menu as they are on the toolbar.
static SHARED: &[ModelRow] = &[
    ModelRow { key: "model.note", word: || tr("Note"), tool: Tool::Note, choice: None },
    ModelRow { key: "model.link", word: || tr("Link note"), tool: Tool::Link, choice: None },
];

/// Every row of every kind of `kinds` (a shell's, as the table of actions asks), each with the kind
/// it is of, then the shared ones, of no kind.
pub(crate) fn rows(kinds: &[&'static dyn DiagramKind]) -> impl Iterator<Item = (Option<&'static str>, &'static ModelRow)> {
    let own = kinds.iter().flat_map(|kind| kind.menu_rows().iter().copied().flatten().map(|row| (Some(kind.kind()), row)));
    own.chain(SHARED.iter().map(|row| (None, row)))
}

/// The row `id` among those of the shell's kinds and the shared ones, with the kind it is of.
fn row(shell: &Shell, id: &str) -> Option<(Option<&'static str>, &'static ModelRow)> {
    rows(shell.kinds).find(|(_, row)| row.key == id)
}

/// The name of the row `id`, as its kind words it; none for an id that is no row of the menu.
pub(crate) fn label(shell: &Shell, id: &str) -> Option<String> {
    row(shell, id).map(|(_, row)| (row.word)())
}

/// Whether the row `id` can arm its tool now ([`can_pick`]); an id that is no row of the menu
/// cannot.
pub(crate) fn available(shell: &Shell, id: &str) -> bool {
    row(shell, id).is_some_and(|(kind, _)| can_pick(shell, kind))
}

/// Runs the row `id` ([`pick`]); an id that is no row of the menu does nothing.
pub(crate) fn run(shell: &mut Shell, id: &str) {
    if let Some((kind, row)) = row(shell, id) {
        pick(shell, kind, row);
    }
}

/// The rows of a diagram of `kind` (none: no diagram), by section: the kind's own, then the shared ones.
fn sections_of(kind: Option<&dyn DiagramKind>) -> impl Iterator<Item = &'static [ModelRow]> {
    kind.map_or(&[][..], |k| k.menu_rows()).iter().copied().chain([SHARED])
}

/// The ids of the menu's rows now, by section: the open diagram's kind's, then the shared ones.
pub fn sections(shell: &Shell) -> Vec<Vec<&'static str>> {
    sections_of(active_kind(shell)).map(|section| section.iter().map(|row| row.key).collect()).collect()
}

/// The rows the menu can show at once, one list per kind of `kinds`, for the access letters' tests.
#[cfg(any(test, feature = "testing"))]
pub(crate) fn every_menu(kinds: &[&'static dyn DiagramKind]) -> Vec<Vec<&'static str>> {
    kinds.iter().map(|kind| sections_of(Some(*kind)).flatten().map(|row| row.key).collect()).collect()
}

/// Whether a version's details or a changed element covers the canvas.
pub(crate) fn covered(shell: &Shell) -> bool {
    shell.state.explorer == Explorer::SourceControl && (shell.versions.details.is_some() || shell.versions.element.is_some())
}

/// Whether a tool can be armed: a diagram of `kind` (any, for `None`) on the canvas of a project
/// that can change, with no version's details or changed element over it and nothing over the window.
fn can_pick(shell: &Shell, kind: Option<&str>) -> bool {
    let shown = shell.active.is_some() && active_kind(shell).is_some_and(|k| kind.is_none_or(|wanted| k.kind() == wanted));
    shown && !covered(shell) && free(shell) && shell.session.as_ref().is_some_and(|s| !s.read_only())
}

/// Arms the tool of `row`, a row of diagrams of `kind` (any, for `None`), with the choice its
/// popover makes first, closing the toolbar's popover. Asks again: a chosen row runs a frame later.
fn pick(shell: &mut Shell, kind: Option<&str>, row: &ModelRow) {
    if can_pick(shell, kind) {
        if let Some(choose) = row.choice {
            choose(&mut shell.notation_states);
        }
        shell.canvas.tool = row.tool;
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
