use crate::{EdfPass, edf_pass::parse::ast::AppParameters};

use super::parse::ast::EdfTask;
use proc_macro2::Ident;
use rtic_core::parse_utils::RticAttr;
use syn::{Item, ItemMod, ItemStruct, Visibility};

pub mod ast;

/// Type to represent an RTIC application for deadline to priority conversion
pub struct App {
    pub mod_visibility: Visibility,
    pub mod_ident: Ident,
    pub app_parameters: AppParameters,
    pub tasks: Vec<EdfTask>,
    pub rest_of_code: Vec<Item>,
}

impl App {
    pub fn parse(params: &RticAttr, mut app_mod: ItemMod) -> syn::Result<Self> {
        let app_parameters = AppParameters::parse(params)?;

        let app_mod_items = app_mod.content.take().unwrap_or_default().1;

        let mut task_structs = Vec::new();
        let mut rest_of_code = Vec::with_capacity(app_mod_items.len());

        for item in app_mod_items {
            match item {
                Item::Struct(strct) => {
                    if let Some(attr_idx) = is_struct_with_attr(&strct, "task") {
                        task_structs.push((strct, attr_idx))
                    } else if let Some(attr_idx) = is_struct_with_attr(&strct, "sw_task") {
                        task_structs.push((strct, attr_idx))
                    } else {
                        rest_of_code.push(Item::Struct(strct))
                    }
                }
                _ => rest_of_code.push(item),
            }
        }
        //rest_of_code.push(quote!(hello));
        let tasks = task_structs
            .into_iter()
            .map(EdfTask::from_struct)
            .collect::<syn::Result<_>>()?;

        Ok(Self {
            mod_ident: app_mod.ident,
            mod_visibility: app_mod.vis,
            app_parameters,
            tasks,
            rest_of_code,
        })
    }

    pub(super) fn convert_deadlines_to_priorities(&mut self, edf_pass: &EdfPass) {
        let mut deadlines: Vec<_> = self.tasks.iter().map(|t| t.deadline_us).collect();

        deadlines.sort();
        deadlines.dedup();
        // TODO: should this be reversed? What is the priority ordering considered by RTIC?
        // deadlines.reverse();

        if deadlines.len() as u16 > edf_pass.max_priority {
            panic!(
                "Exceeded number of priorities for this platform ({}), please coerce deadlines manually.",
                edf_pass.max_priority
            );
        }

        // Transform deadlines into priorities that will be passed as hardware tasks
        for t in self.tasks.iter_mut() {
            let pos = deadlines.iter().position(|d| *d == t.deadline_us).unwrap();
            t.priority = Some(pos as u32 + 1);
        }
    }

    pub(super) fn scheduler_priority(&self) -> u32 {
        self.tasks
            .iter()
            .flat_map(|t| t.priority)
            .max()
            .map(|p| p + 1)
            .expect("Scheduler should have a priority assigned")
    }
}

/// returns the index of the `attr_name` attribute if found in the attribute list of some struct
fn is_struct_with_attr(strct: &ItemStruct, attr_name: &str) -> Option<usize> {
    for (i, attr) in strct.attrs.iter().enumerate() {
        let path = attr.meta.path();
        if path.segments.len() == 1 && path.segments[0].ident == attr_name {
            return Some(i);
        }
    }
    None
}
