use crate::edf_pass::parse::EdfTask;

use super::parse::App;

use heck::ToSnakeCase;
use proc_macro2::{Span, TokenStream};
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
        self.app.dispatcher_priorities();

        let mut tasks: Vec<_> = self
            .app
            .tasks
            .iter_mut()
            .map(|task| {
                let task_attribute = &task.params;
                let task_struct = &mut task.task_struct;
                // Remove the older task attribute and replace with the updated one, which
                // replaces the deadline parameter with an automatically generated priority
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

        let ret = parse_quote! {
            #mod_visibility mod #mod_ident {

                #scheduler_impl

                #(#other_code)*
                #(#tasks)*

            }
        };
        ret
    }

    fn generate_scheduler_impl(&self) -> TokenStream {
        let dispatchers: Vec<_> = self
            .app
            .tasks
            .iter()
            .enumerate()
            .inspect(|(i, task)| {
                assert_eq!(
                    *i, task.dispatcher_idx as usize,
                    "RTIC codegen bug: Tasks vector is not sequentially sorted."
                );
            })
            .map(|(_, task)| task.dispatcher.get_ident())
            .collect();

        let run_queue_len = self.app.dispatcher_priorities().len();
        let wait_queue_len = self.app.wait_queue_len();
        let num_dispatchers = dispatchers.len();

        let pac_path = &self.app.app_parameters.pac_path;

        parse_quote! {
            const EDF_WAIT_QUEUE_LEN: usize = #wait_queue_len;
            const EDF_RUN_QUEUE_LEN: usize = #run_queue_len;
            const NUM_EDF_DISPATCHERS: usize = #num_dispatchers;

            // TODO: cortex-m leaking here?
            const EDF_DISPATCHERS: [#pac_path::Interrupt; NUM_EDF_DISPATCHERS] = [
                #(#pac_path::Interrupt::#dispatchers,)*
            ];

            use ::rtic_edf_pass::scheduler::Scheduler;
            pub struct NvicScheduler {
               running_queue: ::rtic_edf_pass::scheduler::RunQueue<EDF_RUN_QUEUE_LEN>,
                min_deadline: ::rtic_edf_pass::scheduler::SystemDeadline,
                task_queue: ::rtic_edf_pass::scheduler::WaitQueue<EDF_WAIT_QUEUE_LEN>,
            }

            impl NvicScheduler {
                #[inline]
                pub const fn new() -> Self {
                    Self {
                       running_queue: ::rtic_edf_pass::scheduler::RunQueue::new(),
                        min_deadline: ::rtic_edf_pass::scheduler::SystemDeadline::new(),
                        task_queue: ::rtic_edf_pass::scheduler::WaitQueue::new(),
                    }
                }
            }

            // TODO: cortex-m is leaking here
            impl ::rtic_edf_pass::scheduler::Scheduler<EDF_RUN_QUEUE_LEN, EDF_WAIT_QUEUE_LEN> for NvicScheduler {
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
        }
    }

    fn generate_task_signal_bindings(&self) -> Vec<TokenStream> {
        self.app
            .tasks
            .iter()
            .map(|t| t.generate_timestamper_binding(self.app.timestamper_priority))
            .collect()
    }

    fn generate_dispatcher_bindings(&self) -> Vec<TokenStream> {
        let mut tokens = vec![];

        for task in self.app.tasks.iter() {
            let dispatcher_prio = task.dispatcher_priority;
            let dispatcher_binding = &task.dispatcher;
            let rq_idx = task.rq_idx;
            let task_ident = &task.task_struct.ident;

            let static_ident = syn::Ident::new(
                &task
                    .task_struct
                    .ident
                    .to_string()
                    .to_snake_case()
                    .to_uppercase(),
                Span::call_site(),
            );

            let dispatcher_ident = format_ident!("__edf_scheduler_dispatch_{task_ident}");
            tokens.push(parse_quote! {

                #[task(priority = #dispatcher_prio, binds = #dispatcher_binding)]
                #[allow(non_camel_case_types)]
                struct #dispatcher_ident {}

                impl RticTask for #dispatcher_ident {
                    fn init() -> Self {
                        Self {}
                    }

                    fn exec(&mut self) {
                        const RUN_QUEUE_IDX: u16 = #rq_idx;

                        let task_to_run =  unsafe { #static_ident.assume_init_mut() };
                        let deadline_to_restore = SCHEDULER.dispatcher_entry(RUN_QUEUE_IDX);
                        task_to_run.exec();
                        SCHEDULER.dispatcher_exit::<#task_ident>(deadline_to_restore);
                    }
                }
            })
        }

        tokens
    }
}

impl EdfTask {
    pub fn generate_timestamper_binding(&self, priority: u16) -> TokenStream {
        let binds = &self.timestamper_binding;
        let task_struct_ident = &self.task_struct.ident;

        let dispatcher_idx = self.dispatcher_idx;
        let rq_idx = self.rq_idx;
        let sched_task_ident = format_ident!("__edf_scheduler_signal_{task_struct_ident}");
        let deadline_us = self.deadline_us;

        parse_quote! {
            #[task(priority = #priority, binds = #binds)]
            #[allow(non_camel_case_types)]
            pub struct #sched_task_ident {}

            impl RticTask for #sched_task_ident {
                fn init() -> Self {
                    Self {}
                }

                fn exec(&mut self) {
                    use ::rtic_edf_pass::task::EdfTaskBinding;

                    #task_struct_ident::mask_timestamper_interrupt();
                    SCHEDULER.schedule(
                        ::rtic_edf_pass::task::Task::new(
                            #deadline_us,
                            <#task_struct_ident as ::rtic_edf_pass::task::EdfTaskBinding>::DISPATCHER_IDX,
                            <#task_struct_ident as ::rtic_edf_pass::task::EdfTaskBinding>::RUN_QUEUE_IDX,
                        ),
                    );

                }
            }

            // TODO: cortex-m is leaking here
            impl ::rtic_edf_pass::task::EdfTaskBinding for #task_struct_ident {
                const DISPATCHER_IDX: u16 = #dispatcher_idx;
                const RUN_QUEUE_IDX: u16 = #rq_idx;

                #[inline]
                unsafe fn unmask_timestamper_interrupt() {
                    // TODO this is sort of sketchy, we should somehow get the right path to the interrupt enum variant
                    unsafe { ::cortex_m::peripheral::NVIC::unmask(Interrupt::#binds); }
                }

                #[inline]
                fn unpend_timestamper_interrupt() {
                    // TODO this is sort of sketchy, we should somehow get the right path to the interrupt enum variant
                    ::cortex_m::peripheral::NVIC::unpend(Interrupt::#binds);
                }

                #[inline]
                 fn mask_timestamper_interrupt() {
                    // TODO this is sort of sketchy, we should somehow get the right path to the interrupt enum variant
                    ::cortex_m::peripheral::NVIC::mask(Interrupt::#binds);
                }
           }
        }
    }
}
