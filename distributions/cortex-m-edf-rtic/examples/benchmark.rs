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
    [11_179, 14_277, 17_426, 20_292, 22_965, 25_034]
);

// -------------------------------- RESULTS -----------------------------------------
// cmd: DEFMT_LOG=info cargo r --release --example benchmark -F check-missed-deadlines,rtic-edf-pass/defmt
// profile: opt-level = "s", lto = "fat"
//
// Prio:              6           5           4           3           2           1
// Max queue len:     48          40          32          24          16          8
// Rel DL:          11_179      14_277      17_426      20_292      22_965      25_034
// Diff:                   3098        3152        2866        2673        2069
//
// ----------------------------------------------------------------------------------
