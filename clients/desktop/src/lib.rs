struct App {
    state: Option<AppState>,
    proxy: EventLoopProxy<UserEvent>,
}

struct AppState {
    title: String,
    window: Arc<Window>,
    lb: WgpuLockbook<'static>,
    egui_winit: egui_winit::State,
    clipboard: arboard::Clipboard,
    pending_paste: bool,
    last_pointer_pos: Pos2,
    close_requested: bool,
    next_repaint: Option<Instant>,
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let icon_data = load_icon_data();
        let window_icon = icon_data
            .as_ref()
            .and_then(|i| Icon::from_rgba(i.rgba.clone(), i.width, i.height).ok());

        let window_attrs = Window::default_attributes()
            .with_title("Lockbook")
            // todo: respect or deprecate setting for
            // starting maximized
            .with_inner_size(LogicalSize::new(1300, 800))
            .with_window_icon(window_icon);

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("failed to create window"),
        );

        #[cfg(target_os = "macos")]
        if let Some(icon) = &icon_data {
            set_macos_app_icon(&icon.rgba, icon.width, icon.height);
        }

        let dark_mode = dark_light::detect()
            .map(|m| m == dark_light::Mode::Dark)
            .unwrap_or(false);
        let mut lb = init_lockbook(Arc::clone(&window), dark_mode);

        let proxy = self.proxy.clone();
        lb.renderer
            .context
            .set_request_repaint_callback(move |info| {
                let when = Instant::now() + info.delay;
                let _ = proxy.send_event(UserEvent::RepaintRequested {
                    when,
                    cumulative_pass_nr: info.current_cumulative_pass_nr,
                    viewport_id: info.viewport_id,
                });
            });

        let scale_factor = window.scale_factor() as f32;
        let size = window.inner_size();
        lb.renderer.set_native_pixels_per_point(scale_factor);
        lb.renderer.screen.size_in_pixels = [size.width, size.height];

        let egui_winit = egui_winit::State::new(
            lb.renderer.context.clone(),
            lb.renderer.context.viewport_id(),
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        let clipboard = arboard::Clipboard::new().expect("failed to initialize clipboard");

        self.state = Some(AppState {
            window,
            lb,
            egui_winit,
            clipboard,
            pending_paste: false,
            last_pointer_pos: Pos2::ZERO,
            close_requested: false,
            next_repaint: None,
            title: "Lockbook".to_string(),
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        if let WindowEvent::CursorMoved { position, .. } = &event {
            state.last_pointer_pos = Pos2::new(position.x as f32, position.y as f32);
        }

        if let WindowEvent::DroppedFile(path) = &event {
            let content = vec![ClipContent::Files(vec![path.clone()])];
            state
                .lb
                .renderer
                .context
                .push_event(workspace_rs::Event::Drop {
                    content,
                    position: state.last_pointer_pos,
                });
        }

        if !matches!(event, WindowEvent::ThemeChanged(_)) {
            // todo: check this -- if there's no clipboard it falls back to a "manual" string which
            // may be populated by egui itself and result in double paste.
            let response = state.egui_winit.on_window_event(&state.window, &event);

            if response.repaint && !matches!(event, WindowEvent::RedrawRequested) {
                state.window.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                state.close_requested = true;
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.lb.renderer.screen.size_in_pixels = [size.width, size.height];
                state.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state
                    .lb
                    .renderer
                    .set_native_pixels_per_point(scale_factor as f32);
                state.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                state.render(event_loop);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    let modifiers = state.egui_winit.egui_input().modifiers;
                    if modifiers.command
                        && event.logical_key == winit::keyboard::Key::Character("v".into())
                    {
                        state.pending_paste = true;
                        state.window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
        let Some(state) = &mut self.state else { return };
        match event {
            UserEvent::RepaintRequested { when, cumulative_pass_nr, viewport_id } => {
                let current_pass_nr = state
                    .lb
                    .renderer
                    .context
                    .cumulative_pass_nr_for(viewport_id);
                if current_pass_nr != cumulative_pass_nr
                    && current_pass_nr != cumulative_pass_nr + 1
                {
                    return;
                }
                state.next_repaint = Some(
                    state
                        .next_repaint
                        .map_or(when, |existing| existing.min(when)),
                );
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(state) = &mut self.state else { return };
        match state.next_repaint {
            Some(deadline) if deadline <= Instant::now() => {
                state.next_repaint = None;
                state.window.request_redraw();
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            Some(deadline) => {
                event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
            }
            None => {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
        }
    }
}

impl AppState {
    fn render(&mut self, _event_loop: &ActiveEventLoop) {
        let mut raw_input = self.egui_winit.take_egui_input(&self.window);

        // we do clipboard things ourselves because we do image things
        if self.pending_paste {
            self.pending_paste = false;
            if !self.handle_image_paste() {
                if let Ok(text) = self.clipboard.get_text() {
                    if !text.is_empty() {
                        raw_input.events.push(egui::Event::Paste(text));
                    }
                }
            }
        }

        if self.close_requested {
            if let Some(viewport) = raw_input.viewports.get_mut(&raw_input.viewport_id) {
                viewport.events.push(egui::ViewportEvent::Close);
            }
        }

        // Carry forward events queued during the previous frame — handle_paste
        // (called for ViewportCommand::RequestPaste from the right-click menu)
        // pushes Event::Paste(text) onto renderer.raw_input.events after
        // lb.frame() has already taken its input for the current frame.
        let mut carried = std::mem::take(&mut self.lb.renderer.raw_input.events);
        carried.append(&mut raw_input.events);
        raw_input.events = carried;

        self.lb.renderer.raw_input = raw_input;

        let Output { mut platform, viewport, app: lbeguiapp::Response { close } } = self.lb.frame();

        if close {
            std::process::exit(0);
        }

        for command in &platform.commands {
            match command {
                egui::OutputCommand::CopyText(text) => {
                    let _ = self.clipboard.set_text(text.clone());
                }
                egui::OutputCommand::CopyImage(image) => {
                    let _ = self.clipboard.set_image(arboard::ImageData {
                        width: image.width(),
                        height: image.height(),
                        bytes: std::borrow::Cow::Borrowed(image.as_raw()),
                    });
                }
                _ => {}
            }
        }
        platform.commands.retain(|c| {
            !matches!(c, egui::OutputCommand::CopyText(_) | egui::OutputCommand::CopyImage(_))
        });

        self.egui_winit
            .handle_platform_output(&self.window, platform);

        if let Some(viewport) = viewport.values().next() {
            for cmd in &viewport.commands {
                match cmd {
                    ViewportCommand::Title(title) => {
                        if self.title != *title {
                            self.title = title.clone();
                            self.window.set_title(title);
                        }
                    }
                    ViewportCommand::RequestPaste => {
                        self.handle_paste();
                    }
                    ViewportCommand::CancelClose => {
                        self.close_requested = false;
                    }
                    _ => {}
                }
            }
        }

        if self.close_requested {
            self.window.request_redraw();
        }
    }

    fn handle_image_paste(&mut self) -> bool {
        if let Ok(paths) = self.clipboard.get().file_list() {
            let images: Vec<ClipContent> = paths
                .into_iter()
                .filter_map(|p| {
                    let bytes = std::fs::read(&p).ok()?;
                    image::guess_format(&bytes).ok()?;
                    Some(ClipContent::Image(bytes))
                })
                .collect();
            if !images.is_empty() {
                self.lb
                    .renderer
                    .context
                    .push_event(workspace_rs::Event::Paste {
                        content: images,
                        position: self.last_pointer_pos,
                    });
                return true;
            }
        }

        let Ok(img) = self.clipboard.get_image() else {
            return false;
        };
        let Some(rgba) =
            image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
        else {
            return false;
        };
        let mut png_bytes = Vec::new();
        if rgba
            .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
            .is_err()
        {
            return false;
        }
        let content = vec![ClipContent::Image(png_bytes)];
        self.lb
            .renderer
            .context
            .push_event(workspace_rs::Event::Paste { content, position: self.last_pointer_pos });
        true
    }

    fn handle_paste(&mut self) {
        if self.handle_image_paste() {
            return;
        }
        if let Ok(text) = self.clipboard.get_text() {
            self.lb
                .renderer
                .raw_input
                .events
                .push(egui::Event::Paste(text));
        }
    }
}

fn init_lockbook(window: Arc<Window>, dark_mode: bool) -> WgpuLockbook<'static> {
    // Safety: window is kept alive in Arc for lifetime of app
    let renderer = unsafe {
        RendererState::from_surface(wgpu::SurfaceTargetUnsafe::from_window(&window).unwrap())
    };

    init_with_renderer(renderer, dark_mode)
}

fn init_with_renderer(
    mut renderer: RendererState<'static>, dark_mode: bool,
) -> WgpuLockbook<'static> {
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

    workspace_rs::theme::visuals::init(&renderer.context);
    let mode = if dark_mode { Mode::Dark } else { Mode::Light };
    renderer.context.set_lb_theme(Theme::default(mode));

    let app = lbeguiapp::Lockbook::new(&renderer.context);
    app.deferred_init(&renderer.context);

    let mut lb = WgpuLockbook {
        renderer,
        queued_events: Default::default(),
        double_queued_events: Default::default(),
        app,
    };

    lb.frame();
    lb
}

#[derive(Debug)]
enum UserEvent {
    RepaintRequested { when: Instant, cumulative_pass_nr: u64, viewport_id: ViewportId },
}

struct IconData {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

fn load_icon_data() -> Option<IconData> {
    let png_bytes = include_bytes!("../lockbook.png");
    let img = image::load_from_memory(png_bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(IconData { rgba: img.into_raw(), width, height })
}

// `winit::Window::with_window_icon` is unsupported on macOS, so we set the dock
// / app-switcher icon directly via `NSApplication.setApplicationIconImage`.
#[cfg(target_os = "macos")]
fn set_macos_app_icon(rgba: &[u8], width: u32, height: u32) {
    use objc2::ClassType as _;
    use objc2_app_kit::{NSApplication, NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage};
    use objc2_foundation::NSSize;

    unsafe extern "C" {
        static NSApp: Option<&'static NSApplication>;
    }

    let mut bytes = rgba.to_vec();

    unsafe {
        let Some(app) = NSApp else {
            log::debug!("NSApp is null; skipping app icon");
            return;
        };

        let Some(image_rep) = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            [bytes.as_mut_ptr()].as_mut_ptr(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            (width * 4) as isize,
            32,
        ) else {
            log::warn!("failed to create NSBitmapImageRep for app icon");
            return;
        };

        let app_icon =
            NSImage::initWithSize(NSImage::alloc(), NSSize::new(width as f64, height as f64));
        app_icon.addRepresentation(&image_rep);
        app.setApplicationIconImage(Some(&app_icon));
    }
}

pub fn run() {
    env_logger::init();

    let mut builder = EventLoop::<UserEvent>::with_user_event();

    #[cfg(target_os = "linux")]
    {
        // winit's Wayland backend doesn't deliver file drag-and-drop events. Default to X11
        // unless the user has opted in via settings (or set WINIT_UNIX_BACKEND).
        use winit::platform::x11::EventLoopBuilderExtX11;

        let allow_wayland = lbeguiapp::Settings::read_from_file()
            .map(|s| s.allow_wayland)
            .unwrap_or(false);

        if !allow_wayland && std::env::var_os("WINIT_UNIX_BACKEND").is_none() {
            builder.with_x11();
        }
    }

    let event_loop = builder.build().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App { state: None, proxy: event_loop.create_proxy() };
    event_loop.run_app(&mut app).expect("event loop failed");
}

use std::sync::Arc;
use std::time::Instant;

use egui::{Pos2, ViewportCommand, ViewportId};
use egui_wgpu_renderer::RendererState;
use lbeguiapp::{Output, WgpuLockbook};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Icon, Window, WindowId};
use workspace_rs::tab::{ClipContent, ExtendedInput};
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};
