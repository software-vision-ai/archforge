//! The command table: what a command is, the `cmd!` forms that register one, and the registry a
//! notation builds from its areas' tables.

pub mod edit;

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::params::{ParamDoc, Params, parse_params};
use crate::{CommandError, Document, Reason, View};

/// What a handler acts on: the document, which undo covers, the view, which it never touches, the
/// counter new ids come from, which undo never rolls back, so an id is never given twice, and the
/// revision the call runs on, which it only reads.
pub struct Ctx<'a, D, V> {
    pub doc: &'a mut D,
    pub view: &'a mut V,
    pub next: &'a mut u64,
    pub(crate) at: Revision,
}

/// A point in a session's undo history: which history, and the revision within it. A revision
/// means something only within its own history, as every session counts its revisions from 0, so
/// a point another session, an earlier opening of the project or, once the host has seeded the
/// numbering ([`crate::seed_histories`]), an earlier run gave never matches by chance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Revision {
    pub history: u64,
    pub revision: u64,
}

/// The parameter a command that changes the model only on the revision its caller read takes:
/// `{ "history": h, "revision": r }`, as the project state gives them ([`crate::Session::token`]).
/// Such a command declares it `object?` and calls [`expected_revision`] first, so a token left out
/// or null reaches the check and is refused typed, not as a missing parameter.
pub const EXPECTED_REVISION: &str = "expected_revision";

impl<D, V> Ctx<'_, D, V> {
    /// The revision the call runs on: the session's token when the call began (in a batch, the
    /// batch's), a preview's own while a gesture shows one ([`crate::Session::token`]).
    pub fn revision(&self) -> Revision {
        self.at
    }

    /// The next number for a new id, or None when the counter is spent.
    pub fn fresh(&mut self) -> Option<u64> {
        let n = *self.next;
        let after = n.checked_add(1)?;
        *self.next = after;
        Some(n)
    }
}

/// Refuses, typed as [`Reason::StaleRevision`], unless `params` holds the
/// [`EXPECTED_REVISION`] the call runs on: its history and its revision both, as whole numbers.
/// Left out, null, or not of that shape, it is refused alike (none expected), naming the revision
/// the call runs on. Like
/// every refusal it changes nothing and leaves no undo step. A journal entry that holds one is
/// refused when replayed in another session, whose history differs.
pub fn expected_revision<D, V>(ctx: &Ctx<'_, D, V>, params: &Params) -> Result<(), CommandError> {
    let current = ctx.revision();
    let field = |v: &Value, key: &str| v.get(key).and_then(Value::as_u64);
    let expected = params.get(EXPECTED_REVISION).and_then(|v| Some(Revision { history: field(v, "history")?, revision: field(v, "revision")? }));
    if expected == Some(current) {
        return Ok(());
    }
    let message = match expected {
        Some(e) => format!(
            "the model is at revision {} of history {}, not revision {} of history {}; read the project again and retry",
            current.revision, current.history, e.revision, e.history
        ),
        None => format!(
            "it must be the history and the revision the project state gives, now {{\"history\": {}, \"revision\": {}}}",
            current.history, current.revision
        ),
    };
    Err(CommandError::about(EXPECTED_REVISION, message).because(Reason::StaleRevision { expected, current }))
}

/// [`expected_revision`] for a command that takes the revision optionally: given, it is checked
/// and refused alike when it is not the one the call runs on; left out, or null, which the
/// parameters leave out, the call runs on whichever revision the model is at.
pub fn expected_revision_if_given<D, V>(ctx: &Ctx<'_, D, V>, params: &Params) -> Result<(), CommandError> {
    if params.get(EXPECTED_REVISION).is_some() { expected_revision(ctx, params) } else { Ok(()) }
}

/// What an enabled predicate sees: the state the command would run on, and the history.
pub struct Facts<'a, D, V> {
    pub doc: &'a D,
    pub view: &'a V,
    pub can_undo: bool,
    pub can_redo: bool,
}

