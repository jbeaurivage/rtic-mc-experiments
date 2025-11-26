use crate::util::{Deadline, Timestamp};

pub trait Runnable {
    fn run(&mut self);
}

pub struct Task<'a> {
    rel_deadline: Deadline,
    dispatcher_prio: u16,
    task: &'a mut dyn Runnable,
    // callback: fn(),
}

impl<'a> Task<'a> {
    pub fn new(rel_deadline: Deadline, dispatcher_prio: u16, tsk: &'a mut dyn Runnable) -> Self {
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
            dispatcher_prio: self.dispatcher_prio,
            task: self.task,
        }
    }
}

pub struct ScheduledTask<'a> {
    deadline: Timestamp,
    dispatcher_prio: u16,
    task: &'a mut dyn Runnable,
    // callback: fn(),
}

impl ScheduledTask<'_> {
    pub fn abs_deadline(&self) -> Timestamp {
        self.deadline
    }

    pub fn dispatcher_prio(&self) -> u16 {
        self.dispatcher_prio
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

pub struct RunningTask<'a> {
    prev_deadline: Timestamp,
    task: &'a mut dyn Runnable,
    // callback: fn(),
}

impl<'a> RunningTask<'a> {
    pub fn from_scheduled(task: ScheduledTask<'a>, prev_deadline: Timestamp) -> Self {
        Self {
            prev_deadline,
            task: task.task,
        }
    }

    pub fn prev_deadline(&self) -> Timestamp {
        self.prev_deadline
    }

    pub fn task_to_run<'b>(&'a mut self) -> &'b mut dyn Runnable
    where
        'a: 'b,
    {
        self.task
    }

    // pub fn callback(&self) -> fn() {
    //     self.callback
    // }
}
