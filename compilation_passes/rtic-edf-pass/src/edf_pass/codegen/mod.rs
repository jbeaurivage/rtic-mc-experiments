use super::parse::App;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemMod, parse_quote, token::Token};

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

        let trampoline = self.generate_trampoline();

        // let shared_resources = self.app.shared_resources.iter().map(|s| &s.shared_struct);
        let ret = parse_quote! {
            #mod_visibility mod #mod_ident {

                #scheduler_impl

                #(#other_code)*
                // #(#shared_resources)*
                #(#tasks)*

                #trampoline
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
            const NUM_DISPATCHERS: usize = #num_dispatchers;
                const DISPATCHERS: [#pac_path::Interrupt; NUM_DISPATCHERS] = [
                    #(#pac_path::Interrupt::#dispatchers,)*
                ];

                static RUNNING_STACK: ::rtic_edf_pass::scheduler::TaskStack<NUM_DISPATCHERS> =
                    ::rtic_edf_pass::scheduler::TaskStack::new();
                static MIN_DEADLINE: ::rtic_edf_pass::scheduler::MinDeadline =
                    ::rtic_edf_pass::scheduler::MinDeadline::new();
                const QUEUE_LEN: usize = #queue_len;
                static TASK_QUEUE: ::rtic_edf_pass::scheduler::TaskQueue<QUEUE_LEN> =
                    ::rtic_edf_pass::scheduler::TaskQueue::new();

                use ::rtic_edf_pass::scheduler::Scheduler;
                pub struct NvicScheduler;

                // TODO: cortex-m is leaking here
                impl NvicScheduler {
                    pub fn init(&self, nvic: &mut ::cortex_m::peripheral::NVIC) {
                        use ::rtic_edf_pass::critical_section::DroppableCriticalSection;

                        // TODO critical section necessary if this runs in RTIC's init?
                        let cs = ::cortex_m_edf_rtic::export::CsGuard::enter();

                        for (prio, interrupt) in DISPATCHERS.iter().enumerate() {
                            // TODO remove this "8" magic number somehow, which is the number of priorities
                            // available on the ATSAMD51J
                            let nvic_prio = (8 - (prio as u8 + 1)) << 4;

                            unsafe {
                                ::cortex_m::peripheral::NVIC::unpend(*interrupt);
                                ::cortex_m::peripheral::NVIC::unmask(*interrupt);
                                nvic.set_priority(*interrupt, nvic_prio);
                            }
                        }

                        // TODO necessary?
                        // interrupt::enable();
                    }
                }

                // TODO: cortex-m is leaking here
                impl Scheduler for NvicScheduler {
                    type CS = ::cortex_m_edf_rtic::export::CsGuard;
                    fn now() -> ::rtic_edf_pass::util::Timestamp {
                        ::cortex_m::peripheral::DWT::cycle_count()
                    }

                    fn pend_priority(prio: u8) {
                        ::cortex_m::peripheral::NVIC::pend(DISPATCHERS[prio as usize]);
                    }
                }
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

    fn generate_trampoline(&self) -> TokenStream {
        parse_quote! {
            /// Trampoline that takes care of launching the task, and restoring the
            /// scheduler state after its execution completes.
            #[inline]
            pub fn __scheduler_trampoline() {
                let (callback, prev_deadline) = unsafe {
                    // TODO: cortex-m is leaking here
                    let cs = ::cortex_m_edf_rtic::export::CsGuard::enter();
                    let task = (&*RUNNING_STACK.get_mut(&cs)).last().unwrap();
                    (task.callback(), task.prev_deadline())
                };

                // Finally call the actual task
                callback();

                // And cleanup after ourselves
                // TODO: cortex-m is leaking here
                let cs = ::cortex_m_edf_rtic::export::CsGuard::enter();
                let (stack, min_deadline) = unsafe {
                    (
                        &mut *RUNNING_STACK.get_mut(&cs),
                        &mut *MIN_DEADLINE.get_mut(&cs),
                    )
                };

                stack.pop().unwrap();
                // Restore previous deadline
                *min_deadline = prev_deadline;

                // It's possible that a task showed up in the queue as the previous task was
                // running. So we need to check if it would preempt the next task in line to
                // run, which would start as soon as the critical section exits.
                let queue = unsafe { &mut *TASK_QUEUE.get_mut(&cs) };
                if let Some(task) = queue.peek()
                    && (task.abs_deadline() < *min_deadline || stack.is_empty())
                {
                    let task = unsafe { queue.pop_unchecked() };
                    Scheduler::execute(cs, task, now());
                }
            }
        }
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
            let dispatcher_ident = format_ident!("__dispatch_prio_{prio}");
            tokens.push(parse_quote! {

                #[task(priority = #prio)]
                struct #dispatcher_ident {}

                impl RticTask for #dispatcher_ident {
                    fn init() -> Self {
                        Self {}
                    }

                    fn exec(&mut self) {
                        __scheduler_trampoline();
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
