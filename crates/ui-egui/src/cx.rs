//! The narrow context over the shell: [`Shell::cx`] splits the shell's borrows into the context a
//! notation's screens and the shared controls change things through ([`Ctx`]), and carries out the
//! [`Request`]s made through it once the call returns; [`Shell::cx_ref`] gives its read-only twin
//! ([`CtxRef`]). The context itself is the shell's interfaces' (`archforge_shell_api::cx`), and
//! keeps its old path here.

pub(crate) use archforge_shell_api::cx::{Ctx, CtxRef, Request};

#[cfg(any())] // AR-796 parked
use crate::Shell;

impl Shell {
    /// Runs `f` with the narrow context over this shell's parts, then carries out the requests `f`
    /// made, in the order made, each once, and gives what `f` gave.
    pub(crate) fn cx<R>(&mut self, f: impl FnOnce(&mut Ctx<'_>) -> R) -> R {
        let looking_only = self.looking_only();
        let mut cx = Ctx {
            session: &mut self.session,
            panels: &mut self.panels,
            canvas: &mut self.canvas,
            registry: &mut self.registry,
            notation_uis: self.notation_uis,
            notation_states: &mut self.notation_states,
            problems: &self.problems,
            frames: self.frames,
            menu_bar: self.menu_bar.hold(),
            looking_only,
            requests: Vec::new(),
        };
        let out = f(&mut cx);
        let requests = cx.requests;
        self.carry_out(requests);
        out
    }

    /// The read-only twin of the narrow context over this shell's parts.
    pub(crate) fn cx_ref(&self) -> CtxRef<'_> {
        CtxRef {
            session: &self.session,
            canvas: &self.canvas,
            notation_uis: self.notation_uis,
            notation_states: &self.notation_states,
            looking_only: self.looking_only(),
        }
    }

    /// Carries out the context's `requests`, in their order.
    fn carry_out(&mut self, requests: Vec<Request>) {
        for request in requests {
            match request {
                Request::ExpandRows(rows) => self.explorer.expanded.extend(rows),
                Request::HideVersionDetails => (self.versions.details, self.versions.element) = (None, None),
                Request::ShowPanel(panel) => self.state.show_panel(panel),
                Request::ShowExplorer(explorer) => self.state.show_explorer(explorer),
                Request::NameInModels(id, name) => {
                    self.explorer.filter.clear();
                    self.explorer.rename = Some(crate::explorer::Rename::fresh(id, name));
                    self.explorer.focused = true;
                }
                Request::AskDelete(delete) => self.explorer.delete = Some(delete),
            }
        }
    }
}
