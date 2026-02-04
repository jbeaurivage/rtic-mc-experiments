use core::sync::atomic::{AtomicU32, Ordering};

static CYCLES: AtomicU32 = AtomicU32::new(0);

#[inline(always)]
pub fn begin_trace() {
    let dwt = unsafe { cortex_m::peripheral::Peripherals::steal() }.DWT;
    CYCLES.store(dwt.cyccnt.read(), Ordering::SeqCst);
}

#[inline(always)]
pub fn print_trace() {
    let dwt = unsafe { cortex_m::peripheral::Peripherals::steal() }.DWT;
    let now = dwt.cyccnt.read();
    let prev = CYCLES.load(Ordering::SeqCst);
    let cycles = now - prev;
    defmt::debug!("[TIMING]: {} cycles", cycles);
}
