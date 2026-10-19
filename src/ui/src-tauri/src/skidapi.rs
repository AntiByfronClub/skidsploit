use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use sysinfo::{ProcessesToUpdate, System};

pub const bridgeBase: &str = "http://127.0.0.1:6767";
pub const robloxProcessName: &str = "RobloxPlayerBeta.exe";

pub struct SkidSession {
    pub pid: u32,
    pub child: Mutex<Option<Child>>,
    pub attached: AtomicBool,
    pub announced: AtomicBool,
    pub autoexecDone: AtomicBool,
    pub spawnWarned: AtomicBool,
}

type pidCallback = Arc<dyn Fn(u32) + Send + Sync>;
type outputCallback = Arc<dyn Fn(u32, i32, String) + Send + Sync>;

static SESSIONS: OnceLock<Mutex<HashMap<u32, Arc<SkidSession>>>> = OnceLock::new();
static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
static TIMER_RUNNING: AtomicBool = AtomicBool::new(false);
static ADDED_EVENTS: OnceLock<Mutex<Vec<pidCallback>>> = OnceLock::new();
static REMOVED_EVENTS: OnceLock<Mutex<Vec<pidCallback>>> = OnceLock::new();
static OUTPUT_EVENTS: OnceLock<Mutex<Vec<outputCallback>>> = OnceLock::new();

fn sessionsMap() -> &'static Mutex<HashMap<u32, Arc<SkidSession>>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn addedEvents() -> &'static Mutex<Vec<pidCallback>> {
    ADDED_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn removedEvents() -> &'static Mutex<Vec<pidCallback>> {
    REMOVED_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn outputEvents() -> &'static Mutex<Vec<outputCallback>> {
    OUTPUT_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn onSessionAdded<F: Fn(u32) + Send + Sync + 'static>(callback: F) {
    addedEvents().lock().unwrap().push(Arc::new(callback));
}

pub fn onSessionRemoved<F: Fn(u32) + Send + Sync + 'static>(callback: F) {
    removedEvents().lock().unwrap().push(Arc::new(callback));
}

pub fn onSessionOutput<F: Fn(u32, i32, String) + Send + Sync + 'static>(callback: F) {
    outputEvents().lock().unwrap().push(Arc::new(callback));
}

pub fn emitOutput(pid: u32, outputType: i32, output: String) {
    let events = outputEvents().lock().unwrap().clone();
    for event in events {
        event(pid, outputType, output.clone());
    }
}

fn emitAdded(pid: u32) {
    let events = addedEvents().lock().unwrap().clone();
    for event in events {
        event(pid);
    }
}

fn emitRemoved(pid: u32) {
    let events = removedEvents().lock().unwrap().clone();
    for event in events {
        event(pid);
    }
}

pub fn exeDir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))
        .unwrap_or_default()
}

fn httpClient() -> &'static reqwest::blocking::Client {
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .expect("failed to build bridge client")
    })
}

pub fn robloxProcesses(sys: &mut System) -> Vec<(u32, PathBuf)> {
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes()
        .iter()
        .filter(|(_, process)| process.name().eq_ignore_ascii_case(robloxProcessName))
        .map(|(pid, process)| {
            (
                pid.as_u32(),
                process.exe().map(|path| path.to_path_buf()).unwrap_or_default(),
            )
        })
        .collect()
}

pub fn probeAttached(pid: u32) -> bool {
    let url = format!("{}/execute?pid={}", bridgeBase, pid);
    match httpClient().post(url).body(String::new()).send() {
        Ok(response) => response
            .text()
            .map(|body| body.trim() == "empty script")
            .unwrap_or(false),
        Err(_) => false,
    }
}

pub fn executeScript(script: &str, pid: u32) -> Result<(), String> {
    let target = if pid == 0 {
        primaryPid().ok_or_else(|| "no roblox session".to_string())?
    } else {
        pid
    };

    let url = format!("{}/execute?pid={}", bridgeBase, target);
    match httpClient().post(url).body(script.to_string()).send() {
        Ok(response) => {
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            if status == 200 {
                Ok(())
            } else if body.is_empty() {
                Err(format!("bridge returned {}", status))
            } else {
                Err(format!("bridge {}: {}", status, body))
            }
        }
        Err(_) => Err("bridge unreachable, executor not running yet".to_string()),
    }
}

pub fn primaryPid() -> Option<u32> {
    let map = sessionsMap().lock().unwrap();
    map.values()
        .find(|session| session.attached.load(Ordering::SeqCst))
        .or_else(|| map.values().min_by_key(|session| session.pid))
        .map(|session| session.pid)
}

