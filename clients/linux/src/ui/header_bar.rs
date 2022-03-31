use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

glib::wrapper! {
    pub struct LbHeaderBar(ObjectSubclass<imp::LbHeaderBar>)
        @extends gtk::Widget, gtk::HeaderBar,
        @implements gtk::Accessible;
}

impl LbHeaderBar {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("failed to create LbHeaderBar")
    }

    pub fn set_title(&self, title: &str) {
        self.imp().title.set_markup(&format!("<b>{}</b>", title));
    }

    pub fn click_search_button(&self) {
        self.imp().search_btn.emit_clicked();
    }

    pub fn search_input(&self) -> String {
        self.imp().search_box.text().to_string()
    }

    pub fn search_completion_model(&self) -> gtk::ListStore {
        self.imp()
            .search_cmpl
            .model()
            .unwrap()
            .downcast::<gtk::ListStore>()
            .unwrap()
    }
}

mod imp {
    use gtk::glib;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    use crate::ui;
    use crate::ui::icons;

    #[derive(Debug, Default)]
    pub struct LbHeaderBar {
        pub app_menu_btn: gtk::MenuButton,
        pub search_btn: gtk::ToggleButton,

        pub title: gtk::Label,
        pub search_box: gtk::Entry,
        pub search_cmpl: gtk::EntryCompletion,
        pub center: gtk::Stack,

        pub base: gtk::HeaderBar,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LbHeaderBar {
        const NAME: &'static str = "LbHeaderBar";
        type Type = super::LbHeaderBar;
        type ParentType = gtk::Widget;

        fn class_init(c: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            c.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for LbHeaderBar {
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

            let model = gtk::ListStore::new(&[String::static_type(), String::static_type()]);
            self.search_cmpl.set_model(Some(&model));
            self.search_cmpl.set_popup_completion(true);
            self.search_cmpl.set_inline_selection(true);
            self.search_cmpl.set_text_column(0);
            self.search_cmpl.set_match_func(|_, _, _| true);

            self.search_box.set_width_request(400);
            self.search_box.set_primary_icon_name(Some(icons::SEARCH));
            self.search_box.set_completion(Some(&self.search_cmpl));

            self.search_box.connect_changed(|search_box| {
                search_box
                    .activate_action("app.update-search", None)
                    .unwrap();
            });

            let search_key_press = gtk::EventControllerKey::new();
            search_key_press.connect_key_pressed({
                let search_box = self.search_box.clone();
                let search_btn = self.search_btn.clone();

                move |_, key, _, _| {
                    if key == gtk::gdk::Key::Escape {
                        search_box.set_text("");
                        search_btn.emit_clicked();
                    }
                    gtk::Inhibit(false)
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

        fn properties() -> &'static [glib::ParamSpec] {
            use once_cell::sync::Lazy;
            static PROPS: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| vec![]);
            PROPS.as_ref()
        }

        fn set_property(
            &self, _obj: &Self::Type, _id: usize, _value: &glib::Value, pspec: &glib::ParamSpec,
        ) {
            match pspec.name() {
                _ => unimplemented!(),
            }
        }

        fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            match pspec.name() {
                _ => unimplemented!(),
            }
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.base.unparent();
        }
    }

    impl WidgetImpl for LbHeaderBar {}

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
}
