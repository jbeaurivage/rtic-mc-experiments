use core::cell::UnsafeCell;
use heapless::{binary_heap::Min, BinaryHeap, Vec};

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

pub trait Scheduler {
    type CS: DroppableCriticalSection;
    // fn init();
    fn now() -> Timestamp;
    fn pend_priority(prio: u8);

    fn schedule<const S: usize, const Q: usize>(
        task_stack: &TaskStack<S>,
        queue: &TaskQueue<Q>,
        min_dl: &MinDeadline,
        task: Task,
    ) {
        // TODO
        // self.check_init();

        let cs = Self::CS::enter();
        let now = Self::now();

        let task = task.into_queued(now);
        let (stack, dl_ref) = unsafe { (&mut *task_stack.get_mut(&cs), *min_dl.get_mut(&cs)) };

        if task.abs_deadline() < dl_ref || stack.is_empty() {
            Self::execute(cs, task_stack, min_dl, task);
        } else {
            {
                let queue = unsafe { &mut *queue.get_mut(&cs) };

                queue.push(task).unwrap();
            }
        }
    }

    fn execute<const S: usize>(
        cs: Self::CS,
        task_stack: &TaskStack<S>,
        min_dl: &MinDeadline,
        task: ScheduledTask,
    ) {
        let dl_ref = unsafe { &mut *min_dl.get_mut(&cs) };
        let prev_dl = *dl_ref;
        *dl_ref = task.abs_deadline();

        let stack = unsafe { &mut *task_stack.get_mut(&cs) };

        stack
            .push(crate::task::RunningTask::from_scheduled(task, prev_dl))
            .unwrap();
        let max_prio = stack.len() as u8;

        Self::pend_priority(max_prio);
    }
}

// pub struct Scheduler<CS>
// where
//     CS: CsImpl,
// {
//     ready: AtomicBool,
//     _cs: PhantomData<CS>,
// }

// impl<CS> Scheduler<CS>
// where
//     CS: CsImpl,
// {
//     pub const fn new() -> Self {
//         Self {
//             ready: AtomicBool::new(false),
//             _cs: PhantomData,
//         }
//     }

//     pub fn check_init(&self) {
//         if !self.ready.load(Ordering::SeqCst) {
//             panic!("Scheduler not initialized");
//         }
//     }

//     // TODO: should be in codegen
//     pub fn init(&self, nvic: &mut NVIC, scb: &mut SCB) {
//         let cs = CS::take();
//         // interrupt::disable();

//         for (level, interrupt) in DISPATCHERS.iter().enumerate() {
//             // TODO remove this "8" magic number somehow, which is the number of priorities
//             // available on the ATSAMD51J
//             let nvic_prio = (8 - (level as u8 + 1)) << 4;

//             unsafe {
//                 NVIC::unpend(*interrupt);
//                 NVIC::unmask(*interrupt);
//                 nvic.set_priority(*interrupt, nvic_prio);
//             }
//         }

//         self.ready.swap(true, Ordering::SeqCst);
//         // interrupt::enable();
//     }
// }
