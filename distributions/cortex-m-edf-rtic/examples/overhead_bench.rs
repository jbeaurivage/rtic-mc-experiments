#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};

use atsamd_hal::clock::GenericClockController;
use atsamd_hal::pac::{DWT, Interrupt, NVIC, Peripherals, interrupt};
use atsamd_hal::prelude::InterruptDrivenTimer;
use atsamd_hal::timer::TimerCounter;
use cortex_m::asm;
use cortex_m::peripheral::scb::SystemHandler;
use fugit::{Duration, ExtU32};
use {defmt_rtt as _, panic_probe as _};

use rtic_edf_pass::scheduler::{Scheduler, benchmark};
use rtic_edf_pass::task::Task;

type Deadline = Duration<u32, 1, 48_000_000>;

const EDF_WAIT_QUEUE_LEN: usize = 48usize;
const EDF_RUN_QUEUE_LEN: usize = 1usize;
const NUM_EDF_DISPATCHERS: usize = 49usize;
const EDF_DISPATCHERS: [atsamd_hal::pac::Interrupt; NUM_EDF_DISPATCHERS] = [
    atsamd_hal::pac::Interrupt::SERCOM4_0,
    atsamd_hal::pac::Interrupt::SERCOM4_1,
    atsamd_hal::pac::Interrupt::SERCOM4_2,
    atsamd_hal::pac::Interrupt::SERCOM4_OTHER,
    atsamd_hal::pac::Interrupt::SERCOM5_0,
    atsamd_hal::pac::Interrupt::SERCOM5_1,
    atsamd_hal::pac::Interrupt::SERCOM5_2,
    atsamd_hal::pac::Interrupt::SERCOM5_OTHER,
    atsamd_hal::pac::Interrupt::USB_OTHER,
    atsamd_hal::pac::Interrupt::USB_SOF_HSOF,
    atsamd_hal::pac::Interrupt::USB_TRCPT0,
    atsamd_hal::pac::Interrupt::USB_TRCPT1,
    atsamd_hal::pac::Interrupt::TCC0_OTHER,
    atsamd_hal::pac::Interrupt::TCC0_MC0,
    atsamd_hal::pac::Interrupt::TCC0_MC1,
    atsamd_hal::pac::Interrupt::TCC0_MC2,
    atsamd_hal::pac::Interrupt::TCC0_MC3,
    atsamd_hal::pac::Interrupt::TCC0_MC4,
    atsamd_hal::pac::Interrupt::TCC0_MC5,
    atsamd_hal::pac::Interrupt::TCC1_OTHER,
    atsamd_hal::pac::Interrupt::TCC1_MC0,
    atsamd_hal::pac::Interrupt::TCC1_MC1,
    atsamd_hal::pac::Interrupt::TCC1_MC2,
    atsamd_hal::pac::Interrupt::TCC1_MC3,
    atsamd_hal::pac::Interrupt::TCC2_OTHER,
    atsamd_hal::pac::Interrupt::TCC2_MC0,
    atsamd_hal::pac::Interrupt::TCC2_MC1,
    atsamd_hal::pac::Interrupt::TCC2_MC2,
    atsamd_hal::pac::Interrupt::TCC3_OTHER,
    atsamd_hal::pac::Interrupt::TCC3_MC0,
    atsamd_hal::pac::Interrupt::TCC3_MC1,
    atsamd_hal::pac::Interrupt::TCC4_OTHER,
    atsamd_hal::pac::Interrupt::TCC4_MC0,
    atsamd_hal::pac::Interrupt::TCC4_MC1,
    atsamd_hal::pac::Interrupt::TC0,
    atsamd_hal::pac::Interrupt::TC1,
    atsamd_hal::pac::Interrupt::TC2,
    atsamd_hal::pac::Interrupt::TC3,
    atsamd_hal::pac::Interrupt::TC4,
    atsamd_hal::pac::Interrupt::TC5,
    atsamd_hal::pac::Interrupt::PDEC_OTHER,
    atsamd_hal::pac::Interrupt::PDEC_MC0,
    atsamd_hal::pac::Interrupt::PDEC_MC1,
    atsamd_hal::pac::Interrupt::ADC0_OTHER,
    atsamd_hal::pac::Interrupt::ADC0_RESRDY,
    atsamd_hal::pac::Interrupt::ADC1_OTHER,
    atsamd_hal::pac::Interrupt::ADC1_RESRDY,
    atsamd_hal::pac::Interrupt::AC,
    atsamd_hal::pac::Interrupt::DAC_OTHER,
    //     atsamd_hal::pac::Interrupt::DAC_EMPTY_0,
    //     atsamd_hal::pac::Interrupt::DAC_EMPTY_1,
    //     atsamd_hal::pac::Interrupt::DAC_RESRDY_0,
    //     atsamd_hal::pac::Interrupt::DAC_RESRDY_1,
    //     atsamd_hal::pac::Interrupt::I2S,
];

