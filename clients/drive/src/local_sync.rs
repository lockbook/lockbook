use crate::event::DriveEvent;
use crate::Drive;
use notify::{event::ModifyKind, Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    fs::{self, File},
    io::Read,
    path::PathBuf,
    sync::mpsc::channel,
    thread,
    time::Duration,
};

use file_events;

static SLEEP_DURATION: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct WatcherState {
    umatched_event: Option<notify::Event>,
    dest: Option<PathBuf>,
}

impl Drive {
    pub fn check_for_changes(&self, mut dest: PathBuf) {
        self.prep_destination(dest);
        let cloned_drive = self.clone();
        std::thread::spawn(move || {
            cloned_drive.watch_for_changes();
        });

        let cloned_drive = self.clone();
        std::thread::spawn(move || {
            cloned_drive.handle_changes();
        });
        loop {}
    }

    pub fn prep_destination(&self, mut dest: PathBuf) {
        println!("performing sync");

        self.c.get_account().unwrap();
        self.sync();

        println!("exporting account in {:?}", dest);

        self.c
            .export_file(self.c.get_root().unwrap().id, dest.clone(), false, None)
            .unwrap();

        dest.push(self.c.get_root().unwrap().name);
        dest = dest.canonicalize().unwrap();

        File::open(&dest).unwrap().sync_all().unwrap();
        self.watcher_state.lock().unwrap().dest = Some(dest);
    }

    pub fn get_dest(&self) -> PathBuf {
        self.watcher_state.lock().unwrap().dest.clone().unwrap()
    }

    fn watch_for_changes(&self) {
        let dest = self.get_dest();
        println!("watching for changes inside {:?}", dest);
        let watcher = file_events::Watcher::new(dest.clone());
        let rx = watcher.watch_for_changes();
        while let Ok(res) = rx.recv() {
            println!("received event: {:?}", res);
            match res {
                file_events::FileEvent::Create(path) => {
                    let core_path = get_lockbook_path(path.clone(), dest.clone());
                    let core_path = core_path.to_str().unwrap().to_string();

                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::Create(core_path));
                }
                file_events::FileEvent::Remove(path) => {
                    let core_path = get_lockbook_path(path.clone(), dest.clone());
                    let core_path = core_path.to_str().unwrap().to_string();

                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::Delete(core_path));
                }
                file_events::FileEvent::Rename(oldpath, newpath) => {
                    let core_path = get_lockbook_path(oldpath.clone(), dest.clone());
                    let core_path = core_path.to_str().unwrap().to_string();

                    let new_name = newpath.iter().last().unwrap();
                    let new_name = new_name.to_str().unwrap().to_string();

                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::Rename(core_path, new_name));
                }
                file_events::FileEvent::MoveWithin(oldpath, newpath) => {
                    let core_path = get_lockbook_path(oldpath.clone(), dest.clone());
                    let core_path = core_path.to_str().unwrap().to_string();

                    let new_path = newpath.parent().unwrap();
                    let new_path = new_path.to_str().unwrap().to_string();

                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::Move(core_path, new_path));
                }
                file_events::FileEvent::MoveOut(path) => {
                    let core_path = get_lockbook_path(path.clone(), dest.clone());
                    let core_path = core_path.to_str().unwrap().to_string();

                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::Delete(core_path));
                }
                file_events::FileEvent::Write(path) => {
                    let mut f = File::open(path.clone()).unwrap();
                    let mut buffer = vec![];
                    f.read_to_end(&mut buffer).unwrap();
                    let core_path = get_lockbook_path(path.clone(), dest.clone());
                    let core_path = core_path.to_str().unwrap().to_string();
                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::DocumentModified(core_path, buffer));
                }
                file_events::FileEvent::MoveAndRename(oldpath, newpath) => {
                    let core_path = get_lockbook_path(oldpath.clone(), dest.clone());
                    let core_path = core_path.to_str().unwrap().to_string();

                    let old_name = oldpath.iter().last().unwrap();
                    let old_name = old_name.to_str().unwrap().to_string();

                    let new_path = newpath.parent().unwrap();
                    let new_path = new_path.to_str().unwrap().to_string();

                    let new_name = newpath.iter().last().unwrap();
                    let new_name = new_name.to_str().unwrap().to_string();

                    let mut temp_path = new_path.clone();
                    temp_path.push('/');
                    temp_path.push_str(&old_name);

                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::Move(core_path, new_path));

                    self.pending_events
                        .lock()
                        .unwrap()
                        .push_back(DriveEvent::Rename(temp_path, new_name));
                }
            }
        }
    }

    fn handle_changes(&self) {
        loop {
            let event = self.pending_events.lock().unwrap().pop_front();
            match event {
                Some(DriveEvent::Create(path)) => {
                    let check = self.c.get_by_path(&path);
                    if check.is_err() {
                        self.c.create_at_path(&path).unwrap();
                    }
                }
                Some(DriveEvent::Delete(path)) => self
                    .c
                    .delete_file(self.c.get_by_path(&path).unwrap().id)
                    .unwrap(),
                Some(DriveEvent::DocumentModified(path, content)) => self
                    .c
                    .write_document(self.c.get_by_path(&path).unwrap().id, &content)
                    .unwrap(),
                Some(DriveEvent::Rename(path, new_name)) => self
                    .c
                    .rename_file(self.c.get_by_path(&path).unwrap().id, &new_name)
                    .unwrap(),
                Some(DriveEvent::Move(from_path, to_path)) => self
                    .c
                    .move_file(
                        self.c.get_by_path(&from_path).unwrap().id,
                        self.c.get_by_path(&to_path).unwrap().id,
                    )
                    .unwrap(),
                None => thread::sleep(Duration::from_millis(100)),
            }
            self.sync();
        }
    }

    fn sync(&self) {
        self.c
            .sync(Some(Box::new(|sp: lb::SyncProgress| {
                use lb::ClientWorkUnit::*;
                match sp.current_work_unit {
                    PullMetadata => println!("pulling file tree updates"),
                    PushMetadata => println!("pushing file tree updates"),
                    PullDocument(f) => println!("pulling: {}", f.name),
                    PushDocument(f) => println!("pushing: {}", f.name),
                };
            })))
            .unwrap();
    }
}

pub fn get_lockbook_path(event_path: PathBuf, dest: PathBuf) -> PathBuf {
    let mut ep_iter = event_path.iter();
    for _ in &dest {
        ep_iter.next();
    }
    //ep_iter.next();
    PathBuf::from(&ep_iter)
}
