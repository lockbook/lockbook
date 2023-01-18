use crate::apple::keyboard::NSKeys;
use crate::{Editor, WgpuEditor};
use egui::{Context, Event, Pos2, Vec2};
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use std::ffi::{c_char, c_void, CStr, CString};

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn init_editor(
    metal_layer: *mut c_void, content: *const c_char,
) -> *mut c_void {
    let backend = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance = wgpu::Instance::new(backend);
    let surface = instance.create_surface_from_core_animation_layer(metal_layer);
    let (adapter, device, queue) = pollster::block_on(request_device(&instance, backend, &surface));
    let surface_format = surface.get_supported_formats(&adapter)[0];
    let screen =
        ScreenDescriptor { physical_width: 10000, physical_height: 10000, scale_factor: 1.0 };
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface.get_supported_formats(&adapter)[0],
        width: screen.physical_width, // TODO get from context or something
        height: screen.physical_height,
        present_mode: wgpu::PresentMode::Fifo,
    };
    surface.configure(&device, &surface_config);
    let rpass = egui_wgpu_backend::RenderPass::new(&device, surface_format, 1);

    let context = Context::default();
    let mut editor = Editor::default();
    editor.set_font(&context);
    editor.buffer = CStr::from_ptr(content).to_str().unwrap().into();

    let mut obj = WgpuEditor {
        device,
        queue,
        surface,
        adapter,
        rpass,
        screen,
        context,
        raw_input: Default::default(),
        editor,
    };

    obj.frame();

    // TODO we need to free this memory
    Box::into_raw(Box::new(obj)) as *mut c_void
}

#[no_mangle]
pub extern "C" fn draw_editor(obj: *mut c_void) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };
    obj.frame();
}

#[no_mangle]
pub extern "C" fn resize_editor(obj: *mut c_void, width: f32, height: f32, scale: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };
    obj.screen.physical_width = width as u32;
    obj.screen.physical_height = height as u32;
    obj.screen.scale_factor = scale;
}

#[no_mangle]
pub extern "C" fn set_scale(obj: *mut c_void, scale: f32) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };
    obj.screen.scale_factor = scale;
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn key_event(
    obj: *mut c_void, key_code: u16, shift: bool, ctrl: bool, option: bool, command: bool,
    pressed: bool, characters: *const c_char,
) {
    let obj = &mut *(obj as *mut WgpuEditor);

    let modifiers = egui::Modifiers { alt: option, ctrl, shift, mac_cmd: command, command };

    obj.raw_input.modifiers = modifiers;

    let key = NSKeys::from(key_code).unwrap();

    // Event::Text
    if pressed && (modifiers.shift_only() || modifiers.is_none()) && key.valid_text() {
        let text = CStr::from_ptr(characters).to_str().unwrap().to_string();
        obj.raw_input.events.push(Event::Text(text));
    }

    // Event::Key
    if let Some(key) = key.egui_key() {
        obj.raw_input
            .events
            .push(Event::Key { key, pressed, modifiers });
    }
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn scroll_wheel(obj: *mut c_void, scroll_wheel: f32) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input
        .events
        .push(Event::PointerMoved(Pos2::new(250.0, 250.0)));
    obj.raw_input
        .events
        .push(Event::Scroll(Vec2::new(0.0, scroll_wheel * 2.0)))
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn get_text(obj: *mut c_void) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuEditor);

    let value = obj.editor.buffer.raw.as_str();

    CString::new(value)
        .expect("Could not Rust String -> C String")
        .into_raw()
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn free_text(s: *mut c_void) {
    if s.is_null() {
        return;
    }
    let _ = CString::from_raw(s as *mut c_char);
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn deinit_editor(obj: *mut c_void) {
    drop(Box::from_raw(obj as *mut WgpuEditor));
}

async fn request_device(
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
