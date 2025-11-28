use core::cell::UnsafeCell;
use heapless::{BinaryHeap, binary_heap::Min};

use crate::{
    critical_section::DroppableCriticalSection,
    task::{Runnable, RunningTask, ScheduledTask, Task},
    util::Timestamp,
};

enum DispatcherSlot {
    Pending(RunningTask<'static>),
    Running,
    Ready,
}

pub struct DispatchQueue<const N: usize>(UnsafeCell<[DispatcherSlot; N]>);

impl<const N: usize> DispatchQueue<N> {
    #[expect(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(UnsafeCell::new([const { DispatcherSlot::Ready }; N]))
    }

    #[expect(clippy::mut_from_ref)]
    unsafe fn slot<CS: DroppableCriticalSection>(
        &self,
        _cs: &CS,
        dispatcher_idx: usize,
    ) -> &mut DispatcherSlot {
        let queue = unsafe { &mut *self.0.get() };
        queue
            .get_mut(dispatcher_idx)
            .expect("BUG: dispatcher idx doesn't exist")
    }

    /// Insert a pending task to the queue for later retrieval
    fn pend_task<CS: DroppableCriticalSection>(
        &self,
        cs: &CS,
        task: RunningTask<'static>,
        dispatcher_idx: usize,
    ) {
        let slot = unsafe { self.slot(cs, dispatcher_idx) };

        if !matches!(slot, DispatcherSlot::Ready) {
            panic!("Task has been skipped!");
        }

        let _ = core::mem::replace(slot, DispatcherSlot::Pending(task));
    }

    /// Retrieve the task to run, and mark it as running in the message queue
    fn retrieve<CS: DroppableCriticalSection>(
        &self,
        cs: &CS,
        dispatcher_idx: usize,
    ) -> Option<RunningTask<'_>> {
        let slot = unsafe { self.slot(cs, dispatcher_idx) };
        if let DispatcherSlot::Pending(_) = slot {
            let DispatcherSlot::Pending(t) = core::mem::replace(slot, DispatcherSlot::Running)
            else {
                panic!("BUG: task should be pending");
            };
            Some(t)
        } else {
            None
        }
    }

    /// Signal that the task has completed by marking the queue slot as idle
    fn complete_task<CS: DroppableCriticalSection>(&self, cs: &CS, dispatcher_idx: usize) {
        let slot = unsafe { self.slot(cs, dispatcher_idx) };

        if !matches!(slot, DispatcherSlot::Running) {
            panic!("Pending task set to idle!");
        }

        let _ = core::mem::replace(slot, DispatcherSlot::Ready);
    }

    /// Whether or not the dispatcher is ready to accept a new task to run
    fn ready<CS: DroppableCriticalSection>(&self, cs: &CS, dispatcher_idx: usize) -> bool {
        let slot = unsafe { self.slot(cs, dispatcher_idx) };
        matches!(slot, DispatcherSlot::Ready)
    }
}

unsafe impl<const N: usize> Sync for DispatchQueue<N> {}

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

pub struct WaitQueue<const N: usize>(UnsafeCell<BinaryHeap<ScheduledTask<'static>, Min, N>>);

unsafe impl<const N: usize> Sync for WaitQueue<N> {}

impl<const N: usize> WaitQueue<N> {
    #[expect(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(UnsafeCell::new(BinaryHeap::new()))
    }

    fn get_mut<CS: DroppableCriticalSection>(
        &self,
        _cs: &CS,
    ) -> *mut BinaryHeap<ScheduledTask<'static>, Min, N> {
        self.0.get()
    }
}

pub trait Scheduler<const S: usize, const Q: usize> {
    type CS: DroppableCriticalSection;

    fn now() -> Timestamp;
    fn pend_dispatcher(idx: u16);

    fn dispatch_queue(&self) -> &DispatchQueue<S>;
    fn min_deadline(&self) -> &MinDeadline;
    fn wait_queue(&self) -> &WaitQueue<Q>;

