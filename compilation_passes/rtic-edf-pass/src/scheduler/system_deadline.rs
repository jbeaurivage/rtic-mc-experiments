#![expect(clippy::new_without_default)]

use core::cell::UnsafeCell;

use crate::{critical_section::DroppableCriticalSection, types::Timestamp};

/// The global system deadline representing the minimum deadline task currently
/// executing
pub struct SystemDeadline(UnsafeCell<Timestamp>);

impl SystemDeadline {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(Timestamp::MAX))
    }

    /// Get the system deadline
    #[inline]
    pub(super) fn get<CS: DroppableCriticalSection>(&self, _cs: &CS) -> Timestamp {
        unsafe { *self.0.get() }
    }

    /// Replaces the system deadline with the deadline provided, and returns the
    /// old deadline
    #[inline]
    #[must_use]
    pub(super) fn replace<CS: DroppableCriticalSection>(
        &self,
        _cs: &CS,
        new_dl: Timestamp,
    ) -> Timestamp {
        unsafe { core::ptr::replace(self.0.get(), new_dl) }
    }
}

unsafe impl Sync for SystemDeadline {}
