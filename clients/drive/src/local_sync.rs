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

static SLEEP_DURATION: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct WatcherState {
    umatched_event: Option<notify::Event>,
}

impl Drive {
    pub fn check_for_changes(&self, mut dest: PathBuf) {
        dest = self.prep_destination(dest);
        let cloned_drive = self.clone();
        std::thread::spawn(move || {
            cloned_drive.watch_for_changes(dest);
        });

        let cloned_drive = self.clone();
        std::thread::spawn(move || {
            cloned_drive.handle_changes();
        });
    }

    pub fn prep_destination(&self, mut dest: PathBuf) -> PathBuf {
        self.c.get_account().unwrap();
        self.sync();
        dest.push(self.c.get_root().unwrap().name);
        // jdest = dest.canonicalize().unwrap();
        // jfs::remove_dir_all(&dest).unwrap();

        self.c
            .export_file(self.c.get_root().unwrap().id, dest.clone(), false, None)
            .unwrap();
        File::open(&dest).unwrap().sync_all().unwrap();
        dest
    }

    pub fn watch_for_changes(&self, dest: PathBuf) {
        // Create a channel to receive file events
        let (tx, rx) = channel();

        // Create a new watcher object
        let mut watcher = RecommendedWatcher::new(tx, Config::default()).unwrap();

        // Register the file for watching
        watcher.watch(&dest, RecursiveMode::Recursive).unwrap();

        println!("Watching for changes in {:?}", dest);
        for res in rx {
            println!("{:#?}", res);
            match res {
                Ok(event) => match event.kind {
                    EventKind::Create(_) => {
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let core_path = core_path.to_str().unwrap().to_string();

                        self.pending_events
                            .lock()
                            .unwrap()
                            .push_back(DriveEvent::Create(core_path));

                        // watcher.unwatch(&dest).unwrap();
                    }
                    EventKind::Modify(ModifyKind::Data(_)) => {
                        let mut f = File::open(event.paths[0].clone()).unwrap();
                        let mut buffer = vec![];
                        f.read_to_end(&mut buffer).unwrap();
                        println!("file read: {:?}", &buffer[..]);
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let core_path = core_path.to_str().unwrap().to_string();
                        self.pending_events
                            .lock()
                            .unwrap()
                            .push_back(DriveEvent::DocumentModified(core_path, buffer));
                    }
                    EventKind::Modify(ModifyKind::Name(_)) => {
                        let mut watcher_state = self.watcher_state.lock().unwrap();
                        if let Some(prior_event) = watcher_state.umatched_event.take() {
                            let prior_core_path =
                                get_lockbook_path(prior_event.paths[0].clone(), dest.clone());
                            let prior_core_path = prior_core_path.to_str().unwrap().to_string();

                            let new_core_path =
                                get_lockbook_path(event.paths[0].clone(), dest.clone());
                            let new_filename = new_core_path
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_string();

                            // differentiate between:
                            //
                            // /sid/abcd.md -> /sid/abcd2.md (unsafely presently implemented)
                            //
                            // rename as a file write
                            //
                            // rename as a move
                            // /sid/work/abcd.md -> /sid/abcd.md
                            // (core expects this to be DriveEvent::Move(/sid/work/abcd.md, /sid)

                            self.pending_events
                                .lock()
                                .unwrap()
                                .push_back(DriveEvent::Rename(prior_core_path, new_filename));
                        }

                        // After a 100 milliseconds, consider this to be a move out of this folder
                        self.watcher_state.lock().unwrap().umatched_event = Some(event);
                        let thread_state = self.clone();
                        let cloned_dest = dest.clone();
                        std::thread::spawn(move || {
                            thread::sleep(SLEEP_DURATION);
                            let watcher_state = thread_state.watcher_state.lock().unwrap();
                            match &watcher_state.umatched_event {
                                Some(unmatched_event) => {
                                    let core_path = get_lockbook_path(
                                        unmatched_event.paths[0].clone(),
                                        cloned_dest,
                                    );
                                    let core_path = core_path.to_str().unwrap().to_string();
                                    thread_state
                                        .pending_events
                                        .lock()
                                        .unwrap()
                                        .push_back(DriveEvent::Delete(core_path));
                                }
                                None => {}
                            }
                        });
                    }
                    EventKind::Remove(_) => {
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let core_path = core_path.to_str().unwrap().to_string();

                        self.pending_events
                            .lock()
                            .unwrap()
                            .push_back(DriveEvent::Delete(core_path));
                    }
                    _ => println!("unhandled event: {:#?}", event),
                },
                Err(e) => println!("watch error: {:?}", e),
            }
        }
    }

    fn handle_changes(&self) {
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

fn get_lockbook_path(event_path: PathBuf, dest: PathBuf) -> PathBuf {
    let mut ep_iter = event_path.iter();
    for _ in &dest {
        ep_iter.next();
    }
    PathBuf::from(&ep_iter)
}
