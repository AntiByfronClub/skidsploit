#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

mod skidapi;

use std::env;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sysinfo::System;
use tauri::{Emitter, State};

#[derive(Serialize, Clone)]
pub struct RbxProcess {
    pub name: String,
    pub pid: u32,
    pub path: String,
}

#[derive(Serialize, Clone)]
struct SessionPayload {
    pid: u32,
}

#[derive(Serialize, Clone)]
struct SessionOutputPayload {
    pid: u32,
    output_type: i32,
    output: String,
}

#[derive(Serialize, Clone)]
struct ConnectionInfo {
    id: u32,
    name: String,
}

#[derive(Deserialize)]
struct SettingsArg {
    settings: String,
}

fn settingsPath() -> std::path::PathBuf {
    skidapi::exeDir().join("settings").join("settings.json")
}

fn readSettings() -> Value {
    let contents = fs::read_to_string(settingsPath()).unwrap_or_default();
    serde_json::from_str(&contents).unwrap_or(Value::Object(Default::default()))
}

fn writeSettings(json: &Value) {
    let path = settingsPath();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, json.to_string());
}

#[tauri::command]
fn get_exe_dir() -> String {
    skidapi::exeDir().to_string_lossy().to_string()
}

#[tauri::command]
fn rename_file(old_path: String, new_path: String) -> Result<(), String> {
    fs::rename(&old_path, &new_path).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    fs::remove_file(&path).map_err(|error| error.to_string())
}

#[tauri::command]
fn open_folder(path: String) {
    let _ = open::that(path);
}

#[tauri::command]
fn open_synz_folder() {
    let path = skidapi::exeDir().join("workspace");
    let _ = fs::create_dir_all(&path);
    let _ = open::that(path);
}

#[tauri::command]
fn launch_robloxproc() {
    let _ = open::that("roblox-player://");
}

#[tauri::command]
fn create_lnk_folders() -> &'static str {
    let base = skidapi::exeDir();
    let autoexec = base.join("autoexec");
    let workspace = base.join("workspace");

    let _ = fs::create_dir_all(&autoexec);
    let _ = fs::create_dir_all(&workspace);

    let shortcuts = [(&autoexec, "skid-autoexec.lnk"), (&workspace, "skid-workspace.lnk")];

    for (target, lnkName) in &shortcuts {
        let lnkPath = base.join(lnkName);
        let targetStr = match target.to_str() {
            Some(value) => value,
            None => return "Invalid path encoding.",
        };
        let lnkStr = match lnkPath.to_str() {
            Some(value) => value,
            None => return "Invalid .lnk path encoding.",
        };

        let script = format!(
            "$ws = New-Object -ComObject WScript.Shell; \
             $s = $ws.CreateShortcut('{lnk}'); \
             $s.TargetPath = '{target}'; \
             $s.Save()",
            lnk = lnkStr,
            target = targetStr,
        );

        let ok = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);

        if !ok {
            return "Failed to create .lnk file.";
        }
    }
    "Created .lnk of workspace and autoexec on your ui directory."
}

#[tauri::command]
fn get_setting_output_redirection() -> bool {
    readSettings()["output_redirection"].as_bool().unwrap_or(false)
}

#[tauri::command]
fn set_setting_output_redirection(value: bool) {
    let mut json = readSettings();
    json["output_redirection"] = Value::Bool(value);
    writeSettings(&json);
}

#[tauri::command]
fn get_setting_topmost() -> bool {
    readSettings()["topmost"].as_bool().unwrap_or(false)
}

#[tauri::command]
fn set_setting_topmost(value: bool) {
    let mut json = readSettings();
    json["topmost"] = Value::Bool(value);
    writeSettings(&json);
}

#[tauri::command]
fn get_settings() -> String {
    readSettings()["settings"]
        .as_str()
        .unwrap_or("1,1,0")
        .to_string()
}

#[tauri::command]
fn set_settings(arg: SettingsArg) {
    let mut json = readSettings();
    json["settings"] = Value::String(arg.settings);
    writeSettings(&json);
}

#[tauri::command]
fn get_roblox_pids(sys: State<Mutex<System>>) -> Vec<RbxProcess> {
    let mut sys = sys.lock().unwrap();
    skidapi::robloxProcesses(&mut sys)
        .into_iter()
        .map(|(pid, path)| RbxProcess {
            name: skidapi::robloxProcessName.to_string(),
            pid,
            path: path.to_string_lossy().to_string(),
        })
        .collect()
}

#[tauri::command]
fn is_synz(pid: u32) -> bool {
    skidapi::probeAttached(pid)
}

#[tauri::command]
fn execute_script(script: String, pid: u32) {
    let target = if pid == 0 {
        skidapi::primaryPid().unwrap_or(0)
    } else {
        pid
    };

    if target == 0 {
        skidapi::emitOutput(pid, 3, "no roblox session to execute on".to_string());
        return;
    }

    match skidapi::executeScript(&script, target) {
        Ok(()) => skidapi::emitOutput(target, 1, "Executed.".to_string()),
        Err(error) => skidapi::emitOutput(target, 3, format!("execute failed: {}", error)),
    }
}

#[tauri::command]
fn get_connections() -> Vec<ConnectionInfo> {
    skidapi::instances()
        .into_iter()
        .map(|pid| ConnectionInfo {
            id: pid,
            name: pid.to_string(),
        })
        .collect()
}

#[tauri::command]
fn get_instances() -> Vec<u32> {
    skidapi::instances()
}

#[tauri::command]
fn start_instances_timer(app: tauri::AppHandle) {
    let appAdded = app.clone();
    skidapi::onSessionAdded(move |pid| {
        let _ = appAdded.emit("session_added", SessionPayload { pid });
    });

    let appRemoved = app.clone();
    skidapi::onSessionRemoved(move |pid| {
        let _ = appRemoved.emit("session_removed", SessionPayload { pid });
    });

    skidapi::onSessionOutput(move |pid, outputType, output| {
        let _ = app.emit(
            "session_output_debug",
            SessionOutputPayload {
                pid,
                output_type: outputType,
                output,
            },
        );
    });

    skidapi::startTimer();
}

#[tauri::command]
fn stop_instances_timer() {
    skidapi::stopTimer();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let sys = Mutex::new(System::new_all());

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(sys)
        .invoke_handler(tauri::generate_handler![
            get_exe_dir,
            rename_file,
            delete_file,
            open_folder,
            open_synz_folder,
            launch_robloxproc,
            create_lnk_folders,
            get_setting_output_redirection,
            set_setting_output_redirection,
            get_setting_topmost,
            set_setting_topmost,
            get_settings,
            set_settings,
            get_roblox_pids,
            is_synz,
            execute_script,
            get_connections,
            get_instances,
            start_instances_timer,
            stop_instances_timer,
        ])
        .setup(|_app| {
            let exePath = env::current_exe().expect("GetExecutable Fail.");
            let exeDir = exePath.parent().expect("GetDir Fail.").to_path_buf();
            let _ = env::set_current_dir(&exeDir);

            for folder in &["scripts", "settings", "workspace", "autoexec"] {
                let path = exeDir.join(folder);
                if !path.exists() {
                    let _ = fs::create_dir_all(path);
                }
            }

            if !Path::new(&skidapi::exeDir().join("settings").join("settings.json")).exists() {
                writeSettings(&Value::Object(Default::default()));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_, event| {
        if let tauri::RunEvent::Exit = event {
            skidapi::stopTimer();
            skidapi::killChildren();
        }
    });
}
