use std::borrow::BorrowMut;
use std::f32::consts::PI;
use std::fmt::Write;
use std::sync::mpsc;

use eframe::egui;
use minidom::Element;
use resvg::tiny_skia::{Path, PathBuilder, PathSegment, Pixmap};
use resvg::usvg::{self, Size, TreeWriting};

use crate::theme::{Icon, ThemePalette};
use crate::widgets::Button;
const ICON_SIZE: f32 = 30.0;
const COLOR_SWATCH_BTN_RADIUS: f32 = 9.0;
const THICKNESS_BTN_X_MARGIN: f32 = 5.0;
const THICKNESS_BTN_WIDTH: f32 = 30.0;
pub const INITIAL_SVG_CONTENT: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" ></svg>";
const ZOOM_STOP: f32 = 0.1;

pub struct SVGEditor {
    pub content: String,
    root: Element,
    draw_rx: mpsc::Receiver<(egui::Pos2, usize)>,
    draw_tx: mpsc::Sender<(egui::Pos2, usize)>,
    path_builder: PathBuilder,
    zoom_factor: f32,
    id_counter: usize,
    toolbar: Toolbar,
    inner_rect: egui::Rect,
    sao_offset: egui::Vec2,
}

struct Toolbar {
    components: Vec<Component>,
    active_color: Option<ColorSwatch>,
    active_stroke_width: u32,
}

