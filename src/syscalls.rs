use std::ffi::c_void;
use std::sync::OnceLock;

use windows_sys::Win32::System::Memory::{
    VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};

pub const readVirtualMemorySyscall: u32 = 63;
pub const writeVirtualMemorySyscall: u32 = 58;
pub const openProcessSyscall: u32 = 38;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct clientId {
    pub uniqueProcess: *mut c_void,
    pub uniqueThread: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct objectAttributes {
    pub length: u32,
    pub rootDirectory: *mut c_void,
    pub objectName: *mut c_void,
    pub attributes: u32,
    pub securityDescriptor: *mut c_void,
    pub securityQualityOfService: *mut c_void,
}

pub type ntReadVirtualMemoryFn = unsafe extern "C" fn(
    processHandle: *mut c_void,
    baseAddress: *const c_void,
    buffer: *mut c_void,
    bufferSize: usize,
    bytesRead: *mut usize,
) -> i32;

pub type ntWriteVirtualMemoryFn = unsafe extern "C" fn(
    processHandle: *mut c_void,
    baseAddress: *mut c_void,
    buffer: *const c_void,
    bufferSize: usize,
    bytesWritten: *mut usize,
) -> i32;

pub type ntOpenProcessFn = unsafe extern "C" fn(
    processHandle: *mut *mut c_void,
    desiredAccess: u32,
    objectAttributes: *mut objectAttributes,
    clientId: *mut clientId,
) -> i32;

#[derive(Clone, Copy)]
pub struct stubs {
    pub readVirtualMemory: ntReadVirtualMemoryFn,
    pub writeVirtualMemory: ntWriteVirtualMemoryFn,
    pub openProcess: ntOpenProcessFn,
}

static STUBS: OnceLock<stubs> = OnceLock::new();

pub fn stubs() -> &'static stubs {
    STUBS.get().expect("syscall not initialized")
}

fn installStub(base: *mut u8, offset: usize, ssn: u32) -> usize {
    let slotBytes = {
        let ssnBytes = ssn.to_le_bytes();
        [
            0x4C, 0x8B, 0xD1, 0xB8, ssnBytes[0], ssnBytes[1], ssnBytes[2], ssnBytes[3], 0x0F, 0x05,
            0xC3, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC,
        ]
    };

    unsafe {
        std::ptr::copy_nonoverlapping(slotBytes.as_ptr(), base.add(offset), slotBytes.len());
    }

    base as usize + offset
}

pub fn initialize() {
    if STUBS.get().is_some() {
        return;
    }

    let base = unsafe {
        VirtualAlloc(
            std::ptr::null_mut(),
            4096,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    };

    if base.is_null() {
        panic!("failed to allocate syscall page");
    }

    let base = base as *mut u8;

    let stubTable = stubs {
        readVirtualMemory: unsafe {
            std::mem::transmute::<usize, ntReadVirtualMemoryFn>(installStub(base, 0, readVirtualMemorySyscall))
        },
        writeVirtualMemory: unsafe {
            std::mem::transmute::<usize, ntWriteVirtualMemoryFn>(installStub(base, 16, writeVirtualMemorySyscall))
        },
        openProcess: unsafe {
            std::mem::transmute::<usize, ntOpenProcessFn>(installStub(base, 32, openProcessSyscall))
        },
    };

    let _ = STUBS.set(stubTable);
}
