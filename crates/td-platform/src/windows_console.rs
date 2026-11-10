//! Narrow Windows console exception (MDG §8.2), IM-28.
//!
//! GUI-subsystem executables create no console. AttachConsole reuses an existing parent
//! console; it never creates one. Preserve inherited file/pipe handles across attachment.
//! API contracts: https://learn.microsoft.com/en-us/windows/console/attachconsole
//! and https://learn.microsoft.com/en-us/windows/console/getstdhandle .
use std::ffi::c_void;

type Handle = *mut c_void;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
    fn GetStdHandle(kind: u32) -> Handle;
    fn SetStdHandle(kind: u32, handle: Handle) -> i32;
    fn GetFileType(handle: Handle) -> u32;
}

pub(super) fn attach_parent() -> bool {
    // SAFETY: The ABI and fixed-width types match the documented Win32 signatures.
    // These calls only inspect/replace this process's standard handle table or attach this
    // process to its parent console. Handles are borrowed, never dereferenced or closed;
    // GetFileType rejects invalid handles. No caller-provided pointers or memory are used.
    // This runs once at entry to main, before threads and standard-stream initialization.
    unsafe {
        let kinds = [(-10i32) as u32, (-11i32) as u32, (-12i32) as u32];
        let inherited = kinds.map(|kind| {
            let handle = GetStdHandle(kind);
            (GetFileType(handle) != 0).then_some(handle)
        });
        let _ = AttachConsole(u32::MAX); // ATTACH_PARENT_PROCESS; failure is normal in Explorer.
        for (kind, handle) in kinds.into_iter().zip(inherited) {
            if let Some(handle) = handle {
                let _ = SetStdHandle(kind, handle);
            }
        }
        kinds[1..]
            .iter()
            .any(|&kind| GetFileType(GetStdHandle(kind)) != 0)
    }
}
