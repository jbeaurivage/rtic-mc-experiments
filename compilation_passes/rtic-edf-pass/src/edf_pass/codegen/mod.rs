use super::parse::App;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemMod, parse_quote};

pub struct CodeGen {
    app: App,
}

impl CodeGen {
    pub fn new(app: App) -> CodeGen {
        Self { app }
    }

    pub fn run(&mut self) -> ItemMod {
        let mut tasks: Vec<_> = self
            .app
            .tasks
            .iter_mut()
            .map(|task| {
                let task_attribute = &task.params;
                let task_struct = &mut task.task_struct;
                // remove the older task attribute and replace with the updated one which includes an
                // automatically assigned core
                task_struct.attrs.remove(task.attr_idx);
                quote! {
                    #task_attribute
                    #task_struct
                }
            })
            .collect();

        let mod_visibility = &self.app.mod_visibility;
        let mod_ident = &self.app.mod_ident;
        let other_code = &self.app.rest_of_code;

        let scheduler_impl = self.generate_scheduler_impl();

        let scheduler_signal_bindings = self.generate_task_signal_bindings();
        tasks.extend(scheduler_signal_bindings);

        let scheduler_dispatcher_bindings = self.generate_dispatcher_bindings();
        tasks.extend(scheduler_dispatcher_bindings);

        // let shared_resources = self.app.shared_resources.iter().map(|s| &s.shared_struct);
        let ret = parse_quote! {
            #mod_visibility mod #mod_ident {

                #scheduler_impl

                #(#other_code)*
                // #(#shared_resources)*
                #(#tasks)*

            }
        };
        ret
    }

    fn generate_scheduler_impl(&self) -> TokenStream {
        let dispatchers = self
            .app
            .app_parameters
            .dispatchers
            .iter()
            .map(|d| d.get_ident())
            .collect::<Vec<_>>();

        let num_dispatchers = dispatchers.len();
        let queue_len = self.app.app_parameters.queue_len;
        let pac_path = &self.app.app_parameters.pac_path;

        parse_quote! {
            const EDF_QUEUE_LEN: usize = #queue_len;
            const NUM_EDF_DISPATCHERS: usize = #num_dispatchers;
            const EDF_DISPATCHERS: [#pac_path::Interrupt; NUM_EDF_DISPATCHERS] = [
                #(#pac_path::Interrupt::#dispatchers,)*
            ];

            use ::rtic_edf_pass::scheduler::Scheduler;
            pub struct NvicScheduler {
                running_stack: ::rtic_edf_pass::scheduler::TaskStack<NUM_EDF_DISPATCHERS>,
                min_deadline: ::rtic_edf_pass::scheduler::MinDeadline,
                task_queue: ::rtic_edf_pass::scheduler::TaskQueue<EDF_QUEUE_LEN>,
            }

            impl NvicScheduler {
                pub const fn new() -> Self {
                    Self {
                        running_stack: ::rtic_edf_pass::scheduler::TaskStack::new(),
                        min_deadline: ::rtic_edf_pass::scheduler::MinDeadline::new(),
                        task_queue: ::rtic_edf_pass::scheduler::TaskQueue::new(),
                    }
                }
            }

            // TODO: cortex-m is leaking here
            impl ::rtic_edf_pass::scheduler::Scheduler<NUM_EDF_DISPATCHERS, EDF_QUEUE_LEN> for NvicScheduler {
                type CS = ::cortex_m_edf_rtic::export::CsGuard;

                #[inline]
                fn now() -> ::rtic_edf_pass::util::Timestamp {
                    ::cortex_m::peripheral::DWT::cycle_count()
                }

                #[inline]
                fn running_stack(&self) -> &::rtic_edf_pass::scheduler::TaskStack<NUM_EDF_DISPATCHERS> {
                    &self.running_stack
                }

                #[inline]
                fn min_deadline(&self) -> &::rtic_edf_pass::scheduler::MinDeadline {
                    &self.min_deadline
                }

                #[inline]
                fn task_queue(&self) -> &::rtic_edf_pass::scheduler::TaskQueue<EDF_QUEUE_LEN> {
                    &self.task_queue
                }

                #[inline]
                fn pend_priority(prio: u16) {
                    ::cortex_m::peripheral::NVIC::pend(EDF_DISPATCHERS[prio as usize]);
                }
            }

            static SCHEDULER: NvicScheduler = NvicScheduler::new();
        }
    }

    fn generate_task_signal_bindings(&self) -> Vec<TokenStream> {
        let scheduler_priority = self.app.scheduler_priority();

        self.app
            .tasks
            .iter()
            .map(|t| t.generate_signal_binding(scheduler_priority))
            .collect()
    }

    fn generate_dispatcher_bindings(&self) -> Vec<TokenStream> {
        let dispatchers = self
            .app
            .app_parameters
            .dispatchers
            .iter()
            .enumerate()
            .map(|(p, d)| (p, d.get_ident()))
            .collect::<Vec<_>>();

        let mut tokens = vec![];

        for d in dispatchers {
            let prio = d.0 + 1;
            let binding = d.1;
            let dispatcher_ident = format_ident!("EdfPrio{prio}Dispatcher");
            tokens.push(parse_quote! {

                #[task(priority = #prio, binds = #binding)]
                struct #dispatcher_ident {}

                impl RticTask for #dispatcher_ident {
                    fn init() -> Self {
                        Self {}
                    }

                    fn exec(&mut self) {
                        SCHEDULER.trampoline();
                    }
                }
            })
        }

        tokens
    }
}

// For every EDF task, we need to:
//
// * Assign it to some dispatcher trampoline (can this be a software task?)
// * Generate the signalling IRQ (ie, calls the scheduler op)
