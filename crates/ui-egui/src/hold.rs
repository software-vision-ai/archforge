//! The project hold (`docs/design/versioning.md`, section 6): whatever replaces the session holds
//! the project still from before its first step until it has ended, one at a time. While held the
//! session is read-only, Save is refused, and rows go to the host until the latest have gone over.

use crate::Shell;

/// Why a hold did not start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldRefused {
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

/// The hold under way, if any.
#[derive(Default)]
pub(crate) struct Holding {
    hold: Option<Hold>,
}

impl Holding {
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
        if self.is_held() {
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

    /// Ends the hold: the session shown is as read-only as before it. A replacement that took the
    /// session's place sets its own state after this.
    pub fn end_hold(&mut self) {
        let Some(hold) = self.holding.hold.take() else { return };
        if let Some(session) = self.session.as_mut()
            && let Err(e) = session.set_read_only(hold.was_read_only)
        {
            log::warn!("the session's read-only state could not be put back: {e}");
        }
    }
}
