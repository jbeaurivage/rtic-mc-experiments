use crate::{EdfPass, edf_pass::parse::ast::AppParameters, types::Deadline};

use super::parse::ast::TaskStructDef;
use proc_macro2::Ident;
use rtic_core::parse_utils::RticAttr;
use syn::{Item, ItemMod, ItemStruct, Path, Visibility};

pub mod ast;

#[derive(Debug, Clone)]
pub struct EdfTask {
    pub params: RticAttr,
    pub attr_idx: usize,
    pub task_struct: ItemStruct,
    /// A task's priority, which is initially expressed as an explicit deadline
    pub priority: u16,
    pub dispatcher_idx: u16,
    pub deadline_us: Deadline,
    /// Each task gets assigned its own dispatcher
    pub dispatcher: Path,
    /// Interrupt handler signalling task arrival
    pub binds: Path,
}

/// Type to represent an RTIC application for deadline to priority conversion
pub struct App {
    pub mod_visibility: Visibility,
    pub mod_ident: Ident,
    pub app_parameters: AppParameters,
    pub tasks: Vec<EdfTask>,
    pub rest_of_code: Vec<Item>,
}

impl App {
    pub fn parse(edf_pass: &EdfPass, params: &RticAttr, mut app_mod: ItemMod) -> syn::Result<Self> {
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

        let task_defs = task_structs
            .into_iter()
            .map(TaskStructDef::from_struct)
            .collect::<syn::Result<Vec<_>>>()?;

        let tasks = Self::assign_dispatchers_and_priorities(
            edf_pass,
            task_defs,
            &app_parameters.dispatchers,
        );

        Ok(Self {
            mod_ident: app_mod.ident,
            mod_visibility: app_mod.vis,
            app_parameters,
            tasks,
            rest_of_code,
        })
    }

    fn assign_dispatchers_and_priorities(
        edf_pass: &EdfPass,
        tasks: Vec<TaskStructDef>,
        dispatchers: &[Path],
    ) -> Vec<EdfTask> {
        use itertools::Itertools;

        let mut sorted_tasks = tasks;
        sorted_tasks.sort_by_key(|t| t.deadline_us);
        sorted_tasks.reverse();

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

        let task_map: Vec<_> = sorted_tasks
            .into_iter()
            .enumerate()
            .zip(prio_groups)
            .zip(dispatchers)
            .map(|(((dispatcher_idx, task), prio), dispatcher_path)| {
                let priority = prio + edf_pass.min_priority;

                EdfTask {
                    params: task.params,
                    attr_idx: task.attr_idx,
                    task_struct: task.task_struct,
                    priority,
                    dispatcher_idx: dispatcher_idx
                        .try_into()
                        .expect("Unsupported dispatcher priority level: over u16::MAX"),
                    dispatcher: dispatcher_path.clone(),
                    deadline_us: task.deadline_us,
                    binds: task.binds,
                }
            })
            .collect();

        eprintln!("scheduler min prio: {}", edf_pass.min_priority);
        for t in task_map.iter() {
            eprintln!(
                "Task: deadline {} => prio: {:?}, dispatcher idx: {:?}, binding: {}",
                t.deadline_us,
                t.priority,
                t.dispatcher_idx,
                t.dispatcher.get_ident().unwrap()
            );
        }
        task_map
    }

    pub(super) fn timestamper_priority(&self) -> u16 {
        self.tasks
            .iter()
            .map(|t| t.priority)
            .max()
            .map(|p| p + 1)
            // Technically this "1" priority is irrelevant if we have no
            // EDF tasks, as no signaller binding will be generated.
            .unwrap_or(1)
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
