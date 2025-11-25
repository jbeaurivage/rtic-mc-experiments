static RUNNING_STACK: TaskStack = TaskStack(UnsafeCell::new(Vec::new()));
static MIN_DEADLINE: MinDeadline = MinDeadline(UnsafeCell::new(u32::MAX));
static PARKED_QUEUE: TaskQueue = TaskQueue(UnsafeCell::new(BinaryHeap::new()));

use core::sync::atomic::{AtomicBool, Ordering};

pub struct Scheduler {
    ready: AtomicBool,
}
impl core::default::Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            ready: AtomicBool::new(false),
        }
    }

    pub fn check_init(&self) {
        if !self.ready.load(Ordering::SeqCst) {
            panic!("Scheduler not initialized");
        }
    }

    pub fn init(&self, nvic: &mut NVIC, scb: &mut SCB) {
        interrupt::disable();

        // Before we can start messing with the vector table, we must first copy it over
        // to RAM
        crate::vector_table::copy_vector_table(scb);

        for (level, interrupt) in DISPATCHERS.iter().enumerate() {
            // TODO remove this "8" magic number somehow, which is the number of priorities
            // available on the ATSAMD51J
            let nvic_prio = (8 - (level as u8 + 1)) << 4;

            unsafe {
                NVIC::unpend(*interrupt);
                NVIC::unmask(*interrupt);
                nvic.set_priority(*interrupt, nvic_prio);
            }
        }

        self.ready.swap(true, Ordering::SeqCst);
        // interrupt::enable();
    }

    pub fn schedule(&self, task: crate::task::Task) {
        self.check_init();

        let cs = CsGuard::new();
        let now = now();

        let task = task.into_queued(now);
        let (stack, min_dl) =
            unsafe { (&mut *RUNNING_STACK.get_mut(&cs), *MIN_DEADLINE.get_mut(&cs)) };

        if task.abs_deadline() < min_dl || stack.is_empty() {
            Self::execute(cs, task, now);
        } else {
            {
                let queue = unsafe { &mut *PARKED_QUEUE.get_mut(&cs) };

                queue.push(task).unwrap();
            }
        }
    }

    fn execute(cs: CsGuard, task: crate::task::ScheduledTask, now: crate::util::Timestamp) {
        let min_dl = unsafe { &mut *MIN_DEADLINE.get_mut(&cs) };
        let prev_dl = *min_dl;
        *min_dl = task.abs_deadline();

        let stack = unsafe { &mut *RUNNING_STACK.get_mut(&cs) };

        stack
            .push(crate::task::RunningTask::from_scheduled(task, prev_dl))
            .unwrap();
        let max_prio = stack.len() as u8;

        let irq = dispatcher_irq(max_prio);
        unsafe { set_handler(irq, run_task) };
        NVIC::pend(dispatcher(max_prio));
    }

    pub fn idle(&self) -> ! {
        unsafe { cortex_m::interrupt::enable() };
        loop {
            cortex_m::asm::wfi();
        }
    }
}
