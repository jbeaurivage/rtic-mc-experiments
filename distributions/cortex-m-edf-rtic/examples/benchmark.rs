#![no_main]
#![no_std]

use defmt_rtt as _;
use panic_probe as _;

benchmark_generator::generate_benchmark_app!(3, 140_000);
