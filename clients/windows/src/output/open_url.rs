use egui::output::OpenUrl;
use windows::Win32::Foundation::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

// https://www.betaarchive.com/wiki/index.php/Microsoft_KB_Archive/224816
pub fn handle(open_url: Option<OpenUrl>) {
    if let Some(OpenUrl { url, .. }) = open_url {
        unsafe {
            ShellExecuteW(
                HWND(0),
                w!("open"),
                &HSTRING::from(url),
                PCWSTR(std::ptr::null()),
                PCWSTR(std::ptr::null()),
                SW_SHOWNORMAL,
            );
        };
    }
}
