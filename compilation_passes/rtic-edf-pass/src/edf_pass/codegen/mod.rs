use super::parse::App;

use quote::quote;
use syn::{parse_quote, ItemMod};

pub struct CodeGen {
    app: App,
}

impl CodeGen {
    pub fn new(app: App) -> CodeGen {
        Self { app }
    }

    pub fn run(&mut self) -> ItemMod {
        let tasks = self.app.tasks.iter_mut().map(|task| {
            let task_attribute = &task.params;
            let task_struct = &mut task.task_struct;
            // remove the older task attribute and replace with the updated one which includes an
            // automatically assigned core
            task_struct.attrs.remove(task.attr_idx);
            quote! {
                #task_attribute
                #task_struct
            }
        });
        let mod_visibility = &self.app.mod_visibility;
        let mod_ident = &self.app.mod_ident;
        let other_code = &self.app.rest_of_code;
        let num_dispatchers = self.app.app_parameters.dispatchers.len();
        let mut dispatchers = vec![];
        for d in self.app.app_parameters.dispatchers.iter() {
            if let Some(d) = d.get_ident() {
                dispatchers.push(d);
            } else {
                todo!()
            }
        }
        // let shared_resources = self.app.shared_resources.iter().map(|s| &s.shared_struct);
        let pac_path = &self.app.app_parameters.pac_path;
        let ret = parse_quote! {
            #mod_visibility mod #mod_ident {
                //#(#dispatchers)*
                use #pac_path::Interrupt;
                use rtic_edf_pass::Vec;
                const NUM_DISPATCHERS: usize = #num_dispatchers;
                const DISPATCHERS: [Interrupt; NUM_DISPATCHERS] = [
                    #(Interrupt::#dispatchers,)*
                ];
                use rtic_edf_pass::task::RunningTask;
                struct TaskStack(core::cell::UnsafeCell<Vec<RunningTask, NUM_DISPATCHERS, u8>>);
                // TODO
                impl rtic_edf_pass::InternalTaskStack<cortex_m_edf_rtic::export::CsGuard, NUM_DISPATCHERS> for TaskStack {
                        fn get_mut(&self, _cs: &cortex_m_edf_rtic::export::CsGuard) -> *mut Vec<RunningTask, NUM_DISPATCHERS, u8> {
                            self.0.get()
                        }
                }

                #(#other_code)*
                // #(#shared_resources)*
                #(#tasks)*
            }
        };
        ret
    }
}
