use crate::event::DriveEvent;
use crate::Drive;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    fs::{self, File},
    io::Read,
    path::PathBuf,
    sync::mpsc::channel,
    thread,
    time::Duration,
};

pub struct WatcherState {
    rename_candidate: 
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

    fn prep_destination(&self, mut dest: PathBuf) -> PathBuf {
        self.c.get_account().unwrap();
        self.sync();
        dest.push(self.c.get_root().unwrap().name);
        dest = dest.canonicalize().unwrap();
        fs::remove_dir_all(&dest).unwrap();

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
                    EventKind::Any => {}
                    EventKind::Access(_) => {}
                    EventKind::Create(_) => {
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let core_path = core_path.to_str().unwrap().to_string();

                        self.pending_events
                            .lock()
                            .unwrap()
                            .push_back(DriveEvent::Create(core_path));
                    }
                    EventKind::Modify(_) => {
                        let mut f = File::open(event.paths[0].clone()).unwrap();
                        let mut buffer = vec![];
                        f.read_to_end(&mut buffer).unwrap();
                        println!("{:?}", &buffer[..]);
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let to_modify = self.c.get_by_path(core_path.to_str().unwrap()).unwrap();
                        self.c.write_document(to_modify.id, &buffer[..]).unwrap();
                    }
                    EventKind::Remove(_) => {
                        let core_path = get_lockbook_path(event.paths[0].clone(), dest.clone());
                        let core_path = core_path.to_str().unwrap().to_string();

                        self.pending_events
                            .lock()
                            .unwrap()
                            .push_back(DriveEvent::Delete(core_path));
                    }
                    EventKind::Other => {}
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

#[test]
fn test() {
    let a = PathBuf::from("/a/b/c/d");
    let b = PathBuf::from("/a/b/c");
    let c = get_lockbook_path(a, b);
    assert_eq!(c, PathBuf::from("d"));
}

#[test]
fn test2() {
    let a = PathBuf::from(
        "/Users/siddhantsapra/Desktop/lockbook/lockbook/clients/drive/siddhant/test.md",
    );
    let b = PathBuf::from("/a/b/c");
    let c = get_lockbook_path(a, b);
    assert_eq!(c, PathBuf::from("d"));
}
