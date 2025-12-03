//! examples/hello_world.rs

// #![deny(unsafe_code)]
#![deny(warnings)]
#![no_main]
#![no_std]

use defmt_rtt as _;
use panic_probe as _;

use atsamd_hal::{
    clock::GenericClockController,
    fugit::ExtU32,
    pac::{CorePeripherals, Interrupt, NVIC, Peripherals},
    prelude::InterruptDrivenTimer,
    timer::TimerCounter,
};

#[cortex_m_edf_rtic::app(
    device = atsamd_hal::pac,
    dispatchers = [SERCOM0_0, SERCOM0_1, SERCOM0_2],
    cpu_freq = 120_000_000,
)]
mod app {
    use benchmark_generator::generate_benchmark;

    use super::*;

    #[shared]
    struct Shared {
        x: u32,
    }

    #[init]
    fn system_init() -> Shared {
        let mut peripherals = Peripherals::take().unwrap();
        let mut core = CorePeripherals::take().unwrap();

        let mut clocks = GenericClockController::with_external_32kosc(
            peripherals.gclk,
            &mut peripherals.mclk,
            &mut peripherals.osc32kctrl,
            &mut peripherals.oscctrl,
            &mut peripherals.nvmctrl,
        );

        // TODO: is systick really needed in this example?
        core.SYST.set_reload(8_000_000 - 1);
        core.SYST.clear_current();
        // core.SYST.enable_interrupt();
        core.SYST.enable_counter();

        let timer_clock = clocks.gclk0();
        let tc45 = &clocks.tc4_tc5(&timer_clock).unwrap();

        // Instantiate a timer object for the TC4 timer/counter
        let mut timer = TimerCounter::tc4_(tc45, peripherals.tc4, &mut peripherals.mclk);
        timer.start(500.millis());
        timer.enable_interrupt();

        // Instantiate a timer object for the TC5 timer/counter
        let mut timer = TimerCounter::tc5_(tc45, peripherals.tc5, &mut peripherals.mclk);
        timer.start(100.millis());
        timer.enable_interrupt();

        Shared { x: 0 }
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
            // Manually pend a manual task for fun
            NVIC::pend(Interrupt::SERCOM1_1);
            loop {
                core::hint::spin_loop();
                // WFI would give inaccurate cycle counts when benchmarking
                // defmt::trace!("Idle");
                // cortex_m::asm::wfi();
            }
        }
    }

    // TODO: currently the deadline is counted in cycles.
    // We need to retrieve the clock frequency if we want a chance at specifying
    // them in us instead
    #[task(deadline_us = 1_000_000, binds = SERCOM1_1, shared = [x])]
    pub struct ManualTask {}

    impl RticTask for ManualTask {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            let mut a = 0;
            self.shared().x.lock(|x| {
                *x += 1;
                a = *x;
            });

            defmt::info!("Manual task: x = {}", a);
        }
    }

    // TODO: currently the deadline is counted in cycles.
    // We need to retrieve the clock frequency if we want a chance at specifying
    // them in us instead
    #[task(deadline_us = 4_000_000, binds = TC5, shared = [x])]
    pub struct ShortTimerTask {}

    impl RticTask for ShortTimerTask {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            let tc5 = unsafe { Peripherals::steal().tc5 };
            tc5.count16().intflag().write(|w| w.ovf().set_bit());

            let mut a = 0;
            self.shared().x.lock(|x| {
                *x += 1;
                a = *x;
            });

            cortex_m::asm::delay(1_000_000);
            defmt::info!("Short Timer task x = {}", a);
        }
    }

    // TODO: currently the deadline is counted in cycles.
    // We need to retrieve the clock frequency if we want a chance at specifying
    // them in us instead
    #[task(deadline_us = 8_000_000, binds = TC4, shared = [x])]
    pub struct LongTimerTask {}

    impl RticTask for LongTimerTask {
        fn init() -> Self {
            Self {}
        }

        fn exec(&mut self) {
            let tc4 = unsafe { Peripherals::steal().tc4 };
            tc4.count16().intflag().write(|w| w.ovf().set_bit());

            let mut a = 0;
            self.shared().x.lock(|x| {
                *x += 1;
                a = *x;
            });

            cortex_m::asm::delay(4_000_000);
            defmt::warn!("Long Timer task x = {}", a);
        }
    }
}
