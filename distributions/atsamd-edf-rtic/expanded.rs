#![feature(prelude_import)]
//! examples/hello_world.rs
#![deny(unsafe_code)]
#![no_main]
#![no_std]
#[macro_use]
extern crate core;
#[prelude_import]
use core::prelude::rust_2024::*;
pub mod app {
    /// Include peripheral crate(s) that defines the vector table
    use atsamd_hal::pac as _;
    use atsamd_hal::pac::Interrupt;
    use rtic_edf_pass::Vec;
    use rtic_edf_pass::task::RunningTask;
    /// Module defining rtic traits
    pub use rtic_traits::*;
    pub mod rtic_traits {
        /// Trait for a hardware task
        pub trait RticTask {
            /// Associated type that can be used to make [Self::init] take arguments
            type InitArgs: Sized;
            /// Task local variables initialization routine
            fn init(args: Self::InitArgs) -> Self;
            /// Function to be bound to a HW Interrupt
            fn exec(&mut self);
        }
        /// Trait for an idle task
        pub trait RticIdleTask {
            /// Associated type that can be used to make [Self::init] take arguments
            type InitArgs: Sized;
            /// Task local variables initialization routine
            fn init(args: Self::InitArgs) -> Self;
            /// Function to be executing when no other task is running
            fn exec(&mut self) -> !;
        }
        pub trait RticMutex {
            type ResourceType;
            fn lock(&mut self, f: impl FnOnce(&mut Self::ResourceType));
        }
    }
    /// critical section function
    #[inline]
    pub fn __rtic_interrupt_free<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        unsafe {
            asm!("cpsid i");
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        unsafe { OLD_CS = CS };
        unsafe { CS = true };
        let r = f();
        if unsafe { !OLD_CS } {
            unsafe { OLD_CS = false };
            core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
            unsafe {
                asm!("cpsie i");
            }
        }
        r
    }
    const NUM_DISPATCHERS: usize = 2usize;
    const DISPATCHERS: [Interrupt; NUM_DISPATCHERS] = [
        Interrupt::SERCOM0_0,
        Interrupt::SERCOM0_1,
    ];
    struct TaskStack(core::cell::UnsafeCell<Vec<RunningTask, NUM_DISPATCHERS, u8>>);
    impl rtic_edf_pass::InternalTaskStack<
        cortex_m_edf_rtic::export::CsGuard,
        NUM_DISPATCHERS,
    > for TaskStack {
        fn get_mut(
            &self,
            _cs: &cortex_m_edf_rtic::export::CsGuard,
        ) -> *mut Vec<RunningTask, NUM_DISPATCHERS, u8> {
            self.0.get()
        }
    }
    /// # CORE 0
    static mut SHARED: core::mem::MaybeUninit<Shared> = core::mem::MaybeUninit::uninit();
    struct Shared {}
    fn system_init() -> Shared {
        Shared {}
    }
    static mut MY_IDLE_TASK: core::mem::MaybeUninit<MyIdleTask> = core::mem::MaybeUninit::uninit();
    pub struct MyIdleTask {
        count: u32,
    }
    const _: fn() = || {
        __rtic_trait_checks::implements_rtic_idle_task::<MyIdleTask>();
    };
    impl RticIdleTask for MyIdleTask {
        fn init(_: ()) -> Self {
            Self { count: 0 }
        }
        fn exec(&mut self) -> ! {
            loop {}
        }
        type InitArgs = ();
    }
    impl MyIdleTask {
        pub const fn priority() -> u16 {
            15u16
        }
    }
    impl MyIdleTask {
        pub const fn current_core() -> __rtic__internal__Core0 {
            unsafe { __rtic__internal__Core0::new() }
        }
    }
    ///Unique type for core 0
    pub use core0_type_mod::__rtic__internal__Core0;
    mod core0_type_mod {
        struct __rtic__internal__Core0Inner;
        pub struct __rtic__internal__Core0(__rtic__internal__Core0Inner);
        impl __rtic__internal__Core0 {
            pub const unsafe fn new() -> Self {
                __rtic__internal__Core0(__rtic__internal__Core0Inner)
            }
        }
    }
    static mut OLD_CS: bool = false;
    static mut CS: bool = false;
    use atsamd_hal::pac::NVIC_PRIO_BITS;
    /// Type representing tasks that need explicit user initialization
    /// Entry of
    /// # CORE 0
    #[no_mangle]
    fn main() -> ! {
        __rtic_interrupt_free(|| {
            let shared_resources = system_init();
            unsafe {
                SHARED.write(shared_resources);
            }
            unsafe {}
            unsafe {}
        });
        unsafe {
            MY_IDLE_TASK.write(MyIdleTask::init(()));
            MY_IDLE_TASK.assume_init_mut().exec();
        }
    }
    /// Utility functions used to enforce implementing appropriate task traits
    mod __rtic_trait_checks {
        use super::*;
        pub fn implements_rtic_idle_task<T: RticIdleTask>() {}
    }
}
use core::panic::PanicInfo;
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
