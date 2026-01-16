#![expect(clippy::new_without_default)]

use core::cell::UnsafeCell;

use crate::{critical_section::DroppableCriticalSection, task::RunningTask};

/// A 1-deep message-passing buffer, which holds one slot for each dispatcher
/// priority in the system.
///
/// We only need one slot per priority, because for each priority, only one
/// dispatcher is guaranteed to run at any instant
pub struct RunQueue<const N: usize>([UnsafeCell<Option<RunningTask>>; N]);

impl<const N: usize> RunQueue<N> {
    pub const fn new() -> Self {
        Self([const { UnsafeCell::new(None) }; N])
    }

    fn slot<CS: DroppableCriticalSection>(&self, _cs: &CS, idx: u16) -> *mut Option<RunningTask> {
        #[cfg(debug_assertions)]
        let slot = self
            .0
            .get(idx as usize)
            .expect("BUG: dispatcher idx doesn't exist");

        #[cfg(not(debug_assertions))]
        let slot = unsafe { self.0.get_unchecked(idx as usize) };

        slot.get()
    }

    /// Insert a pending task to the queue for later retrieval
    pub(super) fn insert<CS: DroppableCriticalSection>(
        &self,
        cs: &CS,
        task: RunningTask,
        idx: u16,
    ) {
        let slot = unsafe { &mut *self.slot(cs, idx) };

        debug_assert!(slot.is_none(), "Task has been skipped!");
        slot.replace(task);
    }

    /// Peek at the task to run, without marking the slot as ready
    pub(super) fn peek<'a, 'cs: 'a, CS: DroppableCriticalSection>(
        &self,
        cs: &'cs CS,
        idx: u16,
    ) -> Option<&'a RunningTask> {
        let slot = unsafe { &*self.slot(cs, idx) };
        slot.as_ref()
    }

    /// Signal that the task has completed by taking the task out of the buffer,
    /// therefore marking the queue slot as ready
    pub(super) fn mark_complete<CS: DroppableCriticalSection>(&self, cs: &CS, idx: u16) {
        let slot = unsafe { &mut *self.slot(cs, idx) };
        debug_assert!(slot.is_some(), "Pending task set to idle!");
        let _ = slot.take();
    }

    /// Whether or not this dispatcher is ready to accept a new task to run
    pub(super) fn is_ready<CS: DroppableCriticalSection>(&self, cs: &CS, idx: u16) -> bool {
        let slot = unsafe { &*self.slot(cs, idx) };
        slot.is_none()
    }
}

unsafe impl<const N: usize> Sync for RunQueue<N> {}
