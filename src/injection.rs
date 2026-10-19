use std::time::{Duration, Instant};

use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};

use crate::bridge;
use crate::bytecode;
use crate::helpers;
use crate::offsets;
use crate::rbx;
use crate::skid;

pub const mainScript: &str = "main.luau";
pub const init: &str = include_str!("env/init.luau");

const injectAttempts: usize = 3;
const initTimeout: Duration = Duration::from_secs(4);

const robloxClients: [&str; 1] = ["robloxplayerbeta.exe"];

pub fn findRobloxPid() -> Option<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut result = None;
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while ok {
        let name = String::from_utf16_lossy(
            &entry.szExeFile[..entry.szExeFile.iter().position(|&v| v == 0).unwrap_or(entry.szExeFile.len())],
        )
        .to_lowercase();

        if robloxClients.contains(&name.as_str()) {
            result = Some(entry.th32ProcessID);
            break;
        }
        ok = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }

    unsafe { CloseHandle(snapshot) };
    result
}

fn readMainScript() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    std::fs::read_to_string(dir.join(mainScript)).ok()
}

fn pushMainScript(executedFor: &mut Option<usize>, datamodel: usize) {
    if *executedFor == Some(datamodel) {
        return;
    }
    *executedFor = Some(datamodel);

    if let Some(source) = readMainScript() {
        bridge::requestExecution(source);
    }
}

fn waitGameLoaded(datamodel: &rbx::instance) {
    let k = skid::skid();
    while k.read::<u8>(datamodel.address + offsets::datamodel::gameLoaded) == 0 {
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn waitPlayersReady(datamodel: &rbx::instance) -> rbx::instance {
    loop {
        let players = datamodel.findFirstChild("Players");
        if players.nonZero() && !players.children().is_empty() {
            return players;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn waitForInit(datamodel: &rbx::instance) -> bool {
    let deadline = Instant::now() + initTimeout;
    while Instant::now() < deadline {
        if datamodel.find(&["CoreGui", "skid"]).nonZero() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn trigger(jest: &rbx::instance, plm: &rbx::instance, bytes: &[u8], hwnd: isize) {
    let k = skid::skid();

    k.write(jest.address + offsets::script::moduleScript::moduleState, &0i32);

    let revert = jest.setBytecode(bytes);

    plm.spoofTo(*jest);

    let oldForeground = helpers::getForegroundWindow();
    helpers::forceForeground(hwnd);
    std::thread::sleep(Duration::from_millis(50));
    helpers::pressEscape();
    std::thread::sleep(Duration::from_millis(50));

    if let Some(revert) = revert {
        revert();
    }

    plm.spoofTo(*plm);
    helpers::pressEscape();

    if oldForeground != 0 && oldForeground != hwnd {
        helpers::forceForeground(oldForeground);
    }
}

fn inject(datamodel: &rbx::instance, clientPid: &str) -> bool {
    let k = skid::skid();

    let hwnds = helpers::getHwnds(k.pid);
    let hwnd = hwnds.first().copied().unwrap_or(0);

    let scriptContext = datamodel.getScriptContext();
    scriptContext.setBypass(true);

    waitGameLoaded(datamodel);
    waitPlayersReady(datamodel);

    let luauInit = init.replacen("%client_pid%", clientPid, 1);

    let bytes = match bytecode::prepare(&luauInit, false) {
        Ok(value) => value,
        Err(_) => return false,
    };

    std::thread::sleep(Duration::from_millis(2000));

    for _ in 0..injectAttempts {
        let plm = datamodel.find(&[
            "CoreGui",
            "RobloxGui",
            "Modules",
            "PlayerList",
            "PlayerListManager",
        ]);
        let jest = datamodel.find(&[
            "CorePackages",
            "Packages",
            "_Index",
            "CollisionMatchers2D",
            "CollisionMatchers2D",
            "Jest",
        ]);

        if !plm.nonZero() || !jest.nonZero() {
            continue;
        }

        trigger(&jest, &plm, &bytes, hwnd);

        if waitForInit(datamodel) {
            bridge::markAttached(true);
            println!("Injected in {}.", k.pid);
            return true;
        }
    }

    false
}

pub fn watch() {
    let clientPid = bridge::clientPid();
    let mut lastDatamodel = 0usize;
    let mut firstPrint = true;
    let mut executedFor: Option<usize> = None;

    loop {
        let k = skid::skid();
        let fake = k.read::<usize>(k.baseAddr + offsets::datamodel::pointer);
        let real = if fake != 0 {
            k.read::<usize>(fake + offsets::datamodel::real)
        } else {
            0
        };

        if real != lastDatamodel {
            if real != 0 {
                let datamodel = rbx::instance::new(real);
                let name = datamodel.name();

                if !name.is_empty() && name != "LuaApp" {
                    if !datamodel.find(&["CoreGui", &clientPid]).nonZero() {
                        if inject(&datamodel, &clientPid) {
                            pushMainScript(&mut executedFor, real);
                        }
                    } else {
                        bridge::markAttached(true);
                        pushMainScript(&mut executedFor, real);
                    }
                } else if name == "LuaApp" {
                    let scriptContext = datamodel.getScriptContext();
                    scriptContext.setBypass(false);
                    bridge::markAttached(false);
                }
            } else {
                if !firstPrint {
                    crate::logErr!("roblox closed");
                }
                executedFor = None;
            }

            lastDatamodel = real;
            firstPrint = false;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
