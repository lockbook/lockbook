use windows::Win32::UI::WindowsAndMessaging::*;

// todo: take inspiration from eframe implementation and allow files to finish saving first or whatever
pub fn handle(close: bool) {
    if close {
        unsafe { PostQuitMessage(0) };
    }
}
