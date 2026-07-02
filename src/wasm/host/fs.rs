use wasmi::{Caller, Error, Linker};
use crate::wasm::state::HostState;
use super::{read_guest_bytes, read_guest_string, write_to_guest, HostModule};

pub struct FsModule;

// Return codes shared by every function in this module:
//   -1  operation failed (not found / write / mkdir / delete failed)
//   -2  invalid data pointer or length
//   -3  invalid (non-UTF-8) path pointer or length

impl HostModule for FsModule {
    fn namespace(&self) -> &'static str { "os::fs" }

    fn register(&self, linker: &mut Linker<HostState>) -> Result<(), Error> {
        let ns = self.namespace();

        linker.func_wrap(ns, "read_file", |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32, buf_ptr: i32, buf_max_len: i32| -> i32 {
            let Some(path) = read_guest_string(&caller, path_ptr, path_len) else { return -3; };
            let Some(data) = crate::fs::read_file(&path) else { return -1; };
            write_to_guest(&mut caller, buf_ptr, buf_max_len, &data)
        })?;

        linker.func_wrap(ns, "write_file", |caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32, data_ptr: i32, data_len: i32| -> i32 {
            let Some(path) = read_guest_string(&caller, path_ptr, path_len) else { return -3; };
            let Some(data) = read_guest_bytes(&caller, data_ptr, data_len) else { return -2; };
            if crate::fs::write_file(&path, &data) { 0 } else { -1 }
        })?;

        linker.func_wrap(ns, "append_file", |caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32, data_ptr: i32, data_len: i32| -> i32 {
            let Some(path) = read_guest_string(&caller, path_ptr, path_len) else { return -3; };
            let Some(data) = read_guest_bytes(&caller, data_ptr, data_len) else { return -2; };
            if crate::fs::append_file(&path, &data) { 0 } else { -1 }
        })?;

        linker.func_wrap(ns, "delete_file", |caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let Some(path) = read_guest_string(&caller, path_ptr, path_len) else { return -3; };
            if crate::fs::delete_file(&path) { 0 } else { -1 }
        })?;

        linker.func_wrap(ns, "create_dir", |caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let Some(path) = read_guest_string(&caller, path_ptr, path_len) else { return -3; };
            if crate::fs::create_dir(&path) { 0 } else { -1 }
        })?;

        linker.func_wrap(ns, "delete_dir", |caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
            let Some(path) = read_guest_string(&caller, path_ptr, path_len) else { return -3; };
            if crate::fs::delete_dir(&path) { 0 } else { -1 }
        })?;

        Ok(())
    }
}
