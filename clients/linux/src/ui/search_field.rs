use std::cell::RefCell;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;

use crate::ui;
use crate::ui::icons;

#[derive(Clone, Debug, Default)]
pub struct SearchField {
    pub real_input: Rc<RefCell<String>>,
    pub entry: gtk::Entry,
    pub result_list_cntr: gtk::Box,
    pub result_list: gtk::ListBox,
    pub loading: gtk::Spinner,
    no_results: gtk::Label,
    on_update: Rc<RefCell<Func>>,
    on_activate: Rc<RefCell<Func>>,
    on_blur: Rc<RefCell<Func>>,
}

impl SearchField {
    pub fn init(&self) {
        self.result_list.set_hexpand(true);
        self.result_list.connect_row_activated({
            let on_activate = self.on_activate.clone();
            move |_, _| on_activate.borrow().0()
        });

        self.result_list.connect_row_selected({
            let entry = self.entry.clone();
            let real_input = self.real_input.clone();

            move |_, maybe_row| {
                if let Some(row) = maybe_row {
                    let path = row
                        .child()
                        .unwrap()
                        .downcast_ref::<ui::SearchRow>()
                        .unwrap()
                        .path();
                    entry.set_text(&path);
                    entry.select_region(0, -1);
                } else {
                    // fill in user entered text
                    entry.set_text(&real_input.borrow());
                    entry.set_position(-1);
                }
            }
        });

        let result_area_inner = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        result_area_inner.set_width_request(400);
        result_area_inner.add_css_class("contents");
        result_area_inner.append(&self.result_list);

        self.result_list_cntr
            .set_orientation(gtk::Orientation::Vertical);
        self.result_list_cntr.add_css_class("view");
        self.result_list_cntr.set_width_request(400);
        self.result_list_cntr.set_halign(gtk::Align::Center);
        self.result_list_cntr.set_valign(gtk::Align::Start);
        self.result_list_cntr.append(&result_area_inner);

        self.entry.set_width_request(400);
        self.entry.set_primary_icon_name(Some(icons::SEARCH));

        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter({
            let result_list_cntr = self.result_list_cntr.clone();
            move |_| result_list_cntr.show()
        });
        focus.connect_leave({
            let result_list_cntr = self.result_list_cntr.clone();
            move |_| result_list_cntr.hide()
        });
        self.entry.add_controller(&focus);

        let search_key_press = gtk::EventControllerKey::new();
        search_key_press.set_propagation_phase(gtk::PropagationPhase::Capture);
        search_key_press.connect_key_pressed({
            let this = self.clone();

            move |_, key, code, _| {
                if key == gdk::Key::Escape {
                    this.on_blur.borrow().0();
                    this.entry.set_text("");
                    while let Some(row) = this.result_list.row_at_index(0) {
                        this.result_list.remove(&row);
                    }
                    this.loading.hide();
                    this.no_results.hide();
                } else if code == ARROW_DOWN {
                    let next_index = this
                        .result_list
                        .selected_row()
                        .map(|row| row.index() + 1)
                        .unwrap_or_default();
                    if next_index == 0 {
                        *this.real_input.borrow_mut() = this.entry.text().to_string();
                    }
                    this.result_list
                        .select_row(this.result_list.row_at_index(next_index).as_ref());
                } else if code == ARROW_UP {
                    let mut prev_index = this
                        .result_list
                        .selected_row()
                        .map(|row| row.index() - 1)
                        .unwrap_or(-2);
                    if prev_index == -2 {
                        prev_index = n_listbox_rows(&this.result_list) as i32;
                        *this.real_input.borrow_mut() = this.entry.text().to_string();
                    }
                    this.result_list
                        .select_row(this.result_list.row_at_index(prev_index).as_ref());
                } else if code == ENTER {
                    this.on_activate.borrow().0();
                }
                gtk::Inhibit(false)
            }
        });
        search_key_press.connect_key_released({
            let on_update = self.on_update.clone();

            move |_, _, code, _| match code {
                ALT_L | ALT_R | CTRL_L | CTRL_R | ARROW_DOWN | ARROW_UP | ENTER => {}
                _ => on_update.borrow().0(),
            }
        });
        self.entry.add_controller(&search_key_press);

        self.loading.hide();
        self.loading.set_halign(gtk::Align::Start);
        self.loading.set_margin_top(8);
        self.loading.set_margin_bottom(8);
        self.loading.set_margin_start(8);
        self.result_list_cntr.append(&self.loading);

        self.no_results.hide();
        self.no_results.set_halign(gtk::Align::Start);
        self.no_results.set_markup("<i>No matches found!</i>");
        self.no_results.set_margin_top(8);
        self.no_results.set_margin_bottom(8);
        self.no_results.set_margin_start(8);
        self.result_list_cntr.append(&self.no_results);
    }

    pub fn connect_update<F: Fn() + 'static>(&self, f: F) {
        *self.on_activate.borrow_mut() = Func(Box::new(f))
    }

    pub fn connect_activate<F: Fn() + 'static>(&self, f: F) {
        *self.on_activate.borrow_mut() = Func(Box::new(f))
    }

    pub fn connect_blur<F: Fn() + 'static>(&self, f: F) {
        *self.on_blur.borrow_mut() = Func(Box::new(f))
    }
}

fn n_listbox_rows(list: &gtk::ListBox) -> u32 {
    let mut n = 0;
    loop {
        if list.row_at_index(n + 1).is_none() {
            break;
        }
        n += 1;
    }
    n as u32
}

struct Func(Box<dyn Fn()>);

impl Default for Func {
    fn default() -> Self {
        Self(Box::new(|| {}))
    }
}

impl std::fmt::Debug for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Func").finish()
    }
}

const ALT_L: u32 = 64;
const ALT_R: u32 = 108;
const CTRL_L: u32 = 37;
const CTRL_R: u32 = 105;
const ARROW_UP: u32 = 111;
const ARROW_DOWN: u32 = 116;
const ENTER: u32 = 36;
