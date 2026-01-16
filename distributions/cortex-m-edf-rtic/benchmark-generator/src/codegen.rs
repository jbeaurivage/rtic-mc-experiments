use proc_macro2::TokenStream as TokenStream2;
use syn::parse_quote;

use crate::{Handler, Settings, handlers::Task};

pub(crate) fn generate_app(handlers: Vec<Handler>, app_settings: Settings) -> TokenStream2 {
    let mut task_tokens = vec![];
    let mut pend_chain = vec![];

    let num_deadlines = app_settings.deadline_timings.len();

    let (task_handlers, dispatcher_handlers) = handlers.split_at(handlers.len() / 2);

    let mut deadline_idx = 0;
    let mut deadline_count = 0;
    let max_deadline_count =
        (app_settings.tasks_per_priority as usize).min(task_handlers.len() / num_deadlines);

    let mut tasks_dl = Vec::with_capacity(task_handlers.len());

    // Equally distribute tasks among available deadlines
    for (i, handler) in task_handlers.iter().enumerate() {
        let deadline = app_settings.deadline_timings[deadline_idx];
        deadline_count += 1;

        tasks_dl.push(Task {
            deadline,
            handler: handler.clone(),
            idx: i,
            num_delay_cycles: app_settings.task_delay,
        });

        if deadline_count >= max_deadline_count && deadline_idx < num_deadlines - 1 {
            deadline_idx += 1;
            deadline_count = 0;
        } else if deadline_count >= max_deadline_count && deadline_idx >= num_deadlines - 1 {
            break;
        }
    }

    // Count the number of tasks per unique deadline for logging
    let mut deadline_counts = std::collections::HashMap::new();
    for task in &tasks_dl {
        *deadline_counts.entry(task.deadline).or_insert(0) += 1;
    }

    let mut deadline_counts = deadline_counts.into_iter().collect::<Vec<_>>();
    deadline_counts.sort();

    for (deadline, count) in &deadline_counts {
        eprintln!("{} tasks with deadline: {} ", count, deadline);
    }

    for (_i, task) in tasks_dl.iter().enumerate() {
        let timestamper = &task.handler.ident;

        // dispatchers.push(task.dispatcher.ident.clone());
        let task_stream = task.generate();
        task_tokens.push(task_stream);

        let pend_timestamper: TokenStream2 = parse_quote! {
            ::cortex_m::peripheral::NVIC::pend(crate::app::Interrupt::#timestamper);
        };

        pend_chain.push(pend_timestamper);
    }

    let dispatcher_handlers = dispatcher_handlers.iter().map(|h| {
        let ident = &h.ident;
        quote::quote!(#ident)
    });

    parse_quote! {
        use defmt_rtt as _;
        use panic_probe as _;

        #[cortex_m_edf_rtic::app(
            device = atsamd_hal::pac,
            dispatchers = [#(#dispatcher_handlers,)*],
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

                // Start CPU clock at 120 MHz
                let mut clocks = GenericClockController::with_external_32kosc(
                    peripherals.gclk,
                    &mut peripherals.mclk,
                    &mut peripherals.osc32kctrl,
                    &mut peripherals.oscctrl,
                    &mut peripherals.nvmctrl,
                );

                // cortex_m::asm::delay(1_000_000);
                defmt::info!("Beginning test...");

                #(#pend_chain)*


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
                    defmt::info!("Test completed!");
                    loop {
                        ::cortex_m::asm::wfi();
                    }
                }
            }

            #(#task_tokens)*

        }
    }
}
