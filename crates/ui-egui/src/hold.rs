//! The project hold (`docs/design/versioning.md`, section 6): whatever replaces the session holds
//! the project still from before its first step until it has ended, one at a time. Showing another
//! tab or Home, opening, closing and quitting wait and run when it ends; a close or a quit that
//! waited through a failed hold asks again, so it never acts on a state the person did not see.

use std::collections::VecDeque;
use std::path::PathBuf;

use crate::close_ask::Ask;
use crate::{Opened, Shell};

/// Why a hold did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldRefused {
    /// Another hold runs, or what waited for the last one has not run yet.
    Another,
    NoProject,
    /// A gesture runs, which would still end in a change.
    Gesture,
}

/// A hold under way: whether the session was read-only before it, and whether the latest rows have
/// gone to the host, after which no more go.
struct Hold {
    was_read_only: bool,
    handed_over: bool,
}

/// An action that waits for the hold to end.
pub(crate) enum Waiting {
    Show(PathBuf),
    Home,
    Open(PathBuf),
    Opened(Box<Opened>),
    Close(PathBuf),
    /// Save was answered to the closing's question.
    SaveAndClose(PathBuf),
    /// Don't save (`true`) or Close without writing (`false`) was answered to it.
    Discard(PathBuf, bool),
    Quit,
}

/// The hold under way, what waits for it, and whether the hold just ended failed.
#[derive(Default)]
pub(crate) struct Holding {
    hold: Option<Hold>,
    waiting: VecDeque<Waiting>,
    failed: bool,
    /// The path and name of the project the failed hold held, which the question after it names.
    held: Option<(PathBuf, String)>,
}

impl Holding {
    /// Whether a hold runs or something still waits for one.
    pub(crate) fn pending(&self) -> bool {
        self.hold.is_some() || !self.waiting.is_empty()
    }

    /// Whether the latest rows have gone over, so that no more may.
    pub(crate) fn writes_stopped(&self) -> bool {
        self.hold.as_ref().is_some_and(|h| h.handed_over)
    }
}

impl Shell {
    /// Whether the project shown is held still.
    pub fn is_held(&self) -> bool {
        self.holding.hold.is_some()
    }

    /// Starts the hold: the session shown becomes read-only.
    pub fn begin_hold(&mut self) -> Result<(), HoldRefused> {
        if self.holding.pending() {
            return Err(HoldRefused::Another);
        }
        let session = self.session.as_mut().ok_or(HoldRefused::NoProject)?;
        let was_read_only = session.read_only();
        session.set_read_only(true).map_err(|_| HoldRefused::Gesture)?;
        self.holding.hold = Some(Hold { was_read_only, handed_over: false });
        Ok(())
    }

    /// Hands the latest rows to the host and stops handing any over for the rest of the hold. False,
    /// with handing over left on, without a hold or when the rows could not be made.
    pub fn hand_over_held(&mut self) -> bool {
        if !self.is_held() {
            return false;
        }
        self.write_changes();
        let revision = self.session.as_ref().map(|s| s.revision());
        if revision.is_none() || self.written != revision {
            return false;
        }
        if let Some(hold) = self.holding.hold.as_mut() {
            hold.handed_over = true;
        }
        true
    }

    /// Ends the hold: the session shown is as read-only as before it, and what waited runs from the
    /// next frame. A replacement that took the session's place sets its own state after this.
    pub fn end_hold(&mut self) {
        let Some(hold) = self.holding.hold.take() else { return };
        self.holding.failed = false;
        if let Some(session) = self.session.as_mut()
            && let Err(e) = session.set_read_only(hold.was_read_only)
        {
            log::warn!("the session's read-only state could not be put back: {e}");
        }
    }

    /// [`Shell::end_hold`] for a hold that failed: a close or a quit that waited asks again.
    pub fn fail_hold(&mut self) {
        if self.is_held() {
            self.holding.held = self.project.as_ref().map(|p| (p.path.clone(), p.name.clone()));
            self.end_hold();
            self.holding.failed = true;
        }
    }

    /// While held, keeps `action` for when the hold ends (a quit once) and gives true.
    pub(crate) fn remember(&mut self, action: Waiting) -> bool {
        if !self.is_held() {
            return false;
        }
        if !(matches!(action, Waiting::Quit) && self.holding.waiting.iter().any(|w| matches!(w, Waiting::Quit))) {
            self.holding.waiting.push_back(action);
        }
        true
    }

    /// Runs what waited for the hold, each action once the question or quit before it is answered.
    pub(crate) fn run_after_hold(&mut self, ctx: &egui::Context) {
        while !self.is_held() && self.ask.is_none() && !self.quitting() {
            let Some(action) = self.holding.waiting.pop_front() else { return };
            let failed = self.holding.failed;
            match action {
                Waiting::Show(path) => self.tab_of(&path).into_iter().for_each(|i| self.activate(i)),
                Waiting::Home => self.go_home(),
                Waiting::Open(path) => self.open_path(&path),
                Waiting::Opened(opened) => self.open_project(*opened),
                Waiting::Quit if failed && !(0..self.tabs.len()).any(|i| self.tab_unsaved(i)) => {
                    // The held project, not the one shown: what waited before may have moved on.
                    let Some((path, held)) = self.holding.held.clone().filter(|(path, _)| self.tab_of(path).is_some()) else {
                        return self.start_quit(ctx);
                    };
                    self.quitting = Some(Vec::new());
                    self.ask = Some(Ask::HoldFailed { path, held });
                }
                Waiting::Quit => self.start_quit(ctx),
                Waiting::Close(path) | Waiting::SaveAndClose(path) | Waiting::Discard(path, _) if failed => self.ask_before_closing(&path),
                Waiting::Close(path) => self.tab_of(&path).into_iter().for_each(|i| self.close_tab(i)),
                Waiting::SaveAndClose(path) => self.tab_of(&path).into_iter().for_each(|i| self.save_and_close(i)),
                Waiting::Discard(path, reset) => self.tab_of(&path).into_iter().for_each(|i| self.discard_tab(i, reset)),
            }
        }
    }

    /// After a failed hold: the question a closing asks, or, with nothing unsaved, whether to close.
    fn ask_before_closing(&mut self, path: &std::path::Path) {
        let Some(i) = self.tab_of(path) else { return };
        if self.tab_unsaved(i) || self.tabs[i].close_error.is_some() {
            self.close_tab(i);
        } else {
            let held = self.holding.held.as_ref().map_or_else(|| self.tabs[i].name.clone(), |(_, name)| name.clone());
            self.ask = Some(Ask::HoldFailed { path: path.to_path_buf(), held });
        }
    }
}
