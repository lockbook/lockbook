use crate::apple::keyboard::NSKeys;
use crate::{Editor, WgpuEditor};
use egui::PointerButton::{Primary, Secondary};
use egui::{Context, Event, Pos2, Vec2, Visuals};
use egui_wgpu_backend::wgpu::CompositeAlphaMode;
use egui_wgpu_backend::{wgpu, ScreenDescriptor};
use std::ffi::{c_char, c_void, CStr, CString};

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn init_editor(
    metal_layer: *mut c_void, content: *const c_char, dark_mode: bool,
) -> *mut c_void {
    let backends = wgpu::util::backend_bits_from_env().unwrap_or_else(wgpu::Backends::all);
    let instance_desc = wgpu::InstanceDescriptor { backends, ..Default::default() };
    let instance = wgpu::Instance::new(instance_desc);
    let surface = instance.create_surface_from_core_animation_layer(metal_layer);
    let (adapter, device, queue) =
        pollster::block_on(request_device(&instance, backends, &surface));
    let format = surface.get_capabilities(&adapter).formats[0];
    let screen =
        ScreenDescriptor { physical_width: 10000, physical_height: 10000, scale_factor: 1.0 };
    let surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: screen.physical_width, // TODO get from context or something
        height: screen.physical_height,
        present_mode: wgpu::PresentMode::Fifo,
        alpha_mode: CompositeAlphaMode::Auto,
        view_formats: vec![],
    };
    surface.configure(&device, &surface_config);
    let rpass = egui_wgpu_backend::RenderPass::new(&device, format, 1);

    let context = Context::default();
    context.set_visuals(if dark_mode { Visuals::dark() } else { Visuals::light() });
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
        from_egui: None,
        from_host: None,
        editor,
    };

    obj.frame();

    Box::into_raw(Box::new(obj)) as *mut c_void
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn set_text(obj: *mut c_void, content: *const c_char) {
    let obj = unsafe { &mut *(obj as *mut WgpuEditor) };
    obj.editor
        .set_text(CStr::from_ptr(content).to_str().unwrap().into());
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

    let mut clip_event = false;
    if pressed && key == NSKeys::V && modifiers.command {
        let clip = obj.from_host.take().unwrap_or_default();
        obj.raw_input.events.push(Event::Text(clip));
        clip_event = true
    }

    // Event::Text
    if !clip_event && pressed && (modifiers.shift_only() || modifiers.is_none()) && key.valid_text()
    {
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
pub unsafe extern "C" fn mouse_moved(obj: *mut c_void, x: f32, y: f32) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input
        .events
        .push(Event::PointerMoved(Pos2 { x, y }))
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn mouse_button(
    obj: *mut c_void, x: f32, y: f32, pressed: bool, primary: bool,
) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.raw_input.events.push(Event::PointerButton {
        pos: Pos2 { x, y },
        button: if primary { Primary } else { Secondary },
        pressed,
        modifiers: Default::default(),
    })
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn dark_mode(obj: *mut c_void, dark: bool) {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.context
        .set_visuals(if dark { Visuals::dark() } else { Visuals::light() });
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn get_text(obj: *mut c_void) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuEditor);

    let value = obj.editor.buffer.current.text.as_str();

    CString::new(value)
        .expect("Could not Rust String -> C String")
        .into_raw()
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn has_coppied_text(obj: *mut c_void) -> bool {
    let obj = &mut *(obj as *mut WgpuEditor);
    obj.from_egui.is_some()
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn get_coppied_text(obj: *mut c_void) -> *const c_char {
    let obj = &mut *(obj as *mut WgpuEditor);

    let coppied_text = obj.from_egui.take().unwrap_or_default();

    CString::new(coppied_text.as_str())
        .expect("Could not Rust String -> C String")
        .into_raw()
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn system_clipboard_changed(obj: *mut c_void, content: *const c_char) {
    let obj = &mut *(obj as *mut WgpuEditor);
    let content = CStr::from_ptr(content).to_str().unwrap().into();
    obj.from_host = Some(content)
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
