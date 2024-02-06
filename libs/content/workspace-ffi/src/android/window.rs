use core::ffi::c_void;
use egui_wgpu_backend::wgpu;
use jni::sys::jobject;
use jni::JNIEnv;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HasRawDisplayHandle, HasRawWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

pub struct NativeWindow {
    a_native_window: *mut ndk_sys::ANativeWindow,
}

impl NativeWindow {
    pub fn new(env: &JNIEnv, surface: jobject) -> Self {
        let a_native_window = unsafe {
            ndk_sys::ANativeWindow_fromSurface(env.get_raw() as *mut _, surface as *mut _)
        };
        Self { a_native_window }
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

unsafe impl HasRawWindowHandle for NativeWindow {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = AndroidNdkWindowHandle::empty();
        handle.a_native_window = self.a_native_window as *mut _ as *mut c_void;
        RawWindowHandle::AndroidNdk(handle)
    }
}

unsafe impl HasRawDisplayHandle for NativeWindow {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::empty())
    }
}

pub async fn request_device(
    instance: &wgpu::Instance, backend: wgpu::Backends, surface: &wgpu::Surface,
) -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let adapter =
        wgpu::util::initialize_adapter_from_env_or_default(instance, backend, Some(surface))
            .await
            .expect("No suitable GPU adapters found on the system!");
    let adapter_info = adapter.get_info();
    println!("Using {} ({:?})", adapter_info.name, adapter_info.backend);
    let res = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                features: adapter.features(),
                limits: adapter.limits(),
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
