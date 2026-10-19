pub mod datamodel {
    pub const pointer: usize = 0x8dc8000;
    pub const real: usize = 0x1f8;
    pub const scriptContext: usize = 0x440;
    pub const requireBypass: usize = 0xa81;
    pub const gameLoaded: usize = 0x5d0;
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
        pub const pointer: usize = 0x128;
        pub const coreScript: usize = 0x158;
        pub const detection: usize = 0x158 - 0x4;
    }
    pub const pointer: usize = 0x18;
    pub const size: usize = 0x28;
}
