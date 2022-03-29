use gtk::prelude::*;

use crate::ui;
use crate::ui::icons;

pub fn new() -> gtk::HeaderBar {
    let p = gtk::Popover::new();

    /*let btn_search = ui::MenuItemBuilder::new()
    .action("app.prompt-search")
    .icon(icons::SEARCH)
    .label("Search")
    .accel("Ctrl - L")
    .popsdown(&p)
    .build();*/

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
    //app_menu.append(&btn_search);
    app_menu.append(&btn_sync);
    app_menu.append(&btn_prefs);
    app_menu.append(&btn_about);

    p.set_halign(gtk::Align::Start);
    p.set_child(Some(&app_menu));

    let app_menu_btn = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .popover(&p)
        .build();

    let hb = gtk::HeaderBar::new();
    hb.pack_end(&app_menu_btn);
    hb
}
