use windows_sys::Win32::Foundation::BOOL;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::System::Console::{
    FillConsoleOutputAttribute, FillConsoleOutputCharacterA, GetConsoleScreenBufferInfo, GetStdHandle,
    SetConsoleCursorPosition, CONSOLE_SCREEN_BUFFER_INFO, COORD, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, INPUT_0,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowThreadProcessId,
    IsWindowVisible, SetForegroundWindow,
};
struct enumContext {
    targetPid: u32,
    windows: Vec<isize>,
}

pub fn setConsoleTitle(title: &str) {
    use windows_sys::Win32::System::Console::SetConsoleTitleA;

    let mut bytes = title.to_string().into_bytes();
    bytes.push(0);
    unsafe {
        SetConsoleTitleA(bytes.as_ptr());
    }
}

pub fn clearConsole() {
    let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    let mut info: CONSOLE_SCREEN_BUFFER_INFO = unsafe { std::mem::zeroed() };
    if unsafe { GetConsoleScreenBufferInfo(handle, &mut info) } == 0 {
        return;
    }

    let origin = COORD { X: 0, Y: 0 };
    let total = (info.dwSize.X as u32) * (info.dwSize.Y as u32);
    let mut written: u32 = 0;

    unsafe {
        FillConsoleOutputCharacterA(handle, 32i8, total, origin, &mut written);
        FillConsoleOutputAttribute(handle, info.wAttributes, total, origin, &mut written);
        SetConsoleCursorPosition(handle, origin);
    }
}

unsafe extern "system" fn enumWindowsProc(hwnd: *mut core::ffi::c_void, lParam: isize) -> BOOL {
    let context = &mut *(lParam as *mut enumContext);
    let mut windowPid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut windowPid);

    if windowPid == context.targetPid && IsWindowVisible(hwnd) != 0 {
        context.windows.push(hwnd as isize);
    }
    1
}

pub fn getHwnds(targetPid: u32) -> Vec<isize> {
    let mut context = enumContext {
        targetPid,
        windows: Vec::new(),
    };

    unsafe {
        EnumWindows(
            Some(enumWindowsProc),
            &mut context as *mut enumContext as isize,
        );
    }

    context.windows
}

pub fn getForegroundWindow() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

pub fn forceForeground(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }

    let target = hwnd as *mut core::ffi::c_void;
    let foreground = unsafe { GetForegroundWindow() };
    if foreground == target {
        return true;
    }

    let foregroundThread = if !foreground.is_null() {
        unsafe { GetWindowThreadProcessId(foreground, std::ptr::null_mut()) }
    } else {
        0
    };
    let currentThread = unsafe { GetCurrentThreadId() };

    unsafe {
        if foregroundThread != 0 {
            AttachThreadInput(currentThread, foregroundThread, 1);
            BringWindowToTop(target);
            SetForegroundWindow(target);
            AttachThreadInput(currentThread, foregroundThread, 0);
        } else {
            BringWindowToTop(target);
            SetForegroundWindow(target);
        }
    }

    unsafe { GetForegroundWindow() == target }
}

pub fn pressEscape() {
    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: 0x01,
                dwFlags: KEYEVENTF_SCANCODE,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let up = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: 0x01,
                dwFlags: KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let mut inputs = [down, up];
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}
