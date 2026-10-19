use windows_sys::Win32::System::Console::{
    CONSOLE_SCREEN_BUFFER_INFO, BACKGROUND_BLUE, BACKGROUND_GREEN, BACKGROUND_RED, FOREGROUND_BLUE,
    FOREGROUND_GREEN, FOREGROUND_INTENSITY, FOREGROUND_RED, GetConsoleScreenBufferInfo, GetStdHandle,
    SetConsoleTextAttribute, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::Foundation::SYSTEMTIME;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum logType {
    Debug,
    Info,
    Error,
}

fn kindColor(kind: logType) -> u16 {
    match kind {
        logType::Debug => {
            FOREGROUND_RED | FOREGROUND_GREEN | FOREGROUND_BLUE | FOREGROUND_INTENSITY | BACKGROUND_BLUE
        }
        logType::Info => {
            FOREGROUND_RED | FOREGROUND_GREEN | FOREGROUND_BLUE | FOREGROUND_INTENSITY | BACKGROUND_GREEN
        }
        logType::Error => {
            FOREGROUND_RED | FOREGROUND_GREEN | FOREGROUND_BLUE | FOREGROUND_INTENSITY | BACKGROUND_RED
        }
    }
}

fn timestamp() -> String {
    let mut time: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut time) };
    format!("{:02}:{:02}:{:02}", time.wHour, time.wMinute, time.wSecond)
}

pub fn log(kind: logType, message: &str) {
    if kind != logType::Error {
        return;
    }

    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
    let hasInfo = unsafe { GetConsoleScreenBufferInfo(handle, &mut info) } != 0;
    let saved = if hasInfo { info.wAttributes } else { kindColor(logType::Debug) };

    unsafe {
        SetConsoleTextAttribute(handle, kindColor(kind));
    }

    let prefix = match kind {
        logType::Debug => "[Debug]",
        logType::Info => "[Info]",
        logType::Error => "[Error]",
    };

    print!("{}[{}][{}] ", prefix, timestamp(), std::process::id());
    println!("{}", message);

    unsafe { SetConsoleTextAttribute(handle, saved) };
}

#[macro_export]
macro_rules! logDebug {
    ($($arg:tt)*) => { $crate::logging::log($crate::logging::logType::Debug, &format!($($arg)*)) };
}

#[macro_export]
macro_rules! logInfo {
    ($($arg:tt)*) => { $crate::logging::log($crate::logging::logType::Info, &format!($($arg)*)) };
}

#[macro_export]
macro_rules! logErr {
    ($($arg:tt)*) => { $crate::logging::log($crate::logging::logType::Error, &format!($($arg)*)) };
}
