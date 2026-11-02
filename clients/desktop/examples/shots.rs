//! Headless shell renderer for store screenshots.
//!
//! `LOCKBOOK_PATH=… cargo run -p lockbook-desktop --features win-chrome --example shots -- scene.txt out/`
//!
//! One command per line: `size W H SCALE` · `ready` · `open PATH` ·
//! `pane files|recents|shared` · `search [path|content]` · `sidebar on|off` · `sidebar-width W` · `expand PATH` · `settings` · `reveal` · `share PATH` ·
//! `key [cmd+][shift+]NAME [COUNT]` · `type TEXT` · `move X Y` · `click X Y` · `wheel DX DY` ·
//! `frames N` · `shot NAME` · `#` comments.

use std::path::PathBuf;
use std::time::Duration;

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, ViewportId, pos2, vec2};
use egui_wgpu_renderer::{egui_wgpu, wgpu};
use lockbook_desktop::ShellApp;
use lockbook_desktop::shell::action::{Action, SidebarPane};
use lockbook_desktop::shell::session::Session;
use workspace_rs::search::SearchType;

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SAMPLES: u32 = 4;

struct Shots {
    ctx: egui::Context,
    app: ShellApp,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
    size: (u32, u32),
    scale: f32,
    pointer: Pos2,
    last: Option<(Vec<egui::ClippedPrimitive>, f32)>,
    out: PathBuf,
    time: f64,
}

