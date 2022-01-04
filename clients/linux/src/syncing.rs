use std::{sync::Arc, thread, time};

use gtk::prelude::*;
use gtk::Orientation::Vertical;

use lockbook_models::work_unit::WorkUnit;

use crate::app::LbApp;
use crate::backend::{LbCore, LbSyncMsg};
use crate::background_work::BackgroundWork;
use crate::error::{LbErrTarget, LbResult};
use crate::messages::Msg;
use crate::util;

pub fn perform_sync(lb: &LbApp) -> LbResult<()> {
    if let Ok(mut background_work) = lb.state.borrow().background_work.try_lock() {
        background_work.auto_sync_state.last_sync = BackgroundWork::current_time();
    }

    let sync_ui = lb.gui.account.status().clone();
    sync_ui.set_syncing(true);

    let (ch, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    rx.attach(
        None,
        glib::clone!(@strong lb => move |msgopt: Option<LbSyncMsg>| {
            if let Some(msg) = msgopt {
                sync_ui.set_sync_progress(&msg);
            } else {
                sync_ui.set_syncing(false);
                lb.messenger.send(Msg::RefreshSyncStatus);
                thread::spawn(glib::clone!(@strong lb.messenger as m => move || {
                    thread::sleep(time::Duration::from_secs(5));
                    m.send(Msg::RefreshUsageStatus);
                }));
            }
            glib::Continue(true)
        }),
    );

    thread::spawn(glib::clone!(
        @strong lb.core as c,
        @strong lb.messenger as m
        => move || {
            if let Err(err) = c.sync(ch) {
                match err.target() {
                    LbErrTarget::Dialog => m.send_err_dialog("syncing", err),
                    LbErrTarget::StatusPanel => m.send_err_status_panel(err.msg())
                }
            }
            m.send(Msg::RefreshTree);
        }
    ));

    Ok(())
}

pub fn refresh_status(lb: &LbApp) -> LbResult<()> {
    thread::spawn(
        glib::clone!(@strong lb.core as c, @strong lb.messenger as m => move || {
            match c.sync_status() {
                Ok(txt) => m.send(Msg::SetStatus(txt, None)),
                Err(err) => match err.target() {
                    LbErrTarget::Dialog => m.send_err_dialog("getting sync status", err),
                    LbErrTarget::StatusPanel => m.send_err_status_panel(err.msg()),
                }
            }
        }),
    );

    Ok(())
}

pub fn show_details_dialog(lb: &LbApp) -> LbResult<()> {
    const RESP_REFRESH: u16 = 1;

    let details = sync_details_ui(&lb.core)?;

    let d = lb.gui.new_dialog("Sync Details");
    d.get_content_area().set_center_widget(Some(&details));
    d.add_button("Refresh", gtk::ResponseType::Other(RESP_REFRESH));
    d.add_button("Close", gtk::ResponseType::Close);
    d.connect_response(glib::clone!(@strong lb => move |d, r| match r {
        gtk::ResponseType::Other(RESP_REFRESH) => match sync_details_ui(&lb.core) {
            Ok(details) => {
                lb.messenger.send(Msg::RefreshSyncStatus);
                d.get_content_area().set_center_widget(Some(&details));
                d.get_content_area().show_all();
                d.set_position(gtk::WindowPosition::CenterAlways);
            }
            Err(err) => lb.messenger.send_err_dialog("building sync details ui", err),
        },
        _ => d.close(),
    }));
    d.show_all();

    Ok(())
}

fn sync_details_ui(c: &Arc<LbCore>) -> LbResult<gtk::Box> {
    let work = c.calculate_work()?;
    let n_units = work.work_units.len();

    let cntr = gtk::Box::new(Vertical, 0);
    cntr.set_hexpand(true);
    if n_units == 0 {
        let lbl = gtk::Label::new(Some("All synced up!"));
        lbl.set_margin_top(12);
        lbl.set_margin_bottom(16);
        cntr.add(&lbl);
    } else {
        let desc = util::gui::text_left(&format!(
            "The following {} to sync:",
            if n_units > 1 {
                format!("{} changes need", n_units)
            } else {
                "change needs".to_string()
            }
        ));
        desc.set_margin_start(12);
        desc.set_margin_top(12);

        let tree_add_col = |tree: &gtk::TreeView, name: &str, id| {
            let cell = gtk::CellRendererText::new();
            cell.set_padding(12, 4);

            let c = gtk::TreeViewColumn::new();
            c.set_title(name);
            c.pack_start(&cell, true);
            c.add_attribute(&cell, "text", id);
            tree.append_column(&c);
        };

        let model = gtk::TreeStore::new(&[glib::Type::String, glib::Type::String]);
        let tree = gtk::TreeView::with_model(&model);
        tree.get_selection().set_mode(gtk::SelectionMode::None);
        tree.set_enable_search(false);
        tree.set_can_focus(false);
        tree_add_col(&tree, "Name", 0);
        tree_add_col(&tree, "Origin", 1);

        work.work_units.into_iter().for_each(|work_unit| {
            let action = match work_unit {
                WorkUnit::LocalChange { .. } => "Local",
                WorkUnit::ServerChange { .. } => "Server",
            };

            model.insert_with_values(
                None,
                None,
                &[0, 1],
                &[&work_unit.get_metadata().decrypted_name, &action],
            );
        });

        let scrolled = util::gui::scrollable(&tree);
        util::gui::set_margin(&scrolled, 16);
        scrolled.set_size_request(450, 300);

        cntr.add(&desc);
        cntr.pack_start(&scrolled, true, true, 0);
    }
    Ok(cntr)
}
