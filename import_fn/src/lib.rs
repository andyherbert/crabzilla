use proc_macro::TokenStream;
use syn::{
    AttributeArgs,
    NestedMeta,
    Meta,
    Block,
    Ident,
    ItemFn,
    parse_macro_input,
    spanned::Spanned,
    Lit,
};
use quote::{
    quote,
    quote_spanned,
    ToTokens,
};

macro_rules! option_string_to_token_stream {
    ($opt_string:expr) => {
        match $opt_string {
            Some(string) => quote! {
                Some(String::from(#string))
            },
            None => quote! {
                None
            },
        }
    }
}

fn quote_without_return(ident: &Ident, block: &Box<Block>, crab_meta: ImportOptions) -> TokenStream {
    let name = crab_meta.name.unwrap_or(ident.to_string());
    let scope = option_string_to_token_stream!(crab_meta.scope);
    let result = quote! {
        fn #ident() -> crabzilla::ImportedFn {
            crabzilla::create_sync_fn(
                |args: Vec<crabzilla::Value>| -> std::result::Result<crabzilla::Value, crabzilla::AnyError> {
                    Ok(#block)
                },
                #name,
                #scope,
            )
        }
    };
    result.into()
}

fn quote_with_return(ident: &Ident, block: &Box<Block>, crab_meta: ImportOptions) -> TokenStream {
    let name = crab_meta.name.unwrap_or(ident.to_string());
    let scope = option_string_to_token_stream!(crab_meta.scope);
    let result = quote! {
        fn #ident() -> crabzilla::ImportedFn {
            crabzilla::create_sync_fn(
                |args: Vec<crabzilla::Value>| -> std::result::Result<crabzilla::Value, crabzilla::AnyError> {
                    #block
                    Ok(crabzilla::Value::Null)
                },
                #name,
                #scope,
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

#[derive(Default)]
struct ImportOptions {
    scope: Option<String>,
    name: Option<String>,
}

fn validate_literal(name: &String) -> bool {
    if name.is_empty() {
        return false;
    }
    for char in name.chars() {
        if !char.is_ascii() {
            return false;
        }
        if char.is_whitespace() {
            return false;
        }
    }
    true
}

fn parse_meta(metas: Vec<NestedMeta>) -> Result<ImportOptions, TokenStream> {
    let mut options = ImportOptions::default();
    for meta in metas {
        match meta {
            NestedMeta::Meta(meta) => {
                match meta {
                    Meta::NameValue(meta_name_value) => {
                        let string = meta_name_value.path.to_token_stream().to_string();
                        match string.as_str() {
                            "scope" => {
                                match meta_name_value.lit {
                                    Lit::Str(lit_str) => {
                                        let scope = lit_str.value();
                                        if !validate_literal(&scope) {
                                            return Err(error(lit_str, "Invalid scope"));
                                        }
                                        options.scope = Some(scope);
                                    },
                                    _ => return Err(error(meta_name_value.lit, "Unsupported value")),
                                }
                            },
                            "name" => {
                                match meta_name_value.lit {
                                    Lit::Str(lit_str) => {
                                        let name = lit_str.value();
                                        if !validate_literal(&name) {
                                            return Err(error(lit_str, "Invalid name"));
                                        }
                                        options.name = Some(lit_str.value());
                                    },
                                    _ => return Err(error(meta_name_value.lit, "Unsupported value")),
                                }
                            },
                            _ => return Err(error(meta_name_value, "Unsupported meta")),
                        }
                    },
                    _ => return Err(error(meta, "Unsupported meta")),
                }
            },
            _ => return Err(error(meta, "Unsupported meta")),
        }
    }
    Ok(options)
}

/// An attribute macro to convert Rust functions so they can be imported into a runtime.
/// The meta attributes `name` and `scope` can be used to define the scoping of a particular
/// when calling from javascript, for example `scope = "Foo", name = "bar"` would assign
/// the function as Foo.bar. Without a scope the function will be attached to the global
/// object, and without a name it will be assigned with the Rust function name.
#[proc_macro_attribute]
pub fn import_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let attr = parse_macro_input!(attr as AttributeArgs);
    let crab_meta = match parse_meta(attr) {
        Ok(string) => string,
        Err(error) => return error.into(),
    };
    match input.sig.inputs.to_token_stream().to_string().as_str() {
        "" |
        "args : Vec < Value >" |
        "args : Vec < crabzilla :: Value >" => {},
        "args : std :: vec :: Vec < Value >" => {},
        "args : std :: vec :: Vec < crabzilla :: Value >" => {},
        "args : :: vec :: Vec < Value >" => {},
        "args : :: vec :: Vec < crabzilla :: Value >" => {},
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
                    => quote_without_return(&input.sig.ident, &input.block, crab_meta),
                "-> ()" | ""
                    => quote_with_return(&input.sig.ident, &input.block, crab_meta),
                _ => error(
                    input.sig.output,
                    "Illegal return type, should be empty or \"Value\""
                ),
            }
        }
    }
}
