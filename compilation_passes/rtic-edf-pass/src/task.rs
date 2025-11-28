use core::fmt::Debug;

use crate::util::{Deadline, Timestamp};

pub trait Runnable: 'static {
    fn run(&mut self);
    fn mask_interrupt(&mut self);

    /// # Safety
    ///
    /// May break interrupt masking-based critical sections if misused
    unsafe fn unmask_interrupt(&mut self);
}

pub struct Task<'a, R: Runnable> {
    rel_deadline: Deadline,
    dispatcher_prio: u16,
    task: &'a mut R,
    // callback: fn(),
}

impl<'a, R: Runnable> Task<'a, R> {
    pub fn new(rel_deadline: Deadline, dispatcher_prio: u16, tsk: &'a mut R) -> Self {
        Self {
            rel_deadline,
            dispatcher_prio,
            task: tsk,
        }
    }

    pub fn rel_deadline(&self) -> Deadline {
        self.rel_deadline
    }

    pub fn set_deadline(&mut self, deadline: Deadline) {
        self.rel_deadline = deadline;
    }

    pub fn into_queued(self, now: Timestamp) -> ScheduledTask<'a> {
        ScheduledTask {
            deadline: now.wrapping_add(self.rel_deadline),
            dispatcher_idx: self.dispatcher_prio,
            task: self.task,
        }
    }
}

impl<R: Runnable> Debug for Task<'_, R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Task")
            .field("rel_deadline", &self.rel_deadline)
            .field("dispatcher_prio", &self.dispatcher_prio)
            .finish()
    }
}

pub struct ScheduledTask<'a> {
    deadline: Timestamp,
    dispatcher_idx: u16,
    task: &'a mut dyn Runnable,
    // callback: fn(),
}

impl ScheduledTask<'_> {
    pub fn abs_deadline(&self) -> Timestamp {
        self.deadline
    }

    pub fn dispatcher_idx(&self) -> u16 {
        self.dispatcher_idx
    }
}

impl PartialEq for ScheduledTask<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for ScheduledTask<'_> {}

impl PartialOrd for ScheduledTask<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask<'_> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

impl Debug for ScheduledTask<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Task")
            .field("deadline", &self.deadline)
            .field("dispatcher_idx", &self.dispatcher_idx)
            .finish()
    }
}

pub struct RunningTask<'a> {
    prev_deadline: Timestamp,
    abs_deadline: Timestamp,
    task: &'a mut dyn Runnable,
    // callback: fn(),
}

impl<'a> RunningTask<'a> {
    pub fn from_scheduled(task: ScheduledTask<'a>, prev_deadline: Timestamp) -> Self {
        Self {
            prev_deadline,
            abs_deadline: task.deadline,
            task: task.task,
        }
    }

    #[inline]
    pub(crate) fn run(&mut self) {
        self.task.run();
    }

    #[inline]
    pub(crate) unsafe fn unmask_interrupt(&mut self) {
        unsafe {
            self.task.unmask_interrupt();
        }
    }

    pub fn prev_deadline(&self) -> Timestamp {
        self.prev_deadline
    }

    pub fn abs_deadline(&self) -> Timestamp {
        self.abs_deadline
    }

    // pub fn callback(&self) -> fn() {
    //     self.callback
    // }
}