pub fn instances() -> Vec<u32> {
    let map = sessionsMap().lock().unwrap();
    map.values()
        .filter(|session| session.attached.load(Ordering::SeqCst))
        .map(|session| session.pid)
        .collect()
}

fn childAlive(session: &Arc<SkidSession>) -> bool {
    let mut guard = session.child.lock().unwrap();
    match guard.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(None)),
        None => false,
    }
}

fn markAttached(session: &Arc<SkidSession>) {
    if session.attached.swap(true, Ordering::SeqCst) {
        return;
    }
    if !session.announced.swap(true, Ordering::SeqCst) {
        emitAdded(session.pid);
    }
    runAutoexec(session);
}

fn runAutoexec(session: &Arc<SkidSession>) {
    if session.autoexecDone.swap(true, Ordering::SeqCst) {
        return;
    }

    let dir = exeDir().join("autoexec");
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "lua" | "luau" | "txt"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();

    for file in files {
        let name = file
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        let Ok(source) = fs::read_to_string(&file) else {
            continue;
        };
        match executeScript(&source, session.pid) {
            Ok(()) => emitOutput(session.pid, 1, format!("autoexec: executed {}", name)),
            Err(error) => emitOutput(session.pid, 3, format!("autoexec {} failed: {}", name, error)),
        }
    }
}

fn spawnExecutor(session: &Arc<SkidSession>) -> bool {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            if !session.spawnWarned.swap(true, Ordering::SeqCst) {
                emitOutput(session.pid, 3, format!("failed to locate ui executable: {}", error));
            }
            return false;
        }
    };

    let mut command = Command::new(&exe);
    command
        .arg(session.pid.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            if !session.spawnWarned.swap(true, Ordering::SeqCst) {
                emitOutput(session.pid, 3, format!("failed to start executor: {}", error));
            }
            return false;
        }
    };

    if let Some(stdout) = child.stdout.take() {
        let sessionRef = Arc::clone(session);
        let pid = session.pid;
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.contains("Injected in") {
                    markAttached(&sessionRef);
                }
                let kind = if trimmed.contains("[Error]") { 3 } else { 0 };
                emitOutput(pid, kind, trimmed.to_string());
            }
        });
    }

    *session.child.lock().unwrap() = Some(child);
    true
}

fn tick(sys: &mut System) {
    let pids: Vec<u32> = robloxProcesses(sys).into_iter().map(|(pid, _)| pid).collect();

    let mut removed = Vec::new();
    {
        let mut map = sessionsMap().lock().unwrap();

        let stale: Vec<u32> = map
            .keys()
            .filter(|pid| !pids.contains(pid))
            .cloned()
            .collect();
        for pid in stale {
            if let Some(session) = map.remove(&pid) {
                if let Some(mut child) = session.child.lock().unwrap().take() {
                    let _ = child.kill();
                }
                if session.announced.load(Ordering::SeqCst) {
                    removed.push(pid);
                }
            }
        }

        for pid in &pids {
            if !map.contains_key(pid) {
                let session = Arc::new(SkidSession {
                    pid: *pid,
                    child: Mutex::new(None),
                    attached: AtomicBool::new(false),
                    announced: AtomicBool::new(false),
                    autoexecDone: AtomicBool::new(false),
                    spawnWarned: AtomicBool::new(false),
                });
                map.insert(*pid, session);
            }
        }
    }

    for pid in removed {
        emitOutput(pid, 2, "roblox closed".to_string());
        emitRemoved(pid);
    }

    let sessionsList: Vec<Arc<SkidSession>> =
        sessionsMap().lock().unwrap().values().cloned().collect();

    if !sessionsList.iter().any(childAlive) {
        if let Some(target) = sessionsList.iter().min_by_key(|session| session.pid) {
            spawnExecutor(target);
        }
    }

    for session in &sessionsList {
        if !session.attached.load(Ordering::SeqCst)
            && childAlive(session)
            && probeAttached(session.pid)
        {
            markAttached(session);
        }
    }
}

pub fn startTimer() {
    if TIMER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(|| {
        let mut sys = System::new();
        while TIMER_RUNNING.load(Ordering::SeqCst) {
            tick(&mut sys);
            thread::sleep(Duration::from_millis(1000));
        }
    });
}

pub fn stopTimer() {
    TIMER_RUNNING.store(false, Ordering::SeqCst);
}

pub fn killChildren() {
    let sessionsList: Vec<Arc<SkidSession>> =
        sessionsMap().lock().unwrap().values().cloned().collect();
    for session in sessionsList {
        if let Some(mut child) = session.child.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}