    fn schedule<R: Runnable>(&self, task: Task<'static, R>) {
        let cs = Self::CS::enter();
        let now = Self::now();

        let rel_dl = task.rel_deadline();

        let task = task.into_queued(now);
        let min_dl = unsafe { *self.min_deadline().get_mut(&cs) };

        let dispatcher_ready = self
            .dispatch_queue()
            .ready(&cs, task.dispatcher_idx() as usize);

        #[cfg(feature = "defmt")]
        defmt::debug!(
            "[SCHEDULE] now: {}, rel dl: {}, abs dl: {}, min dl: {}, dispatcher idx: {}, dispatcher ready: {}",
            now,
            rel_dl,
            task.abs_deadline(),
            min_dl,
            task.dispatcher_idx(),
            dispatcher_ready
        );

        if task.abs_deadline() < min_dl || dispatcher_ready {
            #[cfg(feature = "defmt")]
            defmt::debug!("[PREEMPT]");
            self.execute(cs, task);
        } else {
            {
                let queue = unsafe { &mut *self.wait_queue().get_mut(&cs) };
                #[cfg(feature = "defmt")]
                defmt::debug!("[ENQUEUE] queue length: {}", queue.len());

                queue
                    .push(task)
                    .unwrap_or_else(|_| panic!("EDF task queue is full"));
            }
        }
    }

    fn execute(&self, cs: Self::CS, task: ScheduledTask<'static>) {
        let min_dl = unsafe { &mut *self.min_deadline().get_mut(&cs) };
        let prev_dl = *min_dl;
        *min_dl = task.abs_deadline();

        let dispatcher_idx = task.dispatcher_idx();

        self.dispatch_queue().pend_task(
            &cs,
            crate::task::RunningTask::from_scheduled(task, prev_dl),
            dispatcher_idx as usize,
        );
        // let max_prio = stack.len() as u8;

        #[cfg(feature = "defmt")]
        defmt::debug!(
            // "[EXEC] max prio: {}, dispatcher prio: {}, now: {}, new dl: {}, prev dl: {}",
            "[EXEC] dispatcher idx: {}, new dl: {}, prev dl: {}",
            // max_prio,
            dispatcher_idx,
            &*min_dl,
            prev_dl
        );

        Self::pend_dispatcher(dispatcher_idx);
    }

    #[inline]
    fn dispatch<const D_IDX: usize>(&self) {
        let cs = Self::CS::enter();

        let mut task_to_run = self
            .dispatch_queue()
            .retrieve(&cs, D_IDX)
            .expect("BUG: a task should be available to run");

        let prev_deadline = task_to_run.prev_deadline();

        #[cfg(feature = "defmt")]
        defmt::assert!(Self::now() <= task_to_run.abs_deadline(), "Missed deadline");
        #[cfg(not(feature = "defmt"))]
        assert!(Self::now() <= task_to_run.abs_deadline(), "Missed deadline");

        cs.exit();

        // Finally, call the actual task
        task_to_run.run();

        // And cleanup after ourselves
        let cs = Self::CS::enter();
        self.dispatch_queue().complete_task(&cs, D_IDX);
        unsafe {
            task_to_run.unmask_interrupt();
        }

        let min_dl = unsafe { &mut *self.min_deadline().get_mut(&cs) };

        // Restore previous deadline
        *min_dl = prev_deadline;

        #[cfg(feature = "defmt")]
        defmt::debug!(
            // "[COMPLETE TASK] new dl: {}, stack depth: {}",
            "[COMPLETE TASK] new dl: {}, dispatcher idx: {}",
            prev_deadline,
            D_IDX,
            // stack.len(),
        );

        // It's possible that a task showed up in the queue as the previous task was
        // running. So we need to check if it would preempt the next task in line to
        // run, which would start as soon as the critical section exits.
        //
        // If the next task's dispatcher is currently ready to accept tasks, we can
        // send it to its own dispatcher. This is how we can retrieve items from the
        // queue.
        let queue = unsafe { &mut *self.wait_queue().get_mut(&cs) };
        if let Some(task) = queue.peek()
            && (task.abs_deadline() < *min_dl
                || (self
                    .dispatch_queue()
                    .ready(&cs, task.dispatcher_idx() as usize)))
        {
            let task = unsafe { queue.pop_unchecked() };
            #[cfg(feature = "defmt")]
            defmt::debug!("[DEQUEUE TASK] dispatcher idx: {}", task.dispatcher_idx());
            self.execute(cs, task);
        }
    }
}
