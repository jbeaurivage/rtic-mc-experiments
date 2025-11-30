#![expect(clippy::new_without_default)]

use core::cell::UnsafeCell;

use crate::{critical_section::DroppableCriticalSection, task::RunningTask};

/// A 1-deep message-passing queue, which holds one slot for each dispatcher in
/// the system
pub struct DispatchQueue<const N: usize>([UnsafeCell<Option<RunningTask>>; N]);

impl<const N: usize> DispatchQueue<N> {
    pub const fn new() -> Self {
        Self([const { UnsafeCell::new(None) }; N])
    }

    fn slot<CS: DroppableCriticalSection>(
        &self,
        _cs: &CS,
        dispatcher_idx: u16,
    ) -> *mut Option<RunningTask> {
        let slot = self
            .0
            .get(dispatcher_idx as usize)
            .expect("BUG: dispatcher idx doesn't exist");
        slot.get()
    }

    /// Insert a pending task to the queue for later retrieval
    pub(super) fn pend_task<CS: DroppableCriticalSection>(
        &self,
        cs: &CS,
        task: RunningTask,
        dispatcher_idx: u16,
    ) {
        let slot = unsafe { &mut *self.slot(cs, dispatcher_idx) };

        if slot.is_some() {
            panic!("Task has been skipped!");
        }

        slot.replace(task);
    }

    /// Retrieve the task to run, without marking the slot as ready
    pub(super) fn retrieve<'a, 'cs: 'a, CS: DroppableCriticalSection>(
        &self,
        cs: &'cs CS,
        dispatcher_idx: u16,
    ) -> Option<&'a RunningTask> {
        let slot = unsafe { &*self.slot(cs, dispatcher_idx) };
        slot.as_ref()
    }

    /// Signal that the task has completed by marking the queue slot as ready
    pub(super) fn complete_task<CS: DroppableCriticalSection>(&self, cs: &CS, dispatcher_idx: u16) {
        let slot = unsafe { &mut *self.slot(cs, dispatcher_idx) };

        if !slot.is_some() {
            panic!("Pending task set to idle!");
        }

        let _ = slot.take();
    }

    /// Whether or not this dispatcher is ready to accept a new task to run
    pub(super) fn is_ready<CS: DroppableCriticalSection>(
        &self,
        cs: &CS,
        dispatcher_idx: u16,
    ) -> bool {
        let slot = unsafe { &*self.slot(cs, dispatcher_idx) };
        slot.is_none()
    }
}

unsafe impl<const N: usize> Sync for DispatchQueue<N> {}
