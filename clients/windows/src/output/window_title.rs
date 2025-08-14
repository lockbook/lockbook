use windows::Win32::Foundation::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

pub fn handle(hwnd: HWND, window_title: Option<String>) {
    if let Some(title) = window_title {
        unsafe {
            SetWindowTextA(hwnd, PCSTR((title + "\0").as_ptr())).expect("set window title");
        }
    }
}
