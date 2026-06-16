use egui::{Color32, FontDefinitions};
use egui_wgpu_renderer::RendererState;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
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
use workspace_rs::theme::palette_v2::{
    Mode, Palette, Preferences, Theme, ThemeExt as _, ThemeVariant,
};
use workspace_rs::theme::visuals;
use workspace_rs::workspace::Workspace;

use super::render_thread::RenderThread;
use crate::WgpuWorkspace;

pub struct NativeWindow {
    a_native_window: *mut ANativeWindow,
    display_handle: RawDisplayHandle,
}

const WORKSPACE_THEME_VARIANT_SIG: &str = "Lapp/lockbook/workspace/WorkspaceThemeVariant;";
const WORKSPACE_THEME_PREFERENCES_SIG: &str = "Lapp/lockbook/workspace/WorkspaceThemePreferences;";

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
pub unsafe extern "system" fn Java_app_lockbook_workspace_Workspace_initWSOffloaded(
    env: JNIEnv, _: JClass, surface: jobject, core: jlong, theme: JObject,
) -> jlong {
    init_ws(env, surface, core, theme, true)
}

unsafe fn init_ws(
    mut env: JNIEnv, surface: jobject, core: jlong, theme: JObject, offloaded: bool,
) -> jlong {
    let core = unsafe { &mut *(core as *mut Lb) };
    let mut native_window = NativeWindow::new(&env, surface);
    let mut renderer =
        RendererState::from_surface(SurfaceTargetUnsafe::from_window(&mut native_window).unwrap());
    let font_system = workspace_rs::register_font_system(&renderer.context);
    let sample_count = renderer.backend().sample_count;
    let format =
        RendererState::text_format(&renderer.backend().adapter, &renderer.backend().surface);
    let backend = renderer.backend_mut();
    workspace_rs::register_render_callback_resources(
        &backend.device,
        &backend.queue,
        format,
        &mut backend.renderer,
        font_system,
        sample_count,
    );

    visuals::init(&renderer.context);
    renderer
        .context
        .set_lb_theme(android_material_theme_to_lb(&mut env, theme));

    let workspace = Workspace::new(core, &renderer.context, false, None);

    let mut fonts = FontDefinitions::default();
    workspace_rs::register_fonts(&mut fonts);
    renderer.context.set_fonts(fonts);
    egui_extras::install_image_loaders(&renderer.context);

    let render_thread = if offloaded {
        Some(RenderThread::spawn(renderer.context.clone(), renderer.take_backend()))
    } else {
        None
    };
    let obj = WgpuWorkspace { workspace, renderer, render_thread };

    Box::into_raw(Box::new(obj)) as jlong
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_setTheme(
    mut env: JNIEnv, _: JClass, obj: jlong, theme: JObject,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.renderer
        .context
        .set_lb_theme(android_material_theme_to_lb(&mut env, theme));
}

fn android_material_theme_to_lb(env: &mut JNIEnv<'_>, theme: JObject<'_>) -> Theme {
    android_material_theme_to_lb_inner(env, &theme).unwrap_or_else(|| Theme::default(Mode::Light))
}

fn android_material_theme_to_lb_inner(env: &mut JNIEnv<'_>, theme: &JObject<'_>) -> Option<Theme> {
    let is_dark = env.get_field(theme, "isDark", "Z").ok()?.z().ok()?;
    let current = if is_dark { Mode::Dark } else { Mode::Light };

    let dim_obj = theme_object_field(env, theme, "dim", WORKSPACE_THEME_VARIANT_SIG)?;
    let dim = theme_variant_to_lb(env, &dim_obj)?;

    let light_prefs_obj =
        theme_object_field(env, theme, "lightPrefs", WORKSPACE_THEME_PREFERENCES_SIG)?;
    let light_prefs = theme_preferences_to_lb(env, &light_prefs_obj)?;

    let bright_obj = theme_object_field(env, theme, "bright", WORKSPACE_THEME_VARIANT_SIG)?;
    let bright = theme_variant_to_lb(env, &bright_obj)?;

    let dark_prefs_obj =
        theme_object_field(env, theme, "darkPrefs", WORKSPACE_THEME_PREFERENCES_SIG)?;
    let dark_prefs = theme_preferences_to_lb(env, &dark_prefs_obj)?;

    Some(Theme::from_android_material(current, dim, light_prefs, bright, dark_prefs))
}

fn theme_object_field<'local>(
    env: &mut JNIEnv<'local>, obj: &JObject<'_>, name: &str, sig: &str,
) -> Option<JObject<'local>> {
    env.get_field(obj, name, sig).ok()?.l().ok()
}

