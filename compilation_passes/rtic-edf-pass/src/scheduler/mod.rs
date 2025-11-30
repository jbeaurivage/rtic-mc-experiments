use crate::{
    critical_section::DroppableCriticalSection,
    task::{EdfTaskBinding, ScheduledTask, Task},
    types::Timestamp,
};

mod dispatch_queue;
pub use dispatch_queue::DispatchQueue;

mod system_deadline;
pub use system_deadline::SystemDeadline;

mod wait_queue;
pub use wait_queue::WaitQueue;

/// EDF scheduler. This trait is implemented at the `rtic-edf-pass` codegen
/// step.
pub trait Scheduler<const D_LEN: usize, const Q_LEN: usize>: Sized {
    type CS: DroppableCriticalSection;

    fn now() -> Timestamp;
    fn pend_dispatcher(idx: u16);

    fn dispatch_queue(&self) -> &DispatchQueue<D_LEN>;
    fn system_deadline(&self) -> &SystemDeadline;
    fn wait_queue(&self) -> &WaitQueue<Q_LEN>;

    /// Signal to the scheduler that a task wants to run.
    fn schedule(&self, task: Task) {
        let cs = Self::CS::enter();
        let now = Self::now();

        let rel_dl = task.rel_deadline();

        let task = task.into_queued(now);

        let dispatcher_ready = self.dispatch_queue().is_ready(&cs, task.dispatcher_idx());
        let sys_dl = self.system_deadline().get(&cs);

        #[cfg(feature = "defmt")]
        defmt::debug!(
            "[SCHEDULE] now: {}, rel dl: {}, abs dl: {}, min dl: {}, dispatcher idx: {}, dispatcher ready: {}",
            now,
            rel_dl,
            task.abs_deadline(),
            sys_dl,
            task.dispatcher_idx(),
            dispatcher_ready
        );

        if task.abs_deadline() < sys_dl || dispatcher_ready {
            #[cfg(feature = "defmt")]
            defmt::debug!("[PREEMPT]");
            execute(self, cs, task);
        } else {
            {
                // let queue = unsafe { &mut *self.wait_queue().get_mut(&cs) };
                #[cfg(feature = "defmt")]
                defmt::debug!("[ENQUEUE] queue length: {}", self.wait_queue().len(&cs));

                self.wait_queue().push(&cs, task);
            }
        }
    }

    /// Dispatcher entry
    ///
    /// This function must be called at the top of a dispatcher, before the task
    /// executes.
    ///
    /// # Returns
    ///
    /// The previous timeline, which must be restored when the task completes.
    ///
    /// Each dispatcher should call these functions as follows:
    ///
    /// 1. dispatcher_entry()
    /// 2. Execute its task
    /// 3. dispatcher_exit()
    #[inline]
    fn dispatcher_entry(&self, dispatcher_idx: u16) -> Timestamp {
        let cs = Self::CS::enter();

        let task_to_run = self
            .dispatch_queue()
            .retrieve(&cs, dispatcher_idx)
            .expect("BUG: a task should be available to run");

        let prev_deadline = task_to_run.prev_deadline();

        #[cfg(feature = "defmt")]
        defmt::assert!(Self::now() <= task_to_run.abs_deadline(), "Missed deadline");
        #[cfg(not(feature = "defmt"))]
        assert!(Self::now() <= task_to_run.abs_deadline(), "Missed deadline");

        prev_deadline
    }

    /// Dispatcher exit
    ///
    /// This function must be called immediately after a task has completed in a
    /// dispatcher.
    ///
    /// 1. Restores the previous deadline
    /// 2. Looks in the global queue, and pends the shortest deadline task if it
    ///    has a shorter deadline than the currently running task, or if its
    ///    dispatcher is ready to accept a new task.
    ///
    /// The `prev_deadline` argument must be the timestamp returned by the
    /// `dispatcher_entry` call that happened immediately before the task
    /// was run.
    ///
    /// Unfortunately has to be generic over [`EdfTaskBinding`] because of the
    /// interrupt unmasking associated function, which means it will get
    /// monomorphized.
    #[inline]
    fn dispatcher_exit<T: EdfTaskBinding>(&self, prev_deadline: Timestamp) {
        // And cleanup after ourselves
        let cs = Self::CS::enter();
        self.dispatch_queue().complete_task(&cs, T::DISPATCHER_IDX);
        unsafe {
            T::unmask_timestamper_interrupt();
        }

        // Restore previous deadline
        let _ = self.system_deadline().replace(&cs, prev_deadline);

        #[cfg(feature = "defmt")]
        defmt::debug!(
            // "[COMPLETE TASK] new dl: {}, stack depth: {}",
            "[COMPLETE TASK] new dl: {}, dispatcher idx: {}",
            prev_deadline,
            T::DISPATCHER_IDX,
            // stack.len(),
        );

        // It's possible that a task showed up in the queue as the previous task was
        // running. So we need to check if it would preempt the next task in line to
        // run, which would start as soon as the critical section exits.
        //
        // If the next task's dispatcher is currently ready to accept tasks, we can
        // send it to its own dispatcher. This is how we retrieve items from the
        // queue.
        let sys_dl = self.system_deadline().get(&cs);
        let next_task = self.wait_queue().next_task(&cs);

        if let Some(task) = next_task
            && ((task.abs_deadline() < sys_dl)
                || (self.dispatch_queue().is_ready(&cs, task.dispatcher_idx())))
        {
            let task = unsafe { self.wait_queue().pop_unchecked(&cs) };
            #[cfg(feature = "defmt")]
            defmt::debug!("[DEQUEUE TASK] dispatcher idx: {}", task.dispatcher_idx());
            execute(self, cs, task);
        }
    }
}

/// Execute a task
///
/// This function performs the follwing:
///
/// 1. (unconditionnally) sets the system deadline to the task slated for
///    execution's deadline
/// 2. Add the task its dispatcher's queue
/// 3. Pend the dispatcher interrupt, which will run as soon as there are no
///    higher priority interrupts running
///
/// **note**: This function is excluded from the [`Scheduler`] trait in order to
/// avoid it being callable from within a RTIC app.
#[inline]
fn execute<S, CS, const D_LEN: usize, const Q_LEN: usize>(
    scheduler: &S,
    cs: CS,
    task: ScheduledTask,
) where
    S: Scheduler<D_LEN, Q_LEN, CS = CS>,
    CS: DroppableCriticalSection,
{
    let prev_dl = scheduler
        .system_deadline()
        .replace(&cs, task.abs_deadline());

    let dispatcher_idx = task.dispatcher_idx();

    scheduler.dispatch_queue().pend_task(
        &cs,
        crate::task::RunningTask::from_scheduled(task, prev_dl),
        dispatcher_idx,
    );
    // let max_prio = stack.len() as u8;

    #[cfg(feature = "defmt")]
    defmt::debug!(
        // "[EXEC] max prio: {}, dispatcher prio: {}, now: {}, new dl: {}, prev dl: {}",
        "[EXEC] dispatcher idx: {}, new dl: {}, prev dl: {}",
        // max_prio,
        dispatcher_idx,
        scheduler.system_deadline().get(&cs),
        prev_dl
    );

    S::pend_dispatcher(dispatcher_idx);
}
