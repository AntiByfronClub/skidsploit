-- Main Hook Remove This Comment In Build For Me To Keep Track -&
use std::ffi::c_void-
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, MODULEENTRY32W,
};
use windows_sys::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

use crate::syscalls;

pub struct Skid {
    pub pid: u32,
    pub handle: *mut c_void,
    pub baseAddr: usize,
}

unsafe impl Send for Skid {}
unsafe impl Sync for Skid {}

static SKID: OnceLock<Skid> = OnceLock::new();

pub fn skid() -> &'static Skid {
    SKID.get().expect("skid not attached")
}

pub fn attach(targetPid: u32) -> bool {
    syscalls::initialize();

    let desiredAccess = PROCESS_QUERY_INFORMATION | PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE;
    let mut processHandle: *mut c_void = std::ptr::null_mut();
    let mut attributes = syscalls::objectAttributes {
        length: std::mem::size_of::<syscalls::objectAttributes>() as u32,
        rootDirectory: std::ptr::null_mut(),
        objectName: std::ptr::null_mut(),
        attributes: 0,
        securityDescriptor: std::ptr::null_mut(),
        securityQualityOfService: std::ptr::null_mut(),
    };
    let mut cid = syscalls::clientId {
        uniqueProcess: targetPid as usize as *mut c_void,
        uniqueThread: std::ptr::null_mut(),
    };

    let status = unsafe {
        (syscalls::stubs().openProcess)(&mut processHandle, desiredAccess, &mut attributes, &mut cid)
    };

    if status < 0 || processHandle.is_null() {
        processHandle = unsafe { OpenProcess(desiredAccess, 0, targetPid) };
    }

    if processHandle.is_null() {
        return false;
    }

    let baseAddr = moduleBase(targetPid);

    SKID
        .set(Skid {
            pid: targetPid,
            handle: processHandle,
            baseAddr,
        })
        .is_ok()
}

fn moduleBase(targetPid: u32) -> usize {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, targetPid) };

    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return 0;
    }

    let mut result = 0usize;
    let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;

    if unsafe { Module32FirstW(snapshot, &mut entry) } != 0 {
        result = entry.modBaseAddr as usize;
    }

    unsafe { CloseHandle(snapshot) };
    result
}

impl Skid {
    pub fn readRaw(&self, address: usize, buffer: *mut u8, size: usize) -> bool {
        if self.handle.is_null() || address == 0 || size == 0 {
            return false;
        }

        let mut bytesRead: usize = 0;
        let status = unsafe {
            (syscalls::stubs().readVirtualMemory)(
                self.handle,
                address as *const c_void,
                buffer as *mut c_void,
                size,
                &mut bytesRead,
            )
        };

        status >= 0 && bytesRead == size
    }

    pub fn writeRaw(&self, address: usize, buffer: *const u8, size: usize) -> bool {
        if self.handle.is_null() || address == 0 || size == 0 {
            return false;
        }

        let mut bytesWritten: usize = 0;
        let status = unsafe {
            (syscalls::stubs().writeVirtualMemory)(
                self.handle,
                address as *mut c_void,
                buffer as *const c_void,
                size,
                &mut bytesWritten,
            )
        };

        status >= 0 && bytesWritten == size
    }

    pub fn read<T: Copy + Default>(&self, address: usize) -> T {
        let mut value = T::default();
        self.readRaw(address, &mut value as *mut T as *mut u8, std::mem::size_of::<T>());
        value
    }

    pub fn write<T: Copy>(&self, address: usize, value: &T) -> bool {
        self.writeRaw(address, value as *const T as *const u8, std::mem::size_of::<T>())
    }

    pub fn readBytes(&self, address: usize, size: usize) -> Vec<u8> {
        let mut out = vec![0u8; size];
        if !self.readRaw(address, out.as_mut_ptr(), size) {
            out.clear();
        }
        out
    }

    pub fn readString(&self, mut address: usize, maxLen: usize) -> String {
        let mut out: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 64];

        while out.len() < maxLen {
            let toPageEnd = 0x1000 - (address & 0xFFF);
            let count = chunk.len().min(toPageEnd).min(maxLen - out.len());
            if !self.readRaw(address, chunk.as_mut_ptr(), count) {
                break;
            }

            let mut terminated = false;
            for &byte in &chunk[..count] {
                if byte == 0 {
                    terminated = true;
                    break;
                }
                out.push(byte);
            }

            if terminated {
                break;
            }
            address += count;
        }

        String::from_utf8_lossy(&out).into_owned()
    }

    pub fn allocate(&self, size: usize) -> usize {
        if self.handle.is_null() {
            return 0;
        }
        unsafe {
            VirtualAllocEx(
                self.handle,
                std::ptr::null_mut(),
                size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            ) as usize
        }
    }

    pub fn free(&self, address: usize) -> bool {
        if self.handle.is_null() || address == 0 {
            return false;
        }
        unsafe { VirtualFreeEx(self.handle, address as *mut c_void, 0, MEM_RELEASE) != 0 }
    }
}
