#![expect(clippy::new_without_default)]

use core::cell::UnsafeCell;

use heapless::{binary_heap::Min, BinaryHeap};

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
        #[cfg(debug_assertions)]
        unsafe { &mut *self.0.get() }
            .push(task)
            .expect("[RTIC BUG]: EDF wait queue is full");

        // SAFETY: the length check can be bypassed under 2 conditions:
        //
        // 1. No task can be double-pended (ie, pended while another instance of itself
        //    is already running). This is guaranteed because we mask the task's
        //    interrupt until it has finished running, plus we unpend it before it is
        //    allowed to be signaled again.
        // 2. The queue is of sufficient length (ie, the total number of EDF tasks - the
        //    number of unique priorities), given that for each priority level, we can
        //    bypass the queue one time if the priority is empty before having to go
        //    through the queue.
        #[cfg(not(debug_assertions))]
        unsafe {
            (&mut *self.0.get()).push_unchecked(task);
        }
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
