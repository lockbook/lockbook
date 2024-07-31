use jni::sys::jobject;
use jni::JNIEnv;
use ndk_sys::{
    ANativeWindow, ANativeWindow_fromSurface, ANativeWindow_getHeight, ANativeWindow_getWidth,
    ANativeWindow_release,
};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle,
    HasWindowHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
};
use std::ptr::NonNull;

pub struct NativeWindow {
    a_native_window: *mut ANativeWindow,
    display_handle: RawDisplayHandle,
}

impl NativeWindow {
    pub fn new(env: &JNIEnv, surface: jobject) -> Self {
        let a_native_window =
            unsafe { ANativeWindow_fromSurface(env.get_raw() as *mut _, surface as *mut _) };
        let display_handle = RawDisplayHandle::Android(AndroidDisplayHandle::new());

        Self { a_native_window, display_handle }
    }

    pub fn get_raw_window(&self) -> *mut ANativeWindow {
        self.a_native_window
    }

    pub fn get_width(&self) -> u32 {
        unsafe { ANativeWindow_getWidth(self.a_native_window) as u32 }
    }

    pub fn get_height(&self) -> u32 {
        unsafe { ANativeWindow_getHeight(self.a_native_window) as u32 }
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            ANativeWindow_release(self.a_native_window);
        }
    }
}

impl HasDisplayHandle for NativeWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        unsafe { Ok(DisplayHandle::borrow_raw(self.display_handle)) }
    }
}

impl HasWindowHandle for NativeWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe {
            let ptr: NonNull<ANativeWindow> = NonNull::from(&*self.a_native_window);
            let handle = AndroidNdkWindowHandle::new(ptr.cast());
            return Ok(WindowHandle::borrow_raw(RawWindowHandle::AndroidNdk(handle)));
        }
    }
}
