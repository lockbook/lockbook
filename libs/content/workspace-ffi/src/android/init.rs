use egui::{Context, FontDefinitions};
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::{ScreenDescriptor, wgpu};
use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::*;
use lb_java::Lb;
use ndk_sys::{
    ANativeWindow, ANativeWindow_fromSurface, ANativeWindow_getHeight, ANativeWindow_getWidth,
    ANativeWindow_release,
};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle,
    HasWindowHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
};
use std::ptr::NonNull;
use std::time::Instant;
use workspace_rs::theme::visuals;
use workspace_rs::workspace::Workspace;

use crate::WgpuWorkspace;

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

#[no_mangle]
pub unsafe extern "system" fn Java_app_lockbook_workspace_Workspace_initWS(
    env: JNIEnv, _: JClass, surface: jobject, core: jlong, scale_factor: jfloat, dark_mode: bool,
    old_wgpu: jlong,
) -> jlong {
    let core = unsafe { &mut *(core as *mut Lb) };
    let mut native_window = NativeWindow::new(&env, surface);
    let renderer =
        RendererState::from_surface(SurfaceTargetUnsafe::from_window(&mut native_window));
    visuals::init(&renderer.context, dark_mode);

    let workspace = if old_wgpu != jlong::MAX {
        let mut old_wgpu: Box<WgpuWorkspace> = unsafe { Box::from_raw(old_wgpu as *mut _) };

        old_wgpu
            .workspace
            .invalidate_egui_references(&context, core);
        old_wgpu.workspace
    } else {
        Workspace::new(core, &context, false)
    };

    let mut fonts = FontDefinitions::default();
    workspace_rs::register_fonts(&mut fonts);
    context.set_fonts(fonts);
    egui_extras::install_image_loaders(&context);

    let start_time = Instant::now();
    let obj = WgpuWorkspace {
        start_time,
        device,
        queue,
        surface,
        adapter,
        rpass,
        screen,
        context: context.clone(),
        raw_input: Default::default(),
        workspace,
        surface_width: 0,
        surface_height: 0,
    };

    Box::into_raw(Box::new(obj)) as jlong
}

async fn request_device(
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
                memory_hints: Default::default(),
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

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_resizeWS(
    env: JNIEnv, _: JClass, obj: jlong, surface: jobject, scale_factor: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let native_window = NativeWindow::new(&env, surface);

    obj.screen.physical_width = native_window.get_width();
    obj.screen.physical_height = native_window.get_height();
    obj.screen.scale_factor = scale_factor;
}
