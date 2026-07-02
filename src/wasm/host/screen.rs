use wasmi::{Caller, Error, Linker};
use crate::vga::{self, ColorCode};
use crate::wasm::state::HostState;
use super::{read_guest_string, HostModule};

pub struct ScreenModule;

impl HostModule for ScreenModule {
    fn namespace(&self) -> &'static str { "os::screen" }

    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), Error> {
        let ns = self.namespace();

        linker.func_wrap(ns, "width", |_caller: Caller<'_, HostState>| -> i32 {
            vga::BUFFER_WIDTH as i32
        })?;

        linker.func_wrap(ns, "height", |_caller: Caller<'_, HostState>| -> i32 {
            vga::BUFFER_HEIGHT as i32
        })?;

        // Write a single byte at an arbitrary position without moving the cursor.
        linker.func_wrap(ns, "write_at", |_caller: Caller<'_, HostState>, row: i32, col: i32, byte: i32, color: i32| {
            if row < 0 || col < 0 { return; }
            vga::write_at(row as usize, col as usize, byte as u8, ColorCode::from_byte(color as u8));
        })?;

        // Write a string at an arbitrary position; returns the column after the last
        // char written, or -2 on an invalid guest pointer/length.
        linker.func_wrap(ns, "write_str_at", |caller: Caller<'_, HostState>, row: i32, col: i32, ptr: i32, len: i32, color: i32| -> i32 {
            if row < 0 || col < 0 { return -2; }
            let Some(s) = read_guest_string(&caller, ptr, len) else { return -2; };
            vga::write_str_at(row as usize, col as usize, &s, ColorCode::from_byte(color as u8)) as i32
        })?;

        linker.func_wrap(ns, "clear_row", |_caller: Caller<'_, HostState>, row: i32, color: i32| {
            if row < 0 { return; }
            vga::clear_row(row as usize, ColorCode::from_byte(color as u8));
        })?;

        linker.func_wrap(ns, "clear_screen", |_caller: Caller<'_, HostState>| {
            vga::clear_screen();
        })?;

        linker.func_wrap(ns, "move_cursor", |_caller: Caller<'_, HostState>, row: i32, col: i32| {
            if row < 0 || col < 0 { return; }
            vga::move_cursor(row as usize, col as usize);
        })?;

        // Returns (color << 8) | ascii_char for the cell at (row, col), or -1 if out of bounds.
        linker.func_wrap(ns, "read_at", |_caller: Caller<'_, HostState>, row: i32, col: i32| -> i32 {
            if row < 0 || col < 0 { return -1; }
            match vga::read_at(row as usize, col as usize) {
                Some(ch) => ((ch.color_code.to_byte() as i32) << 8) | ch.ascii_character as i32,
                None => -1,
            }
        })?;

        Ok(())
    }
}
