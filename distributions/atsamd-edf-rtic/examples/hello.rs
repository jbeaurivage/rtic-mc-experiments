//! examples/hello_world.rs

#![deny(unsafe_code)]
// #![deny(warnings)]
#![no_main]
#![no_std]

// #[rtic::app(device=rp2040_hal::pac, peripherals=false, dispatchers=[DMA_IRQ_0])]

#[cortex_m_edf_rtic::app(device = atsamd_hal::pac, dispatchers= [SERCOM0_0, SERCOM0_1])]
mod app {

    #[shared]
    struct Shared {}

    #[init]
    fn system_init() -> Shared {
        Shared {}
    }

    #[idle]
    pub struct MyIdleTask {
        count: u32,
    }
    impl RticIdleTask for MyIdleTask {
        fn init() -> Self {
            Self { count: 0 }
        }

        fn exec(&mut self) -> ! {
            loop {}
        }
    }
}

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}
