mod clip;
mod eraser;
mod history;
mod parser;
mod pen;
mod renderer;
mod selection;
mod toolbar;
mod util;
mod zoom;

use std::time::Instant;

use self::history::History;
use self::zoom::handle_zoom_input;
use crate::tab::svg_editor::toolbar::Toolbar;
pub use eraser::Eraser;
pub use history::DeleteElement;
pub use history::Event;
pub use history::InsertElement;
use lb_rs::Uuid;
pub use parser::Buffer;
pub use pen::CubicBezBuilder;
pub use pen::Pen;
use renderer::Renderer;
use resvg::usvg::{self, ImageKind};
pub use toolbar::Tool;
use usvg_parser::Options;

/// A shorthand for [ImageHrefResolver]'s string function.
pub type ImageHrefStringResolverFn = Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync>;

pub struct SVGEditor {
    buffer: parser::Buffer,
    history: History,
    pub toolbar: Toolbar,
    inner_rect: egui::Rect,
    core: lb_rs::Core,
    open_file: Uuid,
    skip_frame: bool,
    last_render: Instant,
    renderer: Renderer,
}

impl SVGEditor {
    pub fn new(bytes: &[u8], core: lb_rs::Core, open_file: Uuid) -> Self {
        let content = std::str::from_utf8(bytes).unwrap();

        let buffer = parser::Buffer::new(content, &core, open_file);
        let max_id = buffer
            .elements
            .keys()
            .filter_map(|key_str| key_str.parse::<usize>().ok())
            .max()
            .unwrap_or_default()
            + 1;

        let toolbar = Toolbar::new(max_id);

        Self {
            buffer,
            history: History::default(),
            toolbar,
            inner_rect: egui::Rect::NOTHING,
            core,
            open_file,
            skip_frame: false,
            last_render: Instant::now(),
            renderer: Renderer::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        if ui.input(|r| r.key_down(egui::Key::F12)) {
            let frame_cost = Instant::now() - self.last_render;
            self.last_render = Instant::now();
            let mut anchor_count = 0;
            self.buffer.elements.iter().for_each(|(_, el)| {
                if let parser::Element::Path(p) = el {
                    anchor_count += p.data.len()
                }
            });

            let mut top = self.inner_rect.right_top();
            top.x -= 150.0;
            ui.painter().debug_text(
                top,
                egui::Align2::LEFT_TOP,
                egui::Color32::RED,
                format!("{} anchor | {}fps", anchor_count, 1000 / frame_cost.as_millis()),
            );
        }
        ui.vertical(|ui| {
            egui::Frame::default()
                .fill(if ui.visuals().dark_mode {
                    egui::Color32::GRAY.gamma_multiply(0.03)
                } else {
                    ui.visuals().faint_bg_color
                })
                .show(ui, |ui| {
                    self.toolbar.show(
                        ui,
                        &mut self.buffer,
                        &mut self.history,
                        &mut self.skip_frame,
                        self.inner_rect,
                    );

                    self.inner_rect = ui.available_rect_before_wrap();
                    self.renderer
                        .render_svg(ui, &mut self.buffer, self.inner_rect);
                });
        });

        handle_zoom_input(ui, self.inner_rect, &mut self.buffer);

        let start_of_touch = ui.input(|i| {
            i.events.iter().any(|e| {
                if let egui::Event::Touch { device_id: _, id: _, phase, pos: _, force: _ } = e {
                    phase.eq(&egui::TouchPhase::Start)
                } else {
                    false
                }
            })
        });

        if ui.input(|r| r.multi_touch().is_some()) || self.skip_frame || start_of_touch {
            self.skip_frame = false;
            return;
        }

        match self.toolbar.active_tool {
            Tool::Pen => {
                self.toolbar.pen.handle_input(
                    ui,
                    self.inner_rect,
                    &mut self.buffer,
                    &mut self.history,
                );
            }
            Tool::Eraser => {
                self.toolbar.eraser.handle_input(
                    ui,
                    self.inner_rect,
                    &mut self.buffer,
                    &mut self.history,
                );
            }
            Tool::Selection => {
                self.toolbar.selection.handle_input(
                    ui,
                    self.inner_rect,
                    &mut self.buffer,
                    &mut self.history,
                );
            }
        }

        self.handle_clip_input(ui);
    }

    pub fn get_minimal_content(&self) -> String {
        self.buffer.to_string()
    }
}
