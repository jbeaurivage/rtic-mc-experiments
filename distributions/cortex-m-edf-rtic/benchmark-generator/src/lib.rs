use std::{env, fs, path::PathBuf};

use proc_macro::TokenStream;
use quote::format_ident;
use syn::parse_macro_input;

use crate::{handlers::Handler, interrupt_sources::INTERRUPT_SOURCES, parse::Settings};

mod codegen;
mod handlers;
mod interrupt_sources;
mod parse;

#[proc_macro]
pub fn generate_benchmark_app(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as crate::parse::Args);
    let settings: Settings = args
        .try_into()
        .expect("Macro inputs: integer, integer, [integer]");

    let mut handlers: Vec<_> = INTERRUPT_SOURCES
        .entries()
        .map(|(i, idx)| Handler {
            vector_idx: *idx,
            ident: format_ident!("{i}"),
        })
        .collect();
    handlers.sort_by_key(|h| h.vector_idx);

    let generated_code = codegen::generate_app(handlers, settings);

    // Write the generated code to a file in the target directory
    let target_dir = find_target_dir().expect("Failed to locate the `target` directory");
    let bin_name = std::env::var("CARGO_BIN_NAME").unwrap();
    let out_path = target_dir.join(format!("{bin_name}_generated.rs"));

    fs::write(&out_path, generated_code.to_string())
        .expect("Failed to write generated code to file");

    eprintln!("Generated benchmark written to {}", out_path.display());

    // Return the generated code as the macro output
    generated_code.into()
}

/// Traverses the parent directories of `OUT_DIR` until it finds the `target` directory.
fn find_target_dir() -> Option<PathBuf> {
    let mut path = PathBuf::from(env::var("OUT_DIR").ok()?);

    while path.file_name().is_some_and(|name| name != "target") {
        path.pop();
    }

    if path.file_name().is_some_and(|name| name == "target") {
        Some(path)
    } else {
        None
    }
}
