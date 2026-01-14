use egui::FontDefinitions;
use egui_wgpu_renderer::RendererState;
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
use wgpu::SurfaceTargetUnsafe;
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
    env: JNIEnv, _: JClass, surface: jobject, core: jlong, dark_mode: bool,
) -> jlong {
    let core = unsafe { &mut *(core as *mut Lb) };
    let mut native_window = NativeWindow::new(&env, surface);
    let renderer =
        RendererState::from_surface(SurfaceTargetUnsafe::from_window(&mut native_window).unwrap());
    visuals::init(&renderer.context, dark_mode);

    let workspace = Workspace::new(core, &renderer.context, false);

    let mut fonts = FontDefinitions::default();
    workspace_rs::register_fonts(&mut fonts);
    renderer.context.set_fonts(fonts);
    egui_extras::install_image_loaders(&renderer.context);

    let obj = WgpuWorkspace { workspace, renderer };

    Box::into_raw(Box::new(obj)) as jlong
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_resizeWS(
    env: JNIEnv, _: JClass, obj: jlong, surface: jobject, scale_factor: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let native_window = NativeWindow::new(&env, surface);

    obj.renderer.screen.size_in_pixels[0] = native_window.get_width();
    obj.renderer.screen.size_in_pixels[1] = native_window.get_height();
    obj.renderer.screen.pixels_per_point = scale_factor;
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_setBottomInset(
    _env: JNIEnv, _: JClass, obj: jlong, inset: jint,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.renderer.bottom_inset = Some(inset as u32);
}
