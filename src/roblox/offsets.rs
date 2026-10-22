use windows_sys::Win32::System::LibraryLoader::GetModuleHandleA;

#[inline(always)]
pub fn rebase(offset: usize) -> usize {
    unsafe {
        let base = GetModuleHandleA(std::ptr::null()) as usize;
        base + offset
    }
}

pub mod datamodel {
    pub const pointer: usize = 0x8F59810;
    pub const real: usize = 0x1F8;
    pub const scriptContext: usize = 0x440;
    pub const requireBypass: usize = 0x988;
    pub const gameLoaded: usize = 0x5C8;
}

pub mod instance {
    pub const name: usize = 0x70;
    pub const nameStr: usize = 0x8;
    pub const classDesc: usize = 0x18;
    pub const className: usize = 0x8;
    pub const childrenStart: usize = 0x78;
    pub const childrenEnd: usize = 0x8;
}

pub mod script {
    pub mod moduleScript {
        pub const moduleState: usize = 0x160;
        pub const pointer: usize = 0x120;
        pub const coreScript: usize = 0x158;
        pub const detection: usize = 0x154;
    }

    pub const pointer: usize = 0x10;
    pub const size: usize = 0x20;
}
