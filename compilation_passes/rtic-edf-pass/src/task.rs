use core::fmt::Debug;

use crate::types::{Deadline, Timestamp};

pub trait EdfTaskBinding {
    /// Dispatcher index associated with this task
    const DISPATCHER_IDX: u16;

    /// The index this task's dispatcher is associated with in the run queue
    const RUN_QUEUE_IDX: u16;

    /// Mask the task's timestamper interrupt, therefore preventing it from
    /// preempting
    fn mask_timestamper_interrupt();

    /// Unpend the task's timestamper interrupt
    fn unpend_timestamper_interrupt();

    /// Unmask the task's timestamper interrupt, therefore allowing it to resume
    /// preempting
    ///
    /// # Safety
    ///
    /// May break interrupt masking-based critical sections if misused
    unsafe fn unmask_timestamper_interrupt();
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Task {
    rel_deadline: Deadline,
    /// The index of this task's dispatcher in the list of all available
    /// dispatchers
    dispatcher_idx: u16,
    /// The run queue index of this task's dispatcher
    rq_idx: u16,
}

impl Task {
    pub fn new(rel_deadline: Deadline, dispatcher_idx: u16, rq_idx: u16) -> Self {
        Self {
            rel_deadline,
            dispatcher_idx,
            rq_idx,
        }
    }

    pub fn rel_deadline(&self) -> Deadline {
        self.rel_deadline
    }

    pub fn set_deadline(&mut self, deadline: Deadline) {
        self.rel_deadline = deadline;
    }

    pub(crate) fn into_scheduled(self, now: Timestamp) -> ScheduledTask {
        ScheduledTask {
            deadline: now.wrapping_add(self.rel_deadline),
            dispatcher_idx: self.dispatcher_idx,
            rq_idx: self.rq_idx,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ScheduledTask {
    deadline: Timestamp,
    dispatcher_idx: u16,
    rq_idx: u16,
}

impl ScheduledTask {
    pub fn abs_deadline(&self) -> Timestamp {
        self.deadline
    }

    /// Returns the run queue index associated with this task's dispatcher.
    pub fn rq_index(&self) -> u16 {
        self.rq_idx
    }

    /// Returns the dispatcher's index (ie, which dispatcher to pend when we
    /// want to start running the task)
    pub fn dispatcher_index(&self) -> u16 {
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
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct RunningTask {
    prev_deadline: Timestamp,
    #[cfg(feature = "check-missed-deadlines")]
    abs_deadline: Timestamp,
}

impl RunningTask {
    pub(crate) fn from_scheduled(_task: ScheduledTask, prev_deadline: Timestamp) -> Self {
        Self {
            prev_deadline,

            #[cfg(feature = "check-missed-deadlines")]
            abs_deadline: _task.deadline,
        }
    }

    pub(crate) fn prev_deadline(&self) -> Timestamp {
        self.prev_deadline
    }

    #[cfg(feature = "check-missed-deadlines")]
    pub(crate) fn abs_deadline(&self) -> Timestamp {
        self.abs_deadline
    }
}
