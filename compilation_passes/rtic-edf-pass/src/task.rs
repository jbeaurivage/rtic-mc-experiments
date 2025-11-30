use core::fmt::Debug;

use crate::types::{Deadline, Timestamp};

pub trait EdfTaskBinding {
    /// Dispatcher index associated with this task
    const DISPATCHER_IDX: u16;

    /// Mask the task's timestamper interrupt, therefore preventing it from
    /// preempting
    fn mask_timestamper_interrupt();

    /// Unmask the task's timestamper interrupt, therefore allowing it to resume
    /// preempting
    ///
    /// # Safety
    ///
    /// May break interrupt masking-based critical sections if misused
    unsafe fn unmask_timestamper_interrupt();
}

#[derive(Debug)]
pub struct Task {
    rel_deadline: Deadline,
    dispatcher_idx: u16,
}

impl Task {
    pub fn new(rel_deadline: Deadline, dispatcher_idx: u16) -> Self {
        Self {
            rel_deadline,
            dispatcher_idx,
        }
    }

    pub fn rel_deadline(&self) -> Deadline {
        self.rel_deadline
    }

    pub fn set_deadline(&mut self, deadline: Deadline) {
        self.rel_deadline = deadline;
    }

    pub(crate) fn into_queued(self, now: Timestamp) -> ScheduledTask {
        ScheduledTask {
            deadline: now.wrapping_add(self.rel_deadline),
            dispatcher_idx: self.dispatcher_idx,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ScheduledTask {
    deadline: Timestamp,
    dispatcher_idx: u16,
}

impl ScheduledTask {
    pub fn abs_deadline(&self) -> Timestamp {
        self.deadline
    }

    pub fn dispatcher_idx(&self) -> u16 {
        self.dispatcher_idx
    }
}

// Tasks are only compared against each other on the basis of their deadline
impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RunningTask {
    prev_deadline: Timestamp,
    // TODO: this field is not strictly necessary, only if we want to assert that we haven't missed
    // a deadline when we start executing the task
    abs_deadline: Timestamp,
}

impl RunningTask {
    pub(crate) fn from_scheduled(task: ScheduledTask, prev_deadline: Timestamp) -> Self {
        Self {
            prev_deadline,
            abs_deadline: task.deadline,
        }
    }

    pub(crate) fn prev_deadline(&self) -> Timestamp {
        self.prev_deadline
    }

    pub(crate) fn abs_deadline(&self) -> Timestamp {
        self.abs_deadline
    }
}