impl SVGEditor {
    pub fn boxed(bytes: &[u8], _ctx: &egui::Context) -> Box<Self> {
        let (draw_tx, draw_rx) = mpsc::channel();

        // todo: handle invalid utf8
        let mut content = std::str::from_utf8(bytes).unwrap().to_string();
        if content.is_empty() {
            content = INITIAL_SVG_CONTENT.to_string();
        }
        println!("{}", content);
        let root: Element = content.parse().unwrap();

        let components = vec![
            Component::Button(SimpleButton {
                icon: Icon::UNDO,
                callback: || {},
                margin: egui::Margin::symmetric(4.0, 7.0),
                coming_soon_text: Some(
                    "Undo/Redo will be added in the next version. Stay Tuned!".to_string(),
                ),
            }),
            Component::Button(SimpleButton {
                icon: Icon::REDO,
                callback: || {},
                margin: egui::Margin::symmetric(4.0, 7.0),
                coming_soon_text: Some(
                    "Undo/Redo will be added in the next version. Stay Tuned!".to_string(),
                ),
            }),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
            Component::Button(SimpleButton {
                icon: Icon::BRUSH,
                callback: || {},
                coming_soon_text: None,
                margin: egui::Margin::symmetric(4.0, 7.0),
            }),
            Component::Button(SimpleButton {
                icon: Icon::ERASER,
                callback: || {},
                coming_soon_text: Some(
                    "Eraser will be added in the next version. Stay Tuned!".to_string(),
                ),
                margin: egui::Margin::symmetric(4.0, 7.0),
            }),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
            Component::StrokeWidth(3),
            Component::StrokeWidth(6),
            Component::StrokeWidth(9),
            Component::Separator(egui::Margin::symmetric(10.0, 0.0)),
        ];

        let toolbar = Toolbar { components, active_color: None, active_stroke_width: 3 };

        Box::new(Self {
            content,
            draw_rx,
            draw_tx,
            id_counter: 0,
            root,
            toolbar,
            sao_offset: egui::vec2(0.0, 0.0),
            path_builder: PathBuilder::new(),
            inner_rect: egui::Rect::NOTHING,
            zoom_factor: 1.0,
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.setup_draw_events(ui);
        self.draw_event_handler();

        self.define_dynamic_colors(ui);

        ui.vertical(|ui| {
            egui::Frame::default()
                .fill(if ui.visuals().dark_mode {
                    egui::Color32::GRAY.gamma_multiply(0.03)
                } else {
                    ui.visuals().faint_bg_color
                })
                .show(ui, |ui| {
                    self.toolbar.show(ui);
                    ui.set_width(ui.available_width());
                });

            self.inner_rect = ui.available_rect_before_wrap();
            self.render_svg(ui);
        });
    }

    pub fn get_minimal_content(&self) -> String {
        let utree: usvg::Tree =
            usvg::TreeParsing::from_data(self.content.as_bytes(), &usvg::Options::default())
                .unwrap();
        utree.to_string(&usvg::XmlOptions::default())
    }
    fn render_svg(&mut self, ui: &mut egui::Ui) {
        let mut utree: usvg::Tree =
            usvg::TreeParsing::from_data(self.content.as_bytes(), &usvg::Options::default())
                .unwrap();
        let available_rect = ui.available_rect_before_wrap();
        utree.size = Size::from_wh(available_rect.width(), available_rect.height()).unwrap();

        utree.view_box.rect = usvg::NonZeroRect::from_ltrb(
            available_rect.left(),
            available_rect.top(),
            available_rect.right(),
            available_rect.bottom(),
        )
        .unwrap();

        let tree = resvg::Tree::from_usvg(&utree);

        let pixmap_size = tree.size.to_int_size();
        let mut pixmap = Pixmap::new(pixmap_size.width(), pixmap_size.height()).unwrap();

        tree.render(usvg::Transform::default(), &mut pixmap.as_mut());
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [pixmap.width() as usize, pixmap.height() as usize],
            pixmap.data(),
        );

        let texture = ui
            .ctx()
            .load_texture("svg_image", image, egui::TextureOptions::LINEAR);

        self.sao_offset = egui::ScrollArea::both()
            .show(ui, |ui| {
                ui.add(
                    egui::Image::new(
                        &texture,
                        egui::vec2(
                            texture.size()[0] as f32 * self.zoom_factor,
                            texture.size()[1] as f32 * self.zoom_factor,
                        ),
                    )
                    .sense(egui::Sense::click()),
                );
            })
            .state
            .offset;
    }

    fn define_dynamic_colors(&mut self, ui: &mut egui::Ui) {
        if self.root.attr("data-dark-mode").is_none() {
            self.root
                .set_attr("data-dark-mode", format!("{}", ui.visuals().dark_mode));
            self.build_color_defs(ui);
        }

        if let Some(svg_flag) = self.root.attr("data-dark-mode") {
            let svg_flag: bool = svg_flag.parse().unwrap_or(false);

            if svg_flag != ui.visuals().dark_mode {
                self.build_color_defs(ui);
                self.root
                    .set_attr("data-dark-mode", format!("{}", ui.visuals().dark_mode));
            }
        }
    }

    fn build_color_defs(&mut self, ui: &mut egui::Ui) {
        let theme_colors = ThemePalette::as_array(ui.visuals().dark_mode);
        if self.toolbar.active_color.is_none() {
            self.toolbar.active_color = Some(ColorSwatch {
                id: "fg".to_string(),
                color: theme_colors.iter().find(|p| p.0.eq("fg")).unwrap().1,
            });
        }

        let btns = theme_colors.iter().map(|theme_color| {
            Component::ColorSwatch(ColorSwatch { id: theme_color.0.clone(), color: theme_color.1 })
        });
        self.toolbar.components = self
            .toolbar
            .components
            .clone()
            .into_iter()
            .filter(|c| !matches!(c, Component::ColorSwatch(_)))
            .chain(btns)
            .collect();

        theme_colors.iter().for_each(|theme_color| {
            let rgb_color =
                format!("rgb({} {} {})", theme_color.1.r(), theme_color.1.g(), theme_color.1.b());
            let gradient = Element::builder("linearGradient", "")
                .attr("id", theme_color.0.as_str())
                .append(
                    Element::builder("stop", "")
                        .attr("stop-color", rgb_color)
                        .build(),
                )
                .build();
            self.root.borrow_mut().append_child(gradient);

            let mut buffer = Vec::new();
            self.root.write_to(&mut buffer).unwrap();
            self.content = std::str::from_utf8(&buffer).unwrap().to_string();
            self.content = self.content.replace("xmlns='' ", "");
        });
    }

    fn draw_event_handler(&mut self) {
        while let Ok((pos, id)) = self.draw_rx.try_recv() {
            let mut current_path = self.root.children_mut().find(|e| {
                if let Some(id) = e.attr("id") {
                    id == self.id_counter.to_string()
                } else {
                    false
                }
            });

            if let Some(node) = current_path.as_mut() {
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();
                let data = get_path_data(path);

                node.set_attr("d", data);

                if let Some(color) = &self.toolbar.active_color {
                    node.set_attr("stroke", format!("url(#{})", color.id));
                } else {
                    node.set_attr("stroke", "url(#fg)");
                }
            } else {
                self.path_builder.clear();

                self.path_builder.move_to(pos.x, pos.y);
                self.path_builder.line_to(pos.x, pos.y);
                let path = self.path_builder.clone().finish().unwrap();
                let data = get_path_data(path);
                let child = Element::builder("path", "")
                    .attr("stroke-width", self.toolbar.active_stroke_width.to_string())
                    .attr("fill", "none")
                    .attr("stroke-linejoin", "round")
                    .attr("stroke-linecap", "round")
                    .attr("id", id)
                    .attr("d", data)
                    .build();

                self.root.append_child(child);
            }

            let mut buffer = Vec::new();

            self.root.write_to(&mut buffer).unwrap();
            self.content = std::str::from_utf8(&buffer).unwrap().to_string();
            self.content = self.content.replace("xmlns='' ", "");
        }
    }

    fn setup_draw_events(&mut self, ui: &mut egui::Ui) {
        if let Some(mut cursor_pos) = ui.ctx().pointer_hover_pos() {
            if !self.inner_rect.contains(cursor_pos) || !ui.is_enabled() {
                return;
            }

            if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::PlusEquals)) {
                self.zoom_factor += ZOOM_STOP;
            }

            if ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Minus)) {
                self.zoom_factor -= ZOOM_STOP;
            }

            println!("{:#?}", self.sao_offset);
            println!("{:#?}", cursor_pos);

            cursor_pos.x = (cursor_pos.x + self.sao_offset.x) / self.zoom_factor;
            cursor_pos.y = (cursor_pos.y + self.sao_offset.y) / self.zoom_factor;

            if ui.input(|i| i.pointer.primary_down()) {
                self.draw_tx.send((cursor_pos, self.id_counter)).unwrap();
            }
            if ui.input(|i| i.pointer.primary_released()) {
                self.id_counter += 1;
            }
        }
    }
}

