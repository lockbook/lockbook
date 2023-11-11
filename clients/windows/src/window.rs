use raw_window_handle::{
    HasRawDisplayHandle, HasRawWindowHandle, RawDisplayHandle, RawWindowHandle, Win32WindowHandle,
    WindowsDisplayHandle,
};
use windows::Win32::{Foundation::*, UI::WindowsAndMessaging::*};

pub struct Window {
    handle: Win32WindowHandle,
}

// Smails implementations adapted for windows with reference to winit's windows implementation:
// https://github.com/lockbook/lockbook/pull/1835/files#diff-0f28854a868a55fcd30ff5f0fda476aed540b2e1fc3762415ac6e0588ed76fb6
// https://github.com/rust-windowing/winit/blob/ee0db52ac49d64b46c500ef31d7f5f5107ce871a/src/platform_impl/windows/window.rs#L334-L346
impl Window {
    pub fn new(window: HWND) -> Self {
        let mut handle = Win32WindowHandle::empty();
        handle.hwnd = window.0 as *mut _;
        let hinstance = unsafe { get_window_long(window, GWLP_HINSTANCE) };
        handle.hinstance = hinstance as *mut _;

        Self { handle }
    }
}

unsafe impl HasRawWindowHandle for Window {
    fn raw_window_handle(&self) -> RawWindowHandle {
        RawWindowHandle::Win32(self.handle)
    }
}

unsafe impl HasRawDisplayHandle for Window {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }
}

#[inline(always)]
unsafe fn get_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe { GetWindowLongPtrW(hwnd, nindex) };
    #[cfg(target_pointer_width = "32")]
    return unsafe { GetWindowLongW(hwnd, nindex) as isize };
}
