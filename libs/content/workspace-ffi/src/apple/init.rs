use crate::WgpuWorkspace;
use egui::FontDefinitions;
use egui_wgpu_renderer::RendererState;
use lb_c::Lb;
use std::ffi::c_void;
use wgpu::SurfaceTargetUnsafe;
use workspace_rs::register_fonts;
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use workspace_rs::theme::visuals;
use workspace_rs::workspace::Workspace;

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn init_ws(
    core: *mut c_void, metal_layer: *mut c_void, dark_mode: bool, show_tabs: bool,
) -> *mut c_void {
    let core = unsafe { &mut *(core as *mut Lb) };
    let renderer =
        RendererState::from_surface(SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer));

    visuals::init(&renderer.context);
    let mode = if dark_mode {
        Mode::Dark
    } else {
         Mode::Light
    };
    renderer.context.set_theme(Theme::default(mode));

    let workspace = Workspace::new(core, &renderer.context, show_tabs);
    let mut fonts = FontDefinitions::default();
    register_fonts(&mut fonts);
    renderer.context.set_fonts(fonts);
    egui_extras::install_image_loaders(&renderer.context);

    let obj = WgpuWorkspace { renderer, workspace };

    Box::into_raw(Box::new(obj)) as *mut c_void
}

#[no_mangle]
pub extern "C" fn resize_editor(obj: *mut c_void, width: f32, height: f32, scale: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuWorkspace) };
    obj.renderer.screen.size_in_pixels[0] = width as u32;
    obj.renderer.screen.size_in_pixels[1] = height as u32;
    obj.renderer.screen.pixels_per_point = scale;
}
