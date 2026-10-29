//! A document that records by copying: for small models, and for notations written before the
//! change-recording core. Its delta is the state before and the state after.

use std::ops::{Deref, DerefMut};

use crate::{Document, View};

/// A state that is cheap enough to copy on every edit. It is compared for equality, so it never
/// holds a NaN.
pub trait Snapshot: Clone + PartialEq + Send + 'static {
    /// An estimate of its size in bytes.
    fn weight(&self) -> usize;
}

/// The delta of a [`Snapshotted`] document: both states, whole.
#[derive(Debug, Clone, PartialEq)]
pub struct Replace<T> {
    pub before: T,
    pub after: T,
}

/// A [`Snapshot`] as a [`Document`]: the state, and the copy taken at `begin`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshotted<T> {
    current: T,
    base: Option<T>,
}

impl<T> Snapshotted<T> {
    pub fn new(state: T) -> Self {
        Snapshotted { current: state, base: None }
    }

    pub fn into_inner(self) -> T {
        self.current
    }
}

impl<T> Deref for Snapshotted<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.current
    }
}

impl<T> DerefMut for Snapshotted<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.current
    }
}

impl<T: Snapshot> Document for Snapshotted<T> {
    type Delta = Replace<T>;

    fn begin(&mut self) {
        self.base = Some(self.current.clone());
    }

    fn end(&mut self) -> Option<Replace<T>> {
        let before = self.base.take()?;
        if before == self.current { None } else { Some(Replace { before, after: self.current.clone() }) }
    }

    fn apply(&mut self, delta: &Replace<T>) -> Result<(), String> {
        if self.current != delta.before {
            return Err("the delta does not fit the document's state".into());
        }
        self.current = delta.after.clone();
        Ok(())
    }

    fn invert(delta: &Replace<T>) -> Replace<T> {
        Replace { before: delta.after.clone(), after: delta.before.clone() }
    }

    fn compose(first: Replace<T>, second: Replace<T>) -> Replace<T> {
        Replace { before: first.before, after: second.after }
    }

    fn is_empty(delta: &Replace<T>) -> bool {
        delta.before == delta.after
    }

    fn weight(delta: &Replace<T>) -> usize {
        delta.before.weight().saturating_add(delta.after.weight())
    }
}

/// A view of the state is a view of the snapshotted document.
impl<T: Snapshot, V: View<T>> View<Snapshotted<T>> for V {
    fn retain_valid(&mut self, doc: &Snapshotted<T>) {
        View::<T>::retain_valid(self, &doc.current);
    }
}
