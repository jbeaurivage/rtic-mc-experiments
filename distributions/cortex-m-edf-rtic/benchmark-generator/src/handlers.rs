use proc_macro2::TokenStream as TokenStream2;
use quote::format_ident;
use syn::{Ident, parse_quote};

#[derive(Clone, Debug)]
pub(crate) struct Handler {
    pub vector_idx: u16,
    pub ident: Ident,
}

#[derive(Debug)]
pub(crate) struct Task {
    pub deadline: u32,
    pub handler: Handler,
    pub idx: usize,
    pub num_delay_cycles: usize,
}

impl Task {
    pub fn generate(&self) -> TokenStream2 {
        let deadline = &self.deadline;
        let delay_cycles = self.num_delay_cycles.max(1);

        let timestamper_ident = format_ident!("{}", self.handler.ident);
        let task_ident = format_ident!("Task{}", self.idx);

        let busy_work: Vec<TokenStream2> =
            std::iter::repeat_n(parse_quote! {cortex_m::asm::nop();}, delay_cycles).collect();

        parse_quote! {
            #[task(deadline_us = #deadline, binds = #timestamper_ident)]
            pub struct #task_ident {}

            impl RticTask for #task_ident {
                fn init() -> Self {
                    Self {}
                }

                fn exec(&mut self) {
                    // cortex_m::peripheral::NVIC::pend(crate::app::Interrupt::#_to_pend);
                    #(#busy_work)*
                }
            }
        }
    }
}
