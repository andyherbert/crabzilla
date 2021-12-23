/*! Crabzilla provides a _simple_ interface for running JavaScript modules alongside Rust code.
# Example
```
use crabzilla::*;
use std::io::stdin;

#[import_fn(name="read", scope="Stdin")]
fn read_from_stdin() -> Value {
    let mut buffer = String::new();
    println!("Type your name: ");
    stdin().read_line(&mut buffer)?;
    buffer.pop(); // Remove newline
    if buffer.is_empty() {
        throw!("Expected name!");
    }
    json!(buffer)
}

#[import_fn(name="sayHello", scope="Stdout")]
fn say_hello(args: Vec<Value>) {
    if let Some(string) = args.get(0) {
        if let Value::String(string) = string {
            println!("Hello, {}", string);
        }
    }
}

#[tokio::main]
async fn main() {
    let mut runtime = runtime! {
        read_from_stdin,
        say_hello,
    };
    if let Err(error) = runtime.load_module("./module.js").await {
        eprintln!("{}", error);
    }
}
```
In `module.js`:
```
const user = Stdin.read();
Stdout.sayHello(user);
```
*/
pub use deno_core::error::custom_error;
pub use deno_core::error::AnyError;
pub use deno_core::serde_json::{json, value::Value};
use deno_core::{op_sync, resolve_path, FsModuleLoader, JsRuntime, OpFn, OpState};
pub use import_fn::import_fn;
use std::rc::Rc;

fn get_args(value: &Value) -> Vec<Value> {
    if let Value::Object(map) = &value {
        if let Some(Value::Array(args)) = &map.get("args") {
            return args.to_owned();
        }
    }
    unreachable!();
}

/// Represents an imported Rust function.
pub struct ImportedFn {
    op_fn: Box<OpFn>,
    name: String,
    scope: Option<String>,
}

/// Receives a Rust function and returns a structure that can be imported in to a runtime.
pub fn create_sync_fn<F>(imported_fn: F, name: &str, scope: Option<String>) -> ImportedFn
where
    F: Fn(Vec<Value>) -> Result<Value, AnyError> + 'static,
{
    let op_fn = op_sync(
        move |_state: &mut OpState, value: Value, _: ()| -> Result<Value, AnyError> {
            imported_fn(get_args(&value))
        },
    );
    ImportedFn {
        op_fn,
        name: name.to_string(),
        scope,
    }
}

struct ImportedName {
    name: String,
    scope: Option<String>,
}

/// Represents a JavaScript runtime instance.
pub struct Runtime {
    runtime: JsRuntime,
    imported_names: Vec<ImportedName>,
    scopes: Vec<String>,
}

impl Default for Runtime {
    fn default() -> Self {
        let runtime = JsRuntime::new(deno_core::RuntimeOptions {
            module_loader: Some(Rc::new(FsModuleLoader)),
            ..Default::default()
        });
        let imported_names = vec![];
        let scopes = vec![];
        Runtime {
            runtime,
            imported_names,
            scopes,
        }
    }
}

impl Runtime {
    /// Creates a new Runtime
    pub fn new() -> Self {
        Default::default()
    }

    /// Imports a new ImportedFn
    pub fn import<F>(&mut self, imported_fn: F)
    where
        F: Fn() -> ImportedFn,
    {
        let import_fn = imported_fn();
        if let Some(scope) = &import_fn.scope {
            self.scopes.push(scope.clone());
        }
        self.runtime.register_op(&import_fn.name, import_fn.op_fn);
        self.imported_names.push(ImportedName {
            name: import_fn.name,
            scope: import_fn.scope,
        });
    }

    /// Generates JavaScript hooks for the Runtime when all functions have been imported
    pub fn importing_finished(&mut self) {
        let mut scope_definitions = String::new();
        for scope in self.scopes.iter() {
            scope_definitions.push_str(&format!("        window[{:?}] = {{}};\n", scope));
        }
        let mut name_definitions = String::new();
        for import in &self.imported_names {
            let scope = match &import.scope {
                Some(scope) => format!("window[{:?}][{:?}]", scope, import.name),
                None => format!("window[{:?}]", import.name),
            };
            name_definitions.push_str(&format!(
                "        {} = (...args) => Deno.core.opSync({:?}, {{args}});\n",
                scope, import.name
            ));
        }
        let js_source = format!(
            "\"use strict\";\n((window) => {{\n{}{}}})(this);",
            scope_definitions, name_definitions,
        );
        self.runtime.sync_ops_cache();
        self.runtime
            .execute_script("rust:core.js", &js_source)
            .expect("runtime exporting");
    }

    /// Loads a JavaScript module and evaluates it
    pub async fn load_module(&mut self, path_str: &str) -> Result<(), AnyError> {
        let specifier = resolve_path(path_str)?;
        let id = self.runtime.load_main_module(&specifier, None).await?;
        let result = self.runtime.mod_evaluate(id);
        self.runtime.run_event_loop(false).await?;
        result.await?
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
    };
}