pub struct NvicScheduler {
    running_queue: ::rtic_edf_pass::scheduler::RunQueue<EDF_RUN_QUEUE_LEN>,
    min_deadline: ::rtic_edf_pass::scheduler::SystemDeadline,
    task_queue: ::rtic_edf_pass::scheduler::WaitQueue<EDF_WAIT_QUEUE_LEN>,
}
impl NvicScheduler {
    #[inline]
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self {
            running_queue: ::rtic_edf_pass::scheduler::RunQueue::new(),
            min_deadline: ::rtic_edf_pass::scheduler::SystemDeadline::new(),
            task_queue: ::rtic_edf_pass::scheduler::WaitQueue::new(),
        }
    }
}
impl ::rtic_edf_pass::scheduler::Scheduler<EDF_RUN_QUEUE_LEN, EDF_WAIT_QUEUE_LEN>
    for NvicScheduler
{
    type CS = ::cortex_m_edf_rtic::export::CsGuard;
    #[inline]
    fn now() -> ::rtic_edf_pass::types::Timestamp {
        ::cortex_m::peripheral::DWT::cycle_count()
    }
    #[inline]
    fn run_queue(&self) -> &::rtic_edf_pass::scheduler::RunQueue<EDF_RUN_QUEUE_LEN> {
        &self.running_queue
    }
    #[inline]
    fn system_deadline(&self) -> &::rtic_edf_pass::scheduler::SystemDeadline {
        &self.min_deadline
    }
    #[inline]
    fn wait_queue(&self) -> &::rtic_edf_pass::scheduler::WaitQueue<EDF_WAIT_QUEUE_LEN> {
        &self.task_queue
    }
    #[inline]
    fn pend_dispatcher(idx: u16) {
        ::cortex_m::peripheral::NVIC::pend(EDF_DISPATCHERS[idx as usize]);
    }
}
static SCHEDULER: NvicScheduler = NvicScheduler::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let mut peripherals = atsamd_hal::pac::Peripherals::take().unwrap();
    let mut core = atsamd_hal::pac::CorePeripherals::take().unwrap();

    let _clocks = GenericClockController::with_external_32kosc(
        peripherals.gclk,
        &mut peripherals.mclk,
        &mut peripherals.osc32kctrl,
        &mut peripherals.oscctrl,
        &mut peripherals.nvmctrl,
    );

    // Initialize dispatchers
    for (level, interrupt) in EDF_DISPATCHERS.iter().enumerate() {
        // Override: all dispatchers get a logical priority of 4
        let level = 4;

        // TODO remove this "8" magic number somehow, which is the number of priorities
        // available on the ATSAMD51J
        let nvic_prio = (8 - (level as u8 + 1)) << 4;

        unsafe {
            NVIC::unpend(*interrupt);
            NVIC::unmask(*interrupt);
            core.NVIC.set_priority(*interrupt, nvic_prio);
        }
    }

    // Enable cycle counter, which acts as our system "timer"
    DWT::unlock();
    unsafe {
        core.DCB.demcr.modify(|r| r | (1 << 24));
    }
    reset_cyccnt();

    for i in 0..NUM_EDF_DISPATCHERS {
        schedule_task(500 + i as u32, i as u16);
    }
    // benchmark::print_trace();

    defmt::info!("[IDLE START]");
    unsafe { cortex_m::interrupt::enable() };
    loop {
        cortex_m::asm::wfi();
    }
}

// Task dispatcher
#[interrupt]
fn SERCOM4_0() {
    defmt::trace!("Dispatcher");
}

fn reset_cyccnt() {
    let mut dwt = unsafe { cortex_m::peripheral::Peripherals::steal() }.DWT;
    dwt.set_cycle_count(0);
    dwt.enable_cycle_counter();
}

fn schedule_task(deadline_ms: u32, dispatcher_idx: u16) {
    // Simulate a timestamper interrupt running at highest prio
    cortex_m::interrupt::free(|_| {
        benchmark::begin_trace();
        SCHEDULER.schedule(Task::new(
            Deadline::millis(deadline_ms).ticks(),
            dispatcher_idx,
            0,
        ));
    });
}
