use std::sync::Arc;

use egui::{OutputCommand, Pos2, ViewportCommand};
use egui_wgpu_renderer::RendererState;
use lbeguiapp::{Output, WgpuLockbook};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Icon, Window, WindowId};
use workspace_rs::tab::{ClipContent, ExtendedInput};
use workspace_rs::theme::palette_v2::{Mode, Theme, ThemeExt};

fn load_icon() -> Option<Icon> {
    let png_bytes = include_bytes!("../lockbook.png");
    let img = image::load_from_memory(png_bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Icon::from_rgba(img.into_raw(), width, height).ok()
}

pub fn run() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).expect("event loop failed");
}

#[derive(Default)]
struct App {
    state: Option<AppState>,
}

struct AppState {
    window: Arc<Window>,
    lb: WgpuLockbook<'static>,
    egui_winit: egui_winit::State,
    clipboard: arboard::Clipboard,
    pending_paste: bool,
    last_pointer_pos: Pos2,
    close_requested: bool,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Lockbook")
            .with_inner_size(PhysicalSize::new(1300, 800))
            .with_window_icon(load_icon());

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("failed to create window"),
        );

        let dark_mode = dark_light::detect()
            .map(|m| m == dark_light::Mode::Dark)
            .unwrap_or(false);
        let mut lb = init_lockbook(Arc::clone(&window), dark_mode);

        // Set initial scale factor and screen size
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
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match &mut self.state {
            Some(s) => s,
            None => return,
        };

        // Track cursor position
        if let WindowEvent::CursorMoved { position, .. } = &event {
            state.last_pointer_pos = Pos2::new(position.x as f32, position.y as f32);
        }

        // Handle file drops before egui-winit consumes the event
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

        // Let egui-winit handle the event
        let response = state.egui_winit.on_window_event(&state.window, &event);

        if response.repaint {
            state.window.request_redraw();
        }

        match event {
            WindowEvent::CloseRequested => {
                // Don't exit immediately - let the app handle graceful shutdown
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
                // Handle paste shortcut (Ctrl+V / Cmd+V)
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Nothing needed - we use ControlFlow::Wait and request_redraw()
    }
}

impl AppState {
    fn render(&mut self, event_loop: &ActiveEventLoop) {
        // Handle pending paste
        if self.pending_paste {
            self.pending_paste = false;
            self.handle_paste();
        }

        // Gather input from egui-winit
        let mut raw_input = self.egui_winit.take_egui_input(&self.window);

        // Signal close request to egui so app can handle graceful shutdown
        if self.close_requested {
            if let Some(viewport) = raw_input.viewports.get_mut(&raw_input.viewport_id) {
                viewport.events.push(egui::ViewportEvent::Close);
            }
        }

        self.lb.renderer.raw_input = raw_input;

        // Run frame
        let Output { platform, viewport, app: lbeguiapp::Response { close } } = self.lb.frame();

        // Handle app close request
        if close {
            event_loop.exit();
            return;
        }

        // Handle platform outputs
        self.egui_winit
            .handle_platform_output(&self.window, platform.clone());

        // Handle clipboard copy
        for cmd in &platform.commands {
            if let OutputCommand::CopyText(text) = cmd {
                if let Err(e) = self.clipboard.set_text(text.clone()) {
                    log::warn!("clipboard copy failed: {e}");
                }
            }
            if let OutputCommand::OpenUrl(url) = cmd {
                let _ = open::that(&url.url);
            }
        }

        // Handle viewport commands
        if let Some(viewport) = viewport.values().next() {
            for cmd in &viewport.commands {
                match cmd {
                    ViewportCommand::Title(title) => {
                        self.window.set_title(title);
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

            // Schedule repaint if needed
            if viewport.repaint_delay.as_millis() < 1000 || self.close_requested {
                self.window.request_redraw();
            }
        }
    }

    fn handle_paste(&mut self) {
        let position = self.last_pointer_pos;

        // Try image first
        if let Ok(img) = self.clipboard.get_image() {
            let rgba = image::RgbaImage::from_raw(
                img.width as u32,
                img.height as u32,
                img.bytes.into_owned(),
            );
            if let Some(rgba) = rgba {
                let mut png_bytes = Vec::new();
                if rgba
                    .write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
                    .is_ok()
                {
                    let content = vec![ClipContent::Image(png_bytes)];
                    self.lb
                        .renderer
                        .context
                        .push_event(workspace_rs::Event::Paste { content, position });
                    return;
                }
            }
        }

        // Fall back to text
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
    workspace_rs::register_render_callback_resources(
        &renderer.device,
        &renderer.queue,
        RendererState::text_format(&renderer.adapter, &renderer.surface),
        &mut renderer.renderer,
        workspace_rs::register_font_system(&renderer.context),
        renderer.sample_count,
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
