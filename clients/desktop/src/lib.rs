static NOOB_FACTOR: f64 = 8.0;

struct App {
    state: Option<AppState>,
    proxy: EventLoopProxy<UserEvent>,
}

struct AppState {
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

        let window_attrs = Window::default_attributes()
            .with_title("Lockbook")
            .with_inner_size(LogicalSize::new(1300, 800))
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

        let response = state.egui_winit.on_window_event(&state.window, &event);

        if response.repaint && !matches!(event, WindowEvent::RedrawRequested) {
            state.window.request_redraw();
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
        if self.pending_paste {
            self.pending_paste = false;
            self.handle_image_paste();
        }

        let mut raw_input = self.egui_winit.take_egui_input(&self.window);

        if self.close_requested {
            if let Some(viewport) = raw_input.viewports.get_mut(&raw_input.viewport_id) {
                viewport.events.push(egui::ViewportEvent::Close);
            }
        }

        self.lb.renderer.raw_input = raw_input;

        // Tell the renderer the actual budget for the display the window is on
        // so its dev-shame check uses the right number. NOOB_FACTOR widens the
        // budget for slower-than-real-time development. If the OS doesn't
        // report a refresh rate, the renderer falls back to its 60Hz default.
        if let Some(mhz) = self
            .window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
        {
            let hz = mhz as f64 / 1000.0;
            let budget = Duration::from_secs_f64(NOOB_FACTOR / hz);
            self.lb.renderer.set_frame_budget(budget);
        }

        let Output { platform, viewport, app: lbeguiapp::Response { close, .. } } = self.lb.frame();

        if close {
            std::process::exit(0);
        }

        self.egui_winit
            .handle_platform_output(&self.window, platform);

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
        }

        if self.close_requested {
            self.window.request_redraw();
        }
    }

    fn handle_image_paste(&mut self) -> bool {
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

#[derive(Debug)]
enum UserEvent {
    RepaintRequested { when: Instant, cumulative_pass_nr: u64, viewport_id: ViewportId },
}

fn load_icon() -> Option<Icon> {
    let png_bytes = include_bytes!("../lockbook.png");
    let img = image::load_from_memory(png_bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Icon::from_rgba(img.into_raw(), width, height).ok()
}

pub fn run() {
    env_logger::init();

    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App { state: None, proxy: event_loop.create_proxy() };
    event_loop.run_app(&mut app).expect("event loop failed");
}

use std::sync::Arc;
use std::time::{Duration, Instant};

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
