use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use gtk::glib;

use crate::settings::Settings;

pub enum Op {
    AutoSave(lb::Uuid),
    AutoSync,
}

#[derive(Clone)]
pub struct State {
    bg_op_tx: glib::Sender<Op>,
    edit_data: Arc<RwLock<HashMap<lb::Uuid, FileEditInfo>>>,
    edit_alert_tx: mpsc::Sender<lb::Uuid>,
}

impl State {
    pub fn new(bg_op_tx: glib::Sender<Op>) -> Self {
        let edit_data = Arc::new(RwLock::new(HashMap::<lb::Uuid, FileEditInfo>::new()));

        let (edit_alert_tx, edit_alert_rx) = mpsc::channel();
        {
            let edit_data = edit_data.clone();
            thread::spawn(move || listen_for_edit_alerts(&edit_data, edit_alert_rx));
        }

        Self { bg_op_tx, edit_data, edit_alert_tx }
    }

    pub fn begin_work(&self, api: &Arc<dyn lb::Api>, settings: &Arc<RwLock<Settings>>) {
        // Start auto saving.
        {
            let edit_data = self.edit_data.clone();
            let bg_op_tx = self.bg_op_tx.clone();
            let settings = settings.clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_millis(AUTO_SAVE_CHECK_FREQ));
                scan_for_dirty_files(&edit_data, &bg_op_tx, &settings);
            });
        }
        // Start auto syncing.
        {
            let bg_op_tx = self.bg_op_tx.clone();
            let settings = settings.clone();
            let api = api.clone();
            thread::spawn(move || loop {
                sync_if_ready(&bg_op_tx, &settings, &api);
                thread::sleep(Duration::from_millis(AUTO_SYNC_CHECK_FREQ));
            });
        }
    }

    pub fn track(&self, id: lb::Uuid) -> mpsc::Sender<lb::Uuid> {
        self.edit_data
            .write()
            .unwrap_or_else(|_| panic!("obtaining edit_data write to track '{}'", id))
            .insert(id, Default::default());
        self.edit_alert_tx.clone()
    }

    pub fn untrack(&self, id: lb::Uuid) {
        self.edit_data
            .write()
            .unwrap_or_else(|_| panic!("obtaining edit_data write to untrack '{}'", id))
            .remove(&id);
    }

    pub fn set_last_saved_now(&self, id: lb::Uuid) {
        *self
            .edit_data
            .write()
            .unwrap_or_else(|_| panic!("obtaining edit_data write to last_save for '{}'", id))
            .get(&id)
            .unwrap_or_else(|| panic!("auto save isn't tracking '{}'", id))
            .last_save
            .write()
            .unwrap_or_else(|_| panic!("obtaining write on last_save for '{}'", id)) = time_now();
    }

    pub fn is_dirty(&self, id: lb::Uuid) -> bool {
        self.edit_data
            .read()
            .unwrap_or_else(|_| panic!("obtaining auto save read: checking if '{}' is dirty", id))
            .get(&id)
            .unwrap_or_else(|| panic!("auto save isn't tracking '{}'", id))
            .is_dirty()
    }
}

fn scan_for_dirty_files(
    edit_data: &Arc<RwLock<HashMap<lb::Uuid, FileEditInfo>>>, bg_op_tx: &glib::Sender<Op>,
    settings: &Arc<RwLock<Settings>>,
) {
    if settings
        .read()
        .expect("obtaining read on settings to check if auto_save is on")
        .auto_save
    {
        let edit_data = edit_data
            .read()
            .expect("obtaining read on edit_data to check for dirty files");
        for (id, edit_info) in edit_data.iter() {
            if edit_info.is_dirty() {
                bg_op_tx
                    .send(Op::AutoSave(*id))
                    .unwrap_or_else(|_| panic!("sending id '{}' off for auto save", id));
            }
        }
    }
}

fn listen_for_edit_alerts(
    edit_data: &Arc<RwLock<HashMap<lb::Uuid, FileEditInfo>>>,
    edit_alert_rx: mpsc::Receiver<lb::Uuid>,
) {
    for id in edit_alert_rx.iter() {
        edit_data
            .write()
            .unwrap_or_else(|_| {
                panic!("obtaining write on edit_data to set last_change for '{}'", id)
            })
            .get_mut(&id)
            .unwrap_or_else(|| {
                panic!("getting mutable FileEditInfo value (key: '{}') to set last change", id)
            })
            .last_change = time_now();
    }
}

fn sync_if_ready(
    bg_op_tx: &glib::Sender<Op>, settings: &Arc<RwLock<Settings>>, api: &Arc<dyn lb::Api>,
) {
    if settings
        .read()
        .expect("obtaining read on settings to check if auto_sync is on")
        .auto_sync
    {
        let last_synced = api
            .last_synced()
            .map(|ts| ts as u128)
            .expect("getting last synced");
        if time_now() - last_synced > SYNC_AFTER_SYNC_DELAY {
            bg_op_tx.send(Op::AutoSync).expect("sending auto sync op");
        }
    }
}

#[derive(Debug, Default)]
pub struct FileEditInfo {
    last_save: RwLock<u128>,
    last_change: u128,
}

impl FileEditInfo {
    fn is_dirty(&self) -> bool {
        *self.last_save.read().unwrap() < self.last_change
    }
}

fn time_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

const AUTO_SAVE_CHECK_FREQ: u64 = 2500;
const AUTO_SYNC_CHECK_FREQ: u64 = 4000;
const SYNC_AFTER_SYNC_DELAY: u128 = 60000;
