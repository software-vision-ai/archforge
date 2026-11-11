//! The Model menu: a row for each thing the diagram tools make, arming the tool its toolbar button
//! arms, so the canvas then makes it through the same commands. Note and Link note are for every
//! diagram; the other rows are the open diagram's notation's. Rows answer as the table of actions does.

use crate::Shell;
use crate::actions::{Action, free};
use crate::canvas::{self, Tool};
use crate::home::off_words;
use crate::i18n::tr;
use crate::kinds::active_kind;
use crate::state::Explorer;
use crate::uml_classifier::{self, Classifier};
use crate::uml_relate::Relate;

const ER: Option<&str> = Some(archforge_er::DIAGRAM_KIND);
const UML: Option<&str> = Some(archforge_uml::CLASS_DIAGRAM);

/// A row of diagrams of `$kind` (any, for `None`) that arms a tool through `$set`.
macro_rules! row {
    ($kind:expr, $id:literal, $word:expr, $set:expr) => {
        Action { id: $id, label: |_| $word, available: |s| can_pick(s, $kind), run: |s| pick(s, $kind, $set), keys: None, search: None }
    };
}

/// A row that arms the classifier tool with `$kind`, as the tool's own menu does.
macro_rules! classifier {
    ($id:literal, $kind:ident) => {
        row!(UML, $id, Classifier::$kind.word(), |c| (c.uml_classifier, c.tool) = (Classifier::$kind, Tool::Place(uml_classifier::TOOL)))
    };
}

/// A row that arms the Relationship tool with `$kind`, as the tool's own menu does.
macro_rules! relation {
    ($id:literal, $kind:ident) => {
        row!(UML, $id, tr(Relate::$kind.word()), |c| (c.uml_relation, c.tool) = (Relate::$kind, Tool::Connect("relationship")))
    };
}

/// The rows of every diagram, last in the menu as they are on the toolbar.
static SHARED: &[Action] =
    &[row!(None, "model.note", tr("Note"), |c| c.tool = Tool::Note), row!(None, "model.link", tr("Link note"), |c| c.tool = Tool::Link)];

/// An ER diagram's rows.
static ER_ROWS: &[&[Action]] = &[&[
    row!(ER, "model.entity", tr("Entity"), |c| c.tool = Tool::Place("entity")),
    row!(ER, "model.relationship", tr("Relationship"), |c| c.tool = Tool::Connect("relationship")),
]];

/// A class diagram's rows: the classifier tool's kinds and Package, then the Relationship tool's sections.
static UML_ROWS: &[&[Action]] = &[
    &[
        classifier!("model.class", Class),
        classifier!("model.interface", Interface),
        classifier!("model.enumeration", Enumeration),
        classifier!("model.data-type", DataType),
        classifier!("model.primitive-type", PrimitiveType),
        row!(UML, "model.package", tr("Package"), |c| c.tool = Tool::Place("package")),
    ],
    &[
        relation!("model.relate.association", Association),
        relation!("model.relate.directed", Directed),
        relation!("model.relate.aggregation", Aggregation),
        relation!("model.relate.composition", Composition),
        relation!("model.relate.generalization", Generalization),
        relation!("model.relate.dependency", Dependency),
    ],
    &[relation!("model.relate.realization", Realization), relation!("model.relate.association-class", AssociationClass)],
    &[relation!("model.relate.n-ary", NAry), relation!("model.relate.template-binding", TemplateBinding)],
];

/// Every row of every kind.
pub(crate) fn rows() -> impl Iterator<Item = &'static Action> {
    [ER_ROWS, UML_ROWS].into_iter().flatten().flat_map(|section| section.iter()).chain(SHARED)
}

/// The ids of the menu's rows now, by section: the open diagram's notation's, then the shared ones.
pub fn sections(shell: &Shell) -> Vec<Vec<&'static str>> {
    let own = match active_kind(shell).map(|k| k.kind()) {
        kind if kind == ER => ER_ROWS,
        kind if kind == UML => UML_ROWS,
        _ => &[],
    };
    own.iter().copied().chain([SHARED]).map(|section| section.iter().map(|a| a.id).collect()).collect()
}

/// Whether a tool can be armed: a diagram of `kind` (any, for `None`) on the canvas of a project
/// that can change, with no version's details or changed element over it and nothing over the window.
fn can_pick(shell: &Shell, kind: Option<&str>) -> bool {
    let covered = shell.state.explorer == Explorer::SourceControl && (shell.versions.details.is_some() || shell.versions.element.is_some());
    let shown = shell.active.is_some() && active_kind(shell).is_some_and(|k| kind.is_none_or(|wanted| k.kind() == wanted));
    shown && !covered && free(shell) && shell.session.as_ref().is_some_and(|s| !s.read_only())
}

/// Arms a tool through `set`, closing the toolbar's popover. Asks again: a chosen row runs a frame later.
fn pick(shell: &mut Shell, kind: Option<&str>, set: fn(&mut canvas::State)) {
    if can_pick(shell, kind) {
        set(&mut shell.canvas);
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