fn theme_variant_to_lb(env: &mut JNIEnv, variant: &JObject) -> Option<ThemeVariant> {
    Some(ThemeVariant {
        black: color32_from_argb(int_field(env, variant, "black")?),
        grey: color32_from_argb(int_field(env, variant, "grey")?),
        red: color32_from_argb(int_field(env, variant, "red")?),
        green: color32_from_argb(int_field(env, variant, "green")?),
        yellow: color32_from_argb(int_field(env, variant, "yellow")?),
        blue: color32_from_argb(int_field(env, variant, "blue")?),
        magenta: color32_from_argb(int_field(env, variant, "magenta")?),
        cyan: color32_from_argb(int_field(env, variant, "cyan")?),
        white: color32_from_argb(int_field(env, variant, "white")?),
    })
}

fn theme_preferences_to_lb(env: &mut JNIEnv, prefs: &JObject) -> Option<Preferences> {
    Some(Preferences {
        primary: Palette::try_from(string_field(env, prefs, "primary")?.as_str()).ok()?,
        secondary: Palette::try_from(string_field(env, prefs, "secondary")?.as_str()).ok()?,
        tertiary: Palette::try_from(string_field(env, prefs, "tertiary")?.as_str()).ok()?,
        quaternary: Palette::try_from(string_field(env, prefs, "quaternary")?.as_str()).ok()?,
    })
}

fn int_field(env: &mut JNIEnv, obj: &JObject, name: &str) -> Option<i32> {
    env.get_field(obj, name, "I").ok()?.i().ok()
}

fn string_field(env: &mut JNIEnv, obj: &JObject, name: &str) -> Option<String> {
    let string = env
        .get_field(obj, name, "Ljava/lang/String;")
        .ok()?
        .l()
        .ok()?;
    env.get_string(&JString::from(string)).ok().map(Into::into)
}

/// some bit shifting and masking to convert from Android's ARGB format to egui's RGBA format
fn color32_from_argb(argb: i32) -> Color32 {
    let argb = argb as u32;
    Color32::from_rgba_unmultiplied(
        ((argb >> 16) & 0xff) as u8,
        ((argb >> 8) & 0xff) as u8,
        (argb & 0xff) as u8,
        ((argb >> 24) & 0xff) as u8,
    )
}

#[no_mangle]
pub unsafe extern "system" fn Java_app_lockbook_workspace_Workspace_dropWS(
    _: JNIEnv, _: JClass, obj: jlong,
) {
    drop(Box::from_raw(obj as *mut WgpuWorkspace));
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_resizeWS(
    env: JNIEnv, _: JClass, obj: jlong, surface: jobject, scale_factor: jfloat,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    let native_window = NativeWindow::new(&env, surface);

    obj.renderer.screen.size_in_pixels[0] = native_window.get_width();
    obj.renderer.screen.size_in_pixels[1] = native_window.get_height();
    obj.renderer.set_native_pixels_per_point(scale_factor);
}

#[no_mangle]
pub extern "system" fn Java_app_lockbook_workspace_Workspace_setBottomInset(
    _env: JNIEnv, _: JClass, obj: jlong, inset: jint,
) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };

    obj.renderer.bottom_inset = Some(inset as u32);
}
