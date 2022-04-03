use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct Titlebar(ObjectSubclass<imp::Titlebar>)
        @extends gtk::Widget, gtk::HeaderBar,
        @implements gtk::Accessible;
}

impl Titlebar {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("failed to create custom Titlebar")
    }

    pub fn set_title(&self, title: &str) {
        self.imp().title.set_markup(&format!("<b>{}</b>", title));
    }

    pub fn toggle_search_on(&self) {
        let btn = &self.imp().search_btn;
        if !btn.is_active() {
            btn.emit_clicked();
        } else {
            self.imp().search_box.grab_focus();
        }
    }

    pub fn toggle_search_off(&self) {
        let btn = &self.imp().search_btn;
        if btn.is_active() {
            btn.emit_clicked();
        }
        self.imp().search_box.set_text("");
    }

    pub fn set_window_overlay(&self, overlay: &gtk::Overlay) {
        let focus = gtk::EventControllerFocus::new();
        focus.connect_enter({
            let overlay = overlay.clone();
            let result_area_wrap_2 = self.imp().result_area_wrap_2.clone();
            move |_| overlay.add_overlay(&result_area_wrap_2)
        });
        focus.connect_leave({
            let overlay = overlay.clone();
            let result_area_wrap_2 = self.imp().result_area_wrap_2.clone();
            move |_| overlay.remove_overlay(&result_area_wrap_2)
        });
        self.imp().search_box.add_controller(&focus);
    }

    pub fn search_result_list(&self) -> gtk::ListBox {
        self.imp().result_list.clone()
    }

    pub fn search_result_area(&self) -> &gtk::Box {
        &self.imp().result_area_wrap_2
    }

    pub fn search_input(&self) -> String {
        self.imp().search_box.text().to_string()
    }
}

impl Default for Titlebar {
    fn default() -> Self {
        Self::new()
    }
}

mod imp {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk::gdk;
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    use crate::ui;
    use crate::ui::icons;

    #[derive(Debug, Default)]
    pub struct Titlebar {
        pub app_menu_btn: gtk::MenuButton,
        pub search_btn: gtk::ToggleButton,

        pub title: gtk::Label,

        pub real_input: Rc<RefCell<String>>,
        pub search_box: gtk::Entry,
        pub result_area_wrap_2: gtk::Box,
        pub result_list: gtk::ListBox,

