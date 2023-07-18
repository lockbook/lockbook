use std::{thread, time::Duration};

use eframe::egui;
use egui_winit::egui::Event;
use lbeditor::{
    element::Element,
    input::canonical::{Modification, Region},
    Editor,
};

use crate::theme::Icon;

use super::Button;

#[derive(Clone)]
struct ToolbarButton {
    icon: Icon,
    callback: fn(&mut Editor, &mut ToolBar),
}
#[derive(Clone)]
pub struct ToolBar {
    pub margin: egui::Margin,
    id: egui::Id,
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
        let margin = egui::Margin::symmetric(15.0, 0.0);
        let buttons = vec![
            ToolbarButton {
                icon: Icon::HEADER_1,
                callback: |e, state| {
                    e.custom_events
                        .push(Modification::Heading(state.header_click_count as u32));
                    if state.header_click_count > 4 {
                        state.header_click_count = 1;
                    } else {
                        state.header_click_count += 1;
                    }
                },
            },
            ToolbarButton {
                icon: Icon::BOLD,
                callback: |e, _| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: Element::Strong,
                    })
                },
            },
            ToolbarButton {
                icon: Icon::ITALIC,
                callback: |e, _| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: Element::Emphasis,
                    })
                },
            },
            ToolbarButton {
                icon: Icon::CODE,
                callback: |e, _| {
                    e.custom_events.push(Modification::ToggleStyle {
                        region: Region::Selection,
                        style: Element::InlineCode,
                    });
                },
            },
            ToolbarButton {
                icon: Icon::NUMBER_LIST,
                callback: |e, _| e.custom_events.push(Modification::NumberListItem),
            },
            ToolbarButton {
                icon: Icon::TODO_LIST,
                callback: |e, _| e.custom_events.push(Modification::TodoListItem),
            },
            ToolbarButton {
                icon: Icon::VISIBILITY_OFF,
                callback: |_, state| {
                    state.visibility = ToolBarVisibility::Minimized;
                },
            },
        ];
        Self {
            margin,
            buttons,
            header_click_count: 1,
            visibility: visibility.to_owned(),
            id: egui::Id::null(),
        }
    }
}

impl ToolBar {
    pub fn show(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let pointer = ui.ctx().pointer_hover_pos().unwrap_or_default();
        let toolbar_rect = self.calculate_rect(ui, editor);

        if !toolbar_rect.contains(pointer) {
            self.header_click_count = 1;
            // if editor.ui_rect.contains(pointer) {
            //     ui.memory_mut(|w| {
            //         if !w.has_focus(editor.debug_id) {
            //             w.request_focus(editor.debug_id)
            //         }
            //     });
            // }
        } else {
        }

        ui.memory_mut(|w| {
            println!("{}", w.focus().unwrap_or(egui::Id::null()).short_debug_format())
        });
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
        println!("editor` has focus? {}", ui.memory_mut(|w| w.has_focus(editor.debug_id)));

        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(10.0, 20.0);

            if matches!(self.visibility, ToolBarVisibility::Minimized) {
                let btn_ui = Button::default().icon(&Icon::VISIBILITY_ON).show(ui);

                if btn_ui.hovered() {
                    btn_ui.request_focus();
                }
                if btn_ui.clicked() {
                    self.visibility = ToolBarVisibility::Maximized;
                }
            } else {
                self.buttons.clone().iter().for_each(|btn| {
                    let btn_ui = Button::default().icon(&btn.icon).show(ui);
                    if btn_ui.hovered() {
                        btn_ui.request_focus();
                    }
                    if btn_ui.clicked() {
                        println!("btn has focus? {}", btn_ui.has_focus());
                        println!(
                            "editor` has focus? {}",
                            ui.memory_mut(|w| w.has_focus(editor.debug_id))
                        );

                        (btn.callback)(editor, self);

                        if btn.icon.icon != Icon::VISIBILITY_OFF.icon {
                            ui.memory_mut(|w| {
                                w.request_focus(editor.debug_id);
                            });
                           
                        }
                        if btn.icon.icon != Icon::HEADER_1.icon {
                            self.header_click_count = 1;
                        }
                    }
                    if btn_ui.lost_focus() {
                        println!("lost");
                    }
                });
            }
        });
    }

    fn width(&self) -> f32 {
        let icon_width = 44; // icon default width is 34 and spacing is defined below as 10
        let width = icon_width * self.buttons.len() + self.margin.sum().x as usize;
        width as f32
    }

    fn calculate_rect(&self, ui: &mut egui::Ui, editor: &mut Editor) -> egui::Rect {
        ui.style_mut().animation_time = 0.1;

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
