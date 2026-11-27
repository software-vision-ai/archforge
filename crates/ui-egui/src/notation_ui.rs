//! The app's notations for the tests (`UIS`), what an opening does to a model with a shell's
//! notations (`open_repair`), how their overlays are drawn over the canvas (`overlays`), and the
//! forwarders over the whole shell for code not on the narrow context yet. What a notation brings
//! to the shell (`NotationUi`, `NotationState`, `NotationStates`), the free functions that ask the
//! shell's notations and the data types they hand across are the shell's interfaces'
//! (`archforge_shell_api::notation_ui`), and keep their old paths here.

use archforge_mof::{Store, Uuid};

pub(crate) use archforge_shell_api::notation_ui::{
    Added, ChatKind, Entry, Inside, Layer, NotationState, NotationStates, NotationUi, Offer, Overlaid, Removal, Words, before_removal,
    description_limit, ends_label, label, longest_name, name_limit_of_cx, of, of_class, say_after_cx,
};

use crate::Shell;

/// [`say_after_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn say_after(shell: &mut Shell, after: Option<String>, lead: &str) {
    shell.cx(|cx| say_after_cx(cx, after, lead));
}

/// Draws every notation's overlays at `layer` over the canvas `on`, in the order of the notations
/// ([`NotationUi::canvas_overlay`]), each through a context of its own, so the shell carries out
/// what one asked before the next draws.
pub(crate) fn overlays(shell: &mut Shell, ui: &mut egui::Ui, layer: Layer, on: &Overlaid<'_>) {
    let uis = shell.notation_uis;
    for notation in uis {
        shell.cx(|cx| notation.canvas_overlay(cx, ui, layer, on));
    }
}

/// The app's notations, ER's and UML's, for the tests that ask them without a shell; a shell
/// knows those of the screens parts it is given ([`Shell::new`]).
#[cfg(test)]
pub(crate) static UIS: &[&dyn NotationUi] = &[&crate::er_ui::ErUi, &crate::uml_ui::UmlUi];

/// What an opening does to a project's model before its session starts with the notations `uis`:
/// each puts its own layout right (`NotationUi::open_repair`), in the order of the notations. A
/// shell takes it for its own notations ([`Shell::open_repair`]) to open, restore and compare
/// models on the threads that do it, and hands it to the host for a restore, so one opening
/// repairs a model the same way wherever it runs.
pub(crate) fn open_repair(uis: &'static [&'static dyn NotationUi]) -> impl Fn(&mut Store) + Copy + Send + 'static {
    move |store: &mut Store| {
        for notation in uis {
            notation.open_repair(store);
        }
    }
}

/// [`name_limit_of_cx`] over the whole shell, for code not on the narrow context yet.
pub(crate) fn name_limit_of(shell: &Shell, id: Uuid) -> usize {
    name_limit_of_cx(&shell.cx_ref(), id)
}