impl Shots {
    fn new(out: PathBuf) -> Self {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
            .expect("no gpu adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .expect("no device");

        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        workspace_rs::register_fonts(&mut fonts);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&ctx);
        if cfg!(feature = "win-chrome") {
            ctx.set_os(egui::os::OperatingSystem::Windows);
        }
        let font_system = workspace_rs::register_font_system(&ctx);
        let mut renderer = egui_wgpu::Renderer::new(
            &device,
            FORMAT,
            egui_wgpu::RendererOptions { msaa_samples: SAMPLES, ..Default::default() },
        );
        workspace_rs::register_render_callback_resources(
            &device,
            &queue,
            FORMAT,
            &mut renderer,
            font_system,
            SAMPLES,
        );

        Self {
            ctx,
            app: ShellApp::default(),
            device,
            queue,
            renderer,
            size: (1325, 917),
            scale: 2.0,
            pointer: pos2(0.0, 0.0),
            last: None,
            out,
            time: 0.0,
        }
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let (w, h) = self.size;
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32))),
            time: Some(self.time),
            events,
            max_texture_side: Some(self.device.limits().max_texture_dimension_2d as usize),
            ..Default::default()
        };
        raw.viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale);
        let mut out = self.ctx.run_pass(raw, |ctx| self.app.ui(ctx));
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                self.renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }
        }
        out.textures_delta.clear();
        let shapes = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        self.last = Some((shapes, out.pixels_per_point));
        // Freed textures are leaked: paint callbacks keep their own handles
        // and a scene is a few hundred frames.
        std::thread::sleep(Duration::from_millis(8));
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    fn ready(&mut self) {
        for _ in 0..3000 {
            if matches!(self.app.session, Session::Ready(_)) {
                self.frames(30);
                return;
            }
            self.frame(Vec::new());
            std::thread::sleep(Duration::from_millis(8));
        }
        panic!("session never became ready: {:?}", self.session_state());
    }

    fn session_state(&self) -> String {
        match &self.app.session {
            Session::Loading { .. } => "loading".into(),
            Session::Error(e) => format!("error: {e}"),
            Session::SignedOut { .. } => "signed out".into(),
            Session::Ready(_) => "ready".into(),
        }
    }

    fn id_for(&self, path: &str) -> lb::Uuid {
        let Session::Ready(r) = &self.app.session else {
            panic!("not ready");
        };
        r.workspace
            .core
            .get_by_path(path)
            .unwrap_or_else(|e| panic!("{path}: {e:?}"))
            .id
    }

    fn act(&mut self, a: Action) {
        self.app.push_action(a);
        self.frames(30);
    }

    fn key(&mut self, spec: &str) {
        let mut modifiers = Modifiers::NONE;
        let mut name = spec;
        while let Some((m, rest)) = name.split_once('+') {
            match m {
                "cmd" => modifiers.command = true,
                "shift" => modifiers.shift = true,
                "alt" => modifiers.alt = true,
                "ctrl" => modifiers.ctrl = true,
                _ => panic!("modifier {m}"),
            }
            name = rest;
        }
        let key = egui::Key::from_name(name).unwrap_or_else(|| panic!("key {name}"));
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
        self.frames(20);
    }

    fn click(&mut self, pos: Pos2) {
        self.pointer = pos;
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frames(2);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.frames(20);
    }

    fn shot(&mut self, name: &str) {
        self.frames(2);
        let (shapes, ppp) = self.last.as_ref().expect("no frame");
        let (w, h) =
            ((self.size.0 as f32 * self.scale) as u32, (self.size.1 as f32 * self.scale) as u32);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: *ppp };
        let size = wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 };
        let tex = |samples, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage,
                view_formats: &[],
            })
        };
        let msaa = tex(SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let target = tex(1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let msaa_view = msaa.create_view(&Default::default());
        let target_view = target.create_view(&Default::default());

        let mut encoder = self.device.create_command_encoder(&Default::default());
        let bufs =
            self.renderer
                .update_buffers(&self.device, &self.queue, &mut encoder, shapes, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &msaa_view,
                    resolve_target: Some(&target_view),
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), shapes, &screen);
        }
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = (w * 4).div_ceil(align) * align;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            size,
        );
        self.queue
            .submit(bufs.into_iter().chain(std::iter::once(encoder.finish())));
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::Wait { submission_index: None, timeout: None })
            .unwrap();
        let data = buffer.slice(..).get_mapped_range().unwrap();
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for row in data.chunks(padded as usize) {
            for px in row[..(w * 4) as usize].chunks(4) {
                rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
        }
        let path = self.out.join(format!("{name}.png"));
        image::RgbaImage::from_raw(w, h, rgba)
            .unwrap()
            .save(&path)
            .unwrap();
        println!("wrote {}", path.display());
    }

    fn run_line(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return;
        }
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let num = |s: &str| s.parse::<f32>().unwrap_or_else(|_| panic!("number: {s}"));
        match cmd {
            "size" => {
                let v: Vec<f32> = rest.split_whitespace().map(num).collect();
                self.size = (v[0] as u32, v[1] as u32);
                if let Some(s) = v.get(2) {
                    self.scale = *s;
                }
            }
            "ready" => self.ready(),
            "open" => {
                let id = self.id_for(rest);
                self.act(Action::OpenFile(id));
            }
            "share" => {
                let id = self.id_for(rest);
                self.act(Action::OpenShare(id));
            }
            "pane" => {
                let pane = match rest {
                    "files" => SidebarPane::Files,
                    "recents" => SidebarPane::Recents,
                    "shared" => SidebarPane::Shared,
                    _ => panic!("pane {rest}"),
                };
                self.act(Action::SelectPane(pane));
            }
            "sidebar" => {
                if self.app.sidebar_open != (rest == "on") {
                    self.act(Action::ToggleSidebar);
                }
            }
            "settings" => self.act(Action::OpenSettings),
            "sidecar" => self.act(Action::ToggleSidecar),
            "sidebar-width" => {
                // Same panel id the shell's sidebar uses.
                let w = num(rest);
                let id = self.ctx.viewport_id().root_ui_id().with("shell_sidebar");
                if let Some(mut state) = egui::containers::panel::PanelState::load(&self.ctx, id) {
                    state.outer_rect.set_right(state.outer_rect.left() + w);
                    self.ctx.data_mut(|d| d.insert_persisted(id, state));
                }
                self.frames(30);
            }
            "search" => {
                let kind = match rest {
                    "content" => SearchType::Content,
                    _ => SearchType::Path,
                };
                self.act(Action::OpenSearch(Some(kind)));
            }
            "reveal" => self.act(Action::RevealPhrase),
            "expand-id" => self.act(Action::ToggleExpand(rest.parse().expect("uuid"))),
            "expand" => {
                let id = self.id_for(rest);
                self.act(Action::ToggleExpand(id));
            }
            "key" => {
                let (spec, count) = rest.split_once(' ').unwrap_or((rest, "1"));
                for _ in 0..num(count) as usize {
                    self.key(spec);
                }
            }
            "type" => {
                self.frame(vec![Event::Text(rest.to_owned())]);
                self.frames(20);
            }
            "move" => {
                let v: Vec<f32> = rest.split_whitespace().map(num).collect();
                self.pointer = pos2(v[0], v[1]);
                self.frame(vec![Event::PointerMoved(self.pointer)]);
                self.frames(20);
            }
            "click" => {
                let v: Vec<f32> = rest.split_whitespace().map(num).collect();
                self.click(pos2(v[0], v[1]));
            }
            "wheel" => {
                // Positive dy scrolls content up (pointer wheel toward the user).
                let v: Vec<f32> = rest.split_whitespace().map(num).collect();
                self.frame(vec![Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(v[0], v[1]),
                    modifiers: Modifiers::NONE,
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Wheel,
                }]);
                self.frames(20);
            }
            "frames" => self.frames(num(rest) as usize),
            "dump" => println!(
                "session={} sidebar_open={} zen={} size={:?} scale={}",
                self.session_state(),
                self.app.sidebar_open,
                self.app.settings.zen_mode,
                self.size,
                self.scale
            ),
            "shot" => self.shot(rest),
            _ => panic!("unknown command: {line}"),
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let script = args.next().expect("usage: shots SCENE OUT_DIR");
    let out = PathBuf::from(args.next().expect("usage: shots SCENE OUT_DIR"));
    std::fs::create_dir_all(&out).unwrap();
    let text = std::fs::read_to_string(&script).unwrap();
    let mut shots = Shots::new(out);
    for line in text.lines() {
        shots.run_line(line);
    }
}
