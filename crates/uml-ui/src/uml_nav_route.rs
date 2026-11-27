//! The return route of a diagram tab (AR-408, the design's "Navigation, ownership and deletion
//! completion" and Decisions 24 and 27): the package entries that led to the diagram shown, in
//! order, so that zooming out returns the way the user came in.
//!
//! - A route keeps the most recent [`MAX_ROUTE`] steps; the oldest goes when one more is made.
//! - Entering a diagram from itself records nothing.
//! - A return consumes steps from the newest back to the nearest whose source can still be
//!   shown, and records nothing itself: returning is not an entry.
//! - Leaving by the path bar or ⌘↑ follows ownership, not the route, and starts the route of the
//!   tab it opens anew (AR-666).
//!
//! A route is browsing state of the project session only: it is never written into the model, the
//! project or its versions, and makes no undo step.

use std::collections::VecDeque;

use archforge_mof::Uuid;
use egui::Vec2;

/// The most steps a route keeps, as deep as the path bar walks a package chain.
pub(crate) const MAX_ROUTE: usize = 256;

/// One package entry: the diagram left, the diagram entered, and what a return to the source
/// needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The diagram the entry left, which a return opens again.
    pub source: Uuid,
    /// The diagram the entry opened.
    pub destination: Uuid,
    /// The source's name when it was left, for saying what a return skipped once it is gone.
    pub source_name: String,
    /// The source's pan and zoom when it was left, for a return to restore; none for an entry made
    /// by zooming in, whose view a return must not restore (AR-668).
    pub view: Option<(Vec2, f32)>,
    /// The undo history of the session the entry was made in (`Session::history`): a step of
    /// another history never leads anywhere, since its diagrams belong to another revision.
    pub history: u64,
}

/// What a return took from a route.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Return {
    /// The step whose source the return opens; none when no step was left that could be.
    pub(crate) reached: Option<Step>,
    /// The steps passed over on the way, newest first: the first is the one the return would
    /// have taken had its source still been there.
    pub(crate) skipped: Vec<Step>,
}

/// The entries that led to a tab's diagram, oldest first, at most [`MAX_ROUTE`] of them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Route {
    steps: VecDeque<Step>,
}

impl Route {
    /// Records an entry as the newest step, dropping the oldest when the route already holds
    /// [`MAX_ROUTE`]. An entry whose source is its destination records nothing.
    pub fn push(&mut self, step: Step) {
        if step.source == step.destination {
            return;
        }
        while self.steps.len() >= MAX_ROUTE {
            self.steps.pop_front();
        }
        self.steps.push_back(step);
    }

    /// Takes the way back: removes steps from the newest until one was made in `history` and its
    /// source passes `valid`, and gives that step with the ones removed before it. Every step
    /// looked at is consumed, so a route of nothing that can be shown ends empty; no step is
    /// added for the return.
    pub(crate) fn take_return(&mut self, history: u64, mut valid: impl FnMut(Uuid) -> bool) -> Return {
        let mut out = Return::default();
        while let Some(step) = self.steps.pop_back() {
            if step.history == history && valid(step.source) {
                out.reached = Some(step);
                break;
            }
            out.skipped.push(step);
        }
        out
    }

    /// The steps, oldest first, left in the route.
    #[cfg(feature = "snapshot")]
    pub fn steps(&self) -> impl DoubleEndedIterator<Item = &Step> {
        self.steps.iter()
    }

    /// Forgets every step.
    #[cfg_attr(not(test), expect(dead_code, reason = "AR-671 (408-F) clears routes when the revision changes"))]
    pub(crate) fn clear(&mut self) {
        self.steps.clear();
    }

    /// How many steps the route holds.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the route holds no step.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The diagram numbered `n`.
    fn d(n: u8) -> Uuid {
        Uuid([n; 16])
    }

    /// An entry from diagram `from` to diagram `to` in history 1.
    fn step(from: u8, to: u8) -> Step {
        Step { source: d(from), destination: d(to), source_name: format!("D{from}"), view: Some((Vec2::new(f32::from(from), 0.0), 1.0)), history: 1 }
    }

    /// The (source, destination) of each step, for comparing.
    fn ends(steps: &[Step]) -> Vec<(Uuid, Uuid)> {
        steps.iter().map(|s| (s.source, s.destination)).collect()
    }

    /// Entering A → B → A → B and zooming out returns B → A → B → A, each return consuming one step and adding none.
    #[test]
    fn a_return_walks_back_repeated_visits_in_order() {
        // A → B → A → B: three entries.
        let mut route = Route::default();
        for (from, to) in [(1, 2), (2, 1), (1, 2)] {
            route.push(step(from, to));
        }
        assert_eq!(route.len(), 3);
        // Back: B → A → B → A, one step consumed each time and none added.
        for (left, expected, after) in [(2, 1, 2), (1, 2, 1), (2, 1, 0)] {
            let back = route.take_return(1, |_| true);
            let reached = back.reached.as_ref().map(|s| (s.destination, s.source));
            assert_eq!(reached, Some((d(left), d(expected))), "{back:?}");
            assert!(back.skipped.is_empty(), "{back:?}");
            assert_eq!(route.len(), after);
        }
        assert_eq!(route.take_return(1, |_| true), Return::default());
    }

