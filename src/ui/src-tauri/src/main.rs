#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    if let Some(pid) = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<u32>().ok())
    {
        skidsploit::runExecutor(pid);
        return;
    }

    skid_lib::run()
}
