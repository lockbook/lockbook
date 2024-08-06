use windows::{
    core::*,
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
};

pub fn handle(hwnd: HWND, window_title: Option<String>) {
    if let Some(title) = window_title {
        unsafe {
            SetWindowTextA(hwnd, PCSTR((title + "\0").as_ptr())).expect("set window title");
        }
    }
}
