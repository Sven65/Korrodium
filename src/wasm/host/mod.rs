pub mod io;
pub mod proc;
pub mod time;

use wasmi::{Caller, Error, Extern, Linker};
use crate::wasm::state::HostState;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use pc_keyboard::{DecodedKey, KeyCode};
use wasmi::errors::HostError;

#[derive(Debug)]
pub struct WaitYield;

impl fmt::Display for WaitYield {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "yield to executor")
    }
}

impl HostError for WaitYield {}

/// Returned by `os::read_key`; the runner blocks for a key, then resumes with it.
#[derive(Debug)]
pub struct WaitKey;
impl fmt::Display for WaitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "wait for key") }
}
impl HostError for WaitKey {}

/// Encode a key as the i32 returned by `os::read_key`:
///   >= 0  Unicode scalar value (printables + control chars: Enter=10/13,
///         Tab=9, Backspace=8, Esc=27, Ctrl+X=24, ...)
///   <  0  special navigation key
pub fn encode_key(key: DecodedKey) -> i32 {
    match key {
        DecodedKey::Unicode(c) => c as i32,
        DecodedKey::RawKey(code) => match code {
            KeyCode::ArrowUp    => -1,
            KeyCode::ArrowDown  => -2,
            KeyCode::ArrowLeft  => -3,
            KeyCode::ArrowRight => -4,
            KeyCode::Home       => -5,
            KeyCode::End        => -6,
            KeyCode::Delete     => -7,
            KeyCode::PageUp     => -8,
            KeyCode::PageDown   => -9,
            _                   => 0,
        },
    }
}

pub fn register_all(linker: &mut Linker<HostState>) -> Result<(), Error> {
    io::register(linker)?;
    Ok(())
}

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

pub fn write_to_guest(caller: &mut Caller<'_, HostState>, ptr: i32, max_len: i32, bytes: &[u8]) -> i32 {
    let n = bytes.len().min(max_len.max(0) as usize);
    let Some(Extern::Memory(mem)) = caller.get_export("memory") else { return -2; };
    match mem.data_mut(caller).get_mut(ptr as usize..ptr as usize + n) {
        Some(dst) => { dst.copy_from_slice(&bytes[..n]); n as i32 }
        None => -2,
    }
}