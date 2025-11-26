//! examples/hello_world.rs

#![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

// #[rtic::app(device=rp2040_hal::pac, peripherals=false, dispatchers=[DMA_IRQ_0])]

#[cortex_m_edf_rtic::app(device = atsamd_hal::pac, dispatchers = [SERCOM0_0, SERCOM0_1], queue_len = 16)]
mod app {

    #[shared]
    struct Shared {}

    #[init]
    fn system_init() -> Shared {
        Shared {}
    }

    #[idle]
    pub struct MyIdleTask {
        _count: u32,
    }
    impl RticIdleTask for MyIdleTask {
        fn init() -> Self {
            Self { _count: 0 }
        }

        fn exec(&mut self) -> ! {
            loop {
                core::hint::spin_loop();
            }
        }
    }

    #[task(deadline_us = 32, binds = SERCOM1_1)]
    pub struct Task1 {}

    impl RticTask for Task1 {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            cortex_m::asm::nop();
        }
    }

    #[task(deadline_us = 64, binds = AC)]
    pub struct Task2 {}

    impl RticTask for Task2 {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            cortex_m::asm::nop();
        }
    }
}

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
