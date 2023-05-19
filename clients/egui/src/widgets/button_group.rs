use eframe::{egui, epaint};

pub struct ButtonGroup<T: Copy + PartialEq> {
    value: Option<ToggleValue<T>>,
    buttons: Vec<(T, ButtonContent)>,
    center: bool,
}

impl<T: Copy + PartialEq> Default for ButtonGroup<T> {
    fn default() -> Self {
        Self { value: None, buttons: Vec::new(), center: false }
    }
}

impl<T: Copy + PartialEq> ButtonGroup<T> {
    pub fn toggle(value_copy: T) -> Self {
        let value = Some(ToggleValue::Copied(value_copy));

        Self { value, ..Self::default() }
    }

    pub fn btn(self, b: T, w: impl Into<egui::WidgetText>) -> Self {
        let mut this = self;
        this.buttons.push((b, ButtonContent::Text(w.into())));
        this
    }

    pub fn btn_icon(self, b: T, w: impl Into<egui::WidgetText>) -> Self {
        let mut this = self;
        this.buttons.push((b, ButtonContent::Icon(w.into())));
        this
    }

    pub fn hcenter(self) -> Self {
        Self { center: true, ..self }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<T> {
        if self.buttons.is_empty() {
            return None;
        }

        if self.center {
            let total_width = self.dims(ui).x * (self.buttons.len() as f32);
            ui.add_space(
                ui.available_size_before_wrap().x / 2.0
                    - total_width / 2.0
                    - ui.spacing().item_spacing.x * 2.5,
            );
        }

        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

        let where_to_put_background = ui.painter().add(egui::Shape::Noop);

        let (resp, maybe_clicked) = self.draw_buttons(ui);
        ui.painter().set(
            where_to_put_background,
            epaint::RectShape {
                rect: resp.rect,
                rounding: egui::Rounding::same(5.0),
                fill: ui.visuals().extreme_bg_color,
                stroke: ui.visuals().widgets.noninteractive.fg_stroke,
            },
        );

        maybe_clicked
    }

    fn draw_buttons(&mut self, ui: &mut egui::Ui) -> (egui::Response, Option<T>) {
        let dims = self.dims(ui);

        let (mut ret_resp, mut ret_maybe_clicked) = self.draw_single_button(ui, 0, dims);

        for i in 1..self.buttons.len() {
            let (resp, maybe_clicked) = self.draw_single_button(ui, i, dims);

            ret_resp = ret_resp.union(resp);
            ret_maybe_clicked = ret_maybe_clicked.or(maybe_clicked);
        }

        (ret_resp, ret_maybe_clicked)
    }

    fn draw_single_button(
        &mut self, ui: &mut egui::Ui, index: usize, dims: egui::Vec2,
    ) -> (egui::Response, Option<T>) {
        let mut clicked: Option<T> = None;

        let btn = &self.buttons[index];
        let wrap_width = ui.available_width();
        let padding = egui::pos2(8.0, 8.0);

        let desired_size = egui::vec2(dims.x + padding.x * 2.0, dims.y + padding.y * 2.0);

        let (rect, resp) = ui.allocate_at_least(desired_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&resp);

            let is_first = index == 0;
            let is_last = index == self.buttons.len() - 1;

            let west = if is_first { 5.0 } else { 0.0 };
            let east = if is_last { 5.0 } else { 0.0 };

            let rounding = egui::Rounding { nw: west, ne: east, sw: west, se: east };

            let fill = if self.value_is(btn.0) {
                ui.style().visuals.faint_bg_color
            } else if resp.hovered() {
                ui.style().visuals.widgets.hovered.bg_fill
            } else {
                ui.style().visuals.widgets.inactive.bg_fill
            };

            ui.painter().rect(rect, rounding, fill, egui::Stroke::NONE);

            match &btn.1 {
                ButtonContent::Text(wtxt) => {
                    let text = wtxt.clone().into_galley(
                        ui,
                        Some(false),
                        wrap_width,
                        egui::TextStyle::Body,
                    );

                    let text_pos = rect.center() - text.size() / 2.0;

                    text.paint_with_visuals(ui.painter(), text_pos, visuals);
                }
                ButtonContent::Icon(wtxt) => {
                    let text = wtxt.clone().into_galley(
                        ui,
                        Some(false),
                        wrap_width,
                        egui::TextStyle::Body,
                    );

                    let text_pos = egui::pos2(
                        rect.center().x - text.size().x / 2.0,
                        rect.center().y - text.size().y / 4.0,
                    );

                    text.paint_with_visuals(ui.painter(), text_pos, visuals);
                } // TODO layout widget
            }
        };

        if resp.clicked() {
            clicked = Some(btn.0);
        }

        if index != self.buttons.len() - 1 {
            ui.painter().vline(
                rect.max.x,
                rect.y_range(),
                (1.0, ui.visuals().widgets.inactive.fg_stroke.color),
            );
        }

        (resp, clicked)
    }

    fn dims(&self, ui: &egui::Ui) -> egui::Vec2 {
        let mut dims = egui::vec2(0.0, 0.0);
        for (_, content) in &self.buttons {
            let size = content.size(ui);
            if size.x > dims.x {
                dims.x = size.x;
            }
            if size.y > dims.y {
                dims.y = size.y;
            }
        }
        dims
    }

    fn value_is(&self, v: T) -> bool {
        if let Some(value) = &self.value {
            match value {
                ToggleValue::Copied(value) => *value == v,
            }
        } else {
            false
        }
    }
}

enum ToggleValue<T> {
    Copied(T),
}

enum ButtonContent {
    Text(egui::WidgetText),
    Icon(egui::WidgetText),
    //Widget { size: egui::Vec2, widget: Box<dyn egui::Widget> },
}

impl ButtonContent {
    fn size(&self, ui: &egui::Ui) -> egui::Vec2 {
        let wrap_width = ui.available_width();
        match self {
            ButtonContent::Text(wtxt) => wtxt
                .clone()
                .into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body)
                .size(),
            ButtonContent::Icon(wtxt) => wtxt
                .clone()
                .into_galley(ui, Some(false), wrap_width, egui::TextStyle::Body)
                .size(),
            //ButtonContent::Widget { size, .. } => *size,
        }
    }
}
