use wasmi::{Caller, Error, Linker};
use crate::wasm::state::HostState;
use super::{write_to_guest, Exit, HostModule};

pub struct ProcModule;

impl HostModule for ProcModule {
    fn namespace(&self) -> &'static str { "os::proc" }

    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), Error> {
        let ns = self.namespace();

        linker.func_wrap(ns, "args_len", |caller: Caller<'_, HostState>| -> i32 {
            let args = &caller.data().args;
            if args.is_empty() { return 0; }
            (args.iter().map(|a| a.len()).sum::<usize>() + args.len() - 1) as i32
        })?;

        linker.func_wrap(ns, "args_get", |mut caller: Caller<'_, HostState>, ptr: i32, max_len: i32| -> i32 {
            let joined = caller.data().args.join("\n");
            write_to_guest(&mut caller, ptr, max_len, joined.as_bytes())
        })?;

        // Terminate the program with the given exit code; the runner catches
        // the trap and reports `HostState::exit_code` as the result of `run`.
        linker.func_wrap(ns, "exit", |mut caller: Caller<'_, HostState>, code: i32| -> Result<(), Error> {
            caller.data_mut().exit_code = Some(code);
            Err(Error::host(Exit))
        })?;

        Ok(())
    }
}