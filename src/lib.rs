#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

pub mod bytecode;
pub mod helpers;
pub mod injection;
pub mod logging;
pub mod syscalls;

#[path = "bridge/bridge.rs"]
pub mod bridge;

#[path = "roblox/offsets.rs"]
pub mod offsets;

#[path = "roblox/rbx.rs"]
pub mod rbx;

pub mod skid;

pub fn runExecutor(pid: u32) {
    if !skid::attach(pid) {
        return;
    }

    bridge::start(pid.to_string());
    injection::watch();
}