pub type RunFn<D, V> = fn(&mut Ctx<'_, D, V>, &Params) -> Result<Value, CommandError>;
pub type EnabledFn<D, V> = fn(&Facts<'_, D, V>) -> Result<(), &'static str>;
/// The check a registry may hold that sees every change just before it becomes kept history: the
/// document as the change left it, and the change. It runs once at each commit point (a command
/// run on its own, a gesture's commit, a whole batch), never per command inside a batch or per
/// preview, so a batch may pass through a state it would refuse. A refusal takes the change back
/// whole and is given to the caller as the command's failure.
pub type CommitCheck<D> = fn(&D, &<D as Document>::Delta) -> Result<(), CommandError>;

/// What runs a command: a handler, or the session's own undo and redo.
pub enum Handler<D, V> {
    Run(RunFn<D, V>),
    Undo,
    Redo,
}

impl<D, V> Clone for Handler<D, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<D, V> Copy for Handler<D, V> {}

/// One registered command.
pub struct CommandSpec<D, V> {
    /// A dotted area-and-verb identifier.
    pub id: &'static str,
    /// The English menu label, also the catalogue key.
    pub label: &'static str,
    /// The menu location, as a path of labels.
    pub menu: &'static [&'static str],
    /// The default chord in platform-neutral notation.
    pub shortcut: Option<&'static str>,
    /// The parameter documentation, one line per key.
    pub params: &'static str,
    pub enabled: EnabledFn<D, V>,
    pub handler: Handler<D, V>,
    /// Whether a call that changed anything is journaled.
    pub journal: bool,
    /// Whether a call that changed the document records an undo step.
    pub undoable: bool,
}

impl<D, V> Clone for CommandSpec<D, V> {
    fn clone(&self) -> Self {
        CommandSpec { ..*self }
    }
}

impl<D, V> CommandSpec<D, V> {
    /// Whether the command is a query: neither journaled nor undoable, run by its own handler (not
    /// the session's undo or redo). The engine refuses a query that changes anything, so a query
    /// only reads. The one test of what a query is: the session's checks and the control channel's
    /// read-only access both ask it.
    pub fn is_query(&self) -> bool {
        !self.journal && !self.undoable && matches!(self.handler, Handler::Run(_))
    }
}

/// The reason a command whose predicate failed is listed as unavailable.
pub const INTERNAL: &str = "Its availability could not be checked.";

/// The enabled predicate of a command that is always available.
pub fn always<D, V>(_: &Facts<'_, D, V>) -> Result<(), &'static str> {
    Ok(())
}

/// Registers a command in one of three forms: journaled and undoable (the default), `query`
/// (neither) and `noundo` (journaled, no undo step).
#[macro_export]
macro_rules! cmd {
    (query $id:expr, $label:expr, $menu:expr, $shortcut:expr, $params:expr, $enabled:expr, $run:expr) => {
        $crate::CommandSpec {
            id: $id,
            label: $label,
            menu: $menu,
            shortcut: $shortcut,
            params: $params,
            enabled: $enabled,
            handler: $crate::Handler::Run($run),
            journal: false,
            undoable: false,
        }
    };
    (noundo $id:expr, $label:expr, $menu:expr, $shortcut:expr, $params:expr, $enabled:expr, $run:expr) => {
        $crate::CommandSpec {
            id: $id,
            label: $label,
            menu: $menu,
            shortcut: $shortcut,
            params: $params,
            enabled: $enabled,
            handler: $crate::Handler::Run($run),
            journal: true,
            undoable: false,
        }
    };
    ($id:expr, $label:expr, $menu:expr, $shortcut:expr, $params:expr, $enabled:expr, $run:expr) => {
        $crate::CommandSpec {
            id: $id,
            label: $label,
            menu: $menu,
            shortcut: $shortcut,
            params: $params,
            enabled: $enabled,
            handler: $crate::Handler::Run($run),
            journal: true,
            undoable: true,
        }
    };
}

/// A command's documented parameter, as `CommandInfo` lists it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParamInfo {
    pub key: String,
    pub kind: &'static str,
    pub default: Option<Value>,
    /// The key may be left out, with no default: what it names is then kept.
    pub optional: bool,
    pub range: Option<(f64, f64)>,
    pub choices: Vec<String>,
    pub meaning: String,
}

/// The serialisable view of a command, for menus, dialogs and the MCP server's schemas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub menu: Vec<&'static str>,
    pub shortcut: Option<&'static str>,
    pub params: Vec<ParamInfo>,
    pub enabled: bool,
    pub disabled_reason: Option<&'static str>,
    pub journal: bool,
    pub undoable: bool,
}

/// The commands of one notation, checked once and shared, and the check every change passes
/// before it is kept, when the notation has one.
pub struct Registry<D: Document, V> {
    specs: Vec<(CommandSpec<D, V>, Vec<ParamDoc>)>,
    check: Option<CommitCheck<D>>,
}

impl<D: Document, V: View<D>> Registry<D, V> {
    /// Builds the table from the areas' specs, refusing a duplicate identifier, one that is not a
    /// dotted area-and-verb name, or parameter documentation that does not parse.
    pub fn new(specs: Vec<CommandSpec<D, V>>) -> Result<Arc<Registry<D, V>>, String> {
        Registry::build(specs, None)
    }

    /// [`Registry::new`] with `check` run on every change at its commit point ([`CommitCheck`]).
    pub fn with_commit_check(specs: Vec<CommandSpec<D, V>>, check: CommitCheck<D>) -> Result<Arc<Registry<D, V>>, String> {
        Registry::build(specs, Some(check))
    }

    /// The commit check, when the registry has one.
    pub(crate) fn commit_check(&self) -> Option<CommitCheck<D>> {
        self.check
    }

