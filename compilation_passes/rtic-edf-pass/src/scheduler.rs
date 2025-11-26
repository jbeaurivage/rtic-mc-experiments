use core::cell::UnsafeCell;
use heapless::{BinaryHeap, Vec, binary_heap::Min};

use crate::{
    critical_section::DroppableCriticalSection,
    task::{RunningTask, ScheduledTask, Task},
    util::Timestamp,
};

pub struct TaskStack<const N: usize>(UnsafeCell<Vec<RunningTask, N>>);

impl<const N: usize> TaskStack<N> {
    #[expect(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(UnsafeCell::new(Vec::new()))
    }

    fn get_mut<CS: DroppableCriticalSection>(&self, _cs: &CS) -> *mut Vec<RunningTask, N> {
        self.0.get()
    }
}

unsafe impl<const N: usize> Sync for TaskStack<N> {}

pub struct MinDeadline(UnsafeCell<Timestamp>);

impl MinDeadline {
    #[expect(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(UnsafeCell::new(Timestamp::MAX))
    }

    fn get_mut<CS: DroppableCriticalSection>(&self, _cs: &CS) -> *mut Timestamp {
        self.0.get()
    }
}

unsafe impl Sync for MinDeadline {}

pub struct TaskQueue<const N: usize>(UnsafeCell<BinaryHeap<ScheduledTask, Min, N>>);

unsafe impl<const N: usize> Sync for TaskQueue<N> {}

impl<const N: usize> TaskQueue<N> {
    #[expect(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(UnsafeCell::new(BinaryHeap::new()))
    }

    fn get_mut<CS: DroppableCriticalSection>(
        &self,
        _cs: &CS,
    ) -> *mut BinaryHeap<ScheduledTask, Min, N> {
        self.0.get()
    }
}

pub trait Scheduler<const S: usize, const Q: usize> {
    type CS: DroppableCriticalSection;
    fn now() -> Timestamp;
    fn pend_priority(prio: u16);

    fn task_queue(&self) -> &TaskQueue<Q>;
    fn min_deadline(&self) -> &MinDeadline;
    fn running_stack(&self) -> &TaskStack<S>;

    fn schedule(&self, task: Task) {
        // TODO
        // self.check_init();

        let cs = Self::CS::enter();
        let now = Self::now();

        let task = task.into_queued(now);
        let (stack, dl_ref) = unsafe {
            (
                &mut *self.running_stack().get_mut(&cs),
                *self.min_deadline().get_mut(&cs),
            )
        };

        if task.abs_deadline() < dl_ref || stack.is_empty() {
            self.execute(cs, task);
        } else {
            {
                let queue = unsafe { &mut *self.task_queue().get_mut(&cs) };

                queue.push(task).unwrap();
            }
        }
    }

    fn execute(&self, cs: Self::CS, task: ScheduledTask) {
        let dl_ref = unsafe { &mut *self.min_deadline().get_mut(&cs) };
        let prev_dl = *dl_ref;
        *dl_ref = task.abs_deadline();

        let stack = unsafe { &mut *self.running_stack().get_mut(&cs) };
        let dispatcher_prio = task.dispatcher_prio();

        stack
            .push(crate::task::RunningTask::from_scheduled(task, prev_dl))
            .unwrap();
        // let max_prio = stack.len() as u8;

        Self::pend_priority(dispatcher_prio);
    }

    #[inline]
    fn trampoline(&self) {
        let (callback, prev_deadline) = unsafe {
            let cs = Self::CS::enter();
            let task = (&*self.running_stack().get_mut(&cs)).last().unwrap();
            (task.callback(), task.prev_deadline())
        };

        // Finally call the actual task
        callback();

        // And cleanup after ourselves
        let cs = Self::CS::enter();
        let (stack, min_dl) = unsafe {
            (
                &mut *self.running_stack().get_mut(&cs),
                &mut *self.min_deadline().get_mut(&cs),
            )
        };

        stack.pop().unwrap();
        // Restore previous deadline
        *min_dl = prev_deadline;

        // It's possible that a task showed up in the queue as the previous task was
        // running. So we need to check if it would preempt the next task in line to
        // run, which would start as soon as the critical section exits.
        let queue = unsafe { &mut *self.task_queue().get_mut(&cs) };
        if let Some(task) = queue.peek()
            && (task.abs_deadline() < *min_dl || stack.is_empty())
        {
            let task = unsafe { queue.pop_unchecked() };
            self.execute(cs, task);
        }
    }
}
