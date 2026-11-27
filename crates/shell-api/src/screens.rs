//! The screens registry (ADR 0003, "Two registries"): the notations' screens parts the app
//! registers at start-up, each a notation's [`NotationUi`] with the diagram kinds its screens
//! serve, and the lists the shell and the narrow context read, made from them.
//!
//! The shell and the context hold their notations and kinds as `'static` slices of trait objects
//! (`archforge_ui_egui::Shell`, [`Ctx::notation_uis`](crate::cx::Ctx::notation_uis)), and a list
//! of parts cannot be flattened into such a slice at compile time. A registry is therefore a
//! `static` of the app's: its parts are listed once, at compile time, and the flat lists are made
//! from them the first time they are asked for, kept in the registry for the rest of the process,
//! so nothing is listed twice and nothing is leaked.

use std::sync::OnceLock;

use crate::i18n::Rows;
use crate::kinds::DiagramKind;
use crate::notation_ui::NotationUi;

/// One notation's screens part, as the app registers it: what the notation brings to the shell
/// and the diagram kinds its screens serve. Its catalogues are its `NotationUi`'s
/// ([`NotationUi::catalogues`]).
pub struct ScreensPart {
    /// What the notation brings to the shell.
    pub ui: &'static dyn NotationUi,
    /// The diagram kinds its screens serve.
    pub kinds: &'static [&'static dyn DiagramKind],
}

/// The screens parts the app registers, in order, with the lists the shell and the context read,
/// each made once from the parts. It is meant to be a `static`: the lists borrow from it for the
/// rest of the process.
pub struct ScreensRegistry {
    /// The parts, as registered.
    parts: &'static [ScreensPart],
    /// Every part's `NotationUi`, in the parts' order, once asked for.
    notation_uis: OnceLock<Vec<&'static dyn NotationUi>>,
    /// Every part's diagram kinds, part after part, once asked for.
    kinds: OnceLock<Vec<&'static dyn DiagramKind>>,
}

impl ScreensRegistry {
    /// A registry of `parts`, in their order; usable as a `static`'s value.
    pub const fn new(parts: &'static [ScreensPart]) -> Self {
        Self { parts, notation_uis: OnceLock::new(), kinds: OnceLock::new() }
    }

    /// The parts, as registered.
    pub fn parts(&self) -> &'static [ScreensPart] {
        self.parts
    }

    /// The notations the shell knows: every part's `NotationUi`, in the parts' order, as the
    /// context holds them ([`Ctx::notation_uis`](crate::cx::Ctx::notation_uis)).
    pub fn notation_uis(&'static self) -> &'static [&'static dyn NotationUi] {
        self.notation_uis.get_or_init(|| self.parts.iter().map(|part| part.ui).collect())
    }

    /// The diagram kinds the shell knows: every part's kinds, part after part, each part's in its
    /// own order.
    pub fn kinds(&'static self) -> &'static [&'static dyn DiagramKind] {
        self.kinds.get_or_init(|| self.parts.iter().flat_map(|part| part.kinds.iter().copied()).collect())
    }

    /// The notations' catalogue tables, one for each part, in the parts' order, each its
    /// `NotationUi`'s ([`NotationUi::catalogues`]), for the shell to register beside its own
    /// ([`i18n::register`](crate::i18n::register)).
    pub fn catalogues(&self) -> impl Iterator<Item = &'static [Rows]> + 'static {
        self.parts.iter().map(|part| part.ui.catalogues())
    }
}
