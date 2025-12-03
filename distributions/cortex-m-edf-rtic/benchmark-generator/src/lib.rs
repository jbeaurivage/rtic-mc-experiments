use std::{
    collections::{HashMap, HashSet},
    env, fs, iter,
    path::PathBuf,
};

use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::format_ident;
use syn::{LitInt, parse::Parser, parse_quote};

use crate::interrupt_sources::INTERRUPT_SOURCES;

mod interrupt_sources;

#[proc_macro]
pub fn generate_benchmark_app(input: TokenStream) -> TokenStream {
    // Parse the input as a comma-separated list of integer literals
    let data = syn::punctuated::Punctuated::<LitInt, syn::Token![,]>::parse_terminated
        .parse(input)
        .unwrap();

    // Convert the integer literals to a vector of u32 values
    let values: Vec<u32> = data
        .iter()
        .map(|lit| {
            lit.base10_parse::<u32>()
                .expect("Expected a valid u32 integer")
        })
        .collect();

    assert!(
        values.len() == 2,
        "Macro takes exactly 2 arguments: the number of tasks per priority, and the number of cycles to wait in each task"
    );

    let mut handlers: HashSet<_> = INTERRUPT_SOURCES
        .entries()
        .map(|(i, _)| format_ident!("{i}"))
        .collect();

    let seed_timestamper = handlers.take(&format_ident!("TC4")).unwrap();
    // let seed_task = generate_seed_task(&seed_timestamper, 4_000_000);

    let seed_dispatcher = handlers.take(&format_ident!("AC")).unwrap();

    let generated_code = generate_app(seed_timestamper, seed_dispatcher, handlers, values[1]);

    // Write the generated code to a file in the `OUT_DIR` directory
    let target_dir = find_target_dir().expect("Failed to locate the `target` directory");
    let out_path = target_dir.join("generated_benchmark.rs");
    eprintln!("Generated benchmark written to {}", out_path.display());
    fs::write(&out_path, generated_code.to_string())
        .expect("Failed to write generated code to file");

    // Return the generated code as the macro output
    generated_code.into()
}

struct Task {
    deadline: u32,
    timestamper: Ident,
    dispatcher: Ident,
    next_timestamper: Ident,
    idx: usize,
    delay: u32,
}

impl Task {
    fn generate(&self) -> TokenStream2 {
        let is_seed_task = self.idx == 0;
        let deadline = &self.deadline;
        let _to_pend = &self.next_timestamper;
        let delay = self.delay;

        let timestamper_ident = format_ident!("{}", self.timestamper);
        let task_ident = if is_seed_task {
            format_ident!("SeedTask")
        } else {
            format_ident!("Task{}", self.idx)
        };

        let seed_ops: TokenStream2 = if is_seed_task {
            parse_quote! {
                let tc4 = unsafe { Peripherals::steal().tc4 };
                    tc4.count16().intflag().write(|w| w.ovf().set_bit());
            }
        } else {
            parse_quote! {}
        };

        parse_quote! {
            #[task(deadline_us = #deadline, binds = #timestamper_ident)]
            pub struct #task_ident {}

            impl RticTask for #task_ident {
                fn init() -> Self {
                    Self {}
                }

                fn exec(&mut self) {
                    #seed_ops
                    // cortex_m::peripheral::NVIC::pend(crate::app::Interrupt::#_to_pend);
                    cortex_m::asm::delay(#delay);
                }
            }
        }
    }
}

fn generate_app(
    seed_timestamper: Ident,
    seed_dispatcher: Ident,
    handlers: HashSet<Ident>,
    task_delay: u32,
) -> TokenStream2 {
    const UNIQUE_DEADLINES: usize = 6;
    // TODO: dynamic
    const DEADLINES: [u32; UNIQUE_DEADLINES] = [
        120_000_000,
        60_000_000,
        30_000_000,
        15_000_000,
        7_500_000,
        4_000_000,
    ];

    let mut dispatchers = vec![];
    let mut task_tokens = vec![];
    let mut pend_chain = vec![];

    let seed = iter::once((seed_timestamper, seed_dispatcher));
    let handlers: HashSet<(_, _)> = handlers.into_iter().tuples().collect();

    let tasks = seed
        .chain(handlers)
        .enumerate()
        .tuple_windows()
        .map(
            |((i, (timestamper, dispatcher)), (_, (next_timestamper, _)))| Task {
                deadline: DEADLINES[i / (UNIQUE_DEADLINES + 4)],
                timestamper,
                dispatcher,
                next_timestamper,
                idx: i,
                delay: task_delay,
            },
        )
        .collect::<Vec<_>>();

    for task in tasks {
        let timestamper = &task.timestamper;
        eprintln!(
            "\t{} => ts: {}\t\tdispatcher: {} =>\t\tnext timestamper {}",
            task.deadline, task.timestamper, task.dispatcher, task.next_timestamper
        );

        dispatchers.push(task.dispatcher.clone());
        let task_stream = task.generate();
        task_tokens.push(task_stream);

        let pend_timestamper: TokenStream2 = parse_quote! {
            ::cortex_m::peripheral::NVIC::pend(crate::app::Interrupt::#timestamper);
        };

        pend_chain.push(pend_timestamper);
    }

    parse_quote! {

        #[cortex_m_edf_rtic::app(
            device = atsamd_hal::pac,
            dispatchers = [#(#dispatchers,)*],
            cpu_freq = 120_000_000,
        )]
        mod app {
            use atsamd_hal::{
                clock::GenericClockController,
                fugit::ExtU32,
                pac::{CorePeripherals, Interrupt, NVIC, Peripherals},
                prelude::InterruptDrivenTimer,
                timer::TimerCounter,
            };

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

                let timer_clock = clocks.gclk0();
                let tc45 = &clocks.tc4_tc5(&timer_clock).unwrap();

                // cortex_m::asm::delay(1_000_000);
                defmt::warn!("begin");

                #(#pend_chain)*


                // Instantiate a timer object for the TC4 timer/counter
                let mut timer = TimerCounter::tc4_(tc45, peripherals.tc4, &mut peripherals.mclk);
                timer.start(500.millis());
                // timer.enable_interrupt();

                Shared { x: 0 }
            }

            #[idle]
            pub struct IdleTask {
                _count: u32,
            }
            impl RticIdleTask for IdleTask {
                fn init() -> Self {
                    Self { _count: 0 }
                }

                fn exec(&mut self) -> ! {
                    loop {
                        core::hint::spin_loop();
                    }
                }
            }

            #(#task_tokens)*

        }
    }
}

/// Traverses the parent directories of `OUT_DIR` until it finds the `target` directory.
fn find_target_dir() -> Option<PathBuf> {
    let mut path = PathBuf::from(env::var("OUT_DIR").ok()?);

    while path.file_name().is_some_and(|name| name != "target") {
        path.pop(); // Move up one directory
    }

    if path.file_name().is_some_and(|name| name == "target") {
        Some(path)
    } else {
        None
    }
}
