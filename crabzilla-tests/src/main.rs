use crabzilla::*;
use std::io::stdin;

#[import_fn(name="read", scope="Stdin")]
fn read_from_stdin() -> Value {
    let mut buffer = String::new();
    println!("Type your name: ");
    stdin().read_line(&mut buffer)?;
    buffer.pop();
    Value::String(buffer)
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
    if let Err(error) = runtime.load_module("crabzilla-tests/js/module.js").await {
        eprintln!("{}", error);
    }
}
