use proc_macro2::TokenStream;
use rtic_core::parse_utils::RticAttr;
use syn::{Expr, ItemStruct, Lit, Path};

pub struct AppParameters {
    pub dispatchers: Vec<Path>,
    pub pac_path: Path,
}

impl AppParameters {
    pub fn parse(args: &RticAttr) -> syn::Result<Self> {
        let mut dispatcher_vec = vec![];
        if let Some(Expr::Array(ref array)) = args.elements.get("dispatchers") {
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
        let pac_path = if let Some(Expr::Path(p)) = args.elements.get("device") {
            p
        } else {
            panic!()
        };
        Ok(Self {
            dispatchers: dispatcher_vec,
            pac_path: pac_path.path.clone(),
        })
    }
}

#[derive(Debug)]
pub struct RticTask {
    pub params: RticAttr,
    pub attr_idx: usize,
    pub task_struct: ItemStruct,
    pub deadline: Option<u32>, // explicit deadline
}

impl RticTask {
    pub fn from_struct((task_struct, attr_idx): (ItemStruct, usize)) -> syn::Result<Self> {
        let params = RticAttr::parse_from_attr(&task_struct.attrs[attr_idx])?;

        let deadline = if let Some(Expr::Lit(syn::ExprLit {
            lit: Lit::Int(int), ..
        })) = params.elements.get("deadline")
        {
            // deadline explicitly assigned by the user
            int.base10_parse().ok()
        } else {
            None
        };

        if let Some(Expr::Lit(syn::ExprLit {
            lit: Lit::Int(_int),
            ..
        })) = params.elements.get("priority")
        {
            panic!("'priority' found, please use 'deadlines' only or compile with --no-default-features.")
        }

        Ok(Self {
            params,
            attr_idx,
            task_struct,
            deadline,
        })
    }
}
