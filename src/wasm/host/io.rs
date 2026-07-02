// VERIFY: src/wasm/host/io.rs — IoModule owns print/read_line/yield_now/read_key
use alloc::string::String;
use wasmi::{Caller, Error, Extern, Linker};
use crate::wasm::state::HostState;
use super::{HostModule, WaitKey, WaitYield};

pub struct IoModule;

fn read_line_blocking() -> Option<String> {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, PS2Keyboard, ScancodeSet1};
    use pc_keyboard::layouts::AnyLayout;

    let layout_str = crate::CONFIG.lock().keyboard_layout.clone();
    let kb_layout = match layout_str.as_str() {
        "fise" => AnyLayout::FiSe105Key(layouts::FiSe105Key),
        _ => AnyLayout::Us104Key(layouts::Us104Key),
    };
    let mut keyboard = PS2Keyboard::new(ScancodeSet1::new(), kb_layout, HandleControl::MapLettersToUnicode);

    let mut line = String::new();
    loop {
        let Some(scancode) = crate::task::keyboard::pop_scancode() else {
            core::hint::spin_loop();
            continue;
        };
        let Ok(Some(event)) = keyboard.add_byte(scancode) else { continue };
        let Some(key) = keyboard.process_keyevent(event) else { continue };

        match key {
            DecodedKey::Unicode('\x03') => { crate::println!("^C"); return None; }
            DecodedKey::Unicode('\n') | DecodedKey::Unicode('\r') => { crate::print!("\n"); return Some(line); }
            DecodedKey::Unicode('\x08') => { if line.pop().is_some() { crate::print!("\x7f"); } }
            DecodedKey::Unicode(c) => { line.push(c); crate::print!("{}", c); }
            DecodedKey::RawKey(_) => {}
        }
    }
}

impl HostModule for IoModule {
    fn namespace(&self) -> &'static str { "os::io" }

    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), Error> {
        let ns = self.namespace();

        linker.func_wrap(ns, "print", |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
            let Some(Extern::Memory(mem)) = caller.get_export("memory") else { return; };
            let data = mem.data(&caller);
            let start = ptr as usize;
            let end = start.saturating_add(len as usize);
            if let Some(bytes) = data.get(start..end) {
                crate::print!("{}", core::str::from_utf8(bytes).unwrap_or("<invalid utf8>"));
            }
        })?;

        linker.func_wrap(ns, "read_line", |mut caller: Caller<'_, HostState>, ptr: i32, max_len: i32| -> i32 {
            let Some(line) = read_line_blocking() else {
                return -1;   // Ctrl+C / avbruten
            };

            super::write_to_guest(&mut caller, ptr, max_len, line.as_bytes())
        })?;

        // Yield to executor: () -> (); the Err is the yield signal.
        linker.func_wrap(ns, "yield_now", |_caller: Caller<'_, HostState>| -> Result<(), Error> {
            Err(Error::host(WaitYield))
        })?;

        // Wait for a key: () -> i32; runner awaits InputFocus, resumes with the encoded key.
        linker.func_wrap(ns, "read_key", |_caller: Caller<'_, HostState>| -> Result<i32, Error> {
            Err(Error::host(WaitKey))
        })?;

        Ok(())
    }
}