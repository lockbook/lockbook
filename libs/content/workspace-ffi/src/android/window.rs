use egui_wgpu_backend::wgpu::{self};
use jni::sys::jobject;
use jni::JNIEnv;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle, WindowHandle,
};
use std::ptr::NonNull;

pub struct NativeWindow {
    a_native_window: *mut ndk_sys::ANativeWindow,
    display_handle: RawDisplayHandle,
}

impl NativeWindow {
    pub fn new(env: &JNIEnv, surface: jobject) -> Self {
        let a_native_window = unsafe {
            ndk_sys::ANativeWindow_fromSurface(env.get_raw() as *mut _, surface as *mut _)
        };
        let display_handle = RawDisplayHandle::Android(AndroidDisplayHandle::new());

        Self { a_native_window, display_handle }
    }

    pub fn get_raw_window(&self) -> *mut ndk_sys::ANativeWindow {
        self.a_native_window
    }

    pub fn get_width(&self) -> u32 {
        unsafe { ndk_sys::ANativeWindow_getWidth(self.a_native_window) as u32 }
    }

    pub fn get_height(&self) -> u32 {
        unsafe { ndk_sys::ANativeWindow_getHeight(self.a_native_window) as u32 }
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        unsafe {
            ndk_sys::ANativeWindow_release(self.a_native_window);
        }
    }
}

impl HasDisplayHandle for NativeWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        unsafe { Ok(raw_window_handle::DisplayHandle::borrow_raw(self.display_handle)) }
    }
}

impl HasWindowHandle for NativeWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        unsafe {
            let ptr: NonNull<ndk_sys::ANativeWindow> = NonNull::from(&*self.a_native_window);
            let handle = AndroidNdkWindowHandle::new(ptr.cast());
            return Ok(WindowHandle::borrow_raw(RawWindowHandle::AndroidNdk(handle)));
        }
    }
}

pub async fn request_device(
    instance: &wgpu::Instance, surface: &wgpu::Surface<'_>,
) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter = wgpu::util::initialize_adapter_from_env_or_default(instance, Some(surface))
        .await
        .expect("No suitable GPU adapters found on the system!");
    let adapter_info = adapter.get_info();
    println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);
    let res = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: adapter.features(),
                required_limits: adapter.limits(),
            },
            None,
        )
        .await;
    match res {
        Err(err) => {
            panic!("request_device failed: {:?}", err);
        }
        Ok((device, queue)) => (adapter, device, queue),
    }
}
