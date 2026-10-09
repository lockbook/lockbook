//! Custom winit + wgpu host for the product shell.

use std::sync::Arc;
use std::time::Instant;

use egui::{Pos2, ViewportId};
use egui_wgpu_renderer::RendererState;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Icon, Window, WindowId};
use workspace_rs::tab::{ClipContent, ExtendedInput};

use crate::{Output, ShellApp, WgpuLockbook};

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

        let icon_data = load_icon_data();
        let window_icon = icon_data
            .as_ref()
            .and_then(|i| Icon::from_rgba(i.rgba.clone(), i.width, i.height).ok());

        let mut window_attrs = Window::default_attributes()
            .with_title("Lockbook")
            .with_inner_size(LogicalSize::new(1300, 800))
            .with_window_icon(window_icon);

        // Frameless product chrome: shell paints its own titlebar / controls.
        // macOS keeps native traffic lights over fullsize content.
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS;
            window_attrs = window_attrs
                .with_fullsize_content_view(true)
                .with_titlebar_transparent(true)
                .with_title_hidden(true);
        }
        #[cfg(not(target_os = "macos"))]
        {
            window_attrs = window_attrs.with_decorations(false);
        }

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("failed to create window"),
        );

        #[cfg(target_os = "macos")]
        if let Some(icon) = &icon_data {
            set_macos_app_icon(&icon.rgba, icon.width, icon.height);
        }

        #[cfg(target_os = "macos")]
        {
            crate::shell::macos_window::disable_automatic_titlebar_drag(window.as_ref());
            crate::shell::macos_window::pin_traffic_lights(window.as_ref());
        }

        let mut lb = init_app(Arc::clone(&window));

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
            window.theme(),
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

        // Host owns paste (text + images/files). Skip forwarding Cmd+V /
        // Key::Paste to egui-winit so it does not also emit Event::Paste.
        if is_paste_shortcut(&state.egui_winit, &event) {
            state.pending_paste = true;
            state.window.request_redraw();
        } else {
            let response = state.egui_winit.on_window_event(&state.window, &event);

            if response.repaint && !matches!(event, WindowEvent::RedrawRequested) {
                state.window.request_redraw();
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                // Native file dialogs (NSOpenPanel) synthesize a window-close on
                // cancel. Ignore it while a picker is in flight.
                if crate::shell::native_file_dialog_open() {
                    return;
                }
                state.close_requested = true;
                state.window.request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.lb.renderer.screen.size_in_pixels = [size.width, size.height];
                #[cfg(target_os = "macos")]
                crate::shell::macos_window::pin_traffic_lights(state.window.as_ref());
                // Nested WM_SIZE loop: paint now or the swapchain lags the HWND.
                #[cfg(target_os = "windows")]
                if size.width > 0 && size.height > 0 {
                    state.render(event_loop);
                }
                #[cfg(not(target_os = "windows"))]
                {
                    state.window.request_redraw();
                }
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
        let size = self.window.inner_size();
        if size.width > 0 && size.height > 0 {
            self.lb.renderer.screen.size_in_pixels = [size.width, size.height];
        }

        #[cfg(target_os = "macos")]
        crate::shell::macos_window::pin_traffic_lights(self.window.as_ref());

        let mut raw_input = self.egui_winit.take_egui_input(&self.window);
        // Frameless chrome reads `viewport.maximized` to toggle restore. Without
        // this, the flag stays unset and double-click / caption max always send
        // Maximized(true). Skipped on macOS inside the helper (AppKit deadlock).
        {
            let info = raw_input
                .viewports
                .entry(raw_input.viewport_id)
                .or_default();
            egui_winit::update_viewport_info(info, &self.lb.renderer.context, &self.window, false);
        }

        // Images/files are not in egui's clipboard; text goes through arboard too.
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

        let Output { mut platform, viewport } = self.lb.frame();

        // Shell already saved tabs if close_requested was set this frame.
        if self.close_requested {
            std::process::exit(0);
        }

        for command in &platform.commands {
            match command {
                egui::OutputCommand::CopyText(text) => {
                    let _ = self.clipboard.set_text(text.clone());
                }
                egui::OutputCommand::CopyImage(image) => {
                    let bytes: Vec<u8> = image
                        .pixels
                        .iter()
                        .flat_map(|px| px.to_srgba_unmultiplied())
                        .collect();
                    let _ = self.clipboard.set_image(arboard::ImageData {
                        width: image.width(),
                        height: image.height(),
                        bytes: std::borrow::Cow::Owned(bytes),
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

        // Titlebar / shell chrome issues ViewportCommands (min/max/close/drag/resize).
        let mut actions: egui::ahash::HashSet<egui_winit::ActionRequested> =
            egui::ahash::HashSet::default();
        let mut info = egui::ViewportInfo::default();
        if let Some(vp) = viewport.values().next() {
            egui_winit::process_viewport_commands(
                &self.lb.renderer.context,
                &mut info,
                vp.commands.iter().cloned(),
                &self.window,
                &mut actions,
            );
        }

        if actions.contains(&egui_winit::ActionRequested::Paste) {
            self.handle_paste();
        }

        if let Some(pos) =
            crate::shell::titlebar::take_window_menu_request(&self.lb.renderer.context)
        {
            self.window
                .show_window_menu(winit::dpi::LogicalPosition::new(pos.x as f64, pos.y as f64));
        }

        #[cfg(target_os = "macos")]
        if crate::shell::titlebar::take_titlebar_double_click(&self.lb.renderer.context) {
            crate::shell::macos_window::perform_titlebar_double_click(self.window.as_ref());
        }

        // Title (and other viewport cmds) make AppKit re-layout the titlebar
        // after paint — Back is a common trigger. Re-pin after those cmds.
        #[cfg(target_os = "macos")]
        crate::shell::macos_window::pin_traffic_lights(self.window.as_ref());

        if info
            .events
            .iter()
            .any(|e| matches!(e, egui::ViewportEvent::Close))
        {
            self.close_requested = true;
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

fn is_paste_shortcut(egui_winit: &egui_winit::State, event: &WindowEvent) -> bool {
    let WindowEvent::KeyboardInput { event, .. } = event else {
        return false;
    };
    if event.state != ElementState::Pressed {
        return false;
    }
    if !egui_winit.egui_input().modifiers.command {
        return false;
    }
    match &event.logical_key {
        winit::keyboard::Key::Character(c) if c.eq_ignore_ascii_case("v") => true,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Paste) => true,
        _ => false,
    }
}

fn init_app(window: Arc<Window>) -> WgpuLockbook<'static> {
    // Safety: window is kept alive in Arc for lifetime of app
    let renderer = unsafe {
        RendererState::from_surface(wgpu::SurfaceTargetUnsafe::from_window(&window).unwrap())
    };

    init_with_renderer(renderer)
}

fn init_with_renderer(mut renderer: RendererState<'static>) -> WgpuLockbook<'static> {
    let mut fonts = egui::FontDefinitions::default();
    workspace_rs::register_fonts(&mut fonts);
    renderer.context.set_fonts(fonts);
    egui_extras::install_image_loaders(&renderer.context);

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

    let mut lb = WgpuLockbook {
        renderer,
        queued_events: Default::default(),
        double_queued_events: Default::default(),
        app: ShellApp::default(),
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

#[cfg(target_os = "linux")]
fn x11_dpi_configured() -> bool {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    (|| -> Option<bool> {
        let (conn, screen_num) = x11rb::connect(None).ok()?;

        let xsettings_atom = conn
            .intern_atom(false, format!("_XSETTINGS_S{screen_num}").as_bytes())
            .ok()?
            .reply()
            .ok()?
            .atom;
        let xsettings_owner = conn
            .get_selection_owner(xsettings_atom)
            .ok()?
            .reply()
            .ok()?
            .owner;
        if xsettings_owner != x11rb::NONE {
            return Some(true);
        }

        let root = conn.setup().roots.get(screen_num)?.root;
        let reply = conn
            .get_property(false, root, AtomEnum::RESOURCE_MANAGER, AtomEnum::STRING, 0, u32::MAX)
            .ok()?
            .reply()
            .ok()?;
        let resources = String::from_utf8_lossy(&reply.value);
        Some(resources_declare_xft_dpi(&resources))
    })()
    .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn resources_declare_xft_dpi(resources: &str) -> bool {
    resources
        .lines()
        .filter_map(|line| line.split_once(':'))
        .any(|(key, value)| key.trim() == "Xft.dpi" && !value.trim().is_empty())
}

/// Product shell.
pub fn run() {
    env_logger::init();

    let config = lb::model::core_config::Config::ui_config("egui");
    lb::service::logging::init(&config).expect("install lockbook logging");
    crate::perf::install_exit_flush();

    let mut builder = EventLoop::<UserEvent>::with_user_event();

    #[cfg(target_os = "linux")]
    {
        // winit's Wayland backend doesn't deliver file drag-and-drop events. Default to X11
        // unless the user has opted in via settings (or set WINIT_UNIX_BACKEND).
        use winit::platform::x11::EventLoopBuilderExtX11;

        if std::env::var_os("WINIT_X11_SCALE_FACTOR").is_none() && !x11_dpi_configured() {
            // SAFETY: set before event loop; single-threaded main.
            unsafe {
                std::env::set_var("WINIT_X11_SCALE_FACTOR", "1");
            }
        }

        let allow_wayland = crate::Settings::read_from_file()
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
