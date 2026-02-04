#![no_main]
#![no_std]

use defmt_rtt as _;
use panic_probe as _;

benchmark_generator::generate_benchmark_app!(
    // Number of tasks per priority
    9,
    // Number of busy-delay cycles (Will be replaced with 1 if set to 0 here)
    1,
    // Deadline timings. Should be stritcly ordered and contain no duplicates.
    [
        60_000_000, 50_000_000, 40_000_000, 30_000_000, 20_000_000,
        10_000_000 // 10_000_000, 20_000_000, 30_000_000, 40_000_000, 50_000_000, 60_000_000
    ]
);
