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
    #[inline]
    pub fn new(rel_deadline: Deadline, dispatcher_idx: u16, rq_idx: u16) -> Self {
        Self {
            rel_deadline,
            dispatcher_idx,
            rq_idx,
        }
    }

    #[inline]
    pub fn rel_deadline(&self) -> Deadline {
        self.rel_deadline
    }

    #[inline]
    pub fn set_deadline(&mut self, deadline: Deadline) {
        self.rel_deadline = deadline;
    }

    #[inline]
    pub(crate) fn into_scheduled(self, now: Timestamp) -> ScheduledTask {
        let (deadline, wrapped) = now.overflowing_add(self.rel_deadline);
        assert!(!wrapped, "Deadline overflowed");
        ScheduledTask {
            // deadline: now.wrapping_add(self.rel_deadline),
            deadline,
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
    #[inline]
    pub fn abs_deadline(&self) -> Timestamp {
        self.deadline
    }

    /// Returns the run queue index associated with this task's dispatcher.
    #[inline]
    pub fn rq_index(&self) -> u16 {
        self.rq_idx
    }

    /// Returns the dispatcher's index (ie, which dispatcher to pend when we
    /// want to start running the task)
    #[inline]
    pub fn dispatcher_index(&self) -> u16 {
        self.dispatcher_idx
    }
}

// Tasks are only compared against each other on the basis of their deadline
impl PartialEq for ScheduledTask {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.deadline == other.deadline
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.deadline.cmp(&other.deadline)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum RunningTask {
    /// If preempted, the task enum contains the previous deadline that it has
    /// preempted
    Preempted(Timestamp),
    /// If it's an early dispatch, the enum contains the task's absolute
    /// deadline
    EarlyDispatch(Timestamp),
}

impl RunningTask {
    #[inline]
    pub(crate) fn early_dispatch(task: ScheduledTask) -> Self {
        Self::EarlyDispatch(task.deadline)
    }

    pub(crate) fn preempt(previous_dl: Timestamp) -> Self {
        Self::Preempted(previous_dl)
    }
}
