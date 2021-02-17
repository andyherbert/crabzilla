use proc_macro::TokenStream;
use syn::{parse_macro_input, ReturnType, Ident, Block};
use quote::quote;

fn quote_without_return(ident: &Ident, block: &Box<Block>, name: String) -> TokenStream {
    let result = quote! {
        fn #ident() -> crabzilla::ImportedFn {
            crabzilla::create_sync_fn(
                |args: Vec<crabzilla::Value>| -> Result<crabzilla::Value, crabzilla::AnyError> {
                    Ok(#block)
                },
                String::from(#name),
            )
        }
    };
    result.into()
}

fn quote_with_return(ident: &Ident, block: &Box<Block>, name: String) -> TokenStream {
    let result = quote! {
        fn #ident() -> crabzilla::ImportedFn {
            crabzilla::create_sync_fn(
                |args: Vec<crabzilla::Value>| -> Result<crabzilla::Value, crabzilla::AnyError> {
                    #block
                    Ok(crabzilla::Value::Null)
                },
                String::from(#name),
            )
        }
    };
    result.into()
}

/// An attribute macro to convert Rust functions so they can be imported into a runtime.
#[proc_macro_attribute]
pub fn import_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    let sig = &input.sig;
    let ident = &sig.ident;
    let name = ident.to_string();
    let block = &input.block;
    match input.sig.asyncness {
        Some(_) => todo!(),
        None => {
            if matches!(input.sig.output, ReturnType::Default) {
                quote_with_return(ident, block, name)
            } else {
                quote_without_return(ident, block, name)
            }
        }
    }
}
