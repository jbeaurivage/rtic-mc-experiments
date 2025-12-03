use proc_macro::TokenStream;
use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use rtic_core::{AppArgs, CorePassBackend, RticMacroBuilder, SubAnalysis, SubApp};
use syn::{ItemFn, parse_quote};
extern crate proc_macro;
use rtic_edf_pass::EdfPass;
struct AtsamdEdfRtic;

// TODO: this should probably take into account the NVIC prio bits somehow?
const MIN_TASK_PRIORITY: u16 = 1;
const MAX_TASK_PRIORITY: u16 = 8;

#[proc_macro_attribute]
pub fn app(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut builder = RticMacroBuilder::new(AtsamdEdfRtic);
    let edf_pass = EdfPass::new(MIN_TASK_PRIORITY, MAX_TASK_PRIORITY);

    builder.bind_pre_core_pass(edf_pass);
    builder.build_rtic_macro(args, input)
}

// =========================================== Trait implementations ===================================================
impl CorePassBackend for AtsamdEdfRtic {
    fn default_task_priority(&self) -> u16 {
        1
    }
    fn post_init(
        &self,
        app_args: &AppArgs,
        sub_app: &SubApp,
        app_analysis: &SubAnalysis,
    ) -> Option<TokenStream2> {
        let peripheral_crate = &app_args.pacs[sub_app.core as usize];
        let initialize_dispatcher_interrupts =
            app_analysis.used_irqs.iter().map(|(irq_name, priority)| {
                quote! {
                    assert!(0 < #priority && #priority <= 1 << NVIC_PRIO_BITS, "priority level not supported");
                    //set interrupt priority
                    #peripheral_crate::CorePeripherals::steal()
                        .NVIC
                        .set_priority(
                            #peripheral_crate::Interrupt::#irq_name,
                            ::cortex_m_edf_rtic::export::cortex_logical2hw(#priority as u8, NVIC_PRIO_BITS)
                        );
                    //unmask interrupt
                    #peripheral_crate::NVIC::unmask(#peripheral_crate::Interrupt::#irq_name);
                }
            });

        let start_dwt_cycle_counter = quote! {
            let (mut dwt, dcb) =  {
                let core = cortex_m::peripheral::Peripherals::steal();
                (core.DWT, core.DCB)
            };

            cortex_m::peripheral::DWT::unlock();
            dcb.demcr.modify(|r| r | (1 << 24));
            dwt.set_cycle_count(0);
            dwt.enable_cycle_counter();
        };

        Some(quote! {
            unsafe {
                #start_dwt_cycle_counter
                #(#initialize_dispatcher_interrupts)*
            }

        })
    }
    fn populate_idle_loop(&self) -> Option<TokenStream2> {
        Some(quote! {
            unsafe { core::arch::asm!("wfi" ); }
        })
    }
    fn generate_interrupt_free_fn(&self, mut empty_body_fn: ItemFn) -> ItemFn {
        // eprintln!("{}", empty_body_fn.to_token_stream().to_string()); // enable comment to see the function signature

        let fn_body = parse_quote! {
            {

                unsafe { core::arch::asm!("cpsid i"); } // critical section begin
                core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
                unsafe { OLD_CS = CS };
                unsafe { CS = true };

                let r = f();

                if unsafe { !OLD_CS }  {
                    unsafe { OLD_CS = false };
                    core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
                    unsafe { core::arch::asm!("cpsie i"); } // critical section end
                }
                r
            }
        };
        empty_body_fn.block = Box::new(fn_body);
        empty_body_fn
    }
    fn generate_global_definitions(
        &self,
        app_args: &AppArgs,
        app_info: &SubApp,
        _app_analysis: &SubAnalysis,
    ) -> Option<TokenStream2> {
        let peripheral_crate = &app_args.pacs[app_info.core as usize];

        // define only once
        if app_info.core == 0 {
            Some(quote! {
                // globals to be used by custom interrupt free critical section
                static mut OLD_CS: bool = false;
                static mut CS: bool = false;
                use #peripheral_crate::NVIC_PRIO_BITS;
            })
        } else {
            None
        }
    }
    fn generate_resource_proxy_lock_impl(
        &self,
        _app_args: &AppArgs,
        _app_info: &SubApp,
        incomplete_lock_fn: syn::ImplItemFn,
    ) -> syn::ImplItemFn {
        let lock_impl: syn::Block = parse_quote! {
            {
                unsafe { ::cortex_m_edf_rtic::export::lock(resource_ptr, CEILING as u8, NVIC_PRIO_BITS, f); }
            }
        };

        let mut completed_lock_fn = incomplete_lock_fn;
        completed_lock_fn.block.stmts.extend(lock_impl.stmts);
        completed_lock_fn
    }

    fn entry_name(&self, _core: u32) -> Ident {
        // same entry name for both cores.
        // two main() functions will be generated but both will be guarded by #[cfg(core = "X")]
        // each generated binary will have have one entry
        format_ident!("main")
    }

    /// Customize how the task is dispatched when its bound interrupt is triggered (save baspri before and restore after executing the task)
    fn wrap_task_execution(
        &self,
        task_prio: u16,
        dispatch_task_call: TokenStream2,
    ) -> Option<TokenStream2> {
        Some(quote! {
            ::cortex_m_edf_rtic::export::run(#task_prio as u8, || {#dispatch_task_call});
        })
    }
    fn pre_codegen_validation(
        &self,
        _app: &rtic_core::App,
        _analysis: &rtic_core::Analysis,
    ) -> syn::Result<()> {
        Ok(())
    }
}
