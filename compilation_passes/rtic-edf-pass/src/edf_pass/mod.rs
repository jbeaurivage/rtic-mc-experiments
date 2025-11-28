mod codegen;
// mod error;
mod parse;

use codegen::CodeGen;
use parse::App;
use proc_macro2::TokenStream;
use rtic_core::RticPass;
use rtic_core::parse_utils::RticAttr;
use syn::{ItemMod, parse_quote};

pub struct EdfPass {
    min_priority: u16,
    max_priority: u16,
}

impl EdfPass {
    #[allow(clippy::new_without_default)]
    pub fn new(min_priority: u16, max_priority: u16) -> Self {
        Self {
            min_priority,
            max_priority,
        }
    }
}

impl RticPass for EdfPass {
    fn run_pass(&self, args: TokenStream, app_mod: ItemMod) -> syn::Result<(TokenStream, ItemMod)> {
        let params = RticAttr::parse_from_tokens(args.clone())?;

        let mut parsed = App::parse(&params, app_mod)?;

        self.analyze(&mut parsed);

        for task in parsed.tasks.iter_mut() {
            if let Some(deadline) = task.priority {
                task.params.elements.remove("deadline");
                let expr: syn::Expr = parse_quote! { #deadline };
                let _ = task.params.elements.insert("priority".into(), expr);
            } else {
                continue;
            }
        }

        let code = CodeGen::new(parsed).run();
        Ok((args, code))
    }

    fn pass_name(&self) -> &str {
        "edf_pass"
    }
}

impl EdfPass {
    fn analyze(&self, app: &mut App) {
        app.convert_deadlines_to_priorities(self);
    }
}
