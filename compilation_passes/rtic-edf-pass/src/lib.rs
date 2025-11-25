// Enable the `no_std` attribute if `no_std` is enabled
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
pub mod edf_pass;

#[cfg(feature = "std")]
pub use edf_pass::*;

pub use heapless::{binary_heap::Min, BinaryHeap, Vec};

use crate::task::RunningTask;

pub mod task;

pub mod util;

pub mod critical_section;
pub trait InternalTaskStack<T: critical_section::CsImpl, const N: usize> {
    fn get_mut(&self, _cs: &T) -> *mut heapless::Vec<RunningTask, N, u8>;
}

pub mod scheduler;
/*
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use heapless::binary_heap::Min;
use heapless::{BinaryHeap, Vec};

pub type Timestamp = u32;
pub mod critical_section;
use crate::dispatchers::{dispatcher, dispatcher_irq, DISPATCHERS, NUM_DISPATCHERS};
use crate::task::{RunningTask, ScheduledTask, Task};
use crate::vector_table::set_handler;
pub use critical_section::*;

//struct TaskStack(UnsafeCell<Vec<RunningTask, NUM_DISPATCHERS, u8>>);

trait EdfPass {
    type DISPATCHER_ARR;
    const NUM_DISPATCHERS: u8;
}

unsafe impl Sync for TaskStack {}

impl TaskStack {
    fn get_mut(&self, _cs: &CsGuard) -> *mut Vec<RunningTask, NUM_DISPATCHERS, u8> {
        self.0.get()
    }
}

struct MinDeadline(UnsafeCell<Timestamp>);

unsafe impl Sync for MinDeadline {}

impl MinDeadline {
    fn get_mut(&self, _cs: &CsGuard) -> *mut Timestamp {
        self.0.get()
    }
}

// TODO get rid of this magic number
struct TaskQueue(UnsafeCell<BinaryHeap<ScheduledTask, Min, 16>>);

unsafe impl Sync for TaskQueue {}

impl TaskQueue {
    fn get_mut(&self, _cs: &CsGuard) -> *mut BinaryHeap<ScheduledTask, Min, 16> {
        self.0.get()
    }
}

static RUNNING_STACK: TaskStack = TaskStack(UnsafeCell::new(Vec::new()));
static MIN_DEADLINE: MinDeadline = MinDeadline(UnsafeCell::new(u32::MAX));
static PARKED_QUEUE: TaskQueue = TaskQueue(UnsafeCell::new(BinaryHeap::new()));

pub struct Scheduler {
    ready: AtomicBool,
}

unsafe impl Sync for Scheduler {}

impl core::default::Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Scheduler<T: CsImpl> {
    fn new() -> Self;
    fn check_init(&self);
    fn init(&self);
    fn schedule(&self, task: Task);
    fn execute(cs: T, task: ScheduledTask, now: Timestamp);
    fn idle();
}

/// Trampoline that takes care of launching the task, and restoring the
/// scheduler state after its execution completes.
extern "C" fn run_task<T: CsImpl>() {
    let (callback, prev_deadline) = unsafe {
        let cs = T::take();
        let task = (&*RUNNING_STACK.get_mut(&cs)).last().unwrap();
        (task.callback(), task.prev_deadline())
    };

    // Finally call the actual task
    callback();

    // And cleanup after ourselves
    let cs = T::take();
    let (stack, min_deadline) = unsafe {
        (
            &mut *RUNNING_STACK.get_mut(&cs),
            &mut *MIN_DEADLINE.get_mut(&cs),
        )
    };

    stack.pop().unwrap();
    // Restore previous deadline
    *min_deadline = prev_deadline;

    #[cfg(feature = "defmt")]
    defmt::debug!(
        "[COMPLETE TASK] new dl: {}, stack depth: {}",
        prev_deadline,
        stack.len(),
    );

    // It's possible that a task showed up in the queue as the previous task was
    // running. So we need to check if it would preempt the next task in line to
    // run, which would start as soon as the critical section exits.
    let queue = unsafe { &mut *PARKED_QUEUE.get_mut(&cs) };
    if let Some(task) = queue.peek()
        && (task.abs_deadline() < *min_deadline || stack.is_empty())
    {
        let task = unsafe { queue.pop_unchecked() };
        #[cfg(feature = "defmt")]
        defmt::debug!("[RESCHEDULE TASK]");
        Scheduler::execute(cs, task, now());
    }
}

fn now() -> u32 {
    todo!()
    //DWT::cycle_count()
}
*/
