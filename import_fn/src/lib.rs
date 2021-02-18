use proc_macro::TokenStream;
use syn::{Block, Ident, parse_macro_input, spanned::Spanned};
use quote::{quote, quote_spanned, ToTokens};

fn quote_without_return(ident: &Ident, block: &Box<Block>) -> TokenStream {
    let name = ident.to_string();
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

fn quote_with_return(ident: &Ident, block: &Box<Block>) -> TokenStream {
    let name = ident.to_string();
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

fn error<T: Spanned>(item: T, msg: &str) -> TokenStream {
    let span = item.span();
    let error = quote_spanned! {
        span => compile_error!(#msg);
    };
    error.into()
}

/// An attribute macro to convert Rust functions so they can be imported into a runtime.
#[proc_macro_attribute]
pub fn import_fn(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    match input.sig.inputs.to_token_stream().to_string().as_str() {
        "" |
        "args : Vec < Value >" |
        "args : Vec < crabzilla :: Value >" => {},
        "args : std :: vec :: Vec < Value >" => {},
        "args : std :: vec :: Vec < crabzilla :: Value >" => {},
        _ => return error(
            input.sig.inputs,
            "Illegal arguments, should be empty or \"args: Vec<Value>\""
        ),
    }
    match input.sig.asyncness {
        Some(_) => todo!(),
        None => {
            match input.sig.output.to_token_stream().to_string().as_str() {
                "-> crabzilla :: Value" | "-> Value"
                    => quote_without_return(&input.sig.ident, &input.block),
                "-> ()" | ""
                    => quote_with_return(&input.sig.ident, &input.block),
                _ => error(
                    input.sig.output,
                    "Illegal return type, should be empty or \"Value\""
                ),
            }
        }
    }
}