fn get_path_data(path: Path) -> String {
    let mut s = String::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => s.write_str(format!("M {} {} ", p.x, p.y).as_str()),
            PathSegment::LineTo(p) => s.write_str(format!("L {} {} ", p.x, p.y).as_str()),
            PathSegment::QuadTo(p0, p1) => {
                s.write_str(format!("Q {} {} {} {} ", p0.x, p0.y, p1.x, p1.y).as_str())
            }
            PathSegment::CubicTo(p0, p1, p2) => s.write_str(
                format!("C {} {} {} {} {} {} ", p0.x, p0.y, p1.x, p1.y, p2.x, p2.y).as_str(),
            ),
            PathSegment::Close => s.write_str("Z "),
        }
        .unwrap();
    }

    s.pop(); // ' '
    s
}

#[derive(Clone)]
enum Component {
    Button(SimpleButton),
    ColorSwatch(ColorSwatch),
    StrokeWidth(u32),
    Separator(egui::Margin),
}
#[derive(Clone)]
struct SimpleButton {
    icon: Icon,
    callback: fn(),
    margin: egui::Margin,
    coming_soon_text: Option<String>,
}
#[derive(Clone)]
struct ColorSwatch {
    id: String,
    color: egui::Color32,
}

trait SizableComponent {
    fn get_width(&self) -> f32;
}
impl SizableComponent for Component {
    fn get_width(&self) -> f32 {
        match self {
            Component::Button(btn) => btn.margin.sum().x + ICON_SIZE,
            Component::Separator(margin) => margin.sum().x,
            Component::ColorSwatch(_color_btn) => COLOR_SWATCH_BTN_RADIUS * PI,
            Component::StrokeWidth(_) => THICKNESS_BTN_WIDTH + THICKNESS_BTN_X_MARGIN * 2.0,
        }
    }
}

