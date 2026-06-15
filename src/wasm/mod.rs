use alloc::string::String;
use alloc::vec::Vec;
use wasmi::{Engine, Error, Linker, Module, ResumableCall, Store};
use crate::task::yield_now;
use crate::wasm::host::WaitYield;
use crate::wasm::state::HostState;

pub mod state;
mod host;



pub async fn run(data: Vec<u8>) -> Result<(), Error> {
    let engine = Engine::default();
    let module = Module::new(&engine, data)?;
    let mut store = Store::new(&engine, HostState::default());
    let mut linker = Linker::new(&engine);
    host::register_all(&mut linker)?;

    let instance = linker.instantiate_and_start(&mut store, &module)?;
    let main = instance
        .get_func(&store, "main")
        .ok_or_else(|| Error::new("program has no `main` export"))?;

    let mut call = main.call_resumable(&mut store, &[], &mut [])?;
    loop {
        match call {
            ResumableCall::Finished => return Ok(()),
            ResumableCall::HostTrap(invocation) => {
                if invocation.host_error().downcast_ref::<WaitYield>().is_some() {
                    yield_now().await;
                    // os::yield_now has no results -> resume with no values.
                    call = invocation.resume(&mut store, &[], &mut [])?;
                } else {
                    // Real host error -> propagate it.
                    return Err(invocation.into_host_error());
                }
            }
            // We don't enable fuel metering, so this shouldn't occur.
            ResumableCall::OutOfFuel(_) => return Err(Error::new("program ran out of fuel")),
        }
    }
}