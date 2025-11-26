use crate::util::{Deadline, Timestamp};

#[derive(Debug)]
pub struct Task {
    rel_deadline: Deadline,
    dispatcher_prio: u16,
    callback: fn(),
}

impl Task {
    pub fn new(rel_deadline: Deadline, dispatcher_prio: u16, callback: fn()) -> Self {
        Self {
            rel_deadline,
            dispatcher_prio,
            callback,
        }
    }

    pub fn rel_deadline(&self) -> Deadline {
        self.rel_deadline
    }

    pub fn set_deadline(&mut self, deadline: Deadline) {
        self.rel_deadline = deadline;
    }

    pub fn into_queued(self, now: Timestamp) -> ScheduledTask {
        ScheduledTask {
            deadline: now.wrapping_add(self.rel_deadline),
            dispatcher_prio: self.dispatcher_prio,
            callback: self.callback,
        }
    }
}

#[allow(unpredictable_function_pointer_comparisons)]
#[derive(Debug, PartialEq, Eq)]
pub struct ScheduledTask {
    deadline: Timestamp,
    dispatcher_prio: u16,
    callback: fn(),
}

impl ScheduledTask {
    pub fn abs_deadline(&self) -> Timestamp {
        self.deadline
    }

    pub fn dispatcher_prio(&self) -> u16 {
        self.dispatcher_prio
    }
}

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

#[derive(Debug)]
pub struct RunningTask {
    prev_deadline: Timestamp,
    callback: fn(),
}

impl RunningTask {
    pub fn from_scheduled(task: ScheduledTask, prev_deadline: Timestamp) -> Self {
        Self {
            prev_deadline,
            callback: task.callback,
        }
    }

    pub fn prev_deadline(&self) -> Timestamp {
        self.prev_deadline
    }

    pub fn callback(&self) -> fn() {
        self.callback
    }
}
