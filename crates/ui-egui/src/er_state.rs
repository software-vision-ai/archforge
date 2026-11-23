//! What ER keeps between frames for its own editors and tools: the Properties editors of a column,
//! a key, a check, an index, a type and a sequence or view, a relationship's role being typed with
//! its More and its key card, and on the canvas the ends a new relationship gets and the mapping
//! table offer. The shell holds it as one notation's state among others
//! ([`crate::notation_ui::NotationState`]); ER's screens reach it through [`er`] and [`er_mut`].

use archforge_mof::{Store, Uuid};

use crate::notation_ui::{NotationState, NotationStates};
use crate::panels::{Editor, Menu, OpenEditor};

/// The column editor's card.
pub(crate) const COLUMN: Editor = Editor { notation: archforge_er::INDEX, card: "column" };
/// The key editor's card.
pub(crate) const KEY: Editor = Editor { notation: archforge_er::INDEX, card: "key" };
/// The check editor's card.
pub(crate) const CHECK: Editor = Editor { notation: archforge_er::INDEX, card: "check" };
/// The index editor's card.
pub(crate) const INDEX: Editor = Editor { notation: archforge_er::INDEX, card: "index" };
/// The domain or enum editor's card.
pub(crate) const TYPE: Editor = Editor { notation: archforge_er::INDEX, card: "type" };
/// The sequence or view editor's card.
pub(crate) const OBJECT: Editor = Editor { notation: archforge_er::INDEX, card: "object" };

/// ER's editors and tools between frames.
#[derive(Debug, Default)]
pub(crate) struct ErState {
    /// The column editor, open on a column or on a column about to be added.
    pub column: Option<crate::panels::ColumnEdit>,
    /// The key editor, open on a key or on a unique constraint about to be added (slice E4).
    pub key: Option<crate::keys::KeyEdit>,
    /// The check and index editors (slice S3).
    pub check: Option<crate::checks::CheckEdit>,
    pub index: Option<crate::indexes::IndexEdit>,
    /// The domain or enum editor (slice S5).
    pub type_edit: Option<crate::types::TypeEdit>,
    /// The sequence or view editor (slice S7).
    pub object_edit: Option<crate::objects::ObjectEdit>,
    /// A relationship's Properties (slice E3): a role being typed, whether More is open, and the
    /// relationship whose switch asked for a key its parent cannot give.
    pub role: Option<crate::relation::RoleEdit>,
    pub more_open: bool,
    pub no_key_card: Option<Uuid>,
    /// The ends a new relationship line gets (slice E2).
    pub relationship: crate::connect::RelKind,
    /// The mapping table offer.
    pub mapping: Option<crate::connect::MappingOffer>,
}

/// ER's state among the shell's; none while ER is not one of its notations.
pub(crate) fn er(states: &NotationStates) -> Option<&ErState> {
    states.own()
}

/// ER's state among the shell's, to change; none while ER is not one of its notations.
pub(crate) fn er_mut(states: &mut NotationStates) -> Option<&mut ErState> {
    states.own_mut()
}

/// Whether the element `id` is gone from `store`.
fn gone(store: &Store, id: Uuid) -> bool {
    store.by_uuid(id).is_none()
}

impl NotationState for ErState {
    /// Drops an editor whose table, schema or item is gone, a role or key card whose relationship
    /// is, and the mapping offer of a relationship no longer alive.
    fn retain_valid(&mut self, store: &Store, canvas: &crate::canvas::State) {
        let _ = canvas;
        let item_gone = |owner: Uuid, target: Option<Uuid>| gone(store, owner) || target.is_some_and(|t| gone(store, t));
        if self.column.as_ref().is_some_and(|c| item_gone(c.entity, c.target)) {
            self.column = None;
        }
        if self.role.as_ref().is_some_and(|e| gone(store, e.target)) {
            self.role = None;
        }
        if self.no_key_card.is_some_and(|id| gone(store, id)) {
            self.no_key_card = None;
        }
        if self.key.as_ref().is_some_and(|k| item_gone(k.entity, k.target)) {
            self.key = None;
        }
        if self.check.as_ref().is_some_and(|c| item_gone(c.entity, c.target)) {
            self.check = None;
        }
        if self.index.as_ref().is_some_and(|i| item_gone(i.entity, i.target)) {
            self.index = None;
        }
        if self.type_edit.as_ref().is_some_and(|t| item_gone(t.schema, t.target)) {
            self.type_edit = None;
        }
        if self.object_edit.as_ref().is_some_and(|o| item_gone(o.schema, o.target)) {
            self.object_edit = None;
        }
        if self.mapping.as_ref().is_some_and(|m| store.by_uuid(m.relationship).is_none_or(|o| !store.is_alive(o))) {
            self.mapping = None;
        }
    }

    /// An editor card open, or a relationship's role being typed.
    fn editing(&self) -> bool {
        self.open_editors().iter().any(|(_, open)| open.is_some()) || self.role.is_some()
    }

    /// The column, key, check, index, type and sequence or view editors: where each edits (a key's
    /// list is its kind, primary or unique; a type's and an object's their kind) and its refusal.
    fn open_editors(&self) -> Vec<(Editor, OpenEditor)> {
        vec![
            (COLUMN, self.column.as_ref().map(|e| ((e.target, 0), e.refused.clone()))),
            (KEY, self.key.as_ref().map(|e| ((e.target, usize::from(e.primary)), e.refused.clone()))),
            (CHECK, self.check.as_ref().map(|e| ((e.target, 0), e.refused.clone()))),
            (INDEX, self.index.as_ref().map(|e| ((e.target, 0), e.refused.clone()))),
            (TYPE, self.type_edit.as_ref().map(|e| ((e.target, e.kind as usize), e.refused.clone()))),
            (OBJECT, self.object_edit.as_ref().map(|e| ((e.target, e.kind as usize), e.refused.clone()))),
        ]
    }

    /// Closes each editor card not opened this frame; none of ER's menus outlives them.
    fn close_editors(&mut self, menu: &mut Option<Menu>) {
        let _ = menu;
        if self.column.as_ref().is_some_and(|e| !e.opened) {
            self.column = None;
        }
        if self.key.as_ref().is_some_and(|e| !e.opened) {
            self.key = None;
        }
        if self.check.as_ref().is_some_and(|e| !e.opened) {
            self.check = None;
        }
        if self.index.as_ref().is_some_and(|e| !e.opened) {
            self.index = None;
        }
        if self.type_edit.as_ref().is_some_and(|e| !e.opened) {
            self.type_edit = None;
        }
        if self.object_edit.as_ref().is_some_and(|e| !e.opened) {
            self.object_edit = None;
        }
    }
}
