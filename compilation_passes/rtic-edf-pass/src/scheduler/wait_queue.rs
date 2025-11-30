#![expect(clippy::new_without_default)]

use core::cell::UnsafeCell;

use heapless::{BinaryHeap, binary_heap::Min};

use crate::{critical_section::DroppableCriticalSection, task::ScheduledTask};

/// Queue for tasks waiting to get dispatched
pub struct WaitQueue<const N: usize>(UnsafeCell<BinaryHeap<ScheduledTask, Min, N>>);

unsafe impl<const N: usize> Sync for WaitQueue<N> {}

impl<const N: usize> WaitQueue<N> {
    pub const fn new() -> Self {
        Self(UnsafeCell::new(BinaryHeap::new()))
    }

    pub fn len<CS: DroppableCriticalSection>(&self, _cs: &CS) -> usize {
        unsafe { &mut *self.0.get() }.len()
    }

    /// Insert a new task into the wait queue
    pub(super) fn push<CS: DroppableCriticalSection>(&self, _cs: &CS, task: ScheduledTask) {
        unsafe { &mut *self.0.get() }
            .push(task)
            .expect("EDF wait queue is full");
    }

    /// Returns the task with the minimum deadline in the queue, if it exists.
    pub(super) fn next_task<'a, 'cs: 'a, CS: DroppableCriticalSection>(
        &self,
        _cs: &CS,
    ) -> Option<&'a ScheduledTask> {
        unsafe { &mut *self.0.get() }.peek()
    }

    /// Pops the task with the minimum deadline from the queue, without checking
    /// whether the queue is empty.
    pub(super) unsafe fn pop_unchecked<CS: DroppableCriticalSection>(
        &self,
        _cs: &CS,
    ) -> ScheduledTask {
        unsafe { (&mut *self.0.get()).pop_unchecked() }
    }
}
