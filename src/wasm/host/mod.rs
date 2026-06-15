pub mod io;
pub mod proc;
pub mod time;

use wasmi::{Caller, Extern, Linker};
use crate::wasm::state::HostState;
use alloc::boxed::Box;
use alloc::vec::Vec;

/// A group of host functions under one wasm import module (e.g. "os::io").
pub trait HostModule {
    /// The wasm import module name, e.g. "os::io"
    fn namespace(&self) -> &'static str;
    /// Register all functions in this module on the linker.
    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), wasmi::Error>;
}

fn modules() -> Vec<Box<dyn HostModule>> {
    alloc::vec![
        Box::new(io::IoModule),
        Box::new(proc::ProcModule),
        Box::new(time::TimeModule),
        // Box::new(fs::FsModule),  // sen
    ]
}

pub fn register_all(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    for m in modules() {
        m.register(linker)?;
    }
    Ok(())
}

pub fn write_to_guest(caller: &mut Caller<'_, HostState>, ptr: i32, max_len: i32, bytes: &[u8]) -> i32 {
    let n = bytes.len().min(max_len.max(0) as usize);
    let Some(Extern::Memory(mem)) = caller.get_export("memory") else { return -2; };
    match mem.data_mut(caller).get_mut(ptr as usize..ptr as usize + n) {
        Some(dst) => { dst.copy_from_slice(&bytes[..n]); n as i32 }
        None => -2,
    }
}