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
        use itertools::Itertools;

        let mut sorted_tasks: Vec<_> = self.tasks.clone();

        sorted_tasks.sort_by_key(|t| t.deadline_us);
        sorted_tasks.reverse();

        if sorted_tasks.len() as u16 > edf_pass.max_priority {
            panic!(
                "Exceeded number of priorities for this platform ({}), please coerce deadlines manually.",
                edf_pass.max_priority
            );
        }

        if self.app_parameters.dispatchers.len() != sorted_tasks.len() {
            panic!(
                "The EDF scheduler needs exactly as many dispatchers as there are tasks. Please add or remove dispatchers accordingly."
            )
        }

        // Get windows of identical deadlines and convert those to priorities
        let prio_groups = std::iter::once(true)
            .chain(
                sorted_tasks
                    .iter()
                    .tuple_windows()
                    .map(|(a, b)| a.deadline_us != b.deadline_us),
            )
            .scan(0, |acc, is_new| {
                if is_new {
                    *acc += 1;
                }
                Some(*acc)
            })
            .collect::<Vec<_>>();

        sorted_tasks
            .iter_mut()
            .enumerate()
            .zip(prio_groups)
            .for_each(|((idx, task), prio)| {
                let prio = prio + edf_pass.min_priority as u32;
                task.priority = Some(prio);
                task.dispatcher_idx = Some(idx);
            });

        eprintln!("min prio: {}", edf_pass.min_priority);
        for t in sorted_tasks.iter() {
            eprintln!(
                "{} => prio: {:?}, idx: {:?} ",
                t.deadline_us, t.priority, t.dispatcher_idx
            );
        }

        // TODO: it would probably better to change the type of the stored rtic task
        // rather than try to bodge with optional priorities and replacing the vec
        let _ = std::mem::replace(&mut self.tasks, sorted_tasks);
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

/// returns the index of the `attr_name` attribute if found in the attribute
/// list of some struct
fn is_struct_with_attr(strct: &ItemStruct, attr_name: &str) -> Option<usize> {
    for (i, attr) in strct.attrs.iter().enumerate() {
        let path = attr.meta.path();
        if path.segments.len() == 1 && path.segments[0].ident == attr_name {
            return Some(i);
        }
    }
    None
}
