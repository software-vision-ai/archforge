//! The command table: what a command is, the `cmd!` forms that register one, and the registry a
//! notation builds from its areas' tables.

pub mod edit;

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::params::{ParamDoc, Params, parse_params};
use crate::{CommandError, Document, View};

/// What a handler acts on: the document, which undo covers, and the view, which it never touches.
pub struct Ctx<'a, D, V> {
    pub doc: &'a mut D,
    pub view: &'a mut V,
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

/// The commands of one notation, checked once and shared.
pub struct Registry<D, V> {
    specs: Vec<(CommandSpec<D, V>, Vec<ParamDoc>)>,
}

impl<D: Document, V: View<D>> Registry<D, V> {
    /// Builds the table from the areas' specs, refusing a duplicate identifier, one that is not a
    /// dotted area-and-verb name, or parameter documentation that does not parse.
    pub fn new(specs: Vec<CommandSpec<D, V>>) -> Result<Arc<Registry<D, V>>, String> {
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
        Ok(Arc::new(Registry { specs: out }))
    }

    pub(crate) fn find(&self, id: &str) -> Option<&(CommandSpec<D, V>, Vec<ParamDoc>)> {
        self.specs.iter().find(|(s, _)| s.id == id)
    }

    /// The error for an identifier no command has, naming close ones.
    pub(crate) fn unknown(&self, id: &str) -> crate::EngineError {
        crate::EngineError::UnknownCommand { id: crate::error::echo(id), close: self.close_to(id), count: self.len() }
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