    /// A route keeps 256 steps; the 257th drops the oldest and only it.
    #[test]
    fn a_route_keeps_the_newest_steps_up_to_its_bound() {
        let mut route = Route::default();
        // Entry `n` leaves a diagram of its own, so each step can be told apart.
        let entry = |n: usize| {
            let [hi, lo] = u16::try_from(n).unwrap().to_be_bytes();
            Step { source: Uuid([hi, lo, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9]), ..step(1, 2) }
        };
        for n in 0..MAX_ROUTE {
            route.push(entry(n));
        }
        assert_eq!(route.len(), MAX_ROUTE);
        let full = route.clone();
        // The 257th entry drops the first one, and only it.
        route.push(entry(MAX_ROUTE));
        assert_eq!(route.len(), MAX_ROUTE);
        let mut reached = Vec::new();
        while let Some(s) = route.take_return(1, |_| true).reached {
            reached.push(s.source);
        }
        let expected: Vec<Uuid> = (1..=MAX_ROUTE).rev().map(|n| entry(n).source).collect();
        assert_eq!(reached, expected);
        assert!(route.is_empty());
        // Below the bound nothing goes.
        let mut kept = full;
        assert_eq!(kept.take_return(1, |_| true).reached.map(|s| s.source), Some(entry(MAX_ROUTE - 1).source));
        let mut last = None;
        while let Some(s) = kept.take_return(1, |_| true).reached {
            last = Some(s.source);
        }
        assert_eq!(last, Some(entry(0).source));
    }

    /// An entry whose source is its destination leaves the route as it was.
    #[test]
    fn entering_a_diagram_from_itself_records_nothing() {
        let mut route = Route::default();
        route.push(step(3, 3));
        assert!(route.is_empty());
        route.push(step(1, 2));
        route.push(step(2, 2));
        assert_eq!(route.len(), 1);
        assert_eq!(route.take_return(1, |_| true).reached, Some(step(1, 2)));
    }

    /// A return passes over one step whose source is gone, consumes it, and names it among the skipped.
    #[test]
    fn a_return_skips_one_source_that_is_gone() {
        let mut route = Route::default();
        for (from, to) in [(1, 2), (2, 3), (3, 4)] {
            route.push(step(from, to));
        }
        let back = route.take_return(1, |s| s != d(3));
        assert_eq!(back.reached, Some(step(2, 3)));
        assert_eq!(ends(&back.skipped), ends(&[step(3, 4)]));
        assert_eq!(back.skipped[0].source_name, "D3");
        // The skipped step is consumed with the one reached.
        assert_eq!(route.len(), 1);
    }

    /// A return passes over several gone sources in a row, newest first, and stops at the nearest one left.
    #[test]
    fn a_return_skips_several_sources_that_are_gone_in_a_row() {
        let mut route = Route::default();
        for (from, to) in [(1, 2), (2, 3), (3, 4), (4, 5)] {
            route.push(step(from, to));
        }
        let back = route.take_return(1, |s| s != d(4) && s != d(3));
        assert_eq!(back.reached, Some(step(2, 3)));
        // Newest first: the first skipped is the return's original destination.
        assert_eq!(ends(&back.skipped), ends(&[step(4, 5), step(3, 4)]));
        assert_eq!(route.len(), 1);
        assert_eq!(route.take_return(1, |_| true).reached, Some(step(1, 2)));
    }

    /// When no source can be shown, a return reaches nothing, skips every step and leaves the route empty.
    #[test]
    fn a_route_with_no_source_left_ends_empty() {
        let mut route = Route::default();
        for (from, to) in [(1, 2), (2, 3), (3, 4)] {
            route.push(step(from, to));
        }
        let back = route.take_return(1, |_| false);
        assert_eq!(back.reached, None);
        assert_eq!(ends(&back.skipped), ends(&[step(3, 4), step(2, 3), step(1, 2)]));
        assert!(route.is_empty());
    }

    /// A step made in another session history is skipped though its source passes the check.
    #[test]
    fn a_step_of_another_history_leads_nowhere() {
        let mut route = Route::default();
        route.push(step(1, 2));
        route.push(Step { history: 7, ..step(2, 3) });
        // Its source still exists, but in another revision: skipped like a deleted one.
        let back = route.take_return(1, |_| true);
        assert_eq!(back.reached, Some(step(1, 2)));
        assert_eq!(ends(&back.skipped), ends(&[step(2, 3)]));
        // And from history 7, the history-1 step is the one that leads nowhere.
        let mut other = Route::default();
        other.push(step(1, 2));
        let back = other.take_return(7, |_| true);
        assert_eq!(back.reached, None);
        assert_eq!(back.skipped, vec![step(1, 2)]);
    }

    /// Clearing a route leaves nothing for a return to take.
    #[test]
    fn clear_forgets_every_step() {
        let mut route = Route::default();
        route.push(step(1, 2));
        route.push(step(2, 3));
        route.clear();
        assert!(route.is_empty());
        assert_eq!(route.take_return(1, |_| true), Return::default());
    }
}