impl Toolbar {
    fn width(&self) -> f32 {
        self.components.iter().map(|c| c.get_width()).sum()
    }
    fn calculate_rect(&self, ui: &mut egui::Ui) -> egui::Rect {
        let height = 0.0;
        let available_rect = ui.available_rect_before_wrap();

        let maximized_min_x = (available_rect.width() - self.width()) / 2.0 + available_rect.left();

        let min_pos = egui::Pos2 { x: maximized_min_x, y: available_rect.top() + height };

        let maximized_max_x =
            available_rect.right() - (available_rect.width() - self.width()) / 2.0;

        let max_pos = egui::Pos2 { x: maximized_max_x, y: available_rect.top() };
        egui::Rect { min: min_pos, max: max_pos }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        let rect = self.calculate_rect(ui);

        ui.allocate_ui_at_rect(rect, |ui| {
            ui.horizontal(|ui| {
                self.components.iter().for_each(|c| match c {
                    Component::Button(btn) => {
                        egui::Frame::default()
                            .inner_margin(btn.margin)
                            .show(ui, |ui| {
                                let btn_res = Button::default().icon(&btn.icon).show(ui);

                                if btn_res.clicked() {
                                    (btn.callback)();
                                }

                                if let Some(tooltip_text) = &btn.coming_soon_text {
                                    btn_res.on_hover_text(tooltip_text);
                                }
                            });
                    }
                    Component::Separator(margin) => {
                        ui.add_space(margin.right);
                        ui.add(egui::Separator::default().shrink(ui.available_height() * 0.3));
                        ui.add_space(margin.left);
                    }
                    Component::ColorSwatch(btn) => {
                        let (response, painter) = ui.allocate_painter(
                            egui::vec2(COLOR_SWATCH_BTN_RADIUS * PI, ui.available_height()),
                            egui::Sense::click(),
                        );
                        if response.clicked() {
                            self.active_color =
                                Some(ColorSwatch { id: btn.id.clone(), color: btn.color });
                        }
                        if let Some(active_color) = &self.active_color {
                            let opacity = if active_color.id.eq(&btn.id) {
                                1.0
                            } else if response.hovered() {
                                ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::PointingHand);
                                0.9
                            } else {
                                0.5
                            };

                            if active_color.id.eq(&btn.id) {
                                painter.rect_filled(
                                    response.rect.shrink2(egui::vec2(0.0, 5.0)),
                                    egui::Rounding::same(8.0),
                                    btn.color.gamma_multiply(0.2),
                                )
                            }
                            painter.circle_filled(
                                response.rect.center(),
                                COLOR_SWATCH_BTN_RADIUS,
                                btn.color.gamma_multiply(opacity),
                            );
                        }
                    }
                    Component::StrokeWidth(thickness) => {
                        ui.add_space(THICKNESS_BTN_X_MARGIN);
                        let (response, painter) = ui.allocate_painter(
                            egui::vec2(THICKNESS_BTN_WIDTH, ui.available_height()),
                            egui::Sense::click(),
                        );

                        let rect = egui::Rect {
                            min: egui::Pos2 {
                                x: response.rect.left(),
                                y: response.rect.center().y - (*thickness as f32 / 3.0),
                            },
                            max: egui::Pos2 {
                                x: response.rect.right(),
                                y: response.rect.center().y + (*thickness as f32 / 3.0),
                            },
                        };

                        if thickness.eq(&self.active_stroke_width) {
                            painter.rect_filled(
                                response.rect.shrink2(egui::vec2(0.0, 5.0)),
                                egui::Rounding::same(8.0),
                                egui::Color32::GRAY.gamma_multiply(0.1),
                            )
                        }
                        if response.clicked() {
                            self.active_stroke_width = *thickness;
                        }
                        if response.hovered() {
                            ui.output_mut(|w| w.cursor_icon = egui::CursorIcon::PointingHand);
                        }

                        painter.rect_filled(
                            rect,
                            egui::Rounding::same(2.0),
                            ui.visuals().text_color().gamma_multiply(0.8),
                        );
                        ui.add_space(THICKNESS_BTN_X_MARGIN);
                    }
                });
            });
        });
        ui.visuals_mut().widgets.noninteractive.bg_stroke.color = ui
            .visuals()
            .widgets
            .noninteractive
            .bg_stroke
            .color
            .gamma_multiply(0.4);
        ui.separator();
    }
}
