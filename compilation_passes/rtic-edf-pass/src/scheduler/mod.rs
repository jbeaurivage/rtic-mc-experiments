use crate::{
    critical_section::{DroppableCriticalSection, NoopCs},
    task::{EdfTaskBinding, RunningTask, ScheduledTask, Task},
    types::Timestamp,
};

mod run_queue;
pub use run_queue::RunQueue;

mod system_deadline;
pub use system_deadline::SystemDeadline;

mod wait_queue;
pub use wait_queue::WaitQueue;

#[cfg(feature = "benchmark")]
pub mod benchmark;

/// EDF scheduler. This trait is implemented at the `rtic-edf-pass` codegen
/// step.
pub trait Scheduler<const NUM_DISPATCH_PRIOS: usize, const Q_LEN: usize>: Sized {
    type CS: DroppableCriticalSection;

    fn now() -> Timestamp;
    fn pend_dispatcher(idx: u16);

    fn run_queue(&self) -> &RunQueue<NUM_DISPATCH_PRIOS>;
    fn system_deadline(&self) -> &SystemDeadline;
    fn wait_queue(&self) -> &WaitQueue<Q_LEN>;

    /// Signal to the scheduler that a task wants to run.
    fn schedule(&self, task: Task) {
        // SAFETY: This is only valid when we are sure that this function runs at the
        // maximum (most urgent) system priority, and thus cannot be preempted.
        let cs = NoopCs::enter();
        let now = Self::now();

        #[cfg(feature = "defmt")]
        let rel_dl = task.rel_deadline();

        let task = task.into_scheduled(now);

        let dispatcher_ready = self.run_queue().is_ready(&cs, task.rq_index());
        let sys_dl = self.system_deadline().get(&cs);

        #[cfg(feature = "defmt")]
        defmt::trace!(
            "[SCHEDULE] now: {}, rel dl: {}, abs dl: {}, sys dl: {}, dispatcher idx: {}, run queue idx: {}, dispatcher ready: {}, abs_dl < sys_dl : {}",
            now,
            rel_dl,
            task.abs_deadline(),
            sys_dl,
            task.dispatcher_index(),
            task.rq_index(),
            dispatcher_ready,
            task.abs_deadline() < sys_dl,
        );

        // We can bypass the queue once per dispatcher priority level (even if the
        // task's deadline is farther in the future than the system deadline), by
        // directly pending the task in its dispatcher if the slot is empty. The
        // scheduling will then happen according to the NVIC's priority
        // configurations, which is what we want (ie, a tiny bit of hardware
        // acceleration to the rescue).
        //
        // This only works because every priority level only has one unique relative
        // deadline, such that no task can ever preempt another with the same deadline
        if task.abs_deadline() < sys_dl || dispatcher_ready {
            let preempt = task.abs_deadline() < sys_dl;

            #[cfg(feature = "defmt")]
            defmt::trace!("[DIRECT EXECUTE] preempt: {}", preempt);

            execute(self, cs, task, preempt);
        } else {
            {
                #[cfg(feature = "defmt")]
                defmt::trace!("[ENQUEUE] queue length: {}", self.wait_queue().len(&cs));

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
    fn dispatcher_entry(&self, rq_idx: u16) -> Timestamp {
        // The dispatcher runs at its own priority (lower than the timestamper prio).
        // Therefore we need a "real" critical section here.
        let cs = Self::CS::enter();

        let task_to_run = self
            .run_queue()
            .peek(&cs, rq_idx)
            .expect("BUG: a task should be available to run");

        #[cfg(feature = "defmt")]
        defmt::trace!(
            "[DISPATCHER ENTRY] sys dl: {}, task: {}",
            self.system_deadline().get(&cs),
            task_to_run
        );

        let (prev_deadline, _abs_dl) = match task_to_run {
            RunningTask::Preempted(previous_dl) => (*previous_dl, self.system_deadline().get(&cs)),
            RunningTask::EarlyDispatch(abs_dl) => {
                (self.system_deadline().replace(&cs, *abs_dl), *abs_dl)
            }
        };

        // Optionally assert that the deadline hasn't been missed
        #[cfg(all(feature = "defmt", feature = "check-missed-deadlines"))]
        {
            // TODO: cortex-m leaking here
            use cortex_m::peripheral::scb::VectActive;

            let now = Self::now();

            let vect_active = cortex_m::peripheral::SCB::vect_active();
            let irqn = match vect_active {
                VectActive::Interrupt { irqn } => Some(irqn),
                _ => None,
            };

            defmt::assert!(
                now <= _abs_dl,
                "Missed deadline. \n\tnow: {}\n\tDeadline: {}\n\tdiff: {}\n\tQueue len: {}\n\tRun queue idx: {}\n\tISR: {}",
                now,
                _abs_dl,
                now - _abs_dl,
                self.wait_queue().len(&cs),
                rq_idx,
                irqn,
            );
        }

        #[cfg(all(not(feature = "defmt"), feature = "check-missed-deadlines"))]
        assert!(Self::now() <= _abs_dl, "Missed deadline");

        prev_deadline

        // critical section is dropped here, therefore reenabling interrupts
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
        // The dispatcher runs at its own priority (lower than the timestamper prio).
        // Therefore we need a "real" critical section here.
        let cs = Self::CS::enter();
        let rq = self.run_queue();
        let wq = self.wait_queue();

        rq.mark_complete(&cs, T::RUN_QUEUE_IDX);

        #[cfg(feature = "defmt")]
        defmt::trace!(
            "[COMPLETE TASK] new dl: {}, dispatcher idx: {}, run queue idx: {}",
            prev_deadline,
            T::DISPATCHER_IDX,
            T::RUN_QUEUE_IDX,
        );

        // The timestamper -> scheduler jump means that we will have exited the
        // timestamper interrupt while the interrupt source is still pending (because
        // the task itself -ie, the user code- must act upon it to clear the interrupt
        // flag - think, for example, of a UART that must read its data register to
        // clear the flag). Therefore the timestamper interrupt will have been
        // erroneously re-pended as soon as it is exited, which would lead to the task
        // being scheduled+executed twice if we didn't manually unpend it.
        T::unpend_timestamper_interrupt();

        // The timestamper interrupt is also masked just before calling `schedule()`.
        // Currently this call is generated to avoid the `dispatcher_entry` method
        // having to take a generic type param to the task binding (see codegen.rs).
        unsafe {
            T::unmask_timestamper_interrupt();
        }

        // Restore previous deadline
        let _ = self.system_deadline().replace(&cs, prev_deadline);

        // It's possible that a task showed up in the queue as the previous (just
        // completed) task was running. So we need to check if it would preempt
        // the next task in line to run, which would start as soon as the
        // critical section exits.
        //
        // If the next task's slot in the run queue is currently ready to accept tasks,
        // we can send it to its own dispatcher. This is how we can empty the
        // wait queue from its non-preempting items.
        //
        // This works because all tasks that share a dispatcher run queue slot have the
        // same deadline, therefore they will never try to preempt each other, but
        // rather be enqueued. Therefore if the run queue's slot is already full for
        // this priority level, it is guaranteed to have a shorter deadline than any
        // dequeued task on the same prio level.
        let sys_dl = self.system_deadline().get(&cs);
        let next_task = wq.next_task(&cs);

        if let Some(task) = next_task
            && (task.abs_deadline() < sys_dl)
            && (rq.is_ready(&cs, task.rq_index()))
        {
            let preempt = task.abs_deadline() < sys_dl;
            let task = unsafe { wq.pop_unchecked(&cs) };
            #[cfg(feature = "defmt")]
            defmt::trace!(
                "[DEQUEUE TASK] now: {}, sys dl: {}, preempt: {}, task dispatcher: {}, task run queue idx: {}, task dl: {}",
                Self::now(),
                sys_dl,
                preempt,
                task.dispatcher_index(),
                task.rq_index(),
                task.abs_deadline(),
            );

            execute(self, cs, task, preempt);
        }

        // critical section is dropped here, therefore reenabling interrupts
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
/// **Note**: This function is excluded from the [`Scheduler`] trait in order to
/// avoid it being callable from within an RTIC app.
#[inline]
fn execute<S, CS, const D_LEN: usize, const Q_LEN: usize>(
    scheduler: &S,
    cs: CS,
    task: ScheduledTask,
    preempt: bool,
) where
    S: Scheduler<D_LEN, Q_LEN>,
    CS: DroppableCriticalSection,
{
    let rq_idx = task.rq_index();
    let dispatcher_idx = task.dispatcher_index();

    if preempt {
        let prev_dl = scheduler
            .system_deadline()
            .replace(&cs, task.abs_deadline());

        #[cfg(feature = "defmt")]
        defmt::trace!(
            "[EXEC preempt] new dl: {}, prev dl: {}",
            scheduler.system_deadline().get(&cs),
            prev_dl
        );

        scheduler
            .run_queue()
            .insert(&cs, RunningTask::preempt(prev_dl), rq_idx);
    } else {
        #[cfg(feature = "defmt")]
        defmt::trace!(
            "[EXEC early dispatch] sys dl: {}, abs dl: {}",
            scheduler.system_deadline().get(&cs),
            task.abs_deadline(),
        );
        scheduler
            .run_queue()
            .insert(&cs, RunningTask::early_dispatch(task), rq_idx);
    }

    S::pend_dispatcher(dispatcher_idx);

    // critical section is dropped here, therefore reenabling interrupts
}
