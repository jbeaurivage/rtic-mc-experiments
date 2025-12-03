use rtic_core::parse_utils::RticAttr;
use syn::{Expr, ItemStruct, Lit, Path};

use crate::types::Deadline;

pub struct AppParameters {
    pub dispatchers: Vec<Path>,
    pub pac_path: Path,
    pub cpu_freq: u32,
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

        let cpu_freq = if let Some(Expr::Lit(syn::ExprLit {
            lit: Lit::Int(int), ..
        })) = args.elements.get("cpu_freq")
        {
            int.base10_parse().ok()
        } else {
            panic!("`cpu_freq` must be a integer literal representing the CPU frequency in Hertz");
        }
        .unwrap_or_else(|| {
            panic!("`cpu_freq` must be a integer literal representing the CPU frequency in Hertz")
        });

        Ok(Self {
            dispatchers: dispatcher_vec,
            pac_path: pac_path.path.clone(),
            cpu_freq,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TaskStructDef {
    pub params: RticAttr,
    pub attr_idx: usize,
    pub task_struct: ItemStruct,
    pub deadline_us: Deadline,
    /// Interrupt handler signalling task arrival
    pub binds: Path,
}

impl TaskStructDef {
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
            binds,
        })
    }
}
