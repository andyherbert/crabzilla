//! Crabzilla provides a _simple_ interface for running JavaScript modules alongside Rust code.
//! <br>
//! # Example
//! ```
//! use crabzilla::*;
//! use std::io::stdin;
//! 
//! #[import_fn]
//! fn read_from_stdin() -> Value {
//!     let mut buffer = String::new();
//!     println!("Type your name: ");
//!     stdin().read_line(&mut buffer)?;
//!     buffer.pop();
//!     Value::String(buffer)
//! }
//! 
//! #[import_fn]
//! fn say_hello(args: Vec<Value>) {
//!     if let Some(string) = args.get(0) {
//!         if let Value::String(string) = string {
//!             println!("Hello, {}", string);
//!         }
//!     }
//! }
//! 
//! #[tokio::main]
//! async fn main() {
//!     let mut runtime = runtime! {
//!         read_from_stdin,
//!         say_hello,
//!     };
//!     if let Err(error) = runtime.load_module("./module.js").await {
//!         eprintln!("{}", error);
//!     }
//! }
//! ```
//! In `module.js`:
//! ```
//! const user = read_from_stdin();
//! say_hello(user);
//! ```
use deno_core::{
    FsModuleLoader,
    JsRuntime,
    ModuleSpecifier,
    OpState,
    OpFn,
    ZeroCopyBuf,
    // json_op_async,
    json_op_sync,
    // BufVec,
};
use std::rc::Rc;
// use std::cell::RefCell;
// use futures::Future;
pub use deno_core::serde_json::value::Value;
pub use deno_core::error::AnyError;
pub use deno_core::error::custom_error;
pub use import_fn::import_fn;

fn get_args(value: &Value) -> Vec<Value> {
    if let Value::Object(map) = &value {
        if let Some(Value::Array(args)) = &map.get("args") {
            return args.to_owned();
        }
    }
    unreachable!();
}

/// Represents an imported Rust function.
pub enum ImportedFn {
    Sync(Box<OpFn>, String),
    Async(Box<OpFn>, String),
}

/// Receives a Rust function and returns a structure that can be imported in to a runtime.
pub fn create_sync_fn<F>(imported_fn: F, name: String) -> ImportedFn where
    F: Fn(Vec<Value>) -> Result<Value, AnyError> + 'static,
{
    let op_fn = json_op_sync(
        move |_state: &mut OpState, value: Value, _buffer: &mut [ZeroCopyBuf]| -> Result<Value, AnyError> {
            imported_fn(get_args(&value))
        }
    );
    ImportedFn::Sync(
        op_fn,
        name,
    )
}

// pub fn create_async_fn<F, R>(imported_fn: F, name: String) -> ImportedFn where
//     F: Fn(Vec<Value>) -> R + 'static,
//     R: Future<Output = Result<Value, AnyError>> + 'static,
// {
//     let op_fn = json_op_async(
//         move |_state: Rc<RefCell<OpState>>, value: Value, _buffer: BufVec| -> R {
//             imported_fn(get_args(&value))
//         }
//     );
//     ImportedFn::Async(
//         op_fn,
//         name,
//     )
// }

enum ImportedName {
    Sync(String),
    Async(String),
}

/// Represents a JavaScript runtime instance.
pub struct Runtime {
    runtime: JsRuntime,
    imported_names: Vec<ImportedName>,
}

impl Runtime {
    pub fn new() -> Self {
        let runtime = JsRuntime::new(deno_core::RuntimeOptions{
            module_loader: Some(Rc::new(FsModuleLoader)),
            ..Default::default()
        });
        let imported_names = vec![];
        Runtime {
            runtime,
            imported_names,
        }
    }

    pub fn import<F>(&mut self, imported_fn: F) where F: Fn() -> ImportedFn {
        match imported_fn() {
            ImportedFn::Sync(op_fn, name) => {
                self.runtime.register_op(&name, op_fn);
                self.imported_names.push(ImportedName::Sync(name));
            },
            ImportedFn::Async(op_fn, name) => {
                self.runtime.register_op(&name, op_fn);
                self.imported_names.push(ImportedName::Async(name));
            },
        }
    }

    pub fn importing_finished(&mut self) {
        let mut definitions = String::new();
        for import in self.imported_names.iter() {
            match import {
                ImportedName::Sync(name) => definitions.push_str(&format!("        window[{:?}] = (...args) => Deno.core.jsonOpSync({0:?}, {{args}});\n", name)),
                ImportedName::Async(name) => definitions.push_str(&format!("        window[{:?}] = (...args) => Deno.core.jsonOpAsync({0:?}, {{args}});\n", name)),
            }
        }
        let js_source = format!(r#"
"use strict";
(
    (window) => {{
        Deno.core.ops();
        Deno.core.registerErrorClass("Error", window.Error);
{}    }}
)(this);"#,
            definitions,
        );
        self.runtime.execute("rust:core.js", &js_source).expect("runtime exporting");
    }

    pub async fn load_module(&mut self, path_str: &str) -> Result<(), AnyError> {
        let specifier = ModuleSpecifier::resolve_path(path_str)?;
        let id = self.runtime.load_module(&specifier, None).await?;
        self.runtime.mod_evaluate(id).await
    }
}

/// Creates a runtime object and imports a list of functions.
///
/// # Example
/// ```
/// #[import_fn]
/// fn foo() {
///   // Do something
/// }
///
/// #[import_fn]
/// fn bar() {
///   // Do something else
/// }
///
/// let mut runtime = runtime! {
///    foo,
///    bar,
///  };
/// ```
#[macro_export]
macro_rules! runtime {
    ($($fn:ident),* $(,)?) => {
        {
            let mut runtime = crabzilla::Runtime::new();
            $(
                runtime.import($fn);
            )*
            runtime.importing_finished();
            runtime
        }
    }
}

/// Throws an error with a custom message in an imported Rust function.
#[macro_export]
macro_rules! throw {
    ($message:expr) => {
        return Err(crabzilla::custom_error("Error", $message));
    }
}
