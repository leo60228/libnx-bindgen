use proc_quote::quote;
use heck::SnakeCase;
use syn::ForeignItemFn;
use syn::visit::Visit;
use std::fs::File;
use std::io::prelude::*;
use itertools::Itertools;

pub struct FunctionVisitor<'a> {
    functions: Vec<ForeignItemFn>,
    prefix: &'a str,
}

impl FunctionVisitor<'_> {
    pub fn new<'a>(prefix: &'a str) -> FunctionVisitor<'a> {
        FunctionVisitor {
            functions: Vec::new(),
            prefix,
        }
    }
}

impl<'a> From<FunctionVisitor<'a>> for Vec<ForeignItemFn> {
    fn from(visitor: FunctionVisitor) -> Self {
        visitor.functions
    }
}

impl<'ast, 'a> Visit<'ast> for FunctionVisitor<'a> {
    fn visit_foreign_item_fn(&mut self, func: &'ast ForeignItemFn) {
        if func.ident.to_string().starts_with(self.prefix) {
            self.functions.push(func.clone());
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (bindgen, prefix) = std::env::args().skip(1).next_tuple().unwrap();

    let bindgen_text = {
        let mut file = File::open(&bindgen)?;
        let mut s = String::new();
        file.read_to_string(&mut s)?;
        s
    };

    let file = syn::parse_file(&bindgen_text)?;

    let functions: Vec<ForeignItemFn> = {
        let mut visitor = FunctionVisitor::new(&prefix);
        visitor.visit_file(&file);
        visitor.into()
    };

    let mut init = None;
    let mut exit = None;
    let mut calls: Vec<ForeignItemFn> = Vec::new();

    for function in functions {
        let name = function.ident.to_string();
        if name.ends_with("Initialize") && name.len() == prefix.len() + "Initialize".len() {
            init = Some(function);
        } else if name.ends_with("Exit") && name.len() == prefix.len() + "Exit".len() {
            exit = Some(function);
        } else {
            calls.push(function);
        }
    }

    let init = init.unwrap();
    let init_ident = &init.ident;

    let exit = exit.unwrap();
    let exit_ident = &exit.ident;

    let call_orig_idents = calls.iter().map(|e| e.ident.clone());
    let call_idents = calls.iter().map(|e| syn::Ident::new(&(e.ident.to_string()[prefix.len()..]).to_snake_case(), proc_macro2::Span::call_site()));
    let call_args = calls.iter().map(|e| e.decl.inputs.iter().map(|arg| {
        if let syn::FnArg::Captured(captured_arg) = arg {
            if let syn::Pat::Ident(pat_ident) = &captured_arg.pat {
                 return pat_ident.ident.clone();
            }
        }

        unreachable!();
    }).collect::<syn::punctuated::Punctuated<syn::Ident, syn::Token![,]>>());
    let call_params = calls.iter().map(|e| e.decl.inputs.clone());
    let call_returns = calls.iter().map(|e| if let syn::ReturnType::Type(_, typ) = &e.decl.output {
        quote! {-> #typ}
    } else {
        quote! {}
    });

    let init_is_result = if let syn::ReturnType::Type(_, typ) = init.decl.output {
        if let syn::Type::Path(typ) = *typ {
            if let Some(segment) = typ.path.segments.last() {
                segment.value().ident.to_string() == "Result"
            } else { false }
        } else { false }
    } else { false };

    let init_pat = if init_is_result {
        quote! { 0 }
    } else {
        quote! { _ }
    };

    let service = quote! {
use crate::macros::handle;

handle!(#init_pat in #init_ident(), #exit_ident(), {
    #(
        pub fn #call_idents(&mut self, #call_params) #call_returns {
            unsafe { sys::#call_orig_idents(#call_args) }
        }
    )*
});
    };

    print!("{}", service);

    Ok(())
}
