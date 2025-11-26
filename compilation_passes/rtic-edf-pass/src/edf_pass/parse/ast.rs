use proc_macro2::TokenStream;
use quote::format_ident;
use rtic_core::parse_utils::RticAttr;
use syn::{Expr, ItemStruct, Lit, Path, parse_quote};

use crate::util::Deadline;

pub struct AppParameters {
    pub dispatchers: Vec<Path>,
    pub pac_path: Path,
    pub queue_len: Option<usize>,
}

impl AppParameters {
    pub fn parse(args: &RticAttr) -> syn::Result<Self> {
        let mut dispatcher_vec = vec![];
        if let Some(Expr::Array(array)) = args.elements.get("dispatchers") {
            for e in array.elems.iter() {
                match e {
                    Expr::Path(p) => {
                        dispatcher_vec.push(p.path.clone());
                    }
                    _ => {
                        todo!()
                    }
                }
            }
        }

        let Some(Expr::Path(pac_path)) = args.elements.get("device") else {
            panic!("`device` must be a valid path to a PAC crate")
        };

        let queue_len = if let Some(Expr::Lit(syn::ExprLit {
            lit: Lit::Int(int), ..
        })) = args.elements.get("queue_len")
        {
            int.base10_parse().ok()
        } else {
            panic!("`queue_len` must be a integer literal");
        };

        Ok(Self {
            dispatchers: dispatcher_vec,
            pac_path: pac_path.path.clone(),
            queue_len,
        })
    }
}

#[derive(Debug)]
pub struct EdfTask {
    pub params: RticAttr,
    pub attr_idx: usize,
    pub task_struct: ItemStruct,
    /// A task's priority, which is initially expressed as an explicit deadline
    pub priority: Option<u32>,
    pub deadline_us: Deadline,
    /// Interrupt handler signalling task arrival
    pub binds: Path,
}

impl EdfTask {
    pub fn from_struct((task_struct, attr_idx): (ItemStruct, usize)) -> syn::Result<Self> {
        let mut params = RticAttr::parse_from_attr(&task_struct.attrs[attr_idx])?;

        let deadline_us = if let Some(Expr::Lit(syn::ExprLit {
            lit: Lit::Int(int), ..
        })) = params.elements.get("deadline_us")
        {
            // deadline explicitly assigned by the user
            int.base10_parse().ok()
        } else {
            None
        }
        .expect("EDF tasks must specify a deadline via the `deadline_us` attribute");

        if let Some(Expr::Lit(syn::ExprLit {
            lit: Lit::Int(_int),
            ..
        })) = params.elements.get("priority")
        {
            panic!(
                "'priority' found, please use 'deadlines' only or compile with --no-default-features."
            )
        }

        let Some(Expr::Path(binds)) = params.elements.remove("binds") else {
            panic!("EDF tasks must specify an interrupt binding via the `binds` attribute.");
        };
        let binds = binds.path.clone();

        Ok(Self {
            params,
            attr_idx,
            task_struct,
            deadline_us,
            priority: None,
            binds,
        })
    }

    pub fn generate_signal_binding(&self, priority: u32) -> TokenStream {
        let binds = &self.binds;
        let task_ident = &self.task_struct.ident;
        let sched_task_ident = format_ident!("__signal_scheduler_{}", self.task_struct.ident);
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
                    Scheduler::schedule(
                        &RUNNING_STACK,
                        &TASK_QUEUE,
                        &MIN_DEADLINE,
                        ::rtic_edf_pass::task::Task::new(#deadline_us, #task_ident::exec),
                    );
                }
           }
        }
    }
}
