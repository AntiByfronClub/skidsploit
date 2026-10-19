#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use skidsploit::{bridge, helpers, injection, logDebug, logErr, skid};

fn main() {
    let requestedPid = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<u32>().ok());

    helpers::setConsoleTitle("skidsploit (external executor for roblox)");
    helpers::clearConsole();

    let pid = match requestedPid.or_else(injection::findRobloxPid) {
        Some(value) => value,
        None => {
            logErr!("Roblox process not found (RobloxPlayerBeta.exe), start Roblox first");
            return;
        }
    };

    logDebug!("Found Roblox PID {}", pid);

    if !skid::attach(pid) {
        logErr!("Failed to attach to Roblox (PID {})", pid);
        return;
    }
    logDebug!("Attached successfully!");

    let base = skid::skid().baseAddr;
    if base != 0 {
        logDebug!("Found Roblox base address (0x{:x})", base);
    }

    bridge::start(pid.to_string());

    injection::watch();
}
