//! First contact with Wasmtime: run a tiny component, then stop a runaway one with fuel.
//!
//! Run with `cargo run -p runtime --example hello_world`.

use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, Trap};

/// A component whose `run` export returns 42.
const ANSWER: &str = r#"
(component
  (core module $m
    (func (export "run") (result i32) i32.const 42))
  (core instance $i (instantiate $m))
  (func (export "run") (result u32) (canon lift (core func $i "run"))))
"#;

/// A component whose `run` export never returns.
const RUNAWAY: &str = r#"
(component
  (core module $m
    (func (export "run") (result i32) (loop $forever (br $forever)) i32.const 0))
  (core instance $i (instantiate $m))
  (func (export "run") (result u32) (canon lift (core func $i "run"))))
"#;

fn run(engine: &Engine, wat: &str, fuel: u64) -> wasmtime::Result<u32> {
    let component = Component::new(engine, wat)?;
    // Nothing is linked: the component gets no host capabilities at all.
    let linker = Linker::<()>::new(engine);
    let mut store = Store::new(engine, ());
    store.set_fuel(fuel)?;
    let instance = linker.instantiate(&mut store, &component)?;
    let run = instance.get_typed_func::<(), (u32,)>(&mut store, "run")?;
    let (value,) = run.call(&mut store, ())?;
    Ok(value)
}

fn main() -> wasmtime::Result<()> {
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config)?;

    println!("answer: {}", run(&engine, ANSWER, 10_000)?);

    match run(&engine, RUNAWAY, 10_000_000) {
        Ok(value) => println!("runaway returned {value} (the fuel limit did not work)"),
        Err(error) if error.downcast_ref::<Trap>() == Some(&Trap::OutOfFuel) => {
            println!("runaway stopped: out of fuel")
        }
        Err(error) => return Err(error),
    }
    Ok(())
}
