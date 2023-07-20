use eframe::egui;
use lbeditor::{
    input::canonical::{Modification, Region},
    style::{InlineNode, MarkdownNode},
    Editor,
};

use crate::theme::Icon;

use super::Button;

#[derive(Clone)]
struct ToolbarButton {
    icon: Icon,
    id: String,
    callback: fn(&mut Editor, &mut ToolBar),
}
#[derive(Clone)]
pub struct ToolBar {
    pub margin: egui::Margin,
    id: egui::Id,
    pub has_focus: bool,
    buttons: Vec<ToolbarButton>,
    header_click_count: usize,
    visibility: ToolBarVisibility,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ToolBarVisibility {
    Minimized,
    Maximized,
    Disabled,
}

impl ToolBar {
    pub fn new(visibility: &ToolBarVisibility) -> Self {
        Self {
            margin: egui::Margin::symmetric(15.0, 0.0),
            buttons: get_buttons(visibility),
            header_click_count: 1,
            has_focus: false,
            visibility: visibility.to_owned(),
            id: egui::Id::null(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        // greedy focus toggle on the editor whenever the pointer is not in the toolbar
        let pointer = ui.ctx().pointer_hover_pos().unwrap_or_default();
        let toolbar_rect = self.calculate_rect(ui, editor);

        if toolbar_rect.contains(pointer) {
            if editor.has_focus == true {
                editor.has_focus = false
            }
        } else {
            self.header_click_count = 1;
            if editor.has_focus == false {
                editor.has_focus = true
            }
        }

        self.id = ui.id();

        ui.allocate_ui_at_rect(toolbar_rect, |ui| {
            egui::Frame::default()
                .fill(ui.visuals().code_bg_color)
                .inner_margin(self.margin)
                .shadow(egui::epaint::Shadow {
                    extrusion: ui.visuals().window_shadow.extrusion,
                    color: ui.visuals().window_shadow.color.gamma_multiply(0.3),
                })
                .rounding(egui::Rounding::same(20.0))
                .show(ui, |ui| self.map_buttons(ui, editor))
        });
    }

    fn map_buttons(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(10.0, 20.0);

            self.buttons.clone().iter().for_each(|btn| {
                let res = Button::default().icon(&btn.icon).show(ui);
                if res.hovered() {
                    if btn.id != "header" {
                        self.header_click_count = 1;
                    }
                }

                if res.clicked() {
                    (btn.callback)(editor, self);

                    ui.memory_mut(|w| {
                        w.request_focus(editor.debug_id);
                    });

                    ui.ctx().request_repaint();
                }
                if btn.id == "header" {
                    res.on_hover_text(format!("H{}", self.header_click_count));
                }
            });
        });
    }

    fn width(&self) -> f32 {
        let icon_width = 44; // icon default width is 34 and spacing is defined below as 10
        let width = icon_width * self.buttons.len() + self.margin.sum().x as usize;
        width as f32
    }

    /// center the toolbar relative to the editor
    fn calculate_rect(&self, ui: &mut egui::Ui, editor: &mut Editor) -> egui::Rect {
        let on = match self.visibility {
            ToolBarVisibility::Minimized | ToolBarVisibility::Disabled => true,
            ToolBarVisibility::Maximized => false,
        };
        let how_on = ui.ctx().animate_bool(egui::Id::from("toolbar_animate"), on);

        let maximized_min_x = (editor.ui_rect.width() - self.width()) / 2.0 + editor.ui_rect.left();

        let minimized_min_x =
            editor.ui_rect.max.x - (self.width() / self.buttons.len() as f32) - 40.0;

        let min_pos = egui::Pos2 {
            x: egui::lerp((maximized_min_x)..=(minimized_min_x), how_on),
            y: editor.ui_rect.bottom() - 90.0,
        };

        let maximized_max_x =
            editor.ui_rect.right() - (editor.ui_rect.width() - self.width()) / 2.0;
        let minimized_max_x = editor.ui_rect.right();

        let max_pos = egui::Pos2 {
            x: egui::lerp((maximized_max_x)..=(minimized_max_x), how_on),
            y: editor.ui_rect.bottom(),
        };

        match self.visibility {
            ToolBarVisibility::Maximized | ToolBarVisibility::Minimized => {
                egui::Rect { min: min_pos, max: max_pos }
            }
            ToolBarVisibility::Disabled => egui::Rect::NOTHING,
        }
    }
}

fn get_buttons(visibility: &ToolBarVisibility) -> Vec<ToolbarButton> {
    match visibility {
        ToolBarVisibility::Minimized => {
            vec![ToolbarButton {
                icon: Icon::VISIBILITY_ON,
                id: "visibility_on".to_string(),
                callback: |_, t| {
                    t.visibility = ToolBarVisibility::Maximized;
                    t.buttons = get_buttons(&t.visibility);
                },
            }]
        }
        ToolBarVisibility::Maximized => vec![
            ToolbarButton {
                icon: Icon::HEADER_1,
                id: "header".to_string(),
                callback: |e, t| {
                    e.custom_events
                        .push(Modification::Heading(t.header_click_count as u32));
                    if t.header_click_count > 5 {
                        t.header_click_count = 6;
                    } else {
                        t.header_click_count += 1;
                    }
                },
            },
            ToolbarButton {
                icon: Icon::BOLD,
                id: "bold".to_string(),
                callback: |e, _| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Bold),
                    })
                },
            },
            ToolbarButton {
                icon: Icon::ITALIC,
                id: "italic".to_string(),
                callback: |e, _| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Italic),
                    })
                },
            },
            ToolbarButton {
                icon: Icon::CODE,
                id: "in_line_code".to_string(),
                callback: |e, _| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: MarkdownNode::Inline(InlineNode::Code),
                    });
                },
            },
            ToolbarButton {
                icon: Icon::NUMBER_LIST,
                id: "number_list".to_string(),
                callback: |e, _| e.custom_events.push(Modification::NumberListItem),
            },
            ToolbarButton {
                icon: Icon::TODO_LIST,
                id: "todo_list".to_string(),
                callback: |e, _| e.custom_events.push(Modification::TodoListItem),
            },
            ToolbarButton {
                icon: Icon::VISIBILITY_OFF,
                id: "visibility_off".to_string(),
                callback: |_, t| {
                    t.visibility = ToolBarVisibility::Minimized;
                    t.buttons = get_buttons(&t.visibility);
                },
            },
        ],
        ToolBarVisibility::Disabled => vec![],
    }
}
