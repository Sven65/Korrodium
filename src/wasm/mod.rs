use alloc::string::String;
use alloc::vec::Vec;
use wasmi::{Engine, Error, Linker, Module, ResumableCall, Store, Val};
use crate::task::yield_now;
use crate::task::keyboard::InputFocus;
use crate::wasm::host::{encode_key, Exit, WaitKey, WaitYield};
use crate::wasm::state::HostState;

pub mod state;
mod host;

/// Runs a wasm program to completion and returns its exit code (0 if it
/// returned from `main` without calling `os::proc::exit`).
pub async fn run(data: Vec<u8>, args: Vec<String>) -> Result<i32, Error> {
    let engine = Engine::default();
    let module = Module::new(&engine, data)?;
    let mut store = Store::new(&engine, HostState { exit_code: None, args });
    let mut linker = Linker::new(&engine);
    host::register_all(&mut linker)?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    let main = instance
        .get_func(&store, "main")
        .ok_or_else(|| Error::new("program has no `main` export"))?;

    // Hold focus for the whole program: keyboard task routes decoded keys into
    // FOCUSED_INPUT while HAS_FOCUS is set, and releases it when `focus` drops.
    let focus = InputFocus::acquire();

    let mut call = main.call_resumable(&mut store, &[], &mut [])?;
    loop {
        match call {
            ResumableCall::Finished => return Ok(store.data().exit_code.unwrap_or(0)),
            ResumableCall::HostTrap(invocation) => {
                if invocation.host_error().downcast_ref::<WaitYield>().is_some() {
                    yield_now().await;
                    // os::io::yield_now -> () : resume with no values.
                    call = invocation.resume(&mut store, &[], &mut [])?;
                } else if invocation.host_error().downcast_ref::<WaitKey>().is_some() {
                    let key = focus.next_key_async().await;   // DecodedKey
                    let encoded = encode_key(key);            // i32 per the scheme
                    // os::io::read_key -> i32 : resume with one i32 result.
                    call = invocation.resume(&mut store, &[Val::I32(encoded)], &mut [])?;
                } else if invocation.host_error().downcast_ref::<Exit>().is_some() {
                    // os::proc::exit already stored the code in HostState.
                    return Ok(store.data().exit_code.unwrap_or(0));
                } else {
                    return Err(invocation.into_host_error());
                }
            }
            ResumableCall::OutOfFuel(_) => return Err(Error::new("program ran out of fuel")),
        }
    }
}