        pub center: gtk::Stack,
        pub base: gtk::HeaderBar,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Titlebar {
        const NAME: &'static str = "Titlebar";
        type Type = super::Titlebar;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            c.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Titlebar {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.app_menu_btn.set_icon_name("open-menu-symbolic");
            self.app_menu_btn.set_popover(Some(&app_menu_popover()));

            self.title.set_markup("<b>Lockbook</b>");

            self.search_btn.set_icon_name(icons::SEARCH);
            self.search_btn.set_active(false);
            self.search_btn.connect_clicked({
                let center = self.center.clone();
                let search_box = self.search_box.clone();

                move |search_btn| {
                    let is_search = search_btn.is_active();
                    if is_search {
                        center.set_transition_type(gtk::StackTransitionType::SlideUp);
                        center.set_visible_child_name("search");
                        search_box.grab_focus();
                    } else {
                        center.set_transition_type(gtk::StackTransitionType::SlideDown);
                        center.set_visible_child_name("title");
                    }
                    search_btn.set_active(is_search);
                }
            });

            self.result_list.set_hexpand(true);
            self.result_list
                .connect_row_activated(move |result_list, _| {
                    result_list
                        .activate_action("app.exec-search", None)
                        .unwrap();
                });
            self.result_list.connect_row_selected({
                let search_box = self.search_box.clone();
                let real_input = self.real_input.clone();

                move |_, maybe_row| {
                    if let Some(row) = maybe_row {
                        let path = row
                            .child()
                            .unwrap()
                            .downcast_ref::<ui::SearchRow>()
                            .unwrap()
                            .path();
                        search_box.set_text(&path);
                        search_box.select_region(0, -1);
                    } else {
                        // fill in user entered text
                        search_box.set_text(&real_input.borrow());
                        search_box.set_position(-1);
                    }
                }
            });

            let result_area_wrap_1 = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            result_area_wrap_1.set_width_request(400);
            result_area_wrap_1.add_css_class("contents");
            result_area_wrap_1.append(&self.result_list);

            self.result_area_wrap_2
                .set_orientation(gtk::Orientation::Vertical);
            self.result_area_wrap_2.add_css_class("view");
            self.result_area_wrap_2.set_width_request(400);
            self.result_area_wrap_2.set_halign(gtk::Align::Center);
            self.result_area_wrap_2.set_valign(gtk::Align::Start);
            self.result_area_wrap_2.append(&result_area_wrap_1);

            self.search_box.set_width_request(400);
            self.search_box.set_primary_icon_name(Some(icons::SEARCH));

            let search_key_press = gtk::EventControllerKey::new();
            search_key_press.set_propagation_phase(gtk::PropagationPhase::Capture);
            search_key_press.connect_key_pressed({
                let search_box = self.search_box.clone();
                let search_btn = self.search_btn.clone();
                let result_list = self.result_list.clone();
                let real_input = self.real_input.clone();

                move |_, key, code, _| {
                    if key == gdk::Key::Escape {
                        search_box.set_text("");
                        search_btn.emit_clicked();
                    } else if code == ARROW_DOWN {
                        let next_index = result_list
                            .selected_row()
                            .map(|row| row.index() + 1)
                            .unwrap_or_default();
                        if next_index == 0 {
                            *real_input.borrow_mut() = search_box.text().to_string();
                        }
                        result_list.select_row(result_list.row_at_index(next_index).as_ref());
                    } else if code == ARROW_UP {
                        let mut prev_index = result_list
                            .selected_row()
                            .map(|row| row.index() - 1)
                            .unwrap_or(-2);
                        if prev_index == -2 {
                            prev_index = n_listbox_rows(&result_list) as i32;
                            *real_input.borrow_mut() = search_box.text().to_string();
                        }
                        result_list.select_row(result_list.row_at_index(prev_index).as_ref());
                    } else if code == ENTER {
                        search_box.activate_action("app.exec-search", None).unwrap();
                    }
                    gtk::Inhibit(false)
                }
            });
            search_key_press.connect_key_released({
                let search_box = self.search_box.clone();

                move |_, _, code, _| match code {
                    ALT_L | ALT_R | CTRL_L | CTRL_R | ARROW_DOWN | ARROW_UP | ENTER => {}
                    _ => search_box
                        .activate_action("app.update-search", None)
                        .unwrap(),
                }
            });
            self.search_box.add_controller(&search_key_press);

            self.center.set_transition_duration(350);
            self.center.add_named(&self.title, Some("title"));
            self.center.add_named(&self.search_box, Some("search"));

            self.base.set_title_widget(Some(&self.center));
            self.base.pack_end(&self.app_menu_btn);
            self.base.pack_end(&self.search_btn);
            self.base.set_parent(obj);
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.base.unparent();
        }
    }

    impl WidgetImpl for Titlebar {}

    fn app_menu_popover() -> gtk::Popover {
        let p = gtk::Popover::new();

        let btn_sync = ui::MenuItemBuilder::new()
            .action("app.sync")
            .icon(icons::SYNC)
            .label("Sync")
            .accel("Alt - S")
            .popsdown(&p)
            .build();

        let btn_prefs = ui::MenuItemBuilder::new()
            .action("app.settings")
            .icon(icons::SETTINGS)
            .label("Settings")
            .accel("Ctrl - ,")
            .popsdown(&p)
            .build();

        let btn_about = ui::MenuItemBuilder::new()
            .action("app.about")
            .icon(icons::ABOUT)
            .label("About")
            .popsdown(&p)
            .build();

        let app_menu = gtk::Box::new(gtk::Orientation::Vertical, 0);
        app_menu.append(&btn_sync);
        app_menu.append(&btn_prefs);
        app_menu.append(&btn_about);

        p.set_halign(gtk::Align::Center);
        p.set_child(Some(&app_menu));
        p
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

    const ENTER: u32 = 36;
    const CTRL_L: u32 = 37;
    const CTRL_R: u32 = 105;
    const ALT_L: u32 = 64;
    const ALT_R: u32 = 108;
    const ARROW_UP: u32 = 111;
    const ARROW_DOWN: u32 = 116;
}
