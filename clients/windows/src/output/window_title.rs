use windows::{
    core::*,
    Win32::{Foundation::*, UI::WindowsAndMessaging::*},
};

pub fn handle(hwnd: HWND, set_window_title: Option<String>) {
    if let Some(title) = set_window_title {
        unsafe {
            SetWindowTextA(hwnd, PCSTR((title + "\0").as_ptr())).expect("set window title");
        }
    }
}