    /// The table of [`Registry::new`], holding `check` when given.
    fn build(specs: Vec<CommandSpec<D, V>>, check: Option<CommitCheck<D>>) -> Result<Arc<Registry<D, V>>, String> {
        let mut out: Vec<(CommandSpec<D, V>, Vec<ParamDoc>)> = Vec::with_capacity(specs.len());
        for spec in specs {
            if !valid_id(spec.id) {
                return Err(format!("{:?} is not a dotted lower-case area-and-verb identifier", spec.id));
            }
            if out.iter().any(|(s, _)| s.id == spec.id) {
                return Err(format!("{} is registered twice", spec.id));
            }
            let docs = parse_params(spec.params).map_err(|e| format!("{}: {e}", spec.id))?;
            out.push((spec, docs));
        }
        Ok(Arc::new(Registry { specs: out, check }))
    }

    pub(crate) fn find(&self, id: &str) -> Option<&(CommandSpec<D, V>, Vec<ParamDoc>)> {
        self.specs.iter().find(|(s, _)| s.id == id)
    }

    /// The error for an identifier no command has, naming close ones.
    pub(crate) fn unknown(&self, id: &str) -> crate::EngineError {
        crate::EngineError::UnknownCommand { id: crate::error::echo(id), close: self.close_to(id), count: self.len() }
    }

    /// Whether the command `id` is a query ([`CommandSpec::is_query`]); `None` for an unknown id.
    pub fn is_query(&self, id: &str) -> Option<bool> {
        self.find(id).map(|(spec, _)| spec.is_query())
    }

    /// Every identifier, in registration order.
    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.specs.iter().map(|(s, _)| s.id)
    }

    /// Up to ten identifiers close to `id`: the same area first, then a small edit distance.
    pub(crate) fn close_to(&self, id: &str) -> Vec<String> {
        let probe: String = id.chars().take(64).collect();
        let area = probe.split('.').next().unwrap_or_default();
        let mut scored: Vec<(usize, &str)> = self
            .specs
            .iter()
            .map(|(s, _)| {
                let same_area = s.id.split('.').next() == Some(area);
                (distance(&probe, s.id).saturating_sub(if same_area { 3 } else { 0 }), s.id)
            })
            .filter(|(d, _)| *d <= 4)
            .collect();
        scored.sort();
        scored.into_iter().take(10).map(|(_, id)| id.to_string()).collect()
    }

    pub fn len(&self) -> usize {
        self.specs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// The commands as menus, dialogs and the MCP server see them, with whether each is enabled
    /// for `facts`.
    pub fn infos(&self, facts: &Facts<'_, D, V>) -> Vec<CommandInfo> {
        self.specs
            .iter()
            .map(|(s, docs)| {
                // A predicate that panics makes its command unavailable, never the listing fail.
                let enabled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (s.enabled)(facts))).unwrap_or(Err(INTERNAL));
                CommandInfo {
                    id: s.id,
                    label: s.label,
                    menu: s.menu.to_vec(),
                    shortcut: s.shortcut,
                    params: docs
                        .iter()
                        .map(|d| ParamInfo {
                            key: d.key.clone(),
                            kind: d.kind_name(),
                            default: d.default.clone(),
                            optional: d.optional,
                            range: d.range,
                            choices: d.choices.clone(),
                            meaning: d.meaning.clone(),
                        })
                        .collect(),
                    enabled: enabled.is_ok(),
                    disabled_reason: enabled.err(),
                    journal: s.journal,
                    undoable: s.undoable,
                }
            })
            .collect()
    }
}

/// `area.verb` and deeper: lower-case segments of letters, digits and dashes, each starting with a
/// letter, at least two of them.
fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    parts.len() >= 2
        && parts
            .iter()
            .all(|p| p.starts_with(|c: char| c.is_ascii_lowercase()) && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'))
}

/// The edit distance of two short strings.
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row.first().copied().unwrap_or(0);
        if let Some(first) = row.first_mut() {
            *first = i + 1;
        }
        for (j, cb) in b.iter().enumerate() {
            let above = row.get(j + 1).copied().unwrap_or(usize::MAX);
            let left = row.get(j).copied().unwrap_or(usize::MAX);
            let next = (above.saturating_add(1)).min(left.saturating_add(1)).min(prev.saturating_add(usize::from(ca != *cb)));
            prev = above;
            if let Some(cell) = row.get_mut(j + 1) {
                *cell = next;
            }
        }
    }
    row.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_follow_the_area_and_verb_form() {
        for good in ["edit.undo", "item.add", "diagram.move-entity", "er.entity.add2"] {
            assert!(valid_id(good), "{good}");
        }
        for bad in ["undo", "Edit.undo", "edit.", ".undo", "edit.Undo", "edit.un do", "edit.2x", "edit..undo"] {
            assert!(!valid_id(bad), "{bad}");
        }
        assert_eq!(distance("edit.undo", "edit.redo"), 2);
        assert_eq!(distance("", "abc"), 3);
    }
}
