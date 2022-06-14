use std::cell::RefCell;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::ui;
use crate::ui::icons;

pub enum SearchOp {
    Exec,
}

glib::wrapper! {
    pub struct Titlebar(ObjectSubclass<TitlebarImp>)
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
            self.imp().search_field.entry.grab_focus();
        }
    }

    pub fn set_searcher(&self, searcher: Option<lb::Searcher>) {
        self.imp().search_field.set_searcher(searcher);
    }

    pub fn clear_search(&self) {
        let btn = &self.imp().search_btn;
        if btn.is_active() {
            btn.emit_clicked();
        }
        self.imp().search_field.entry.set_text("");
        *self.imp().search_field.real_input.borrow_mut() = "".to_string();
        *self.imp().search_field.searcher.borrow_mut() = None;
    }

    pub fn receive_search_ops<F: FnMut(SearchOp) -> glib::Continue + 'static>(&self, f: F) {
        self.imp().search_op_rx.take().unwrap().attach(None, f);
    }

    pub fn search_result_list(&self) -> gtk::ListBox {
        self.imp().search_field.result_list.clone()
    }

    pub fn search_result_area(&self) -> &gtk::Box {
        &self.imp().search_field.result_list_cntr
    }

    pub fn base(&self) -> &gtk::HeaderBar {
        &self.imp().base
    }
}

impl Default for Titlebar {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct TitlebarImp {
    title: gtk::Label,

    search_field: ui::SearchField,
    search_op_rx: RefCell<Option<glib::Receiver<SearchOp>>>,
    search_op_tx: glib::Sender<SearchOp>,

    app_menu_btn: gtk::MenuButton,
    search_btn: gtk::ToggleButton,

    center: gtk::Stack,
    base: gtk::HeaderBar,
}

impl Default for TitlebarImp {
    fn default() -> Self {
        let (search_op_tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        Self {
            title: Default::default(),
            search_field: Default::default(),
            search_op_rx: RefCell::new(Some(rx)),
            search_op_tx,
            app_menu_btn: Default::default(),
            search_btn: Default::default(),
            center: Default::default(),
            base: Default::default(),
        }
    }
}

#[glib::object_subclass]
impl ObjectSubclass for TitlebarImp {
    const NAME: &'static str = "Titlebar";
    type Type = super::Titlebar;
    type ParentType = gtk::Widget;

    fn class_init(c: &mut Self::Class) {
        c.set_layout_manager_type::<gtk::BinLayout>();
    }
}

impl ObjectImpl for TitlebarImp {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        self.title.set_markup("<b>Lockbook</b>");

        self.search_field.init();
        self.search_field.connect_activate({
            let ch = self.search_op_tx.clone();
            move || ch.send(SearchOp::Exec).unwrap()
        });
        self.search_field.connect_blur({
            let search_btn = self.search_btn.clone();
            move || {
                search_btn.grab_focus();
                search_btn.emit_clicked();
            }
        });

        self.app_menu_btn.set_icon_name("open-menu-symbolic");
        self.app_menu_btn.set_popover(Some(&app_menu_popover()));

        self.search_btn.set_icon_name(icons::SEARCH);
        self.search_btn.connect_clicked({
            let center = self.center.clone();

            move |search_btn| {
                let is_search = search_btn.is_active();
                if is_search {
                    center.set_transition_type(gtk::StackTransitionType::SlideUp);
                    center.set_visible_child_name("search");
                    center.activate_action("app.open-search", None).unwrap();
                } else {
                    center.set_transition_type(gtk::StackTransitionType::SlideDown);
                    center.set_visible_child_name("title");
                }
                search_btn.set_active(is_search);
            }
        });

        self.center.set_transition_duration(350);
        self.center.add_named(&self.title, Some("title"));
        self.center
            .add_named(&self.search_field.entry, Some("search"));

        self.base.set_title_widget(Some(&self.center));
        self.base.pack_end(&self.app_menu_btn);
        self.base.pack_end(&self.search_btn);
        self.base.set_parent(obj);
    }

    fn dispose(&self, _obj: &Self::Type) {
        self.base.unparent();
    }
}

impl WidgetImpl for TitlebarImp {}

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
