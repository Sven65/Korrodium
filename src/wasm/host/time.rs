use wasmi::{Caller, Error, Linker};
use crate::time::get_time;
use crate::wasm::host::{write_to_guest, HostModule};
use crate::wasm::state::HostState;

pub struct TimeModule;


impl HostModule for TimeModule {
    fn namespace(&self) -> &'static str { "os::time" }

    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), Error> {
        let ns = self.namespace();
        linker.func_wrap(ns, "get_time", |mut caller: Caller<'_, HostState>, ptr: i32, max_len: i32| {
            let t = get_time();
            let mut buf = [0u8; 8];
            buf[0..2].copy_from_slice(&(t.year as u16).to_le_bytes());
            buf[2] = t.month;
            buf[3] = t.day;
            buf[4] = t.hour;
            buf[5] = t.minute;
            buf[6] = t.second;
            // buf[7] = pad
            write_to_guest(&mut caller, ptr, 8, &buf)
        })?;

        Ok(())
    }
}